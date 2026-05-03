//! ContextEngine — deterministic context packs for every agent action.
//! Replaces ad-hoc prompt assembly with budgeted, priority-ordered context sources.

use serde::{Deserialize, Serialize};

use super::context_relevance::{
    format_canon_line, format_promise_line, score_canon_entity, score_promise, WritingRelevance,
};
use super::kernel::derive_next_beat;
use super::memory::{
    CreativeDecisionSummary, PlotPromiseSummary, StoryContractQuality, WriterMemory,
};
use super::observation::WriterObservation;

/// A single context source with its content and budget info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextExcerpt {
    pub source: ContextSource,
    pub content: String,
    pub char_count: usize,
    pub truncated: bool,
    pub priority: u8,
    pub evidence_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContextSource {
    SystemContract,
    ProjectBrief,
    ChapterMission,
    NextBeat,
    ResultFeedback,
    AuthorStyle,
    CanonSlice,
    PromiseSlice,
    DecisionSlice,
    OutlineSlice,
    RagExcerpt,
    CursorPrefix,
    CursorSuffix,
    SelectedText,
    PreviousChapter,
    NextChapter,
    NeighborText,
}

/// Task type determines which sources to include and their priority order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentTask {
    GhostWriting,
    ContinuityDiagnostic,
    ChapterGeneration,
    InlineRewrite,
    PlanningReview,
    ProposalEvaluation,
    CanonMaintenance,
    ManualRequest,
}

impl AgentTask {
    pub fn default_budget(&self) -> usize {
        match self {
            AgentTask::GhostWriting => 3_000,
            AgentTask::ContinuityDiagnostic => 2_500,
            AgentTask::ChapterGeneration => 20_000,
            AgentTask::InlineRewrite => 4_500,
            AgentTask::PlanningReview => 6_000,
            AgentTask::ProposalEvaluation => 1_400,
            AgentTask::CanonMaintenance => 4_500,
            AgentTask::ManualRequest => 4_500,
        }
    }

