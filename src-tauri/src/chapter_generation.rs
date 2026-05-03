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
    pub evidence: Option<WriterFailureEvidenceBundle>,
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

    pub fn with_evidence(mut self, evidence: WriterFailureEvidenceBundle) -> Self {
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

pub fn build_chapter_context(
    app: &tauri::AppHandle,
    input: BuildChapterContextInput,
) -> Result<BuiltChapterContext, ChapterGenerationError> {
    let instruction = input.user_instruction.trim();
    if instruction.is_empty() {
        return Err(ChapterGenerationError::new(
            "INSTRUCTION_EMPTY",
            "The chapter generation instruction is empty.",
            true,
        ));
    }

    let outline = storage::load_outline(app).map_err(|e| {
        ChapterGenerationError::with_details(
            "STORAGE_READ_FAILED",
            "Failed to read outline.",
            true,
            e,
        )
    })?;

    let target = resolve_target_from_outline(
        &outline,
        input.target_chapter_title.as_deref(),
        input.target_chapter_number,
        input.chapter_summary_override.as_deref(),
    )?;

    let base_revision = storage::chapter_revision(app, &target.title).map_err(|e| {
        ChapterGenerationError::with_details(
            "STORAGE_READ_FAILED",
            "Failed to read target chapter revision.",
            true,
            e,
        )
    })?;

    let query = format!("{}\n{}\n{}", instruction, target.title, target.summary);
    let mut composer = ContextComposer::new(input.budget.total_chars);
    composer.add_source(
        "instruction",
        "user-instruction",
        "User instruction",
        instruction,
        input.budget.instruction_chars,
        None,
    );

    let outline_text = if outline.is_empty() {
        "No outline nodes found.".to_string()
    } else {
        outline
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                format!(
                    "{}. {} [{}]\n{}",
                    idx + 1,
                    node.chapter_title,
                    node.status,
                    node.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    composer.add_source(
        "outline",
        "outline.json",
        "Outline / beat sheet",
        &outline_text,
        input.budget.outline_chars,
        None,
    );

    composer.add_source(
        "target_beat",
        &target.title,
        "Current chapter beat",
        &target.summary,
        input.budget.outline_chars.min(2_000),
        None,
    );

    if let Some(target_index) = target.number.map(|n| n - 1) {
        let previous_nodes =
            select_previous_nodes(&outline, target_index, input.budget.previous_chapter_count);
        let previous_text = build_adjacent_chapter_context(app, previous_nodes);
        composer.add_source(
            "previous_chapters",
            "previous",
            "Previous chapter continuity",
            &previous_text,
            input.budget.previous_chapters_chars,
            None,
        );

        let next_nodes = select_next_nodes(&outline, target_index, input.budget.next_chapter_count);
        let next_text = build_next_chapter_context(next_nodes);
        composer.add_source(
            "next_chapter",
            "next",
            "Next chapter direction",
            &next_text,
            input.budget.next_chapter_chars,
            None,
        );
    }

    if let Ok(existing) = storage::load_chapter(app, target.title.clone()) {
        if !existing.trim().is_empty() {
            composer.add_source(
                "target_existing_text",
                &target.title,
                "Existing target chapter text",
                &existing,
                input.budget.target_existing_chars,
                None,
            );
        }
    }

    let lore_entries = storage::load_lorebook(app)
        .map_err(|e| ChapterGenerationError::new("lorebook_load_failed", e, true))?;
    let selected_lore =
        select_lore_entries(&lore_entries, &query, input.budget.lorebook_entry_count);
    let lore_text = if selected_lore.is_empty() {
        "No directly relevant lorebook entries found.".to_string()
    } else {
        selected_lore
            .iter()
            .map(|(score, entry)| {
                format!("[{}] score {:.1}\n{}", entry.keyword, score, entry.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    composer.add_source(
        "lorebook",
        "lorebook.json",
        "Relevant lorebook entries",
        &lore_text,
        input.budget.lorebook_chars,
        None,
    );

    let rag_chunks = select_rag_chunks(app, &query, input.budget.rag_chunk_count);
    if !rag_chunks.is_empty() {
        let rag_text = rag_chunks
            .iter()
            .map(|(score, reasons, chunk)| {
                format!(
                    "[{} · {} · score {:.1}]\n{}\n{}",
                    chunk.id,
                    chunk.chapter,
                    score,
                    format_text_chunk_relevance(reasons),
                    chunk.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        composer.add_source(
            "project_brain",
            "project_brain.json",
            "Project Brain relevant chunks",
            &rag_text,
            input.budget.rag_chars,
            Some(
                rag_chunks
                    .first()
                    .map(|(score, _, _)| *score)
                    .unwrap_or_default(),
            ),
        );
    }

    let profile_text = input
        .user_profile_entries
        .iter()
        .take(input.budget.user_profile_entry_count)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if !profile_text.trim().is_empty() {
        composer.add_source(
            "user_profile",
            "user_drift_profile",
            "User style preferences",
            &profile_text,
            input.budget.user_profile_chars,
            None,
        );
    }

    let (prompt_context, sources, budget_report) = composer.finish();
    let warnings = budget_report.warnings.clone();

    let request_id = input.request_id;
    let receipt = build_chapter_generation_receipt(
        &request_id,
        &target,
        &base_revision,
        instruction,
        &sources,
        crate::agent_runtime::now_ms(),
    );

    Ok(BuiltChapterContext {
        request_id,
        target,
        base_revision,
        prompt_context,
        sources,
        budget: budget_report,
        warnings,
        receipt,
    })
}

pub fn build_chapter_generation_task_packet(
    project_id: &str,
    session_id: &str,
    context: &BuiltChapterContext,
    user_instruction: &str,
    created_at_ms: u64,
) -> TaskPacket {
    let instruction = user_instruction.trim();
    let instruction_summary = if instruction.is_empty() {
        "Draft the target chapter from the built chapter context.".to_string()
    } else {
        snippet_text(instruction, 180)
    };
    let target_title = snippet_text(&context.target.title, 180);
    let objective = snippet_text(
        &format!(
            "Draft '{}' from the chapter generation context. Instruction: {}",
            target_title, instruction_summary
        ),
        560,
    );
    let mut packet = TaskPacket::new(
        format!("{}:{}:ChapterGeneration", session_id, context.request_id),
        objective,
        TaskScope::Chapter,
        created_at_ms,
    );
    packet.scope_ref = Some(context.target.title.clone());
    packet.intent = Some(Intent::GenerateContent);
    packet.constraints = vec![
        "Preserve established canon unless the author explicitly approves a change.".to_string(),
        "Respect the book contract, chapter mission, outline beat, and known promise ledger."
            .to_string(),
        "Generate chapter prose only; no analysis, markdown fences, or meta commentary."
            .to_string(),
        "Saving generated content must pass revision/conflict checks before overwriting chapters."
            .to_string(),
    ];
    packet.success_criteria = vec![
        "Generated prose passes non-empty and output-size validation.".to_string(),
        "Context sources include the instruction plus chapter/continuity memory before drafting."
            .to_string(),
        "Save completes, or a concrete save conflict is surfaced to the author.".to_string(),
        "Chapter result feedback can be recorded after a successful save.".to_string(),
    ];
    packet.beliefs = chapter_context_beliefs(context, project_id);
    packet.required_context = chapter_required_context(context);
    packet.tool_policy = ToolPolicyContract {
        max_side_effect_level: ToolSideEffectLevel::Write,
        allow_approval_required: true,
        required_tool_tags: vec!["generation".to_string()],
    };
    packet.feedback = FeedbackContract {
        expected_signals: vec![
            PHASE_CONTEXT_BUILT.to_string(),
            PHASE_COMPLETED.to_string(),
            PHASE_CONFLICT.to_string(),
            "chapter_result_summary".to_string(),
        ],
        checkpoints: vec![
            "record chapter generation context sources".to_string(),
            "validate generated content before save".to_string(),
            "check target revision before overwrite".to_string(),
            "record result feedback after successful save".to_string(),
        ],
        memory_writes: vec![
            "chapter_result_summary".to_string(),
            "outline_status".to_string(),
        ],
    };
    packet
}

pub fn build_chapter_generation_receipt(
    request_id: &str,
    target: &ChapterTarget,
    base_revision: &str,
    user_instruction: &str,
    sources: &[ChapterContextSource],
    created_at_ms: u64,
) -> WriterTaskReceipt {
    let instruction = user_instruction.trim();
    let objective = if instruction.is_empty() {
        format!("Draft '{}' from the built chapter context.", target.title)
    } else {
        format!(
            "Draft '{}' from the built chapter context. Instruction: {}",
            target.title,
            snippet_text(instruction, 180)
        )
    };
    let mut required_evidence = vec!["instruction".to_string()];
    for source in sources.iter().filter(|source| source.included_chars > 0) {
        if is_required_chapter_source(&source.source_type)
            && !required_evidence
                .iter()
                .any(|existing| existing == &source.source_type)
        {
            required_evidence.push(source.source_type.clone());
        }
    }
    let source_refs = sources
        .iter()
        .filter(|source| source.included_chars > 0)
        .map(|source| format!("{}:{}", source.source_type, source.id))
        .collect::<Vec<_>>();

    WriterTaskReceipt::new(
        request_id,
        "ChapterGeneration",
        Some(target.title.clone()),
        objective,
        required_evidence,
        vec!["chapter_draft".to_string(), "saved_chapter".to_string()],
        vec![
            "overwrite_without_revision_match".to_string(),
            "change_target_chapter_without_new_receipt".to_string(),
            "ignore_required_context_sources".to_string(),
        ],
        source_refs,
        Some(base_revision.to_string()),
        created_at_ms,
    )
}

fn chapter_context_beliefs(context: &BuiltChapterContext, project_id: &str) -> Vec<TaskBelief> {
    let mut beliefs = context
        .sources
        .iter()
        .filter(|source| source.included_chars > 0)
        .take(8)
        .map(|source| {
            let mut statement = format!(
                "{} contributes {} chars",
                source.label, source.included_chars
            );
            if source.truncated {
                statement.push_str(" after truncation");
            }
            TaskBelief::new(
                source.source_type.clone(),
                statement,
                chapter_source_confidence(&source.source_type),
            )
            .with_source(source.id.clone())
        })
        .collect::<Vec<_>>();

    if beliefs.is_empty() {
        beliefs.push(
            TaskBelief::new(
                "chapter_generation_context",
                format!(
                    "{} has no explicit context sources; fall back to project {}.",
                    context.target.title, project_id
                ),
                0.5,
            )
            .with_source(context.request_id.clone()),
        );
    }

    beliefs
}

fn chapter_required_context(context: &BuiltChapterContext) -> Vec<RequiredContext> {
    let mut required_context = context
        .sources
        .iter()
        .take(12)
        .map(|source| {
            RequiredContext::new(
                source.source_type.clone(),
                chapter_source_purpose(&source.source_type),
                source.included_chars.max(1),
                is_required_chapter_source(&source.source_type),
            )
        })
        .collect::<Vec<_>>();

    if !required_context
        .iter()
        .any(|context| context.required && !context.source_type.trim().is_empty())
    {
        required_context.push(RequiredContext::new(
            "chapter_generation_context",
            "Fallback chapter context required to draft safely.",
            1,
            true,
        ));
    }

    required_context
}

fn is_required_chapter_source(source_type: &str) -> bool {
    matches!(
        source_type,
        "instruction"
            | "outline"
            | "target_beat"
            | "previous_chapters"
            | "lorebook"
            | "project_brain"
    )
}

fn chapter_source_purpose(source_type: &str) -> &'static str {
    match source_type {
        "instruction" => "Capture the author's explicit generation request.",
        "outline" => "Keep the draft aligned with the book-level beat sheet.",
        "target_beat" => "Preserve the target chapter mission and planned payoff.",
        "previous_chapters" => "Maintain continuity from recent chapter outcomes.",
        "next_chapter" => "Avoid blocking the next planned beat.",
        "target_existing_text" => "Respect any existing prose already in the target chapter.",
        "lorebook" => "Ground character, setting, and canon details.",
        "project_brain" => "Recall relevant long-range project memory.",
        "user_profile" => "Preserve learned author style preferences.",
        _ => "Provide supporting context for chapter generation.",
    }
}

fn chapter_source_confidence(source_type: &str) -> f32 {
    match source_type {
        "instruction" | "target_beat" => 0.92,
        "outline" | "lorebook" => 0.88,
        "previous_chapters" | "project_brain" => 0.78,
        "next_chapter" | "target_existing_text" | "user_profile" => 0.70,
        _ => 0.60,
    }
}

fn snippet_text(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

pub async fn generate_chapter_draft(
    settings: &llm_runtime::LlmSettings,
    context: &BuiltChapterContext,
    provider_budget_approval: Option<&WriterProviderBudgetApproval>,
) -> Result<GenerateChapterDraftOutput, ChapterGenerationError> {
    if context.prompt_context.trim().is_empty() {
        return Err(ChapterGenerationError::new(
            "CONTEXT_INVALID",
            "The built chapter context is empty.",
            true,
        ));
    }

    let system_prompt = format!(
        "You are a professional Chinese novelist drafting a complete chapter. \
Use the provided project context, preserve continuity, and write only chapter prose. \
Do not include analysis, markdown fences, action tags, or meta commentary. \
Aim for up to {} Chinese characters unless the beat clearly requires less.",
        DEFAULT_OUTPUT_SOFT_CAP_CHARS
    );
    let user_prompt = format!(
        "Task: {}\n\nTarget chapter: {}\n\nProject context:\n{}",
        context
            .sources
            .iter()
            .find(|s| s.source_type == "instruction")
            .map(|_| "Draft this chapter from the user's instruction.")
            .unwrap_or("Draft this chapter."),
        context.target.title,
        context.prompt_context
    );
    let messages = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_prompt}),
    ];
    let budget_report = apply_provider_budget_approval(
        chapter_generation_provider_budget(settings, &messages),
        provider_budget_approval,
    );
    if budget_report.decision == WriterProviderBudgetDecision::ApprovalRequired {
        return Err(provider_budget_error(
            &context.request_id,
            &context.receipt,
            budget_report,
        ));
    }

    let content = llm_runtime::chat_text(settings, messages, false, PROVIDER_TIMEOUT_SECS)
        .await
        .map_err(map_provider_error)?;

    let content = content.trim().to_string();
    validate_generated_content(&content)?;

    Ok(GenerateChapterDraftOutput {
        output_chars: char_count(&content),
        content,
        finish_reason: "complete".to_string(),
        model: settings.model.clone(),
        provider: "openai-compatible".to_string(),
        base_revision: context.base_revision.clone(),
        provider_budget: budget_report,
    })
}

pub fn chapter_generation_provider_budget(
    settings: &llm_runtime::LlmSettings,
    messages: &[serde_json::Value],
) -> WriterProviderBudgetReport {
    let converted = messages
        .iter()
        .map(|message| LlmMessage {
            role: message
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or("user")
                .to_string(),
            content: message
                .get("content")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        })
        .collect::<Vec<_>>();
    let estimated_input_tokens =
        agent_harness_core::context_window_guard::estimate_request_tokens(&converted, None);
    evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        settings.model.clone(),
        estimated_input_tokens,
        CHAPTER_GENERATION_OUTPUT_TOKENS,
    ))
}

pub fn provider_budget_error(
    request_id: &str,
    receipt: &WriterTaskReceipt,
    report: WriterProviderBudgetReport,
) -> ChapterGenerationError {
    ChapterGenerationError::new(
        "PROVIDER_BUDGET_APPROVAL_REQUIRED",
        "Chapter generation provider budget requires explicit approval before calling the model.",
        true,
    )
    .with_evidence(WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ProviderFailed,
        "PROVIDER_BUDGET_APPROVAL_REQUIRED",
        "Chapter generation provider budget requires explicit approval before calling the model.",
        true,
        Some(request_id.to_string()),
        vec![
            format!("receipt:{}", receipt.task_id),
            format!("model:{}", report.model),
            format!("estimated_tokens:{}", report.estimated_total_tokens),
            format!("estimated_cost_micros:{}", report.estimated_cost_micros),
        ],
        serde_json::json!({
            "providerBudget": report,
            "receipt": receipt,
        }),
        vec![
            "Surface the provider token/cost estimate to the author before retrying.".to_string(),
            "Reduce context budget or requested output length if approval is not granted."
                .to_string(),
        ],
        crate::agent_runtime::now_ms(),
    ))
}

pub fn provider_budget_report_from_error(
    error: &ChapterGenerationError,
) -> Option<WriterProviderBudgetReport> {
    let budget = error
        .evidence
        .as_ref()?
        .details
        .get("providerBudget")?
        .clone();
    serde_json::from_value(budget).ok()
}

pub fn save_generated_chapter(
    app: &tauri::AppHandle,
    input: SaveGeneratedChapterInput,
) -> Result<SaveGeneratedChapterOutput, ChapterGenerationError> {
    if let Some(error) = validate_receipt_for_save(&input) {
        return Err(error);
    }

    if input.generated_content.trim().is_empty() {
        return Err(ChapterGenerationError::new(
            "CONTENT_EMPTY",
            "Generated chapter content is empty.",
            true,
        ));
    }

    if char_count(&input.generated_content) > DEFAULT_OUTPUT_HARD_CAP_CHARS {
        return Err(ChapterGenerationError::new(
            "CONTENT_TOO_LARGE",
            "Generated chapter content exceeds the hard save cap.",
            true,
        ));
    }

    let current_revision = storage::chapter_revision(app, &input.target.title).map_err(|e| {
        ChapterGenerationError::with_details(
            "STORAGE_READ_FAILED",
            "Failed to read current chapter revision.",
            true,
            e,
        )
    })?;

    match decide_save_action(
        &input.target.title,
        &input.request_id,
        input.save_mode,
        &input.base_revision,
        &current_revision,
        input.frontend_state.as_ref(),
    ) {
        SaveDecision::WriteTarget => {
            let new_revision = storage::save_chapter_content_and_revision(
                app,
                &input.target.title,
                &input.generated_content,
            )
            .map_err(|e| {
                ChapterGenerationError::with_details(
                    "STORAGE_WRITE_FAILED",
                    "Failed to save generated chapter.",
                    true,
                    e,
                )
            })?;
            Ok(SaveGeneratedChapterOutput {
                chapter_title: input.target.title,
                new_revision,
                saved_mode: if current_revision == "missing" {
                    "created".to_string()
                } else {
                    "replaced".to_string()
                },
            })
        }
        SaveDecision::WriteDraft {
            draft_title,
            conflict,
        } => {
            tracing::warn!(
                "Saving generated chapter as draft copy after conflict: {}",
                conflict.reason
            );
            let new_revision = storage::save_chapter_content_and_revision(
                app,
                &draft_title,
                &input.generated_content,
            )
            .map_err(|e| {
                ChapterGenerationError::with_details(
                    "STORAGE_WRITE_FAILED",
                    "Failed to save generated draft copy.",
                    true,
                    e,
                )
            })?;
            Ok(SaveGeneratedChapterOutput {
                chapter_title: draft_title,
                new_revision,
                saved_mode: "draft_copy".to_string(),
            })
        }
        SaveDecision::Conflict(conflict) => Err(ChapterGenerationError {
            code: "SAVE_CONFLICT".to_string(),
            message: format!("Save blocked by {}.", conflict.reason),
            recoverable: true,
            details: serde_json::to_string(&conflict).ok(),
            evidence: Some(failure_bundle_from_save_conflict(
                &input.receipt,
                &conflict,
                crate::agent_runtime::now_ms(),
            )),
        }),
    }
}

fn validate_receipt_for_save(input: &SaveGeneratedChapterInput) -> Option<ChapterGenerationError> {
    let mismatches = input.receipt.validate_write_attempt(
        &input.request_id,
        &input.target.title,
        &input.base_revision,
        "saved_chapter",
    );
    if mismatches.is_empty() {
        return None;
    }

    let evidence_refs = mismatches
        .iter()
        .map(|mismatch| {
            format!(
                "{}:{}->{}",
                mismatch.field, mismatch.expected, mismatch.actual
            )
        })
        .collect::<Vec<_>>();
    let evidence = WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ReceiptMismatch,
        "RECEIPT_MISMATCH",
        "Generated chapter save was blocked because the task receipt no longer matches the write attempt.",
        true,
        Some(input.receipt.task_id.clone()),
        evidence_refs,
        serde_json::json!({
            "receipt": input.receipt,
            "attempt": {
                "requestId": input.request_id,
                "chapter": input.target.title,
                "baseRevision": input.base_revision,
                "artifact": "saved_chapter",
            },
            "mismatches": mismatches,
        }),
        vec![
            "Rebuild the chapter generation context for the current target chapter.".to_string(),
            "Retry only after the frontend and storage revisions match.".to_string(),
        ],
        crate::agent_runtime::now_ms(),
    );

    Some(
        ChapterGenerationError::new(
            "RECEIPT_MISMATCH",
            "Generated chapter save was blocked because the task receipt no longer matches the write attempt.",
            true,
        )
        .with_evidence(evidence),
    )
}

pub fn save_conflict_from_error(error: &ChapterGenerationError) -> Option<SaveConflict> {
    if error.code != "SAVE_CONFLICT" {
        return None;
    }
    error
        .details
        .as_deref()
        .and_then(|details| serde_json::from_str(details).ok())
}

pub fn failure_bundle_from_chapter_error(
    request_id: &str,
    error: &ChapterGenerationError,
    created_at_ms: u64,
) -> WriterFailureEvidenceBundle {
    if let Some(bundle) = error.evidence.clone() {
        return bundle;
    }
    let category = failure_category_for_error_code(&error.code);
    let mut evidence_refs = vec![format!("error:{}", error.code)];
    if let Some(details) = error
        .details
        .as_ref()
        .filter(|details| !details.trim().is_empty())
    {
        evidence_refs.push(format!("details:{}", snippet_text(details, 120)));
    }
    WriterFailureEvidenceBundle::new(
        category,
        error.code.clone(),
        error.message.clone(),
        error.recoverable,
        Some(request_id.to_string()),
        evidence_refs,
        serde_json::json!({
            "details": error.details,
        }),
        remediation_for_error_code(&error.code),
        created_at_ms,
    )
}

pub fn failure_bundle_from_save_conflict(
    receipt: &WriterTaskReceipt,
    conflict: &SaveConflict,
    created_at_ms: u64,
) -> WriterFailureEvidenceBundle {
    WriterFailureEvidenceBundle::new(
        WriterFailureCategory::SaveFailed,
        "SAVE_CONFLICT",
        format!("Save blocked by {}.", conflict.reason),
        true,
        Some(receipt.task_id.clone()),
        vec![
            format!("chapter:{}", receipt.chapter.clone().unwrap_or_default()),
            format!("base_revision:{}", conflict.base_revision),
            format!("current_revision:{}", conflict.current_revision),
            format!("save_conflict:{}", conflict.reason),
        ],
        serde_json::json!({
            "receipt": receipt,
            "conflict": conflict,
        }),
        vec![
            "Review the open editor changes before overwriting.".to_string(),
            "Regenerate from the current chapter revision or save as a draft copy.".to_string(),
        ],
        created_at_ms,
    )
}

fn failure_category_for_error_code(code: &str) -> WriterFailureCategory {
    match code {
        "INSTRUCTION_EMPTY"
        | "TARGET_CHAPTER_NOT_FOUND"
        | "TARGET_CHAPTER_AMBIGUOUS"
        | "CONTEXT_INVALID" => WriterFailureCategory::ContextMissing,
        "PROVIDER_TIMEOUT"
        | "PROVIDER_RATE_LIMITED"
        | "PROVIDER_NOT_CONFIGURED"
        | "PROVIDER_CALL_FAILED"
        | "PROVIDER_BUDGET_APPROVAL_REQUIRED"
        | "MODEL_OUTPUT_EMPTY"
        | "MODEL_OUTPUT_TOO_LARGE" => WriterFailureCategory::ProviderFailed,
        "SAVE_CONFLICT"
        | "CONTENT_EMPTY"
        | "CONTENT_TOO_LARGE"
        | "STORAGE_READ_FAILED"
        | "STORAGE_WRITE_FAILED"
        | "OUTLINE_LOAD_FAILED"
        | "OUTLINE_SAVE_FAILED" => WriterFailureCategory::SaveFailed,
        "RECEIPT_MISMATCH" => WriterFailureCategory::ReceiptMismatch,
        _ => WriterFailureCategory::ProviderFailed,
    }
}

fn remediation_for_error_code(code: &str) -> Vec<String> {
    match code {
        "INSTRUCTION_EMPTY" => {
            vec!["Provide a concrete chapter generation instruction.".to_string()]
        }
        "TARGET_CHAPTER_NOT_FOUND" | "TARGET_CHAPTER_AMBIGUOUS" => {
            vec!["Select a concrete target chapter or fix duplicate outline entries.".to_string()]
        }
        "PROVIDER_NOT_CONFIGURED" => vec!["Configure a valid model provider API key.".to_string()],
        "PROVIDER_BUDGET_APPROVAL_REQUIRED" => vec![
            "Review and approve the estimated provider token/cost budget before retrying."
                .to_string(),
            "Reduce context budget or requested output length if approval is not granted."
                .to_string(),
        ],
        "PROVIDER_TIMEOUT" | "PROVIDER_RATE_LIMITED" | "PROVIDER_CALL_FAILED" => vec![
            "Retry after provider recovery or switch to another configured provider.".to_string(),
        ],
        "MODEL_OUTPUT_EMPTY" | "MODEL_OUTPUT_TOO_LARGE" => vec![
            "Regenerate with a narrower chapter objective or smaller output budget.".to_string(),
        ],
        "RECEIPT_MISMATCH" => {
            vec!["Rebuild the task receipt from the latest context before saving.".to_string()]
        }
        "SAVE_CONFLICT" => {
            vec!["Resolve editor/storage revision mismatch or save as a draft copy.".to_string()]
        }
        _ => vec![
            "Inspect the failure evidence bundle and retry from the last safe phase.".to_string(),
        ],
    }
}

pub fn update_outline_after_generation(
    app: &tauri::AppHandle,
    target: &ChapterTarget,
    saved: &SaveGeneratedChapterOutput,
) -> Result<OutlineUpdateOutput, ChapterGenerationError> {
    let mut outline = storage::load_outline(app).map_err(|e| {
        ChapterGenerationError::with_details(
            "OUTLINE_NOT_FOUND",
            "Failed to read outline for status update.",
            true,
            e,
        )
    })?;

    let mut changed = false;
    if let Some(node) = outline.iter_mut().find(|node| {
        node.chapter_title == saved.chapter_title || node.chapter_title == target.title
    }) {
        if node.status != "drafted" {
            node.status = "drafted".to_string();
            changed = true;
        }
    } else {
        outline.push(storage::OutlineNode {
            chapter_title: saved.chapter_title.clone(),
            summary: target.summary.clone(),
            status: "drafted".to_string(),
        });
        changed = true;
    }

    if changed {
        storage::save_outline(app, &outline).map_err(|e| {
            ChapterGenerationError::with_details(
                "OUTLINE_UPDATE_FAILED",
                "Failed to update outline after chapter save.",
                true,
                e,
            )
        })?;
    }

    let outline_json = serde_json::to_string(&outline).unwrap_or_default();
    Ok(OutlineUpdateOutput {
        outline_revision: storage::content_revision(&outline_json),
        changed,
        warnings: vec![],
    })
}

pub async fn run_chapter_generation_pipeline(
    app: tauri::AppHandle,
    settings: llm_runtime::LlmSettings,
    payload: GenerateChapterAutonomousPayload,
    user_profile_entries: Vec<String>,
    mut emit: impl FnMut(ChapterGenerationEvent) + Send,
    mut record_task_packet: impl FnMut(&BuiltChapterContext) + Send,
    mut record_provider_budget: impl FnMut(&BuiltChapterContext, &WriterProviderBudgetReport) + Send,
) -> PipelineTerminal {
    let request_id = payload
        .request_id
        .clone()
        .unwrap_or_else(|| make_request_id("chapter"));

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_STARTED,
        "running",
        "正在理解任务并读取工程结构...",
        5,
        None,
    ));

    let build_input = BuildChapterContextInput {
        request_id: request_id.clone(),
        target_chapter_title: payload.target_chapter_title.clone(),
        target_chapter_number: payload.target_chapter_number,
        user_instruction: payload.user_instruction.clone(),
        budget: payload.budget.clone().unwrap_or_default(),
        chapter_summary_override: payload.chapter_summary_override.clone(),
        user_profile_entries,
    };

    let context = match build_chapter_context(&app, build_input) {
        Ok(context) => context,
        Err(error) => {
            emit(ChapterGenerationEvent::failed(&request_id, error.clone()));
            return PipelineTerminal::Failed(error);
        }
    };

    record_task_packet(&context);

    emit(ChapterGenerationEvent {
        request_id: request_id.clone(),
        phase: PHASE_CONTEXT_BUILT.to_string(),
        status: "done".to_string(),
        message: format!(
            "检索到 {} 个上下文来源，当前提示上下文 {} 字。",
            context.sources.len(),
            context.budget.included_chars
        ),
        progress: 25,
        target_chapter_title: Some(context.target.title.clone()),
        sources: Some(context.sources.clone()),
        budget: Some(context.budget.clone()),
        receipt: Some(context.receipt.clone()),
        saved: None,
        conflict: None,
        error: None,
        warnings: context.warnings.clone(),
    });

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_PROGRESS,
        "running",
        "正在撰写章节初稿...",
        45,
        Some(context.target.title.clone()),
    ));

    let draft = match generate_chapter_draft(
        &settings,
        &context,
        payload.provider_budget_approval.as_ref(),
    )
    .await
    {
        Ok(draft) => {
            record_provider_budget(&context, &draft.provider_budget);
            draft
        }
        Err(error) => {
            if let Some(report) = provider_budget_report_from_error(&error) {
                record_provider_budget(&context, &report);
            }
            emit(ChapterGenerationEvent::failed(&request_id, error.clone()));
            return PipelineTerminal::Failed(error);
        }
    };

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_PROGRESS,
        "running",
        "正在保存章节并检查编辑器冲突...",
        70,
        Some(context.target.title.clone()),
    ));

    let save_input = SaveGeneratedChapterInput {
        request_id: request_id.clone(),
        target: context.target.clone(),
        generated_content: draft.content.clone(),
        base_revision: context.base_revision.clone(),
        save_mode: payload.save_mode,
        frontend_state: payload.frontend_state.clone(),
        receipt: context.receipt.clone(),
    };
    let saved = match save_generated_chapter(&app, save_input) {
        Ok(saved) => saved,
        Err(error) => {
            if let Some(conflict) = save_conflict_from_error(&error) {
                emit(ChapterGenerationEvent::conflict(
                    &request_id,
                    conflict.clone(),
                ));
                return PipelineTerminal::Conflict(conflict);
            }
            emit(ChapterGenerationEvent::failed(&request_id, error.clone()));
            return PipelineTerminal::Failed(error);
        }
    };

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_PROGRESS,
        "running",
        "正在更新大纲状态...",
        85,
        Some(saved.chapter_title.clone()),
    ));

    let mut warnings = Vec::new();
    if let Err(error) = update_outline_after_generation(&app, &context.target, &saved) {
        warnings.push(format!("Outline update skipped: {}", error.message));
    }

    emit(ChapterGenerationEvent {
        request_id,
        phase: PHASE_COMPLETED.to_string(),
        status: "done".to_string(),
        message: format!("{} 初稿已保存。", saved.chapter_title),
        progress: 100,
        target_chapter_title: Some(saved.chapter_title.clone()),
        sources: None,
        budget: None,
        receipt: None,
        saved: Some(saved.clone()),
        conflict: None,
        error: None,
        warnings,
    });

    PipelineTerminal::Completed {
        saved,
        generated_content: draft.content,
    }
}

