use crate::writer_agent::kernel::{
    chapter_result_from_observation, extract_plot_promises, split_sentences,
};
use crate::writer_agent::memory::{BookStateSummary, ChapterResultSummary, PlotPromiseSummary};
use crate::writer_agent::observation::{ObservationReason, ObservationSource, WriterObservation};

#[derive(Debug, Clone)]
pub struct PersistedChapterRuntimeArtifacts {
    pub artifact_refs: Vec<String>,
}

pub fn persist_chapter_runtime_artifacts(
    app: &tauri::AppHandle,
    request_id: &str,
    context: &BuiltChapterContext,
    settlement_delta: &ChapterSettlementDelta,
    length_telemetry: &ChapterLengthTelemetry,
) -> Result<PersistedChapterRuntimeArtifacts, String> {
    let project_dir = crate::storage::active_project_data_dir(app)?;
    let runtime_dir = project_dir.join("chapter_runtime");
    std::fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;

    let stem = format!(
        "{}-{}",
        context
            .target
            .number
            .map(|number| format!("chapter-{:04}", number))
            .unwrap_or_else(|| "chapter-unknown".to_string()),
        request_id
    );

    let intent_path = runtime_dir.join(format!("{}.intent.json", stem));
    let evidence_path = runtime_dir.join(format!("{}.evidence.json", stem));
    let rule_stack_path = runtime_dir.join(format!("{}.rule_stack.json", stem));
    let trace_path = runtime_dir.join(format!("{}.trace.json", stem));
    let settlement_path = runtime_dir.join(format!("{}.settlement.json", stem));
    let length_path = runtime_dir.join(format!("{}.length.json", stem));

    write_json_file(&intent_path, &context.intent_artifact)?;
    write_json_file(&evidence_path, &context.selected_evidence)?;
    write_json_file(&rule_stack_path, &context.rule_stack)?;
    write_json_file(&trace_path, &context.trace_artifact)?;
    write_json_file(&settlement_path, settlement_delta)?;
    write_json_file(&length_path, length_telemetry)?;

    Ok(PersistedChapterRuntimeArtifacts {
        artifact_refs: vec![
            path_ref(&project_dir, &intent_path),
            path_ref(&project_dir, &evidence_path),
            path_ref(&project_dir, &rule_stack_path),
            path_ref(&project_dir, &trace_path),
            path_ref(&project_dir, &settlement_path),
            path_ref(&project_dir, &length_path),
        ],
    })
}

pub fn build_basic_chapter_settlement_delta(
    project_id: &str,
    chapter_title: &str,
    chapter_revision: &str,
    generated_content: &str,
    created_at_ms: u64,
    memory: &crate::writer_agent::memory::WriterMemory,
    continuity_issues: Vec<String>,
) -> ChapterSettlementDelta {
    let observation = settlement_observation(
        project_id,
        chapter_title,
        chapter_revision,
        generated_content,
        created_at_ms,
    );
    let chapter_result = chapter_result_from_observation(&observation, memory);
    let open_promises = memory.get_open_promise_summaries().unwrap_or_default();
    let promise_updates =
        derive_promise_delta_entries(generated_content, &observation, &chapter_result, &open_promises);
    let book_state_updates = derive_book_state_updates(&chapter_result, &promise_updates);
    let arc_updates = derive_arc_updates(&chapter_result, &promise_updates);
    let summary = chapter_result.summary.clone();

    ChapterSettlementDelta {
        chapter_title: chapter_title.to_string(),
        chapter_revision: chapter_revision.to_string(),
        summary,
        chapter_result: ChapterResultDelta {
            summary: chapter_result.summary.clone(),
            state_changes: chapter_result.state_changes.clone(),
            character_progress: chapter_result.character_progress.clone(),
            new_conflicts: chapter_result.new_conflicts.clone(),
            new_clues: chapter_result.new_clues.clone(),
            promise_updates: chapter_result.promise_updates.clone(),
            canon_updates: chapter_result.canon_updates.clone(),
        },
        promise_updates: promise_updates.clone(),
        arc_updates: arc_updates.clone(),
        book_state_updates: book_state_updates.clone(),
        chapter_fact_delta: chapter_fact_lines(&chapter_result),
        promise_delta: promise_updates
            .iter()
            .map(render_promise_delta_line)
            .collect(),
        arc_delta: arc_updates
            .iter()
            .map(|entry| format!("{}: {}", entry.scope, entry.value))
            .collect(),
        book_state_delta: book_state_updates
            .iter()
            .map(render_book_state_delta_line)
            .collect(),
        continuity_issues,
        repairable: true,
    }
}