    pub fn source_priorities(&self) -> Vec<(ContextSource, u8, usize)> {
        match self {
            AgentTask::GhostWriting => vec![
                (ContextSource::CursorPrefix, 10, 800),
                (ContextSource::ChapterMission, 10, 520),
                (ContextSource::NextBeat, 10, 420),
                (ContextSource::ResultFeedback, 9, 420),
                (ContextSource::ProjectBrief, 9, 420),
                (ContextSource::CursorSuffix, 9, 400),
                (ContextSource::CanonSlice, 8, 600),
                (ContextSource::PromiseSlice, 7, 400),
                (ContextSource::DecisionSlice, 7, 300),
                (ContextSource::OutlineSlice, 6, 500),
                (ContextSource::AuthorStyle, 5, 300),
                (ContextSource::RagExcerpt, 4, 400),
            ],
            AgentTask::ContinuityDiagnostic => vec![
                (ContextSource::CursorPrefix, 10, 300),
                (ContextSource::CanonSlice, 10, 800),
                (ContextSource::ChapterMission, 9, 420),
                (ContextSource::NextBeat, 9, 420),
                (ContextSource::ResultFeedback, 9, 420),
                (ContextSource::ProjectBrief, 9, 400),
                (ContextSource::DecisionSlice, 9, 300),
                (ContextSource::OutlineSlice, 9, 500),
                (ContextSource::RagExcerpt, 8, 600),
            ],
            AgentTask::ChapterGeneration => vec![
                (ContextSource::ProjectBrief, 11, 1600),
                (ContextSource::ChapterMission, 11, 2200),
                (ContextSource::NextBeat, 11, 1600),
                (ContextSource::ResultFeedback, 10, 2200),
                (ContextSource::OutlineSlice, 10, 6000),
                (ContextSource::PreviousChapter, 9, 5000),
                (ContextSource::PromiseSlice, 8, 4000),
                (ContextSource::DecisionSlice, 8, 2000),
                (ContextSource::CanonSlice, 7, 4000),
                (ContextSource::AuthorStyle, 6, 2000),
                (ContextSource::RagExcerpt, 5, 4000),
                (ContextSource::NeighborText, 4, 3000),
            ],
            AgentTask::InlineRewrite => vec![
                (ContextSource::SelectedText, 10, 2000),
                (ContextSource::CursorPrefix, 9, 500),
                (ContextSource::CursorSuffix, 8, 500),
                (ContextSource::ChapterMission, 8, 500),
                (ContextSource::NextBeat, 8, 360),
                (ContextSource::ProjectBrief, 8, 400),
                (ContextSource::ResultFeedback, 7, 360),
                (ContextSource::CanonSlice, 7, 400),
                (ContextSource::DecisionSlice, 7, 300),
                (ContextSource::AuthorStyle, 6, 300),
            ],
            AgentTask::PlanningReview => vec![
                (ContextSource::ChapterMission, 11, 900),
                (ContextSource::ProjectBrief, 10, 800),
                (ContextSource::ResultFeedback, 10, 900),
                (ContextSource::NextBeat, 10, 700),
                (ContextSource::PromiseSlice, 9, 900),
                (ContextSource::CanonSlice, 9, 900),
                (ContextSource::DecisionSlice, 8, 600),
                (ContextSource::OutlineSlice, 8, 900),
                (ContextSource::AuthorStyle, 7, 500),
                (ContextSource::SelectedText, 7, 700),
                (ContextSource::CursorPrefix, 6, 700),
                (ContextSource::RagExcerpt, 5, 800),
            ],
            AgentTask::ProposalEvaluation => vec![
                (ContextSource::CanonSlice, 10, 500),
                (ContextSource::PromiseSlice, 9, 300),
                (ContextSource::DecisionSlice, 9, 300),
                (ContextSource::NextBeat, 8, 260),
                (ContextSource::ChapterMission, 8, 360),
                (ContextSource::ProjectBrief, 8, 300),
                (ContextSource::ResultFeedback, 8, 260),
                (ContextSource::AuthorStyle, 8, 300),
            ],
            AgentTask::CanonMaintenance => vec![
                (ContextSource::CanonSlice, 10, 2000),
                (ContextSource::PromiseSlice, 9, 1000),
                (ContextSource::NextBeat, 9, 800),
                (ContextSource::ResultFeedback, 9, 900),
                (ContextSource::DecisionSlice, 9, 800),
                (ContextSource::ChapterMission, 8, 800),
                (ContextSource::ProjectBrief, 8, 600),
                (ContextSource::OutlineSlice, 8, 1000),
            ],
            AgentTask::ManualRequest => vec![
                (ContextSource::SelectedText, 10, 1200),
                (ContextSource::CursorPrefix, 9, 1400),
                (ContextSource::CursorSuffix, 8, 500),
                (ContextSource::ChapterMission, 8, 700),
                (ContextSource::NextBeat, 8, 600),
                (ContextSource::ProjectBrief, 8, 600),
                (ContextSource::ResultFeedback, 8, 700),
                (ContextSource::CanonSlice, 8, 800),
                (ContextSource::PromiseSlice, 7, 600),
                (ContextSource::DecisionSlice, 7, 500),
                (ContextSource::AuthorStyle, 6, 400),
                (ContextSource::RagExcerpt, 5, 400),
            ],
        }
    }

