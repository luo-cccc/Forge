
/// Assembles a ContextPack under strict budget constraints.
pub fn assemble_context_pack(
    task: AgentTask,
    source_provider: &dyn Fn(ContextSource) -> Option<String>,
    total_budget: usize,
) -> WritingContextPack {
    let priorities = task.source_priorities();
    let mut sources = Vec::new();
    let mut used = 0usize;
    let mut source_reports = Vec::new();
    let mut raw_sources = Vec::new();

    for (source, priority, budget) in priorities {
        if let Some(raw) = source_provider(source.clone()) {
            raw_sources.push(SourceDraft {
                source,
                priority,
                requested: budget,
                raw,
                required_budget: 0,
                consumed: 0,
            });
        }
    }

    for (required_source, required_budget) in task.required_source_budgets() {
        if let Some(draft) = raw_sources
            .iter_mut()
            .find(|draft| draft.source == required_source)
        {
            draft.required_budget = required_budget.min(draft.requested);
        }
    }

    for draft in raw_sources
        .iter_mut()
        .filter(|draft| draft.required_budget > 0)
    {
        let remaining = total_budget.saturating_sub(used);
        let alloc = draft.required_budget.min(remaining);
        if alloc == 0 {
            break;
        }

        let content = char_window(&draft.raw, draft.consumed, alloc);
        let char_count = content.chars().count();
        if char_count == 0 {
            continue;
        }
        draft.consumed += char_count;
        used += char_count;
        sources.push(ContextExcerpt {
            source: draft.source.clone(),
            content,
            char_count,
            truncated: draft.raw.chars().count() > draft.consumed,
            priority: draft.priority,
            evidence_ref: None,
        });
    }

    for draft in raw_sources.iter_mut() {
        let remaining = total_budget.saturating_sub(used);
        let requested_remaining = draft.requested.saturating_sub(draft.consumed);
        let alloc = requested_remaining.min(remaining);
        if alloc == 0 {
            continue;
        }

        let content = char_window(&draft.raw, draft.consumed, alloc);
        let char_count = content.chars().count();
        if char_count == 0 {
            continue;
        }
        draft.consumed += char_count;
        used += char_count;

        if let Some(existing) = sources
            .iter_mut()
            .find(|source| source.source == draft.source)
        {
            existing.content.push_str(&content);
            existing.char_count += char_count;
            existing.truncated = draft.raw.chars().count() > draft.consumed;
        } else {
            sources.push(ContextExcerpt {
                source: draft.source.clone(),
                content,
                char_count,
                truncated: draft.raw.chars().count() > draft.consumed,
                priority: draft.priority,
                evidence_ref: None,
            });
        }
    }

    for draft in raw_sources {
        if draft.consumed > 0 || draft.raw.chars().count() > 0 {
            source_reports.push(SourceReport {
                source: format!("{:?}", draft.source),
                requested: draft.requested,
                provided: draft.consumed,
                truncated: draft.raw.chars().count() > draft.consumed,
                reason: source_inclusion_reason(&task, &draft),
                truncation_reason: source_truncation_reason(total_budget, &draft),
            });
        }
    }
    sources.sort_by_key(|right| std::cmp::Reverse(right.priority));

    WritingContextPack {
        task,
        total_chars: used,
        budget_limit: total_budget,
        budget_report: ContextBudgetReport {
            total_budget,
            used,
            wasted: total_budget.saturating_sub(used),
            source_reports,
        },
        sources,
    }
}

pub fn append_context_source_with_budget(
    pack: &mut WritingContextPack,
    source: ContextSource,
    content: String,
    requested: usize,
    priority: u8,
    evidence_ref: Option<String>,
) {
    if content.trim().is_empty() {
        return;
    }

    let remaining = pack.budget_limit.saturating_sub(pack.total_chars);
    let alloc = requested.min(remaining);
    let included = char_window(&content, 0, alloc);
    let provided = included.chars().count();
    if provided == 0 {
        pack.budget_report.source_reports.push(SourceReport {
            source: format!("{:?}", source),
            requested,
            provided: 0,
            truncated: content.chars().count() > 0,
            reason: format!(
                "{:?} derived source was dropped because the context budget was exhausted before allocation.",
                pack.task
            ),
            truncation_reason: Some(format!(
                "ContextPack total budget of {} chars was exhausted before this source could be included.",
                pack.budget_limit
            )),
        });
        return;
    }

    pack.total_chars += provided;
    pack.budget_report.used += provided;
    pack.budget_report.wasted = pack.budget_limit.saturating_sub(pack.total_chars);
    let truncated = content.chars().count() > provided;
    pack.sources.push(ContextExcerpt {
        source: source.clone(),
        content: included,
        char_count: provided,
        truncated,
        priority,
        evidence_ref,
    });
    pack.sources
        .sort_by_key(|right| std::cmp::Reverse(right.priority));
    pack.budget_report.source_reports.push(SourceReport {
        source: format!("{:?}", source),
        requested,
        provided,
        truncated,
        reason: format!(
            "{:?} derived source included after story impact radius calculation.",
            pack.task
        ),
        truncation_reason: if truncated {
            Some(format!(
                "Source content was limited by remaining ContextPack budget of {} chars.",
                provided
            ))
        } else {
            None
        },
    });
}