impl ChapterGenerationEvent {
    pub fn progress(
        request_id: &str,
        phase: &str,
        status: &str,
        message: &str,
        progress: u8,
        target_chapter_title: Option<String>,
    ) -> Self {
        Self {
            request_id: request_id.to_string(),
            phase: phase.to_string(),
            status: status.to_string(),
            message: message.to_string(),
            progress,
            target_chapter_title,
            sources: None,
            budget: None,
            receipt: None,
            saved: None,
            conflict: None,
            error: None,
            warnings: vec![],
        }
    }

    pub fn failed(request_id: &str, error: ChapterGenerationError) -> Self {
        Self {
            request_id: request_id.to_string(),
            phase: PHASE_FAILED.to_string(),
            status: "error".to_string(),
            message: error.message.clone(),
            progress: 100,
            target_chapter_title: None,
            sources: None,
            budget: None,
            receipt: None,
            saved: None,
            conflict: None,
            error: Some(error),
            warnings: vec![],
        }
    }

    pub fn conflict(request_id: &str, conflict: SaveConflict) -> Self {
        Self {
            request_id: request_id.to_string(),
            phase: PHASE_CONFLICT.to_string(),
            status: "conflict".to_string(),
            message: format!("保存被阻止：{}。", conflict.reason),
            progress: 100,
            target_chapter_title: conflict.open_chapter_title.clone(),
            sources: None,
            budget: None,
            receipt: None,
            saved: None,
            conflict: Some(conflict),
            error: None,
            warnings: vec![],
        }
    }
}