    pub fn required_source_budgets(&self) -> Vec<(ContextSource, usize)> {
        match self {
            AgentTask::GhostWriting => vec![
                (ContextSource::CursorPrefix, 240),
                (ContextSource::ChapterMission, 180),
                (ContextSource::NextBeat, 160),
                (ContextSource::ProjectBrief, 160),
                (ContextSource::ResultFeedback, 140),
                (ContextSource::CanonSlice, 180),
                (ContextSource::PromiseSlice, 140),
            ],
            AgentTask::ContinuityDiagnostic => vec![
                (ContextSource::CursorPrefix, 160),
                (ContextSource::CanonSlice, 240),
                (ContextSource::ChapterMission, 140),
                (ContextSource::NextBeat, 140),
                (ContextSource::ProjectBrief, 120),
                (ContextSource::ResultFeedback, 120),
            ],
            AgentTask::ChapterGeneration => vec![
                (ContextSource::ProjectBrief, 500),
                (ContextSource::ChapterMission, 700),
                (ContextSource::NextBeat, 700),
                (ContextSource::ResultFeedback, 700),
                (ContextSource::OutlineSlice, 1_000),
                (ContextSource::PreviousChapter, 800),
                (ContextSource::PromiseSlice, 600),
                (ContextSource::CanonSlice, 600),
            ],
            AgentTask::InlineRewrite => vec![
                (ContextSource::SelectedText, 400),
                (ContextSource::CursorPrefix, 160),
                (ContextSource::ChapterMission, 160),
                (ContextSource::NextBeat, 140),
                (ContextSource::ProjectBrief, 120),
                (ContextSource::ResultFeedback, 120),
            ],
            AgentTask::PlanningReview => vec![
                (ContextSource::ChapterMission, 300),
                (ContextSource::ProjectBrief, 220),
                (ContextSource::ResultFeedback, 220),
                (ContextSource::PromiseSlice, 240),
                (ContextSource::CanonSlice, 240),
            ],
            AgentTask::ProposalEvaluation => vec![
                (ContextSource::CanonSlice, 180),
                (ContextSource::DecisionSlice, 120),
                (ContextSource::NextBeat, 120),
                (ContextSource::ChapterMission, 120),
                (ContextSource::ProjectBrief, 120),
                (ContextSource::ResultFeedback, 120),
            ],
            AgentTask::CanonMaintenance => vec![
                (ContextSource::CanonSlice, 600),
                (ContextSource::PromiseSlice, 240),
                (ContextSource::NextBeat, 240),
                (ContextSource::ResultFeedback, 240),
                (ContextSource::ChapterMission, 240),
                (ContextSource::ProjectBrief, 180),
            ],
            AgentTask::ManualRequest => vec![
                (ContextSource::SelectedText, 300),
                (ContextSource::CursorPrefix, 300),
                (ContextSource::ChapterMission, 220),
                (ContextSource::NextBeat, 220),
                (ContextSource::ProjectBrief, 180),
                (ContextSource::ResultFeedback, 180),
                (ContextSource::CanonSlice, 220),
                (ContextSource::PromiseSlice, 180),
            ],
        }
    }
}