struct SourceDraft {
    source: ContextSource,
    priority: u8,
    requested: usize,
    raw: String,
    required_budget: usize,
    consumed: usize,
}

fn source_inclusion_reason(task: &AgentTask, draft: &SourceDraft) -> String {
    if draft.consumed == 0 {
        return if draft.required_budget > 0 {
            format!(
                "{:?} required source could not be included because the context budget was exhausted.",
                task
            )
        } else {
            format!(
                "{:?} priority {} source was dropped because the context budget was exhausted before allocation.",
                task, draft.priority
            )
        };
    }

    if draft.required_budget > 0 {
        format!(
            "{:?} required source reserved {} chars before priority fill.",
            task, draft.required_budget
        )
    } else {
        format!(
            "{:?} priority {} source included during remaining-budget fill.",
            task, draft.priority
        )
    }
}

fn source_truncation_reason(total_budget: usize, draft: &SourceDraft) -> Option<String> {
    let raw_chars = draft.raw.chars().count();
    if raw_chars <= draft.consumed {
        return None;
    }

    if draft.consumed >= draft.requested {
        Some(format!(
            "Source content was limited by its per-source budget of {} chars.",
            draft.requested
        ))
    } else {
        Some(format!(
            "ContextPack total budget of {} chars was exhausted before this source could be fully included.",
            total_budget
        ))
    }
}

pub fn assemble_observation_context(
    task: AgentTask,
    observation: &WriterObservation,
    memory: &WriterMemory,
    total_budget: usize,
) -> WritingContextPack {
    let project_brief = build_project_brief(&observation.project_id, memory);
    let chapter_mission = build_chapter_mission(&observation.project_id, observation, memory);
    let next_beat = build_next_beat(&observation.project_id, observation, memory);
    let result_feedback = build_result_feedback(&observation.project_id, observation, memory);
    let decisions = memory.list_recent_decisions(6).unwrap_or_default();
    let open_promises = memory.get_open_promise_summaries().unwrap_or_default();
    let decision_slice = build_decision_slice(&decisions);
    let relevance = WritingRelevance::new(
        observation,
        &chapter_mission,
        &next_beat,
        &result_feedback,
        &decision_slice,
    );
    let canon_slice = build_canon_slice(observation, memory, &relevance, &open_promises);
    let promise_slice = build_promise_slice(observation, &open_promises, &relevance, &decisions);
    let author_style = build_style_slice(memory);
    let selected_text = observation.selected_text().to_string();
    let cursor_prefix = if observation.prefix.trim().is_empty() {
        observation.paragraph.clone()
    } else {
        observation.prefix.clone()
    };
    let cursor_suffix = observation.suffix.clone();
    let reader_compensation =
        build_reader_compensation_context(&observation.project_id, observation, memory);

    assemble_context_pack(
        task,
        &|source| match source {
            ContextSource::CursorPrefix => non_empty(cursor_prefix.clone()),
            ContextSource::CursorSuffix => non_empty(cursor_suffix.clone()),
            ContextSource::SelectedText => non_empty(selected_text.clone()),
            ContextSource::ProjectBrief => non_empty(project_brief.clone()),
            ContextSource::ChapterMission => non_empty(chapter_mission.clone()),
            ContextSource::NextBeat => non_empty(next_beat.clone()),
            ContextSource::ResultFeedback => non_empty(result_feedback.clone()),
            ContextSource::CanonSlice => non_empty(canon_slice.clone()),
            ContextSource::PromiseSlice => non_empty(promise_slice.clone()),
            ContextSource::DecisionSlice => non_empty(decision_slice.clone()),
            ContextSource::AuthorStyle => non_empty(author_style.clone()),
            ContextSource::StoryImpactRadius => None,
            ContextSource::ReaderCompensation => non_empty(reader_compensation.clone()),
            _ => None,
        },
        total_budget,
    )
}

