use agent_harness_core::provider::LlmMessage;
use agent_harness_core::{
    FeedbackContract, Intent, RequiredContext, TaskBelief, TaskPacket, TaskScope,
    ToolPolicyContract, ToolSideEffectLevel, VectorDB,
};
use serde::{Deserialize, Serialize};

use crate::writer_agent::context_relevance::{format_text_chunk_relevance, rerank_text_chunks};
use crate::writer_agent::provider_budget::{
    apply_provider_budget_approval, evaluate_provider_budget, WriterProviderBudgetApproval,
    WriterProviderBudgetDecision, WriterProviderBudgetReport, WriterProviderBudgetRequest,
    WriterProviderBudgetTask,
};
use crate::writer_agent::task_receipt::{
    WriterFailureCategory, WriterFailureEvidenceBundle, WriterTaskReceipt,
};
use crate::{llm_runtime, storage};

pub const PHASE_STARTED: &str = "chapter_generation_started";
pub const PHASE_CONTEXT_BUILT: &str = "chapter_generation_context_built";
pub const PHASE_PROGRESS: &str = "chapter_generation_progress";
pub const PHASE_CONFLICT: &str = "chapter_generation_conflict";
pub const PHASE_COMPLETED: &str = "chapter_generation_completed";
pub const PHASE_FAILED: &str = "chapter_generation_failed";