/// The assembled context for an agent action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingContextPack {
    pub task: AgentTask,
    pub sources: Vec<ContextExcerpt>,
    pub total_chars: usize,
    pub budget_limit: usize,
    pub budget_report: ContextBudgetReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudgetReport {
    pub total_budget: usize,
    pub used: usize,
    pub wasted: usize,
    pub source_reports: Vec<SourceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReport {
    pub source: String,
    pub requested: usize,
    pub provided: usize,
    pub truncated: bool,
    pub reason: String,
    pub truncation_reason: Option<String>,
}

impl WritingContextPack {
    /// Human-readable explanation of which sources were included, truncated, or excluded and why.
    pub fn explain(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "ContextPack for {:?}: {} chars / {} budget ({} wasted)",
            self.task, self.total_chars, self.budget_limit, self.budget_report.wasted
        ));
        lines.push(format!("{} sources included:", self.sources.len()));
        for source in &self.sources {
            let trunc_note = if source.truncated { " [TRUNCATED]" } else { "" };
            lines.push(format!(
                "  - {:?} (priority {}, {} chars{})",
                source.source, source.priority, source.char_count, trunc_note
            ));
        }
        let included_sources: Vec<_> = self
            .sources
            .iter()
            .map(|s| format!("{:?}", s.source))
            .collect();
        let all_known = [
            "CursorPrefix",
            "ChapterMission",
            "NextBeat",
            "ResultFeedback",
            "ProjectBrief",
            "CanonSlice",
            "PromiseSlice",
            "DecisionSlice",
            "OutlineSlice",
            "CursorSuffix",
            "AuthorStyle",
            "RagExcerpt",
        ];
        let excluded: Vec<_> = all_known
            .iter()
            .filter(|name| !included_sources.iter().any(|s| s == *name))
            .collect();
        if !excluded.is_empty() {
            lines.push(format!(
                "Excluded sources: {}",
                excluded
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for report in &self.budget_report.source_reports {
            if report.truncated {
                lines.push(format!(
                    "  {:?} truncated: requested {} chars, provided {} chars ({})",
                    report.source,
                    report.requested,
                    report.provided,
                    report
                        .truncation_reason
                        .as_deref()
                        .unwrap_or("budget exhausted")
                ));
            }
        }
        lines.join(
            "
",
        )
    }
}

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
    sources.sort_by(|left, right| right.priority.cmp(&left.priority));

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

pub fn seed_chapter_missions_from_outline(
    project_id: &str,
    outline: &[crate::storage::OutlineNode],
    memory: &WriterMemory,
) -> Result<usize, String> {
    let mut seeded = 0usize;
    for node in outline
        .iter()
        .filter(|node| !node.chapter_title.trim().is_empty())
    {
        let summary = compact_context_line(&node.summary, 180);
        let mission = if summary.is_empty() {
            format!(
                "推进 {} 的章节目标，并保持与书级合同一致。",
                node.chapter_title
            )
        } else {
            summary.clone()
        };
        let must_include = infer_mission_must_include(&node.summary);
        let must_not = infer_mission_must_not(&node.summary);
        let expected_ending = infer_mission_expected_ending(&node.summary);
        let did_seed = memory
            .ensure_chapter_mission_seed(
                project_id,
                &node.chapter_title,
                &mission,
                &must_include,
                &must_not,
                &expected_ending,
                "outline.seed",
            )
            .map_err(|e| e.to_string())?;
        if did_seed {
            seeded += 1;
        }
    }
    Ok(seeded)
}

fn infer_mission_must_include(summary: &str) -> String {
    let mut items = Vec::new();
    if contains_any(summary, &["伏笔", "线索", "玉佩", "密道", "钥匙"]) {
        items.push("保留并推进关键线索");
    }
    if contains_any(summary, &["冲突", "对抗", "危机", "敌"]) {
        items.push("让冲突产生可见后果");
    }
    if contains_any(summary, &["关系", "信任", "背叛", "误会"]) {
        items.push("推进角色关系状态变化");
    }
    if items.is_empty() {
        "保持本章目标与大纲摘要一致".to_string()
    } else {
        items.join("；")
    }
}

fn infer_mission_must_not(summary: &str) -> String {
    let mut items = Vec::new();
    if contains_any(summary, &["谜", "真相", "秘密", "身份"]) {
        items.push("不要过早揭开核心谜底");
    }
    if contains_any(summary, &["试探", "怀疑", "误会"]) {
        items.push("不要让角色过早达成完全信任");
    }
    if items.is_empty() {
        "不要跳过因果铺垫或改写已确认设定".to_string()
    } else {
        items.join("；")
    }
}

fn infer_mission_expected_ending(summary: &str) -> String {
    if contains_any(summary, &["危机", "追杀", "敌", "对抗"]) {
        "以新的压力、危险或选择收束。".to_string()
    } else if contains_any(summary, &["线索", "发现", "秘密", "谜"]) {
        "以新的线索或疑问收束。".to_string()
    } else if contains_any(summary, &["关系", "信任", "背叛", "误会"]) {
        "以角色关系状态变化收束。".to_string()
    } else {
        "以明确的状态变化或下一步钩子收束。".to_string()
    }
}

pub fn seed_story_contract_from_project_assets(
    project_id: &str,
    project_name: &str,
    lorebook: &[crate::storage::LoreEntry],
    outline: &[crate::storage::OutlineNode],
    memory: &WriterMemory,
) -> Result<bool, String> {
    let title = if project_name.trim().is_empty() {
        "Untitled Story"
    } else {
        project_name.trim()
    };
    let genre = infer_contract_genre(lorebook, outline);
    let reader_promise = infer_reader_promise(outline);
    let main_conflict = infer_main_conflict(outline, lorebook);
    let structural_boundary = infer_structural_boundary(lorebook, outline);
    memory
        .ensure_story_contract_seed(
            project_id,
            title,
            &genre,
            &reader_promise,
            &main_conflict,
            &structural_boundary,
        )
        .map_err(|e| e.to_string())
}

fn infer_contract_genre(
    lorebook: &[crate::storage::LoreEntry],
    outline: &[crate::storage::OutlineNode],
) -> String {
    let haystack = project_asset_haystack(lorebook, outline);
    if contains_any(&haystack, &["玄幻", "修仙", "灵力", "宗门", "秘境"]) {
        "玄幻/修仙".to_string()
    } else if contains_any(&haystack, &["悬疑", "案件", "凶手", "线索", "侦探"]) {
        "悬疑".to_string()
    } else if contains_any(&haystack, &["末日", "丧尸", "废土", "灾变"]) {
        "末日/废土".to_string()
    } else if contains_any(&haystack, &["星舰", "宇宙", "机甲", "AI", "人工智能"]) {
        "科幻".to_string()
    } else if contains_any(&haystack, &["宫廷", "朝堂", "皇帝", "王府", "江湖"]) {
        "古风/权谋".to_string()
    } else {
        "待定长篇小说".to_string()
    }
}

fn infer_reader_promise(outline: &[crate::storage::OutlineNode]) -> String {
    let first_nodes = outline
        .iter()
        .take(3)
        .map(|node| compact_context_line(&node.summary, 80))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if first_nodes.is_empty() {
        "保持主线清晰、角色选择有后果，并让每章都推动故事状态。".to_string()
    } else {
        format!("围绕开篇承诺推进: {}", first_nodes.join(" / "))
    }
}

fn infer_main_conflict(
    outline: &[crate::storage::OutlineNode],
    lorebook: &[crate::storage::LoreEntry],
) -> String {
    let outline_conflict = outline
        .iter()
        .find(|node| contains_any(&node.summary, &["冲突", "危机", "对抗", "矛盾", "敌"]))
        .map(|node| compact_context_line(&node.summary, 96));
    if let Some(conflict) = outline_conflict.filter(|value| !value.is_empty()) {
        return conflict;
    }

    lorebook
        .iter()
        .find(|entry| contains_any(&entry.content, &["冲突", "危机", "对抗", "矛盾", "敌"]))
        .map(|entry| compact_context_line(&entry.content, 96))
        .unwrap_or_else(|| "待明确: 主角欲望、阻力与长期对立面需要在开篇阶段定盘。".to_string())
}

fn infer_structural_boundary(
    lorebook: &[crate::storage::LoreEntry],
    outline: &[crate::storage::OutlineNode],
) -> String {
    let mut boundaries = Vec::new();
    if !lorebook.is_empty() {
        boundaries.push("不得违背已记录 Lorebook 设定");
    }
    if !outline.is_empty() {
        boundaries.push("不得跳过当前大纲承诺的因果推进");
    }
    if boundaries.is_empty() {
        "先保护作者已写正文，不自动改写既有事实。".to_string()
    } else {
        boundaries.join("；")
    }
}

fn project_asset_haystack(
    lorebook: &[crate::storage::LoreEntry],
    outline: &[crate::storage::OutlineNode],
) -> String {
    let mut text = String::new();
    for entry in lorebook.iter().take(20) {
        text.push_str(&entry.keyword);
        text.push('\n');
        text.push_str(&entry.content);
        text.push('\n');
    }
    for node in outline.iter().take(20) {
        text.push_str(&node.chapter_title);
        text.push('\n');
        text.push_str(&node.summary);
        text.push('\n');
    }
    text
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn compact_context_line(text: &str, max_chars: usize) -> String {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= max_chars {
        cleaned
    } else {
        cleaned.chars().take(max_chars).collect()
    }
}

pub fn assemble_observation_context_with_default_budget(
    task: AgentTask,
    observation: &WriterObservation,
    memory: &WriterMemory,
) -> WritingContextPack {
    let total_budget = task.default_budget();
    assemble_observation_context(task, observation, memory, total_budget)
}

fn non_empty(text: String) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn char_window(text: &str, start: usize, max_chars: usize) -> String {
    let remaining = text.chars().skip(start).collect::<String>();
    truncate_to_budget(&remaining, max_chars).0
}

fn build_canon_slice(
    observation: &WriterObservation,
    memory: &WriterMemory,
    relevance: &WritingRelevance,
    open_promises: &[PlotPromiseSummary],
) -> String {
    let mut lines = Vec::new();
    if let Ok(entities) = memory.list_canon_entities() {
        let mut scored = entities
            .into_iter()
            .filter_map(|entity| {
                let score = score_canon_entity(&entity, observation, relevance, open_promises);
                if score.score > 0 {
                    Some((score, entity))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        scored.sort_by(|(left, left_entity), (right, right_entity)| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left_entity.name.cmp(&right_entity.name))
        });

        for (score, entity) in scored.into_iter().take(6) {
            lines.push(format_canon_line(&entity, &score.reasons));
        }
    }
    if let Ok(rules) = memory.list_canon_rules(6) {
        for rule in rules {
            lines.push(format!(
                "RULE [{} p{}]: {}",
                rule.category, rule.priority, rule.rule
            ));
        }
    }
    lines.join("\n")
}

fn build_promise_slice(
    observation: &WriterObservation,
    promises: &[PlotPromiseSummary],
    relevance: &WritingRelevance,
    decisions: &[CreativeDecisionSummary],
) -> String {
    let mut scored = promises
        .iter()
        .map(|promise| {
            (
                score_promise(promise, observation, relevance, decisions),
                promise,
            )
        })
        .filter(|(score, _)| score.score > 0)
        .collect::<Vec<_>>();
    scored.sort_by(|(left, left_promise), (right, right_promise)| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right_promise.priority.cmp(&left_promise.priority))
            .then_with(|| left_promise.title.cmp(&right_promise.title))
    });

    scored
        .into_iter()
        .take(6)
        .map(|(score, promise)| format_promise_line(promise, &score.reasons))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_style_slice(memory: &WriterMemory) -> String {
    memory
        .list_style_preferences(6)
        .unwrap_or_default()
        .into_iter()
        .map(|pref| {
            format!(
                "{}: {} (+{} / -{})",
                pref.key, pref.value, pref.accepted_count, pref.rejected_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_decision_slice(decisions: &[CreativeDecisionSummary]) -> String {
    decisions
        .iter()
        .map(|decision| {
            format!(
                "{} [{}]: {}",
                decision.title, decision.decision, decision.rationale
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Truncate text to fit budget, preferring sentence boundaries.
fn truncate_to_budget(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    let truncated: String = text.chars().take(max_chars).collect();
    // Try to break at a sentence boundary using char-level scanning
    let chars: Vec<char> = truncated.chars().collect();
    if let Some(last_period) = chars.iter().rposition(|&c| {
        c == '\u{3002}' || c == '\u{FF01}' || c == '\u{FF1F}'  // 。！？
        || c == '.' || c == '!' || c == '?'
    }) {
        (chars[..=last_period].iter().collect(), true)
    } else {
        (truncated, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer_agent::observation::{ObservationReason, ObservationSource};

    #[test]
    fn test_ghost_writing_priorities() {
        let p = AgentTask::GhostWriting.source_priorities();
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::CursorPrefix));
        assert!(p
            .iter()
            .any(|(s, _, _)| *s == ContextSource::ResultFeedback));
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::NextBeat));
        assert!(p[0].0 == ContextSource::CursorPrefix); // highest priority
    }

    #[test]
    fn test_chapter_gen_includes_all_sources() {
        let p = AgentTask::ChapterGeneration.source_priorities();
        assert!(p
            .iter()
            .any(|(s, _, _)| *s == ContextSource::PreviousChapter));
        assert!(p
            .iter()
            .any(|(s, _, _)| *s == ContextSource::ResultFeedback));
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::NextBeat));
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::PromiseSlice));
    }

    #[test]
    fn test_assemble_respects_budget() {
        let provider = |s: ContextSource| -> Option<String> {
            match s {
                ContextSource::CursorPrefix => Some("前缀文本".repeat(100)),
                ContextSource::CursorSuffix => Some("后缀".repeat(50)),
                ContextSource::CanonSlice => Some("canon数据".repeat(80)),
                _ => None,
            }
        };
        let pack = assemble_context_pack(AgentTask::GhostWriting, &provider, 500);
        assert!(pack.total_chars <= 500);
        assert!(!pack.sources.is_empty());
    }

    #[test]
    fn test_required_sources_survive_tight_budget() {
        let provider = |s: ContextSource| -> Option<String> {
            match s {
                ContextSource::CursorPrefix => Some("长前文。".repeat(500)),
                ContextSource::CanonSlice => Some("林墨 weapon=寒影刀".repeat(20)),
                ContextSource::PromiseSlice => Some("玉佩仍未交代下落。".repeat(20)),
                ContextSource::DecisionSlice => Some("保持克制，不用大段自白。".repeat(20)),
                _ => None,
            }
        };
        let pack = assemble_context_pack(AgentTask::GhostWriting, &provider, 620);

        assert!(pack.total_chars <= 620);
        assert!(pack
            .sources
            .iter()
            .any(|source| source.source == ContextSource::CursorPrefix));
        assert!(pack
            .sources
            .iter()
            .any(|source| source.source == ContextSource::CanonSlice));
        assert!(pack
            .sources
            .iter()
            .any(|source| source.source == ContextSource::PromiseSlice));
        assert!(pack
            .budget_report
            .source_reports
            .iter()
            .any(|report| report.source == "CanonSlice" && report.provided > 0));
    }

    #[test]
    fn test_budget_report_records_dropped_sources() {
        let provider = |s: ContextSource| -> Option<String> {
            match s {
                ContextSource::CursorPrefix => Some("长前文。".repeat(200)),
                ContextSource::ChapterMission => Some("本章必须追查玉佩。".repeat(40)),
                ContextSource::AuthorStyle => Some("对白保持克制，用动作暗示情绪。".repeat(40)),
                _ => None,
            }
        };
        let pack = assemble_context_pack(AgentTask::GhostWriting, &provider, 240);

        assert!(pack.total_chars <= 240);
        assert!(pack
            .budget_report
            .source_reports
            .iter()
            .any(|report| report.source == "CursorPrefix" && report.provided > 0));
        let dropped = pack
            .budget_report
            .source_reports
            .iter()
            .find(|report| report.source == "AuthorStyle")
            .expect("budget report should include dropped source with available content");
        assert_eq!(dropped.provided, 0);
        assert!(dropped.reason.contains("dropped"));
        assert!(dropped.truncated);
        assert!(dropped.truncation_reason.is_some());
    }

    #[test]
    fn test_truncate_sentence_boundary() {
        let text = "第一句。第二句。第三句。第四句。";
        let (result, truncated) = truncate_to_budget(text, 8);
        assert!(truncated, "text longer than budget should be truncated");
        // After truncation, the result should be shorter than input
        assert!(result.chars().count() < text.chars().count());
    }

    #[test]
    fn test_task_distinct_budgets() {
        let ghost = AgentTask::GhostWriting.source_priorities();
        let chapter = AgentTask::ChapterGeneration.source_priorities();
        // Chapter generation gets much larger budget
        let ghost_total: usize = ghost.iter().map(|(_, _, b)| b).sum();
        let chapter_total: usize = chapter.iter().map(|(_, _, b)| b).sum();
        assert!(chapter_total > ghost_total * 3);
    }

    #[test]
    fn test_default_task_budgets_match_agent_paths() {
        assert_eq!(AgentTask::GhostWriting.default_budget(), 3_000);
        assert_eq!(AgentTask::InlineRewrite.default_budget(), 4_500);
        assert_eq!(AgentTask::ManualRequest.default_budget(), 4_500);
        assert!(
            AgentTask::ChapterGeneration.default_budget()
                > AgentTask::GhostWriting.default_budget()
        );
    }

    #[test]
    fn test_manual_request_prioritizes_selection_and_ledgers() {
        let p = AgentTask::ManualRequest.source_priorities();
        assert_eq!(p[0].0, ContextSource::SelectedText);
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::CanonSlice));
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::PromiseSlice));
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::DecisionSlice));
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::AuthorStyle));
    }

    #[test]
    fn test_observation_context_includes_relevant_ledgers() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .ensure_story_contract_seed(
                "default",
                "寒影",
                "玄幻",
                "刀客在旧怨中追查玉佩真相。",
                "林墨必须在复仇和守护之间做选择。",
                "不得提前泄露玉佩来源。",
            )
            .unwrap();
        memory
            .ensure_chapter_mission_seed(
                "default",
                "Chapter-1",
                "林墨在旧门前试探屋内人的真实立场。",
                "保留玉佩线索",
                "不要提前揭开玉佩来源",
                "以新的疑问收束。",
                "test",
            )
            .unwrap();
        memory
            .upsert_canon_entity(
                "character",
                "林墨",
                &[],
                "主角",
                &serde_json::json!({ "weapon": "寒影刀" }),
                0.9,
            )
            .unwrap();
        memory
            .add_promise("clue", "玉佩", "张三拿走玉佩", "Chapter-1", "Chapter-5", 3)
            .unwrap();
        memory
            .upsert_style_preference("dialogue", "prefers_subtext", true)
            .unwrap();
        memory
            .record_decision(
                "Chapter-1",
                "林墨不主动解释",
                "accepted",
                &[],
                "保持克制，不用大段自白。",
                &[],
            )
            .unwrap();
        memory
            .record_chapter_result(&crate::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "default".into(),
                chapter_title: "Chapter-0".into(),
                chapter_revision: "rev-0".into(),
                summary: "上一章林墨确认玉佩仍在张三手里。".into(),
                state_changes: vec!["林墨开始怀疑张三".into()],
                character_progress: vec![],
                new_conflicts: vec!["林墨与张三信任受损".into()],
                new_clues: vec!["玉佩".into()],
                promise_updates: vec![],
                canon_updates: vec![],
                source_ref: "test".into(),
                created_at: 2,
            })
            .unwrap();

        let observation = WriterObservation {
            id: "obs".into(),
            created_at: 1,
            source: ObservationSource::Editor,
            reason: ObservationReason::Idle,
            project_id: "default".into(),
            chapter_title: Some("Chapter-1".into()),
            chapter_revision: Some("rev".into()),
            cursor: None,
            selection: None,
            prefix: "林墨停在门前。".into(),
            suffix: String::new(),
            paragraph: "林墨停在门前。".into(),
            full_text_digest: None,
            editor_dirty: true,
        };
        let pack = assemble_observation_context_with_default_budget(
            AgentTask::GhostWriting,
            &observation,
            &memory,
        );

        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::CursorPrefix));
        assert_eq!(pack.budget_limit, AgentTask::GhostWriting.default_budget());
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::ProjectBrief));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::ChapterMission));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::NextBeat));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::ResultFeedback));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::CanonSlice));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::PromiseSlice));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::DecisionSlice));
        assert!(pack
            .sources
            .iter()
            .any(|s| s.source == ContextSource::AuthorStyle));
    }

    #[test]
    fn test_seed_story_contract_from_project_assets() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let seeded = seed_story_contract_from_project_assets(
            "novel-a",
            "寒影录",
            &[crate::storage::LoreEntry {
                id: "1".to_string(),
                keyword: "林墨".to_string(),
                content: "林墨来自宗门，惯用寒影刀。".to_string(),
            }],
            &[crate::storage::OutlineNode {
                chapter_title: "第一章".to_string(),
                summary: "林墨卷入宗门危机，发现玉佩线索。".to_string(),
                status: "draft".to_string(),
            }],
            &memory,
        )
        .unwrap();

        assert!(seeded);
        let contract = memory.get_story_contract("novel-a").unwrap().unwrap();
        assert_eq!(contract.title, "寒影录");
        assert_eq!(contract.genre, "玄幻/修仙");
        assert!(contract.reader_promise.contains("林墨"));

        let seeded_again =
            seed_story_contract_from_project_assets("novel-a", "新标题不应覆盖", &[], &[], &memory)
                .unwrap();
        assert!(!seeded_again);
        assert_eq!(
            memory.get_story_contract("novel-a").unwrap().unwrap().title,
            "寒影录"
        );
    }

    #[test]
    fn test_seed_chapter_missions_from_outline() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let outline = vec![
            crate::storage::OutlineNode {
                chapter_title: "第一章".to_string(),
                summary: "林墨发现玉佩线索，引出宗门危机。".to_string(),
                status: "draft".to_string(),
            },
            crate::storage::OutlineNode {
                chapter_title: "第二章".to_string(),
                summary: "林墨与张三产生误会，关系开始紧张。".to_string(),
                status: "draft".to_string(),
            },
        ];

        let seeded = seed_chapter_missions_from_outline("novel-a", &outline, &memory).unwrap();

        assert_eq!(seeded, 2);
        let mission = memory
            .get_chapter_mission("novel-a", "第一章")
            .unwrap()
            .unwrap();
        assert!(mission.mission.contains("玉佩"));
        assert!(mission.must_include.contains("线索"));

        let seeded_again =
            seed_chapter_missions_from_outline("novel-a", &outline, &memory).unwrap();
        assert_eq!(seeded_again, 0);
    }
}