fn build_chapter_mission(
    project_id: &str,
    observation: &WriterObservation,
    memory: &WriterMemory,
) -> String {
    observation
        .chapter_title
        .as_deref()
        .and_then(|chapter| {
            memory
                .get_chapter_mission(project_id, chapter)
                .ok()
                .flatten()
        })
        .filter(|mission| !mission.is_empty())
        .map(|mission| mission.render_for_context())
        .unwrap_or_default()
}

fn build_next_beat(
    project_id: &str,
    observation: &WriterObservation,
    memory: &WriterMemory,
) -> String {
    let active_mission = observation.chapter_title.as_deref().and_then(|chapter| {
        memory
            .get_chapter_mission(project_id, chapter)
            .ok()
            .flatten()
    });
    let recent_results = memory
        .list_recent_chapter_results(project_id, 6)
        .unwrap_or_default();
    let open_promises = memory.get_open_promise_summaries().unwrap_or_default();
    derive_next_beat(
        observation.chapter_title.as_deref(),
        active_mission.as_ref(),
        &recent_results,
        &open_promises,
    )
    .filter(|beat| !beat.is_empty())
    .map(|beat| beat.render_for_context())
    .unwrap_or_default()
}

fn build_result_feedback(
    project_id: &str,
    observation: &WriterObservation,
    memory: &WriterMemory,
) -> String {
    let mut results = memory
        .list_recent_chapter_results(project_id, 4)
        .unwrap_or_default()
        .into_iter()
        .filter(|result| !result.is_empty())
        .collect::<Vec<_>>();

    if let Some(current_chapter) = observation.chapter_title.as_deref() {
        results.retain(|result| {
            result.chapter_title != current_chapter
                || observation.reason == crate::writer_agent::observation::ObservationReason::Save
        });
    }

    results
        .into_iter()
        .take(3)
        .map(|result| result.render_for_context())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_project_brief(project_id: &str, memory: &WriterMemory) -> String {
    memory
        .get_story_contract(project_id)
        .ok()
        .flatten()
        .filter(|contract| contract.quality() >= StoryContractQuality::Usable)
        .map(|contract| contract.render_for_context())
        .unwrap_or_default()
}


fn build_reader_compensation_context(
    project_id: &str,
    observation: &WriterObservation,
    memory: &WriterMemory,
) -> String {
    let mut lines = Vec::new();
    if let Ok(Some(profile)) = memory.get_reader_compensation_profile(project_id) {
        if !profile.target_reader.trim().is_empty() {
            push_contract_line(&mut lines, "目标读者", &profile.target_reader);
        }
        if !profile.primary_lack.trim().is_empty() {
            push_contract_line(&mut lines, "主缺口", &profile.primary_lack);
        }
        if !profile.dominant_relationship_soil.trim().is_empty() {
            push_contract_line(&mut lines, "关系土壤", &profile.dominant_relationship_soil);
        }
    }
    if let Some(chapter) = observation.chapter_title.as_deref() {
        if let Ok(Some(mission)) = memory.get_chapter_mission(project_id, chapter) {
            if !mission.pressure_scene.trim().is_empty() {
                push_contract_line(&mut lines, "本章压迫", &mission.pressure_scene);
            }
            if !mission.payoff_target.trim().is_empty() {
                push_contract_line(&mut lines, "本章补偿目标", &mission.payoff_target);
            }
            if !mission.next_lack_opened.trim().is_empty() {
                push_contract_line(&mut lines, "下一层缺口", &mission.next_lack_opened);
            }
        }
    }
    if let Ok(debts) = memory.get_open_emotional_debts(project_id) {
        let recent: Vec<_> = debts.iter().take(3).collect();
        if !recent.is_empty() {
            let debt_lines: Vec<_> = recent
                .iter()
                .map(|d| format!("  {} ({}) - {}", d.title, d.debt_kind, d.payoff_status))
                .collect();
            lines.push(format!("活跃情绪债务:\n{}", debt_lines.join("\n")));
        }
    }
    lines.join("\n")
}
