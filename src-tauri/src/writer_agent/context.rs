//! ContextEngine — deterministic context packs for every agent action.
//! Replaces ad-hoc prompt assembly with budgeted, priority-ordered context sources.

use serde::{Deserialize, Serialize};

use super::memory::WriterMemory;
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
            AgentTask::ProposalEvaluation => 1_400,
            AgentTask::CanonMaintenance => 4_500,
            AgentTask::ManualRequest => 4_500,
        }
    }

    pub fn source_priorities(&self) -> Vec<(ContextSource, u8, usize)> {
        match self {
            AgentTask::GhostWriting => vec![
                (ContextSource::CursorPrefix, 10, 800),
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
                (ContextSource::DecisionSlice, 9, 300),
                (ContextSource::OutlineSlice, 9, 500),
                (ContextSource::RagExcerpt, 8, 600),
            ],
            AgentTask::ChapterGeneration => vec![
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
                (ContextSource::CanonSlice, 7, 400),
                (ContextSource::DecisionSlice, 7, 300),
                (ContextSource::AuthorStyle, 6, 300),
            ],
            AgentTask::ProposalEvaluation => vec![
                (ContextSource::CanonSlice, 10, 500),
                (ContextSource::PromiseSlice, 9, 300),
                (ContextSource::DecisionSlice, 9, 300),
                (ContextSource::AuthorStyle, 8, 300),
            ],
            AgentTask::CanonMaintenance => vec![
                (ContextSource::CanonSlice, 10, 2000),
                (ContextSource::PromiseSlice, 9, 1000),
                (ContextSource::DecisionSlice, 9, 800),
                (ContextSource::OutlineSlice, 8, 1000),
            ],
            AgentTask::ManualRequest => vec![
                (ContextSource::SelectedText, 10, 1200),
                (ContextSource::CursorPrefix, 9, 1400),
                (ContextSource::CursorSuffix, 8, 500),
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
                (ContextSource::CanonSlice, 180),
                (ContextSource::PromiseSlice, 140),
            ],
            AgentTask::ContinuityDiagnostic => vec![
                (ContextSource::CursorPrefix, 160),
                (ContextSource::CanonSlice, 240),
            ],
            AgentTask::ChapterGeneration => vec![
                (ContextSource::OutlineSlice, 1_000),
                (ContextSource::PreviousChapter, 800),
                (ContextSource::PromiseSlice, 600),
                (ContextSource::CanonSlice, 600),
            ],
            AgentTask::InlineRewrite => vec![
                (ContextSource::SelectedText, 400),
                (ContextSource::CursorPrefix, 160),
            ],
            AgentTask::ProposalEvaluation => vec![
                (ContextSource::CanonSlice, 180),
                (ContextSource::DecisionSlice, 120),
            ],
            AgentTask::CanonMaintenance => vec![
                (ContextSource::CanonSlice, 600),
                (ContextSource::PromiseSlice, 240),
            ],
            AgentTask::ManualRequest => vec![
                (ContextSource::SelectedText, 300),
                (ContextSource::CursorPrefix, 300),
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
        if draft.consumed > 0 {
            source_reports.push(SourceReport {
                source: format!("{:?}", draft.source),
                requested: draft.requested,
                provided: draft.consumed,
                truncated: draft.raw.chars().count() > draft.consumed,
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

pub fn assemble_observation_context(
    task: AgentTask,
    observation: &WriterObservation,
    memory: &WriterMemory,
    total_budget: usize,
) -> WritingContextPack {
    let canon_slice = build_canon_slice(&observation.paragraph, memory);
    let promise_slice = build_promise_slice(memory);
    let decision_slice = build_decision_slice(memory);
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
            ContextSource::CanonSlice => non_empty(canon_slice.clone()),
            ContextSource::PromiseSlice => non_empty(promise_slice.clone()),
            ContextSource::DecisionSlice => non_empty(decision_slice.clone()),
            ContextSource::AuthorStyle => non_empty(author_style.clone()),
            _ => None,
        },
        total_budget,
    )
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

fn build_canon_slice(paragraph: &str, memory: &WriterMemory) -> String {
    let mut lines = Vec::new();
    if let Ok(entities) = memory.list_canon_entities() {
        for entity in entities
            .into_iter()
            .filter(|entity| paragraph.contains(&entity.name))
            .take(6)
        {
            let attrs = if let Some(map) = entity.attributes.as_object() {
                map.iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };
            lines.push(format!(
                "{} [{}] {} {}",
                entity.name, entity.kind, entity.summary, attrs
            ));
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

fn build_promise_slice(memory: &WriterMemory) -> String {
    memory
        .get_open_promise_summaries()
        .unwrap_or_default()
        .into_iter()
        .take(6)
        .map(|promise| {
            format!(
                "{} [{}]: {} -> {}",
                promise.title, promise.kind, promise.description, promise.expected_payoff
            )
        })
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

fn build_decision_slice(memory: &WriterMemory) -> String {
    memory
        .list_recent_decisions(6)
        .unwrap_or_default()
        .into_iter()
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
        assert!(p[0].0 == ContextSource::CursorPrefix); // highest priority
    }

    #[test]
    fn test_chapter_gen_includes_all_sources() {
        let p = AgentTask::ChapterGeneration.source_priorities();
        assert!(p
            .iter()
            .any(|(s, _, _)| *s == ContextSource::PreviousChapter));
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
}