pub fn apply_chapter_settlement_delta(
    memory: &crate::writer_agent::memory::WriterMemory,
    project_id: &str,
    delta: &ChapterSettlementDelta,
) -> Result<ChapterSettlementApplyResult, String> {
    let source_ref = format!(
        "chapter_settlement:{}:{}",
        delta.chapter_title, delta.chapter_revision
    );
    let chapter_result = ChapterResultSummary {
        id: 0,
        project_id: project_id.to_string(),
        chapter_title: delta.chapter_title.clone(),
        chapter_revision: delta.chapter_revision.clone(),
        summary: delta.chapter_result.summary.clone(),
        state_changes: delta.chapter_result.state_changes.clone(),
        character_progress: delta.chapter_result.character_progress.clone(),
        new_conflicts: delta.chapter_result.new_conflicts.clone(),
        new_clues: delta.chapter_result.new_clues.clone(),
        promise_updates: delta.chapter_result.promise_updates.clone(),
        canon_updates: delta.chapter_result.canon_updates.clone(),
        source_ref: source_ref.clone(),
        created_at: crate::agent_runtime::now_ms(),
    };
    let chapter_result_snapshot_id = memory
        .upsert_chapter_result(&chapter_result)
        .map_err(|e| e.to_string())?;

    let mut promise_created = 0usize;
    let mut promise_advanced = 0usize;
    let mut promise_resolved = 0usize;
    let mut promise_deferred = 0usize;
    let mut promise_abandoned = 0usize;
    let mut warnings = Vec::new();

    for update in &delta.promise_updates {
        match update.action {
            ChapterPromiseDeltaAction::Introduced => {
                match memory.find_open_promise_by_title(&update.title) {
                    Ok(Some(existing)) => {
                        memory
                            .touch_promise_last_seen(existing.id, &update.chapter, &update.source_ref)
                            .map_err(|e| e.to_string())?;
                        memory
                            .update_promise_status_flags(
                                existing.id,
                                &update.blocked_reason,
                                existing.promoted || update.promoted,
                                existing.core || update.core,
                            )
                            .map_err(|e| e.to_string())?;
                        promise_advanced += 1;
                    }
                    Ok(None) => {
                        memory
                            .add_promise_with_status_flags(
                                &update.kind,
                                &update.title,
                                &update.description,
                                &update.chapter,
                                &update.source_ref,
                                &update.expected_payoff,
                                update.priority,
                                &update.related_entities,
                                &update.blocked_reason,
                                update.promoted,
                                update.core,
                            )
                            .map_err(|e| e.to_string())?;
                        promise_created += 1;
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            ChapterPromiseDeltaAction::Advanced => {
                if let Some(promise_id) = resolve_promise_id(memory, update)? {
                    memory
                        .touch_promise_last_seen(promise_id, &update.chapter, &update.source_ref)
                        .map_err(|e| e.to_string())?;
                    memory
                        .update_promise_status_flags(
                            promise_id,
                            &update.blocked_reason,
                            update.promoted,
                            update.core,
                        )
                        .map_err(|e| e.to_string())?;
                    if !update.expected_payoff.trim().is_empty() {
                        memory
                            .defer_promise(promise_id, &update.expected_payoff)
                            .map_err(|e| e.to_string())?;
                    }
                    promise_advanced += 1;
                } else {
                    warnings.push(format!("promise advance skipped: {}", update.title));
                }
            }
            ChapterPromiseDeltaAction::Resolved => {
                if let Some(promise_id) = resolve_promise_id(memory, update)? {
                    if memory
                        .resolve_promise(promise_id, &update.chapter)
                        .map_err(|e| e.to_string())?
                    {
                        promise_resolved += 1;
                    } else {
                        warnings.push(format!("promise already closed: {}", update.title));
                    }
                } else {
                    warnings.push(format!("promise resolve skipped: {}", update.title));
                }
            }
            ChapterPromiseDeltaAction::Deferred => {
                if let Some(promise_id) = resolve_promise_id(memory, update)? {
                    if memory
                        .defer_promise(promise_id, &update.expected_payoff)
                        .map_err(|e| e.to_string())?
                    {
                        memory
                            .touch_promise_last_seen(promise_id, &update.chapter, &update.source_ref)
                            .map_err(|e| e.to_string())?;
                        memory
                            .update_promise_status_flags(
                                promise_id,
                                &update.blocked_reason,
                                update.promoted,
                                update.core,
                            )
                            .map_err(|e| e.to_string())?;
                        promise_deferred += 1;
                    }
                } else {
                    warnings.push(format!("promise defer skipped: {}", update.title));
                }
            }
            ChapterPromiseDeltaAction::Abandoned => {
                if let Some(promise_id) = resolve_promise_id(memory, update)? {
                    if memory
                        .abandon_promise(promise_id)
                        .map_err(|e| e.to_string())?
                    {
                        promise_abandoned += 1;
                    }
                } else {
                    warnings.push(format!("promise abandon skipped: {}", update.title));
                }
            }
        }
    }

    let existing = memory
        .get_book_state(project_id)
        .map_err(|e| e.to_string())?
        .unwrap_or(BookStateSummary {
            project_id: project_id.to_string(),
            title: project_id.to_string(),
            long_term_constraints: Vec::new(),
            mega_promises: Vec::new(),
            irreversible_changes: Vec::new(),
            source_ref: source_ref.clone(),
            updated_at: String::new(),
        });
    let mut book_state = existing.clone();
    let before = (
        book_state.long_term_constraints.clone(),
        book_state.mega_promises.clone(),
        book_state.irreversible_changes.clone(),
    );
    for update in &delta.book_state_updates {
        let target = match update.bucket {
            ChapterBookStateDeltaBucket::LongTermConstraint => &mut book_state.long_term_constraints,
            ChapterBookStateDeltaBucket::MegaPromise => &mut book_state.mega_promises,
            ChapterBookStateDeltaBucket::IrreversibleChange => &mut book_state.irreversible_changes,
        };
        if !target.iter().any(|item| item == &update.value) {
            target.push(update.value.clone());
        }
    }
    book_state.source_ref = source_ref.clone();
    let book_state_updated = before
        != (
            book_state.long_term_constraints.clone(),
            book_state.mega_promises.clone(),
            book_state.irreversible_changes.clone(),
        );
    if book_state_updated {
        memory
            .upsert_book_state(&book_state)
            .map_err(|e| e.to_string())?;
    }

    memory
        .record_decision(
            &delta.chapter_title,
            "Chapter settlement applied",
            "applied_chapter_settlement_delta",
            &[],
            &format!(
                "Applied typed settlement delta for {} at {}.",
                delta.chapter_title, delta.chapter_revision
            ),
            &[source_ref],
        )
        .map_err(|e| e.to_string())?;

    Ok(ChapterSettlementApplyResult {
        applied: true,
        chapter_result_snapshot_id: Some(chapter_result_snapshot_id),
        promise_created,
        promise_advanced,
        promise_resolved,
        promise_deferred,
        promise_abandoned,
        book_state_updated,
        warnings,
    })
}

fn write_json_file(path: &std::path::Path, value: &impl serde::Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn path_ref(project_dir: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn settlement_observation(
    project_id: &str,
    chapter_title: &str,
    chapter_revision: &str,
    generated_content: &str,
    created_at_ms: u64,
) -> WriterObservation {
    WriterObservation {
        id: format!("settlement-{}-{}", chapter_title, chapter_revision),
        created_at: created_at_ms,
        source: ObservationSource::ChapterSave,
        reason: ObservationReason::Save,
        project_id: project_id.to_string(),
        chapter_title: Some(chapter_title.to_string()),
        chapter_revision: Some(chapter_revision.to_string()),
        cursor: None,
        selection: None,
        prefix: generated_content.to_string(),
        suffix: String::new(),
        paragraph: generated_content
            .lines()
            .rev()
            .find(|line| line.trim().chars().count() >= 8)
            .unwrap_or(generated_content)
            .trim()
            .to_string(),
        full_text_digest: Some(crate::storage::content_revision(generated_content)),
        editor_dirty: false,
    }
}

fn derive_promise_delta_entries(
    generated_content: &str,
    observation: &WriterObservation,
    chapter_result: &ChapterResultSummary,
    open_promises: &[PlotPromiseSummary],
) -> Vec<ChapterPromiseDeltaEntry> {
    let mut updates = Vec::new();
    let lowercase = generated_content.to_lowercase();

    for promise in open_promises {
        let title_hit = !promise.title.trim().is_empty() && generated_content.contains(&promise.title);
        let desc_hit = !promise.description.trim().is_empty()
            && split_sentences(&promise.description)
                .into_iter()
                .any(|fragment| generated_content.contains(fragment.trim()));
        if !(title_hit || desc_hit) {
            continue;
        }
        let resolved = [
            "交代",
            "说出",
            "揭开",
            "归还",
            "放回",
            "找到",
            "解释",
            "兑现",
        ]
        .iter()
        .any(|cue| lowercase.contains(cue));
        let blocked_reason = if chapter_result
            .new_conflicts
            .iter()
            .any(|line| line.contains(&promise.title))
        {
            chapter_result
                .new_conflicts
                .iter()
                .find(|line| line.contains(&promise.title))
                .cloned()
                .unwrap_or_default()
        } else {
            String::new()
        };
        updates.push(ChapterPromiseDeltaEntry {
            action: if resolved {
                ChapterPromiseDeltaAction::Resolved
            } else if !blocked_reason.is_empty() {
                ChapterPromiseDeltaAction::Deferred
            } else {
                ChapterPromiseDeltaAction::Advanced
            },
            promise_id: Some(promise.id),
            kind: promise.kind.clone(),
            title: promise.title.clone(),
            description: promise.description.clone(),
            chapter: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            source_ref: chapter_result.source_ref.clone(),
            expected_payoff: if !blocked_reason.is_empty() {
                next_chapter_label(&chapter_result.chapter_title)
            } else {
                promise.expected_payoff.clone()
            },
            priority: promise.priority,
            related_entities: Vec::new(),
            core: promise.core || promise.priority >= 7,
            promoted: promise.promoted || promise.priority >= 5,
            blocked_reason,
            evidence: chapter_result.summary.clone(),
        });
    }

    for promise in extract_plot_promises(generated_content, observation) {
        if updates.iter().any(|existing| existing.title == promise.title)
            || open_promises.iter().any(|existing| existing.title == promise.title)
        {
            continue;
        }
        updates.push(ChapterPromiseDeltaEntry {
            action: ChapterPromiseDeltaAction::Introduced,
            promise_id: None,
            kind: promise.kind.clone(),
            title: promise.title.clone(),
            description: promise.description.clone(),
            chapter: promise.introduced_chapter.clone(),
            source_ref: chapter_result.source_ref.clone(),
            expected_payoff: promise.expected_payoff.clone(),
            priority: promise.priority,
            related_entities: promise.related_entities.clone(),
            core: promise.priority >= 5,
            promoted: promise.priority >= 4,
            blocked_reason: String::new(),
            evidence: promise.description.clone(),
        });
    }

    updates
}

fn derive_book_state_updates(
    chapter_result: &ChapterResultSummary,
    promise_updates: &[ChapterPromiseDeltaEntry],
) -> Vec<ChapterBookStateDeltaEntry> {
    let mut updates = Vec::new();
    for line in chapter_result.state_changes.iter().take(3) {
        if looks_irreversible(line) {
            updates.push(ChapterBookStateDeltaEntry {
                bucket: ChapterBookStateDeltaBucket::IrreversibleChange,
                value: line.clone(),
                source_ref: chapter_result.source_ref.clone(),
                reason: "chapter state change appears durable".to_string(),
            });
        }
    }
    for promise in promise_updates.iter().filter(|entry| entry.core).take(3) {
        updates.push(ChapterBookStateDeltaEntry {
            bucket: ChapterBookStateDeltaBucket::MegaPromise,
            value: format!("{} -> {}", promise.title, promise.expected_payoff),
            source_ref: promise.source_ref.clone(),
            reason: "core promise should remain visible at book scope".to_string(),
        });
    }
    updates
}

fn derive_arc_updates(
    chapter_result: &ChapterResultSummary,
    promise_updates: &[ChapterPromiseDeltaEntry],
) -> Vec<ChapterArcDeltaEntry> {
    let mut updates = Vec::new();
    if let Some(conflict) = chapter_result.new_conflicts.first() {
        updates.push(ChapterArcDeltaEntry {
            scope: "conflict".to_string(),
            value: conflict.clone(),
            reason: "new conflict should shape upcoming arc planning".to_string(),
        });
    }
    if let Some(promoted) = promise_updates.iter().find(|entry| entry.promoted) {
        updates.push(ChapterArcDeltaEntry {
            scope: "hook".to_string(),
            value: promoted.title.clone(),
            reason: "promoted promise should enter arc planning priority".to_string(),
        });
    }
    updates
}

fn chapter_fact_lines(result: &ChapterResultSummary) -> Vec<String> {
    let mut lines = Vec::new();
    if !result.summary.trim().is_empty() {
        lines.push(result.summary.clone());
    }
    lines.extend(result.state_changes.iter().take(3).cloned());
    lines.extend(result.character_progress.iter().take(2).cloned());
    lines.extend(result.new_clues.iter().map(|clue| format!("线索: {}", clue)).take(2));
    lines
}

fn render_promise_delta_line(entry: &ChapterPromiseDeltaEntry) -> String {
    let action = match entry.action {
        ChapterPromiseDeltaAction::Introduced => "introduced",
        ChapterPromiseDeltaAction::Advanced => "advanced",
        ChapterPromiseDeltaAction::Resolved => "resolved",
        ChapterPromiseDeltaAction::Deferred => "deferred",
        ChapterPromiseDeltaAction::Abandoned => "abandoned",
    };
    let mut line = format!("{}: {}", action, entry.title);
    if !entry.expected_payoff.trim().is_empty() {
        line.push_str(&format!(" -> {}", entry.expected_payoff));
    }
    if !entry.blocked_reason.trim().is_empty() {
        line.push_str(&format!(" | blocked: {}", entry.blocked_reason));
    }
    line
}

fn render_book_state_delta_line(entry: &ChapterBookStateDeltaEntry) -> String {
    let bucket = match entry.bucket {
        ChapterBookStateDeltaBucket::LongTermConstraint => "constraint",
        ChapterBookStateDeltaBucket::MegaPromise => "mega_promise",
        ChapterBookStateDeltaBucket::IrreversibleChange => "irreversible_change",
    };
    format!("{}: {}", bucket, entry.value)
}

fn looks_irreversible(line: &str) -> bool {
    ["失去", "死亡", "断绝", "背叛", "毁掉", "归还", "公开", "暴露"]
        .iter()
        .any(|cue| line.contains(cue))
}

fn next_chapter_label(chapter: &str) -> String {
    let digits = chapter
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits
        .parse::<i64>()
        .ok()
        .map(|number| format!("Chapter-{}", number + 1))
        .unwrap_or_else(|| "later chapter".to_string())
}

fn resolve_promise_id(
    memory: &crate::writer_agent::memory::WriterMemory,
    update: &ChapterPromiseDeltaEntry,
) -> Result<Option<i64>, String> {
    if let Some(id) = update.promise_id {
        return Ok(Some(id));
    }
    Ok(memory
        .find_open_promise_by_title(&update.title)
        .map_err(|e| e.to_string())?
        .map(|promise| promise.id))
}
