//! ContextEngine — deterministic context packs for every agent action.
//! Replaces ad-hoc prompt assembly with budgeted, priority-ordered context sources.

use serde::{Deserialize, Serialize};

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
}

impl AgentTask {
    pub fn source_priorities(&self) -> Vec<(ContextSource, u8, usize)> {
        match self {
            AgentTask::GhostWriting => vec![
                (ContextSource::CursorPrefix, 10, 800),
                (ContextSource::CursorSuffix, 9, 400),
                (ContextSource::CanonSlice, 8, 600),
                (ContextSource::PromiseSlice, 7, 400),
                (ContextSource::OutlineSlice, 6, 500),
                (ContextSource::AuthorStyle, 5, 300),
                (ContextSource::RagExcerpt, 4, 400),
            ],
            AgentTask::ContinuityDiagnostic => vec![
                (ContextSource::CursorPrefix, 10, 300),
                (ContextSource::CanonSlice, 10, 800),
                (ContextSource::OutlineSlice, 9, 500),
                (ContextSource::RagExcerpt, 8, 600),
            ],
            AgentTask::ChapterGeneration => vec![
                (ContextSource::OutlineSlice, 10, 6000),
                (ContextSource::PreviousChapter, 9, 5000),
                (ContextSource::PromiseSlice, 8, 4000),
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
                (ContextSource::AuthorStyle, 6, 300),
            ],
            AgentTask::ProposalEvaluation => vec![
                (ContextSource::CanonSlice, 10, 500),
                (ContextSource::PromiseSlice, 9, 300),
                (ContextSource::AuthorStyle, 8, 300),
            ],
            AgentTask::CanonMaintenance => vec![
                (ContextSource::CanonSlice, 10, 2000),
                (ContextSource::PromiseSlice, 9, 1000),
                (ContextSource::OutlineSlice, 8, 1000),
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

    for (source, priority, budget) in &priorities {
        let remaining = total_budget.saturating_sub(used);
        let alloc = (*budget).min(remaining);
        if alloc == 0 { break; }

        if let Some(raw) = source_provider(source.clone()) {
            let (content, truncated) = truncate_to_budget(&raw, alloc);
            let char_count = content.chars().count();
            used += char_count;

            source_reports.push(SourceReport {
                source: format!("{:?}", source),
                requested: *budget,
                provided: char_count,
                truncated,
            });

            sources.push(ContextExcerpt {
                source: source.clone(),
                content,
                char_count,
                truncated,
                priority: *priority,
                evidence_ref: None,
            });
        }
    }

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

    #[test]
    fn test_ghost_writing_priorities() {
        let p = AgentTask::GhostWriting.source_priorities();
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::CursorPrefix));
        assert!(p[0].0 == ContextSource::CursorPrefix); // highest priority
    }

    #[test]
    fn test_chapter_gen_includes_all_sources() {
        let p = AgentTask::ChapterGeneration.source_priorities();
        assert!(p.iter().any(|(s, _, _)| *s == ContextSource::PreviousChapter));
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
}