fn make_draft_title(target_title: &str, request_id: &str) -> String {
    let suffix = request_id
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{} draft {}", target_title, suffix)
}

pub fn make_request_id(prefix: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{}", prefix, millis)
}

pub fn map_provider_error(error: String) -> ChapterGenerationError {
    let lower = error.to_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        ChapterGenerationError::with_details(
            "PROVIDER_TIMEOUT",
            "The model provider timed out.",
            true,
            error,
        )
    } else if lower.contains("429") || lower.contains("rate limit") {
        ChapterGenerationError::with_details(
            "PROVIDER_RATE_LIMITED",
            "The model provider rate-limited the request.",
            true,
            error,
        )
    } else if lower.contains("api key") || lower.contains("unauthorized") || lower.contains("401") {
        ChapterGenerationError::with_details(
            "PROVIDER_NOT_CONFIGURED",
            "The model provider is not configured.",
            true,
            error,
        )
    } else {
        ChapterGenerationError::with_details(
            "PROVIDER_CALL_FAILED",
            "The model provider call failed.",
            true,
            error,
        )
    }
}

struct ContextComposer {
    max_chars: usize,
    text: String,
    sources: Vec<ChapterContextSource>,
    warnings: Vec<String>,
}

impl ContextComposer {
    fn new(max_chars: usize) -> Self {
        Self {
            max_chars,
            text: String::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn remaining_chars(&self) -> usize {
        self.max_chars.saturating_sub(char_count(&self.text))
    }

    fn add_source(
        &mut self,
        source_type: &str,
        id: &str,
        label: &str,
        content: &str,
        source_cap: usize,
        score: Option<f32>,
    ) {
        if content.trim().is_empty() || self.remaining_chars() == 0 {
            return;
        }

        let header = format!("## {}\n", label);
        let footer = "\n\n";
        let overhead = char_count(&header) + char_count(footer);
        let remaining = self.remaining_chars();
        if remaining <= overhead {
            self.warnings
                .push(format!("Context budget exhausted before adding {}.", label));
            return;
        }

        let allowed = source_cap.min(remaining - overhead);
        let original_chars = char_count(content);
        let (included, included_chars, truncated) = truncate_text_report(content, allowed);

        self.text.push_str(&header);
        self.text.push_str(&included);
        self.text.push_str(footer);

        if truncated {
            self.warnings.push(format!(
                "{} truncated from {} to {} chars.",
                label, original_chars, included_chars
            ));
        }

        self.sources.push(ChapterContextSource {
            source_type: source_type.to_string(),
            id: id.to_string(),
            label: label.to_string(),
            original_chars,
            included_chars,
            truncated,
            score,
        });
    }

    fn finish(
        self,
    ) -> (
        String,
        Vec<ChapterContextSource>,
        ChapterContextBudgetReport,
    ) {
        let included_chars = char_count(&self.text);
        let truncated_source_count = self
            .sources
            .iter()
            .filter(|source| source.truncated)
            .count();
        let report = ChapterContextBudgetReport {
            max_chars: self.max_chars,
            included_chars,
            source_count: self.sources.len(),
            truncated_source_count,
            warnings: self.warnings,
        };
        (self.text, self.sources, report)
    }
}

fn select_previous_nodes(
    outline: &[storage::OutlineNode],
    target_index: usize,
    max_count: usize,
) -> Vec<&storage::OutlineNode> {
    let start = target_index.saturating_sub(max_count);
    outline[start..target_index].iter().collect()
}

fn select_next_nodes(
    outline: &[storage::OutlineNode],
    target_index: usize,
    max_count: usize,
) -> Vec<&storage::OutlineNode> {
    outline
        .iter()
        .skip(target_index + 1)
        .take(max_count)
        .collect()
}

fn build_adjacent_chapter_context(
    app: &tauri::AppHandle,
    nodes: Vec<&storage::OutlineNode>,
) -> String {
    if nodes.is_empty() {
        return "None (first chapter or no previous outline nodes).".to_string();
    }

    nodes
        .iter()
        .map(|node| {
            let text = storage::load_chapter(app, node.chapter_title.clone()).unwrap_or_default();
            if text.trim().is_empty() {
                format!("[{}]\nSummary: {}", node.chapter_title, node.summary)
            } else {
                format!(
                    "[{}]\nSummary: {}\nExisting text:\n{}",
                    node.chapter_title, node.summary, text
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_next_chapter_context(nodes: Vec<&storage::OutlineNode>) -> String {
    if nodes.is_empty() {
        return "No next chapter outline node.".to_string();
    }

    nodes
        .iter()
        .map(|node| format!("[{}]\n{}", node.chapter_title, node.summary))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn select_lore_entries<'a>(
    entries: &'a [storage::LoreEntry],
    query: &str,
    max_count: usize,
) -> Vec<(f32, &'a storage::LoreEntry)> {
    let mut scored = entries
        .iter()
        .map(|entry| {
            let haystack = format!("{}\n{}", entry.keyword, entry.content);
            (relevance_score(query, &haystack), entry)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max_count);
    scored
}

fn select_rag_chunks(
    app: &tauri::AppHandle,
    query: &str,
    max_count: usize,
) -> Vec<(f32, Vec<String>, agent_harness_core::Chunk)> {
    let Ok(path) = storage::brain_path(app) else {
        return vec![];
    };
    let db = match VectorDB::load(&path) {
        Ok(db) => db,
        Err(e) => {
            tracing::warn!(
                "Skipping Project Brain chunks because '{}' is unreadable: {}",
                path.display(),
                e
            );
            return vec![];
        }
    };

    let scored = db
        .chunks
        .into_iter()
        .map(|chunk| {
            let haystack = format!(
                "{}\n{}\n{}\n{}",
                chunk.chapter,
                chunk.keywords.join("\n"),
                chunk.topic.clone().unwrap_or_default(),
                chunk.text
            );
            (relevance_score(query, &haystack), chunk)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect::<Vec<_>>();
    rerank_text_chunks(scored, query, |chunk| {
        format!(
            "{}\n{}\n{}\n{}",
            chunk.chapter,
            chunk.keywords.join("\n"),
            chunk.topic.clone().unwrap_or_default(),
            chunk.text
        )
    })
    .into_iter()
    .take(max_count)
    .collect()
}

fn relevance_score(query: &str, haystack: &str) -> f32 {
    let haystack = haystack.to_lowercase();
    let mut score = 0f32;
    for needle in query_needles(query) {
        if haystack.contains(&needle.to_lowercase()) {
            score += needle.chars().count().max(1) as f32;
        }
    }
    score
}

fn query_needles(query: &str) -> Vec<String> {
    let mut needles = Vec::new();
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_alphanumeric() || is_cjk(ch) {
            current.push(ch);
        } else if !current.is_empty() {
            push_needle(&mut needles, &current);
            current.clear();
        }
    }
    if !current.is_empty() {
        push_needle(&mut needles, &current);
    }
    needles.truncate(64);
    needles
}

fn push_needle(needles: &mut Vec<String>, token: &str) {
    if char_count(token) >= 2 {
        needles.push(token.to_string());
    }

    let chars = token.chars().collect::<Vec<_>>();
    if chars.len() >= 4 && chars.iter().any(|ch| is_cjk(*ch)) {
        for window in chars.windows(2).take(16) {
            needles.push(window.iter().collect());
        }
    }
}

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline() -> Vec<storage::OutlineNode> {
        vec![
            storage::OutlineNode {
                chapter_title: "第一章".to_string(),
                summary: "林墨抵达破庙。".to_string(),
                status: "drafted".to_string(),
            },
            storage::OutlineNode {
                chapter_title: "第二章".to_string(),
                summary: "林墨发现壁画。".to_string(),
                status: "drafted".to_string(),
            },
            storage::OutlineNode {
                chapter_title: "第三章".to_string(),
                summary: "林墨发现密道并遭遇毒雾机关。".to_string(),
                status: "empty".to_string(),
            },
        ]
    }

    #[test]
    fn counts_unicode_chars_instead_of_bytes_for_chinese_text() {
        assert_eq!(char_count("破庙密道"), 4);
        assert_eq!("破庙密道".len(), 12);
    }

    #[test]
    fn truncates_chinese_at_valid_utf8_boundary() {
        let (text, included, truncated) = truncate_text_report("林墨推开破庙石门", 4);
        assert_eq!(text, "林墨推开");
        assert_eq!(included, 4);
        assert!(truncated);
    }

    #[test]
    fn prefers_chinese_sentence_boundary_when_truncating() {
        let (text, _, truncated) =
            truncate_text_report("林墨停下脚步。毒雾从密道深处涌来，像潮水一样。", 16);
        assert_eq!(text, "林墨停下脚步。");
        assert!(truncated);
    }

    #[test]
    fn handles_mixed_chinese_english_and_emoji_without_corruption() {
        let (text, included, truncated) = truncate_text_report("AI提醒林墨：run！🔥继续。", 10);
        assert_eq!(char_count(&text), included);
        assert!(text.is_char_boundary(text.len()));
        assert!(truncated);
    }

    #[test]
    fn resolves_target_chapter_by_outline_number_and_returns_metadata() {
        let target = resolve_target_from_outline(&outline(), None, Some(3), None).unwrap();
        assert_eq!(target.title, "第三章");
        assert_eq!(target.number, Some(3));
        assert!(target.summary.contains("密道"));
    }

    #[test]
    fn rejects_missing_target_chapter_with_typed_error() {
        let err = resolve_target_from_outline(&outline(), Some("第九章"), None, None).unwrap_err();
        assert_eq!(err.code, "TARGET_CHAPTER_NOT_FOUND");
    }

    #[test]
    fn rejects_ambiguous_target_chapter_with_typed_error() {
        let mut data = outline();
        data.push(storage::OutlineNode {
            chapter_title: "第三章".to_string(),
            summary: "重复节点".to_string(),
            status: "empty".to_string(),
        });
        let err = resolve_target_from_outline(&data, Some("第三章"), None, None).unwrap_err();
        assert_eq!(err.code, "TARGET_CHAPTER_AMBIGUOUS");
    }

    #[test]
    fn replaces_chapter_when_revision_matches_and_frontend_is_clean() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "abc",
            Some(&FrontendChapterStateSnapshot {
                open_chapter_title: Some("第三章".to_string()),
                open_chapter_revision: Some("abc".to_string()),
                dirty: false,
            }),
        );
        assert!(matches!(decision, SaveDecision::WriteTarget));
    }

    #[test]
    fn rejects_dirty_open_target_chapter_without_writing() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "abc",
            Some(&FrontendChapterStateSnapshot {
                open_chapter_title: Some("第三章".to_string()),
                open_chapter_revision: Some("abc".to_string()),
                dirty: true,
            }),
        );
        match decision {
            SaveDecision::Conflict(conflict) => {
                assert_eq!(conflict.reason, "frontend_dirty_open_chapter");
            }
            _ => panic!("expected conflict"),
        }
    }

    #[test]
    fn rejects_revision_mismatch_without_writing() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "def",
            None,
        );
        match decision {
            SaveDecision::Conflict(conflict) => {
                assert_eq!(conflict.reason, "revision_mismatch");
            }
            _ => panic!("expected conflict"),
        }
    }

    #[test]
    fn saves_draft_copy_on_conflict_only_when_requested() {
        let decision = decide_save_action(
            "第三章",
            "request-abcdef",
            SaveMode::SaveAsDraft,
            "abc",
            "def",
            None,
        );
        match decision {
            SaveDecision::WriteDraft {
                draft_title,
                conflict,
            } => {
                assert!(draft_title.contains("第三章 draft"));
                assert_eq!(conflict.reason, "revision_mismatch");
            }
            _ => panic!("expected draft decision"),
        }
    }

    #[test]
    fn rejects_empty_generated_content_with_content_empty() {
        let err = validate_generated_content("  ").unwrap_err();
        assert_eq!(err.code, "MODEL_OUTPUT_EMPTY");
    }

    #[test]
    fn maps_http_429_to_provider_rate_limited() {
        let err = map_provider_error("API error 429: too many requests".to_string());
        assert_eq!(err.code, "PROVIDER_RATE_LIMITED");
    }

    #[test]
    fn provider_budget_error_preserves_report_evidence() {
        let target = ChapterTarget {
            title: "第三章".to_string(),
            filename: "第三章.md".to_string(),
            number: Some(3),
            summary: "林墨发现密道。".to_string(),
            status: "empty".to_string(),
        };
        let receipt = build_chapter_generation_receipt(
            "budget-test-1",
            &target,
            "rev-1",
            "写第三章。",
            &[ChapterContextSource {
                source_type: "instruction".to_string(),
                id: "user-instruction".to_string(),
                label: "User instruction".to_string(),
                original_chars: 5,
                included_chars: 5,
                truncated: false,
                score: None,
            }],
            10,
        );
        let report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
            WriterProviderBudgetTask::ChapterGeneration,
            "gpt-4o",
            90_000,
            24_000,
        ));

        let error = provider_budget_error("budget-test-1", &receipt, report);

        assert_eq!(error.code, "PROVIDER_BUDGET_APPROVAL_REQUIRED");
        let evidence = error.evidence.expect("budget error has evidence");
        assert_eq!(evidence.category, WriterFailureCategory::ProviderFailed);
        assert!(evidence.details.get("providerBudget").is_some());
        assert!(!evidence.remediation.is_empty());
    }
}