const DEFAULT_TOTAL_CONTEXT_CHARS: usize = 24_000;
const DEFAULT_INSTRUCTION_CHARS: usize = 1_000;
const DEFAULT_OUTLINE_CHARS: usize = 6_000;
const DEFAULT_PREVIOUS_CHAPTERS_CHARS: usize = 5_000;
const DEFAULT_NEXT_CHAPTER_CHARS: usize = 2_000;
const DEFAULT_TARGET_EXISTING_CHARS: usize = 3_000;
const DEFAULT_LOREBOOK_CHARS: usize = 5_000;
const DEFAULT_USER_PROFILE_CHARS: usize = 4_000;
const DEFAULT_RAG_CHARS: usize = 4_000;
const DEFAULT_PREVIOUS_CHAPTER_COUNT: usize = 2;
const DEFAULT_NEXT_CHAPTER_COUNT: usize = 1;
const DEFAULT_LOREBOOK_ENTRY_COUNT: usize = 4;
const DEFAULT_USER_PROFILE_ENTRY_COUNT: usize = 6;
const DEFAULT_RAG_CHUNK_COUNT: usize = 5;
const DEFAULT_OUTPUT_SOFT_CAP_CHARS: usize = 12_000;
const DEFAULT_OUTPUT_HARD_CAP_CHARS: usize = 30_000;
const PROVIDER_TIMEOUT_SECS: u64 = 120;
const CHAPTER_GENERATION_OUTPUT_TOKENS: u64 = DEFAULT_OUTPUT_SOFT_CAP_CHARS as u64 / 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateChapterAutonomousPayload {
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub target_chapter_title: Option<String>,
    #[serde(default)]
    pub target_chapter_number: Option<usize>,
    pub user_instruction: String,
    #[serde(default)]
    pub budget: Option<ChapterContextBudget>,
    #[serde(default)]
    pub frontend_state: Option<FrontendChapterStateSnapshot>,
    #[serde(default)]
    pub save_mode: SaveMode,
    #[serde(default)]
    pub chapter_summary_override: Option<String>,
    #[serde(default)]
    pub provider_budget_approval: Option<WriterProviderBudgetApproval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendChapterStateSnapshot {
    #[serde(default)]
    pub open_chapter_title: Option<String>,
    #[serde(default)]
    pub open_chapter_revision: Option<String>,
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveMode {
    CreateIfMissing,
    #[default]
    ReplaceIfClean,
    SaveAsDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterContextBudget {
    #[serde(default = "default_total_context_chars")]
    pub total_chars: usize,
    #[serde(default = "default_instruction_chars")]
    pub instruction_chars: usize,
    #[serde(default = "default_outline_chars")]
    pub outline_chars: usize,
    #[serde(default = "default_previous_chapters_chars")]
    pub previous_chapters_chars: usize,
    #[serde(default = "default_next_chapter_chars")]
    pub next_chapter_chars: usize,
    #[serde(default = "default_target_existing_chars")]
    pub target_existing_chars: usize,
    #[serde(default = "default_lorebook_chars")]
    pub lorebook_chars: usize,
    #[serde(default = "default_user_profile_chars")]
    pub user_profile_chars: usize,
    #[serde(default = "default_rag_chars")]
    pub rag_chars: usize,
    #[serde(default = "default_previous_chapter_count")]
    pub previous_chapter_count: usize,
    #[serde(default = "default_next_chapter_count")]
    pub next_chapter_count: usize,
    #[serde(default = "default_lorebook_entry_count")]
    pub lorebook_entry_count: usize,
    #[serde(default = "default_user_profile_entry_count")]
    pub user_profile_entry_count: usize,
    #[serde(default = "default_rag_chunk_count")]
    pub rag_chunk_count: usize,
}

impl Default for ChapterContextBudget {
    fn default() -> Self {
        Self {
            total_chars: DEFAULT_TOTAL_CONTEXT_CHARS,
            instruction_chars: DEFAULT_INSTRUCTION_CHARS,
            outline_chars: DEFAULT_OUTLINE_CHARS,
            previous_chapters_chars: DEFAULT_PREVIOUS_CHAPTERS_CHARS,
            next_chapter_chars: DEFAULT_NEXT_CHAPTER_CHARS,
            target_existing_chars: DEFAULT_TARGET_EXISTING_CHARS,
            lorebook_chars: DEFAULT_LOREBOOK_CHARS,
            user_profile_chars: DEFAULT_USER_PROFILE_CHARS,
            rag_chars: DEFAULT_RAG_CHARS,
            previous_chapter_count: DEFAULT_PREVIOUS_CHAPTER_COUNT,
            next_chapter_count: DEFAULT_NEXT_CHAPTER_COUNT,
            lorebook_entry_count: DEFAULT_LOREBOOK_ENTRY_COUNT,
            user_profile_entry_count: DEFAULT_USER_PROFILE_ENTRY_COUNT,
            rag_chunk_count: DEFAULT_RAG_CHUNK_COUNT,
        }
    }
}

fn default_total_context_chars() -> usize {
    DEFAULT_TOTAL_CONTEXT_CHARS
}

fn default_instruction_chars() -> usize {
    DEFAULT_INSTRUCTION_CHARS
}

fn default_outline_chars() -> usize {
    DEFAULT_OUTLINE_CHARS
}

fn default_previous_chapters_chars() -> usize {
    DEFAULT_PREVIOUS_CHAPTERS_CHARS
}

fn default_next_chapter_chars() -> usize {
    DEFAULT_NEXT_CHAPTER_CHARS
}

fn default_target_existing_chars() -> usize {
    DEFAULT_TARGET_EXISTING_CHARS
}

fn default_lorebook_chars() -> usize {
    DEFAULT_LOREBOOK_CHARS
}

fn default_user_profile_chars() -> usize {
    DEFAULT_USER_PROFILE_CHARS
}

fn default_rag_chars() -> usize {
    DEFAULT_RAG_CHARS
}

fn default_previous_chapter_count() -> usize {
    DEFAULT_PREVIOUS_CHAPTER_COUNT
}

fn default_next_chapter_count() -> usize {
    DEFAULT_NEXT_CHAPTER_COUNT
}

fn default_lorebook_entry_count() -> usize {
    DEFAULT_LOREBOOK_ENTRY_COUNT
}

fn default_user_profile_entry_count() -> usize {
    DEFAULT_USER_PROFILE_ENTRY_COUNT
}

fn default_rag_chunk_count() -> usize {
    DEFAULT_RAG_CHUNK_COUNT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterTarget {
    pub title: String,
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<usize>,
    pub summary: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterContextSource {
    pub source_type: String,
    pub id: String,
    pub label: String,
    pub original_chars: usize,
    pub included_chars: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterContextBudgetReport {
    pub max_chars: usize,
    pub included_chars: usize,
    pub source_count: usize,
    pub truncated_source_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltChapterContext {
    pub request_id: String,
    pub target: ChapterTarget,
    pub base_revision: String,
    pub prompt_context: String,
    pub sources: Vec<ChapterContextSource>,
    pub budget: ChapterContextBudgetReport,
    pub warnings: Vec<String>,
    pub receipt: WriterTaskReceipt,
}

#[derive(Debug, Clone)]
pub struct BuildChapterContextInput {
    pub request_id: String,
    pub target_chapter_title: Option<String>,
    pub target_chapter_number: Option<usize>,
    pub user_instruction: String,
    pub budget: ChapterContextBudget,
    pub chapter_summary_override: Option<String>,
    pub user_profile_entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterGenerationError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Box<WriterFailureEvidenceBundle>>,
}

impl ChapterGenerationError {
    pub fn new(code: &str, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            recoverable,
            details: None,
            evidence: None,
        }
    }

    pub fn with_details(
        code: &str,
        message: impl Into<String>,
        recoverable: bool,
        details: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            recoverable,
            details: Some(details.into()),
            evidence: None,
        }
    }

    pub fn with_evidence(mut self, evidence: Box<WriterFailureEvidenceBundle>) -> Self {
        self.evidence = Some(evidence);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateChapterDraftOutput {
    #[serde(skip_serializing)]
    pub content: String,
    pub finish_reason: String,
    pub model: String,
    pub provider: String,
    pub output_chars: usize,
    pub base_revision: String,
    pub provider_budget: WriterProviderBudgetReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflict {
    pub reason: String,
    pub base_revision: String,
    pub current_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_chapter_title: Option<String>,
    pub dirty: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveGeneratedChapterOutput {
    pub chapter_title: String,
    pub new_revision: String,
    pub saved_mode: String,
}

#[derive(Debug, Clone)]
pub struct SaveGeneratedChapterInput {
    pub request_id: String,
    pub target: ChapterTarget,
    pub generated_content: String,
    pub base_revision: String,
    pub save_mode: SaveMode,
    pub frontend_state: Option<FrontendChapterStateSnapshot>,
    pub receipt: WriterTaskReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineUpdateOutput {
    pub outline_revision: String,
    pub changed: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterGenerationEvent {
    pub request_id: String,
    pub phase: String,
    pub status: String,
    pub message: String,
    pub progress: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_chapter_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<ChapterContextSource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<ChapterContextBudgetReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<WriterTaskReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved: Option<SaveGeneratedChapterOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<SaveConflict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ChapterGenerationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PipelineTerminal {
    Completed {
        saved: SaveGeneratedChapterOutput,
        generated_content: String,
    },
    Conflict(SaveConflict),
    Failed(ChapterGenerationError),
}

#[derive(Debug, Clone)]
pub enum SaveDecision {
    WriteTarget,
    WriteDraft {
        draft_title: String,
        conflict: SaveConflict,
    },
    Conflict(SaveConflict),
}

pub fn char_count(text: &str) -> usize {
    text.chars().count()
}

pub fn truncate_text_report(text: &str, max_chars: usize) -> (String, usize, bool) {
    let original_chars = char_count(text);
    if original_chars <= max_chars {
        return (text.to_string(), original_chars, false);
    }

    if max_chars == 0 {
        return (String::new(), 0, true);
    }

    let end_byte = text
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    let candidate = &text[..end_byte];
    let min_boundary_chars = max_chars.saturating_div(3).max(1);
    let mut chosen_boundary: Option<usize> = None;
    let mut seen_chars = 0usize;

    for (idx, ch) in candidate.char_indices() {
        seen_chars += 1;
        if seen_chars >= min_boundary_chars
            && matches!(ch, '\n' | '。' | '！' | '？' | '；' | ';' | '.' | '!' | '?')
        {
            chosen_boundary = Some(idx + ch.len_utf8());
        }
    }

    let truncated = if let Some(boundary) = chosen_boundary {
        candidate[..boundary].trim_end().to_string()
    } else {
        candidate.trim_end().to_string()
    };
    let included_chars = char_count(&truncated);
    (truncated, included_chars, true)
}

#[allow(clippy::result_large_err)]
pub fn resolve_target_from_outline(
    outline: &[storage::OutlineNode],
    target_chapter_title: Option<&str>,
    target_chapter_number: Option<usize>,
    summary_override: Option<&str>,
) -> Result<ChapterTarget, ChapterGenerationError> {
    if let Some(title) = target_chapter_title
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let matches: Vec<(usize, &storage::OutlineNode)> = outline
            .iter()
            .enumerate()
            .filter(|(_, node)| node.chapter_title == title)
            .collect();

        if matches.len() > 1 {
            return Err(ChapterGenerationError::new(
                "TARGET_CHAPTER_AMBIGUOUS",
                format!("More than one outline node is titled '{}'.", title),
                true,
            ));
        }

        if let Some((idx, node)) = matches.first() {
            return Ok(ChapterTarget {
                title: node.chapter_title.clone(),
                filename: storage::chapter_filename(&node.chapter_title),
                number: Some(idx + 1),
                summary: summary_override
                    .map(str::to_string)
                    .unwrap_or_else(|| node.summary.clone()),
                status: node.status.clone(),
            });
        }

        if let Some(summary) = summary_override {
            return Ok(ChapterTarget {
                title: title.to_string(),
                filename: storage::chapter_filename(title),
                number: None,
                summary: summary.to_string(),
                status: "empty".to_string(),
            });
        }

        return Err(ChapterGenerationError::new(
            "TARGET_CHAPTER_NOT_FOUND",
            format!("No outline node found for '{}'.", title),
            true,
        ));
    }

    if let Some(number) = target_chapter_number {
        if number == 0 {
            return Err(ChapterGenerationError::new(
                "TARGET_CHAPTER_NOT_FOUND",
                "Chapter numbers start at 1.",
                true,
            ));
        }
        if let Some(node) = outline.get(number - 1) {
            return Ok(ChapterTarget {
                title: node.chapter_title.clone(),
                filename: storage::chapter_filename(&node.chapter_title),
                number: Some(number),
                summary: summary_override
                    .map(str::to_string)
                    .unwrap_or_else(|| node.summary.clone()),
                status: node.status.clone(),
            });
        }
        return Err(ChapterGenerationError::new(
            "TARGET_CHAPTER_NOT_FOUND",
            format!("Outline has no chapter {}.", number),
            true,
        ));
    }

    Err(ChapterGenerationError::new(
        "TARGET_CHAPTER_NOT_FOUND",
        "No target chapter title or number was provided.",
        true,
    ))
}

pub fn decide_save_action(
    target_title: &str,
    request_id: &str,
    save_mode: SaveMode,
    base_revision: &str,
    current_revision: &str,
    frontend_state: Option<&FrontendChapterStateSnapshot>,
) -> SaveDecision {
    let mut conflict_reason: Option<String> = None;
    let mut open_title = None;
    let mut dirty = false;

    if let Some(frontend) = frontend_state {
        open_title = frontend.open_chapter_title.clone();
        dirty = frontend.dirty;
        if frontend
            .open_chapter_title
            .as_deref()
            .map(|title| title == target_title)
            .unwrap_or(false)
            && frontend.dirty
        {
            conflict_reason = Some("frontend_dirty_open_chapter".to_string());
        }
    }

    if conflict_reason.is_none()
        && save_mode == SaveMode::CreateIfMissing
        && current_revision != "missing"
    {
        conflict_reason = Some("target_already_exists".to_string());
    }

    if conflict_reason.is_none() && current_revision != base_revision {
        conflict_reason = Some("revision_mismatch".to_string());
    }

    if let Some(reason) = conflict_reason {
        let draft_title = if save_mode == SaveMode::SaveAsDraft {
            Some(make_draft_title(target_title, request_id))
        } else {
            None
        };
        let conflict = SaveConflict {
            reason,
            base_revision: base_revision.to_string(),
            current_revision: current_revision.to_string(),
            open_chapter_title: open_title,
            dirty,
            draft_title: draft_title.clone(),
        };

        if let Some(draft_title) = draft_title {
            SaveDecision::WriteDraft {
                draft_title,
                conflict,
            }
        } else {
            SaveDecision::Conflict(conflict)
        }
    } else {
        SaveDecision::WriteTarget
    }
}

pub fn validate_generated_content(content: &str) -> Result<(), ChapterGenerationError> {
    if content.trim().is_empty() {
        return Err(ChapterGenerationError::new(
            "MODEL_OUTPUT_EMPTY",
            "The model returned empty chapter content.",
            true,
        ));
    }

    let output_chars = char_count(content);
    if output_chars > DEFAULT_OUTPUT_HARD_CAP_CHARS {
        return Err(ChapterGenerationError::new(
            "MODEL_OUTPUT_TOO_LARGE",
            format!(
                "The model returned {} characters, above the hard cap of {}.",
                output_chars, DEFAULT_OUTPUT_HARD_CAP_CHARS
            ),
            true,
        ));
    }

    Ok(())
}

