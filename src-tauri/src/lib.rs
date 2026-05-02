use std::sync::{Mutex, MutexGuard};

use agent_harness_core::{
    ambient::EditorEvent,
    default_writing_tool_registry,
    hermes_memory::HermesDB,
    provider::{openai_compat::OpenAiCompatProvider, LlmMessage},
    writing_domain_profile, AgentLoopEvent,
};

mod agent_runtime;
mod ambient_agents;
mod brain_service;
pub mod chapter_generation;
mod commands;
mod llm_runtime;
mod manual_agent;
mod storage;
mod tool_bridge;
pub mod writer_agent;
use agent_runtime::{AgentObservation, AgentObserveResult, AgentToolDescriptor};
use chapter_generation::{
    ChapterGenerationEvent, FrontendChapterStateSnapshot, GenerateChapterAutonomousPayload,
    PipelineTerminal, SaveMode,
};
use storage::{ChapterInfo, LoreEntry, OutlineNode};
use writer_agent::memory::ManualAgentTurnSummary;

const KEYRING_SERVICE: &str = "agent-writer";
mod events {
    pub const AGENT_CHAIN_OF_THOUGHT: &str = "agent-chain-of-thought";
    pub const AGENT_EPIPHANY: &str = "agent-epiphany";
    pub const AGENT_ERROR: &str = "agent-error";
    pub const AGENT_PROPOSAL: &str = "agent-proposal";
    pub const AGENT_SUGGESTION: &str = "agent-suggestion";
    pub const AGENT_STREAM_CHUNK: &str = "agent-stream-chunk";
    pub const AGENT_STREAM_END: &str = "agent-stream-end";
    pub const BATCH_STATUS: &str = "batch-status";
    pub const CHAPTER_GENERATION: &str = "chapter-generation";
    pub const EDITOR_GHOST_CHUNK: &str = "editor-ghost-chunk";
    pub const EDITOR_GHOST_END: &str = "editor-ghost-end";
    pub const EDITOR_SEMANTIC_LINT: &str = "editor-semantic-lint";
    pub const EDITOR_ENTITY_CARD: &str = "editor-entity-card";
    pub const EDITOR_HOVER_HINT: &str = "editor-hover-hint";
    pub const INLINE_WRITER_OPERATION: &str = "inline-writer-operation";
    pub const STORYBOARD_MARKER: &str = "storyboard-marker";
}

fn load_api_key_from_keychain() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "openai").ok()?;
    entry.get_password().ok()
}

fn safe_filename_component(raw: &str) -> String {
    let mut safe = String::new();
    let mut last_was_dash = false;
    for ch in raw.trim().to_lowercase().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch == ' ' || ch == '-' || ch == '_' {
            Some('-')
        } else {
            None
        };

        if let Some(ch) = next {
            if ch == '-' {
                if last_was_dash {
                    continue;
                }
                last_was_dash = true;
            } else {
                last_was_dash = false;
            }
            safe.push(ch);
        }
    }
    let safe = safe.trim_matches('-');
    if safe.is_empty() {
        "default".to_string()
    } else {
        safe.to_string()
    }
}

#[tauri::command]
fn export_diagnostic_logs(app: tauri::AppHandle) -> Result<String, String> {
    use std::io::Write;

    let log_dir = log_dir()?;
    let out_path = log_dir.join("diagnostic-export.zip");
    let file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    // Pack recent log files
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "log").unwrap_or(false) {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                zip.start_file(&*name, opts).map_err(|e| e.to_string())?;
                zip.write_all(content.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    let storage_snapshot = match storage::project_storage_diagnostics(&app) {
        Ok(snapshot) => serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?,
        Err(e) => serde_json::json!({
            "healthy": false,
            "error": e,
        })
        .to_string(),
    };
    zip.start_file("project-storage-diagnostics.json", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(storage_snapshot.as_bytes())
        .map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(out_path.to_string_lossy().to_string())
}

#[tauri::command]
fn export_writer_agent_trajectory(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> Result<String, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    let export = kernel.export_trajectory(limit.unwrap_or(200).min(1_000));
    let dir = log_dir()?.join("trajectory");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file_name = format!(
        "writer-agent-{}-{}.jsonl",
        safe_filename_component(&export.project_id),
        agent_runtime::now_ms()
    );
    let path = dir.join(file_name);
    std::fs::write(&path, export.jsonl).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn log_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(|p| {
                std::path::PathBuf::from(p)
                    .join("agent-writer")
                    .join("logs")
            })
            .map_err(|_| "APPDATA not set".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .map(|p| p.join(".config").join("agent-writer").join("logs"))
            .ok_or_else(|| "Home dir not found".to_string())
    }
}

fn resolve_api_key() -> Option<String> {
    load_api_key_from_keychain()
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .filter(|k| !k.is_empty())
}

fn require_api_key() -> Result<String, String> {
    resolve_api_key().ok_or_else(|| "API key not set. Go to Settings.".to_string())
}
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
enum HarnessState {
    Idle,
}

struct AppState {
    harness_state: Mutex<HarnessState>,
    hermes_db: Mutex<HermesDB>,
    editor_prediction: Mutex<Option<EditorPredictionTask>>,
    writer_kernel: Mutex<writer_agent::WriterAgentKernel>,
    manual_agent_history: Mutex<ManualAgentHistory>,
}

struct EditorPredictionTask {
    request_id: String,
    cancel: CancellationToken,
}

fn lock_hermes<'a>(state: &'a AppState) -> Result<MutexGuard<'a, HermesDB>, String> {
    state
        .hermes_db
        .lock()
        .map_err(|_| "Hermes memory lock poisoned".to_string())
}

fn lock_harness_state<'a>(state: &'a AppState) -> Result<MutexGuard<'a, HarnessState>, String> {
    state
        .harness_state
        .lock()
        .map_err(|_| "Harness state lock poisoned".to_string())
}

fn lock_editor_prediction<'a>(
    state: &'a AppState,
) -> Result<MutexGuard<'a, Option<EditorPredictionTask>>, String> {
    state
        .editor_prediction
        .lock()
        .map_err(|_| "Editor prediction lock poisoned".to_string())
}

fn legacy_workspace_db_path(filename: &str) -> Option<std::path::PathBuf> {
    std::env::current_dir().ok().map(|dir| dir.join(filename))
}

fn migrate_legacy_db_if_needed(
    target_path: &std::path::Path,
    legacy_path: Option<std::path::PathBuf>,
) -> Result<(), String> {
    if target_path.exists() {
        return Ok(());
    }

    let Some(legacy_path) = legacy_path else {
        return Ok(());
    };
    if legacy_path == target_path || !legacy_path.exists() {
        return Ok(());
    }

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create memory DB directory '{}': {}",
                parent.display(),
                e
            )
        })?;
    }

    std::fs::copy(&legacy_path, target_path).map_err(|e| {
        format!(
            "Failed to migrate memory DB from '{}' to '{}': {}",
            legacy_path.display(),
            target_path.display(),
            e
        )
    })?;
    Ok(())
}

fn active_project_db_path(
    app: &tauri::AppHandle,
    filename: &str,
) -> Result<std::path::PathBuf, String> {
    Ok(storage::active_project_data_dir(app)?.join(filename))
}

fn open_app_hermes_db(app: &tauri::AppHandle) -> Result<HermesDB, String> {
    let path = active_project_db_path(app, storage::HERMES_DB_FILENAME)?;
    let app_data_legacy = storage::app_data_dir(app)?.join(storage::HERMES_DB_FILENAME);
    migrate_legacy_db_if_needed(&path, Some(app_data_legacy))?;
    migrate_legacy_db_if_needed(&path, legacy_workspace_db_path(storage::HERMES_DB_FILENAME))?;
    HermesDB::open(&path).map_err(|e| {
        format!(
            "Failed to open Hermes memory DB at '{}': {}",
            path.display(),
            e
        )
    })
}

fn open_app_writer_kernel(
    app: &tauri::AppHandle,
) -> Result<writer_agent::WriterAgentKernel, String> {
    let project_id = storage::active_project_id(app)?;
    let path = active_project_db_path(app, storage::WRITER_MEMORY_DB_FILENAME)?;
    let app_data_legacy = storage::app_data_dir(app)?.join(storage::WRITER_MEMORY_DB_FILENAME);
    migrate_legacy_db_if_needed(&path, Some(app_data_legacy))?;
    migrate_legacy_db_if_needed(
        &path,
        legacy_workspace_db_path(storage::WRITER_MEMORY_DB_FILENAME),
    )?;
    let memory = writer_agent::memory::WriterMemory::open(&path).map_err(|e| {
        format!(
            "Failed to open writer memory DB at '{}': {}",
            path.display(),
            e
        )
    })?;
    seed_story_model_if_empty(app, &project_id, &memory);
    Ok(writer_agent::WriterAgentKernel::new(&project_id, memory))
}

fn seed_story_model_if_empty(
    app: &tauri::AppHandle,
    project_id: &str,
    memory: &writer_agent::memory::WriterMemory,
) {
    let project_name = storage::active_project_manifest(app)
        .map(|manifest| manifest.name)
        .unwrap_or_else(|_| "Local Project".to_string());
    let lorebook = storage::load_lorebook(app).unwrap_or_default();
    let outline = storage::load_outline(app).unwrap_or_default();
    match writer_agent::context::seed_story_contract_from_project_assets(
        project_id,
        &project_name,
        &lorebook,
        &outline,
        memory,
    ) {
        Ok(true) => tracing::info!("Seeded initial story contract for project {}", project_id),
        Ok(false) => {}
        Err(e) => tracing::warn!("Story contract seed skipped: {}", e),
    }
    match writer_agent::context::seed_chapter_missions_from_outline(project_id, &outline, memory) {
        Ok(count) if count > 0 => {
            tracing::info!(
                "Seeded {} chapter missions for project {}",
                count,
                project_id
            )
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Chapter mission seed skipped: {}", e),
    }
}

fn startup_error(message: String) -> Box<dyn std::error::Error> {
    tracing::error!("{}", message);
    std::io::Error::new(std::io::ErrorKind::Other, message).into()
}

#[derive(Serialize, Clone)]
struct StreamChunk {
    content: String,
}

#[derive(Serialize, Clone)]
struct StreamEnd {
    reason: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InlineWriterOperationEvent {
    request_id: String,
    proposal: writer_agent::proposal::AgentProposal,
    operation: writer_agent::operation::WriterOperation,
}

use agent_harness_core::truncate_context;

use commands::chapters::{create_chapter, get_chapter_revision, load_chapter, read_project_dir};
use commands::lore::{delete_lore_entry, get_lorebook, save_lore_entry};
use commands::outline::{
    delete_outline_node, get_outline, reorder_outline_nodes, save_outline_node,
    update_outline_status,
};
use commands::settings::{check_api_key, set_api_key};
use manual_agent::{
    lock_manual_agent_history, manual_agent_history_messages, merge_manual_agent_history,
    ManualAgentHistory, ManualAgentTurn, MANUAL_AGENT_HISTORY_MAX_CHARS,
    MANUAL_AGENT_HISTORY_MAX_TURNS, MANUAL_AGENT_PERSISTED_HISTORY_LOOKBACK,
};

pub(crate) fn audit_project_file_write(
    app: &tauri::AppHandle,
    scope: &str,
    title: &str,
    decision: &str,
    rationale: &str,
    sources: &[String],
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(kernel) = state.writer_kernel.lock() else {
        return;
    };
    if let Err(e) = kernel
        .memory
        .record_decision(scope, title, decision, &[], rationale, sources)
    {
        tracing::warn!("WriterAgent file-write audit failed: {}", e);
    }
}

fn backup_target_label(target: &storage::BackupTarget) -> String {
    match target {
        storage::BackupTarget::Lorebook => "lorebook".to_string(),
        storage::BackupTarget::Outline => "outline".to_string(),
        storage::BackupTarget::ProjectBrain => "project_brain".to_string(),
        storage::BackupTarget::Chapter { title } => format!("chapter:{}", title),
    }
}

#[tauri::command]
fn save_chapter(app: tauri::AppHandle, title: String, content: String) -> Result<String, String> {
    let revision = storage::save_chapter_content_and_revision(&app, &title, &content)?;
    audit_project_file_write(
        &app,
        &title,
        &format!("Chapter saved: {}", title),
        "saved_chapter",
        &format!(
            "Chapter '{}' saved with revision {} ({} chars).",
            title,
            revision,
            html_to_plain_text(&content).chars().count()
        ),
        &[format!("chapter:{}:{}", title, revision)],
    );
    if let Err(e) = observe_chapter_save(&app, &title, &content, &revision) {
        tracing::warn!("WriterAgent chapter-save observation failed: {}", e);
    }

    if let Some(bus_state) = app.try_state::<Mutex<agent_harness_core::AmbientEventBus>>() {
        if let Ok(bus) = bus_state.lock() {
            let _ = bus.publish(EditorEvent::ChapterSaved {
                chapter: title.clone(),
                content_length: content.chars().count(),
                revision: revision.clone(),
            });
        }
    }

    // Background auto-embed
    let app_clone = app.clone();
    let title_clone = title.clone();
    let content_clone = content.clone();
    tokio::spawn(async move {
        auto_embed_chapter(&app_clone, &title_clone, &content_clone).await;
    });

    Ok(revision)
}

fn observe_chapter_save(
    app: &tauri::AppHandle,
    title: &str,
    content: &str,
    revision: &str,
) -> Result<(), String> {
    let project_id = storage::active_project_id(app)?;
    let text = html_to_plain_text(content);
    let paragraph = last_meaningful_paragraph(&text).unwrap_or_else(|| char_tail(&text, 400));
    let cursor = text.chars().count();
    let observation = writer_agent::observation::WriterObservation {
        id: format!("save-{}", agent_runtime::now_ms()),
        created_at: agent_runtime::now_ms(),
        source: writer_agent::observation::ObservationSource::ChapterSave,
        reason: writer_agent::observation::ObservationReason::Save,
        project_id,
        chapter_title: Some(title.to_string()),
        chapter_revision: Some(revision.to_string()),
        cursor: Some(writer_agent::observation::TextRange {
            from: cursor,
            to: cursor,
        }),
        selection: None,
        prefix: char_tail(&text, 3_000),
        suffix: String::new(),
        paragraph,
        full_text_digest: Some(storage::content_revision(&text)),
        editor_dirty: false,
    };

    let state = app.state::<AppState>();
    let proposals = {
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        refresh_kernel_canon_from_lorebook(app, &mut kernel);
        kernel.observe(observation.clone())?
    };
    for proposal in proposals {
        app.emit(events::AGENT_PROPOSAL, proposal)
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    if resolve_api_key().is_some() {
        spawn_llm_memory_proposals(app.clone(), observation);
    }

    Ok(())
}

fn observe_generated_chapter_result(
    app: &tauri::AppHandle,
    saved: &chapter_generation::SaveGeneratedChapterOutput,
    generated_content: &str,
) {
    if let Err(e) = observe_chapter_save(
        app,
        &saved.chapter_title,
        generated_content,
        &saved.new_revision,
    ) {
        tracing::warn!(
            "WriterAgent generated-chapter result feedback failed for '{}': {}",
            saved.chapter_title,
            e
        );
    }
}

fn last_meaningful_paragraph(text: &str) -> Option<String> {
    text.split('\n')
        .rev()
        .map(str::trim)
        .find(|line| line.chars().count() >= 8)
        .map(ToString::to_string)
}

fn html_to_plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut entity = String::new();
    let mut in_entity = false;

    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            continue;
        }

        if in_entity {
            if ch == ';' {
                out.push_str(&decode_html_entity(&entity));
                entity.clear();
                in_entity = false;
            } else if entity.chars().count() < 12 {
                entity.push(ch);
            } else {
                out.push('&');
                out.push_str(&entity);
                out.push(ch);
                entity.clear();
                in_entity = false;
            }
            continue;
        }

        match ch {
            '<' => in_tag = true,
            '&' => in_entity = true,
            '\r' => {}
            _ => out.push(ch),
        }
    }

    if in_entity {
        out.push('&');
        out.push_str(&entity);
    }

    out.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_html_entity(entity: &str) -> String {
    match entity {
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "nbsp" => " ".to_string(),
        entity if entity.starts_with("#x") || entity.starts_with("#X") => {
            u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| format!("&{};", entity))
        }
        entity if entity.starts_with('#') => entity[1..]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_else(|| format!("&{};", entity)),
        _ => format!("&{};", entity),
    }
}

#[derive(Serialize, Clone)]
struct BatchStatus {
    chapter_title: String,
    status: String,
    error: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChapterGenerationStart {
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentKernelStatus {
    tool_generation: u64,
    tool_count: usize,
    effective_tool_count: usize,
    blocked_tool_count: usize,
    model_callable_tool_count: usize,
    approval_required_tool_count: usize,
    write_tool_count: usize,
    domain_id: String,
    capability_count: usize,
    quality_gate_count: usize,
    trace_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditorStatePayload {
    request_id: String,
    prefix: String,
    suffix: String,
    cursor_position: usize,
    text_cursor_position: Option<usize>,
    paragraph: String,
    chapter_title: Option<String>,
    chapter_revision: Option<String>,
    editor_dirty: Option<bool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorGhostChunk {
    request_id: String,
    proposal_id: Option<String>,
    operation: Option<writer_agent::operation::WriterOperation>,
    cursor_position: usize,
    content: String,
    intent: Option<String>,
    candidates: Vec<EditorGhostCandidate>,
    replace: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorGhostCandidate {
    id: String,
    label: String,
    text: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorGhostEnd {
    request_id: String,
    cursor_position: usize,
    reason: String,
}

#[derive(Debug, Clone)]
struct EditorGhostRenderTarget {
    request_id: String,
    cursor_position: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemanticLintPayload {
    request_id: String,
    paragraph: String,
    paragraph_from: usize,
    cursor_position: usize,
    chapter_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParallelDraftPayload {
    prefix: String,
    suffix: String,
    paragraph: String,
    selected_text: String,
    chapter_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AskAgentContext {
    chapter_title: Option<String>,
    chapter_revision: Option<String>,
    cursor_position: Option<usize>,
    dirty: Option<bool>,
    mode: Option<AskAgentMode>,
    request_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum AskAgentMode {
    Chat,
    InlineOperation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParallelDraft {
    id: String,
    label: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorSemanticLint {
    request_id: String,
    cursor_position: usize,
    from: usize,
    to: usize,
    message: String,
    severity: String,
}

fn realtime_cowrite_enabled() -> bool {
    std::env::var("AGENT_WRITER_REALTIME_COWRITE")
        .map(|value| {
            let normalized = value.trim().to_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "disabled")
        })
        .unwrap_or(true)
}

fn paragraph_hint(paragraph: &str) -> String {
    let trimmed = paragraph.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("\nCurrent paragraph:\n{}\n", trimmed)
    }
}

fn trim_ghost_completion(text: &str) -> String {
    let without_markers = text
        .replace("<|fim_middle|>", "")
        .replace("<|fim_prefix|>", "")
        .replace("<|fim_suffix|>", "");
    let trimmed = without_markers.trim_matches(|c: char| c == '`' || c.is_whitespace());
    trimmed.chars().take(180).collect::<String>()
}

fn clean_ghost_candidate_text(text: &str) -> String {
    trim_ghost_completion(text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn ghost_intent_label(proposal: &writer_agent::proposal::AgentProposal) -> Option<String> {
    proposal
        .rationale
        .split("意图识别:")
        .nth(1)
        .and_then(|tail| tail.split_whitespace().next())
        .map(|label| label.trim_matches(|c: char| c == '.' || c == ',' || c == '，'))
        .filter(|label| !label.is_empty())
        .map(str::to_string)
}

fn proposal_to_ghost_candidate(
    proposal: &writer_agent::proposal::AgentProposal,
    id: &str,
    label: &str,
) -> Option<EditorGhostCandidate> {
    let text = clean_ghost_candidate_text(&proposal.preview);
    if text.is_empty() {
        return None;
    }
    Some(EditorGhostCandidate {
        id: id.to_string(),
        label: label.to_string(),
        text,
    })
}

fn ghost_candidates_from_proposal(
    proposal: &writer_agent::proposal::AgentProposal,
    default_id: &str,
    default_label: &str,
) -> Vec<EditorGhostCandidate> {
    let mut candidates = proposal
        .alternatives
        .iter()
        .filter_map(|alternative| {
            let text = clean_ghost_candidate_text(&alternative.preview);
            if text.is_empty() {
                return None;
            }
            Some(EditorGhostCandidate {
                id: alternative.id.clone(),
                label: alternative.label.clone(),
                text,
            })
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        if let Some(candidate) = proposal_to_ghost_candidate(proposal, default_id, default_label) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn emit_editor_ghost_end(
    app: &tauri::AppHandle,
    target: &EditorGhostRenderTarget,
    reason: &str,
) -> Result<(), String> {
    app.emit(
        events::EDITOR_GHOST_END,
        EditorGhostEnd {
            request_id: target.request_id.clone(),
            cursor_position: target.cursor_position,
            reason: reason.to_string(),
        },
    )
    .map_err(|e| format!("Failed to emit editor ghost end: {}", e))?;
    clear_editor_prediction_for_output(app, Some(&target.request_id));
    Ok(())
}

fn emit_writer_ghost_proposal(
    app: &tauri::AppHandle,
    target: &EditorGhostRenderTarget,
    proposal: &writer_agent::proposal::AgentProposal,
    replace: bool,
    complete: bool,
) -> Result<(), String> {
    let candidates = ghost_candidates_from_proposal(
        proposal,
        if replace { "llm" } else { "a" },
        if replace {
            "AI 增强"
        } else {
            "A 内核接力"
        },
    );
    let Some(first_candidate) = candidates.first() else {
        return Ok(());
    };
    let content = first_candidate.text.clone();
    app.emit(
        events::EDITOR_GHOST_CHUNK,
        EditorGhostChunk {
            request_id: target.request_id.clone(),
            proposal_id: Some(proposal.id.clone()),
            operation: proposal.operations.first().cloned(),
            cursor_position: target.cursor_position,
            content,
            intent: ghost_intent_label(proposal),
            candidates,
            replace,
        },
    )
    .map_err(|e| format!("Failed to emit editor ghost: {}", e))?;
    if complete {
        emit_editor_ghost_end(app, target, "complete")?;
    }
    Ok(())
}

fn trim_parallel_draft(text: &str) -> String {
    text.trim_matches(|c: char| c == '`' || c.is_whitespace())
        .chars()
        .take(1200)
        .collect::<String>()
}

fn parse_parallel_drafts(raw: &str) -> Vec<ParallelDraft> {
    let labels = ["A 顺势推进", "B 冲突加压", "C 情绪转折"];
    let ids = ["a", "b", "c"];
    let mut drafts = Vec::new();
    let mut current_idx: Option<usize> = None;
    let mut current_text = String::new();

    let flush = |drafts: &mut Vec<ParallelDraft>,
                 current_idx: &mut Option<usize>,
                 current_text: &mut String| {
        let Some(idx) = current_idx.take() else {
            current_text.clear();
            return;
        };
        let text = trim_parallel_draft(current_text);
        current_text.clear();
        if text.is_empty() {
            return;
        }
        drafts.push(ParallelDraft {
            id: ids[idx].to_string(),
            label: labels[idx].to_string(),
            text,
        });
    };

    for line in raw.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .split_once(':')
            .or_else(|| trimmed.split_once('：'))
            .and_then(|(head, body)| {
                let idx = match head.trim().chars().next().map(|c| c.to_ascii_uppercase()) {
                    Some('A') => 0,
                    Some('B') => 1,
                    Some('C') => 2,
                    _ => return None,
                };
                Some((idx, body.trim_start()))
            });

        if let Some((idx, body)) = marker {
            flush(&mut drafts, &mut current_idx, &mut current_text);
            current_idx = Some(idx);
            current_text.push_str(body);
        } else if current_idx.is_some() {
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        }
    }
    flush(&mut drafts, &mut current_idx, &mut current_text);
    drafts.truncate(3);
    drafts
}

fn emit_ambient_output(app: &tauri::AppHandle, output: agent_harness_core::AgentOutput) {
    match output {
        agent_harness_core::AgentOutput::GhostText {
            request_id,
            text,
            position,
        } => {
            let request_id_value =
                request_id.unwrap_or_else(|| format!("ambient-{}", agent_runtime::now_ms()));
            let content = trim_ghost_completion(&text);
            if content.is_empty() {
                return;
            }
            let _ = app.emit(
                events::EDITOR_GHOST_CHUNK,
                EditorGhostChunk {
                    request_id: request_id_value.clone(),
                    proposal_id: None,
                    operation: None,
                    cursor_position: position,
                    content,
                    intent: None,
                    candidates: Vec::new(),
                    replace: false,
                },
            );
            let _ = app.emit(
                events::EDITOR_GHOST_END,
                EditorGhostEnd {
                    request_id: request_id_value.clone(),
                    cursor_position: position,
                    reason: "complete".to_string(),
                },
            );
            clear_editor_prediction_for_output(app, Some(&request_id_value));
        }
        agent_harness_core::AgentOutput::MultiGhost {
            request_id,
            position,
            intent,
            candidates,
        } => {
            let request_id_value =
                request_id.unwrap_or_else(|| format!("ambient-{}", agent_runtime::now_ms()));
            let candidates = candidates
                .into_iter()
                .map(|candidate| EditorGhostCandidate {
                    id: candidate.id,
                    label: candidate.label,
                    text: trim_ghost_completion(&candidate.text),
                })
                .filter(|candidate| !candidate.text.is_empty())
                .collect::<Vec<_>>();
            let content = candidates
                .first()
                .map(|candidate| candidate.text.clone())
                .unwrap_or_default();
            if content.is_empty() {
                return;
            }
            let _ = app.emit(
                events::EDITOR_GHOST_CHUNK,
                EditorGhostChunk {
                    request_id: request_id_value.clone(),
                    proposal_id: None,
                    operation: None,
                    cursor_position: position,
                    content,
                    intent: Some(intent),
                    candidates,
                    replace: false,
                },
            );
            let _ = app.emit(
                events::EDITOR_GHOST_END,
                EditorGhostEnd {
                    request_id: request_id_value.clone(),
                    cursor_position: position,
                    reason: "complete".to_string(),
                },
            );
            clear_editor_prediction_for_output(app, Some(&request_id_value));
        }
        agent_harness_core::AgentOutput::GhostEnd {
            request_id,
            position,
            reason,
        } => {
            let request_id_value =
                request_id.unwrap_or_else(|| format!("ambient-{}", agent_runtime::now_ms()));
            let _ = app.emit(
                events::EDITOR_GHOST_END,
                EditorGhostEnd {
                    request_id: request_id_value.clone(),
                    cursor_position: position,
                    reason,
                },
            );
            clear_editor_prediction_for_output(app, Some(&request_id_value));
        }
        agent_harness_core::AgentOutput::SemanticLint {
            message,
            from,
            to,
            severity,
        } => {
            let _ = app.emit(
                events::EDITOR_SEMANTIC_LINT,
                EditorSemanticLint {
                    request_id: format!("ambient-lint-{}", agent_runtime::now_ms()),
                    cursor_position: to,
                    from,
                    to,
                    message,
                    severity,
                },
            );
        }
        agent_harness_core::AgentOutput::HoverHint { message, from, to } => {
            let _ = app.emit(
                events::EDITOR_HOVER_HINT,
                serde_json::json!({
                    "message": message,
                    "from": from,
                    "to": to,
                }),
            );
        }
        agent_harness_core::AgentOutput::EntityCard {
            keyword,
            content,
            chapter,
        } => {
            let _ = app.emit(
                events::EDITOR_ENTITY_CARD,
                serde_json::json!({
                    "keyword": keyword,
                    "content": content,
                    "chapter": chapter,
                }),
            );
        }
        agent_harness_core::AgentOutput::StoryboardMarker {
            chapter,
            message,
            level,
        } => {
            let _ = app.emit(
                events::STORYBOARD_MARKER,
                serde_json::json!({
                    "chapter": chapter,
                    "message": message,
                    "level": level,
                }),
            );
        }
        agent_harness_core::AgentOutput::Epiphany { skill, category } => {
            let _ = app.emit(
                events::AGENT_EPIPHANY,
                serde_json::json!({
                    "skill": skill,
                    "category": category,
                }),
            );
        }
        agent_harness_core::AgentOutput::None => {}
    }
}

fn byte_to_char_index(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

fn find_char_range(text: &str, needle: &str) -> Option<(usize, usize)> {
    let start_byte = text.find(needle)?;
    let start = byte_to_char_index(text, start_byte);
    let end = start + needle.chars().count();
    Some((start, end))
}

fn semantic_lint_enabled() -> bool {
    std::env::var("AGENT_WRITER_AMBIENT_LINTER")
        .map(|value| {
            let normalized = value.trim().to_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "disabled")
        })
        .unwrap_or(true)
}

fn build_lore_conflict_hint(
    paragraph: &str,
    lore_keyword: &str,
    lore_content: &str,
) -> Option<(usize, usize, String)> {
    let keyword_present = !lore_keyword.trim().is_empty() && paragraph.contains(lore_keyword);
    if !keyword_present {
        return None;
    }

    let content = lore_content.to_lowercase();
    let weapon_conflicts: [(&str, &[&str]); 3] = [
        ("剑", &["刀", "弯刀", "短刀", "长刀", "匕首"]),
        ("长剑", &["刀", "弯刀", "短刀", "长刀", "匕首"]),
        ("枪", &["刀", "剑", "弓"]),
    ];

    for (draft_term, lore_terms) in weapon_conflicts {
        if !paragraph.contains(draft_term) {
            continue;
        }

        if let Some(preferred) = lore_terms.iter().find(|term| content.contains(*term)) {
            let (start, end) = find_char_range(paragraph, draft_term)?;
            return Some((
                start,
                end,
                format!(
                    "设定冲突：{} 的设定更接近使用{}，这里写成{}可能需要确认。",
                    lore_keyword, preferred, draft_term
                ),
            ));
        }
    }

    let contradiction_markers = ["不会", "不擅长", "不能", "从不", "禁止", "忌用"];
    for marker in contradiction_markers {
        let Some(marker_byte) = lore_content.find(marker) else {
            continue;
        };
        let after_marker = &lore_content[marker_byte + marker.len()..];
        let term: String = after_marker
            .chars()
            .skip_while(|c| c.is_whitespace() || *c == '用' || *c == '使')
            .take_while(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(c))
            .take(4)
            .collect();

        if term.chars().count() >= 1 && paragraph.contains(&term) {
            let (start, end) = find_char_range(paragraph, &term)?;
            return Some((
                start,
                end,
                format!(
                    "设定冲突：{} 的设定里提到“{}{}”。",
                    lore_keyword, marker, term
                ),
            ));
        }
    }

    None
}

fn find_semantic_lint(
    app: &tauri::AppHandle,
    payload: &SemanticLintPayload,
) -> Option<EditorSemanticLint> {
    let paragraph = payload.paragraph.trim();
    if paragraph.chars().count() < 8 {
        return None;
    }
    let chapter_label = payload
        .chapter_title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or("当前章节");

    if let Some(lint) = find_writer_agent_diagnostic_lint(app, payload, chapter_label) {
        return Some(lint);
    }

    let lore_entries = match storage::load_lorebook(app) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(
                "Semantic lint skipped lorebook because it failed to load: {}",
                e
            );
            Vec::new()
        }
    };
    for entry in lore_entries {
        if let Some((from, to, message)) =
            build_lore_conflict_hint(paragraph, &entry.keyword, &entry.content)
        {
            return Some(EditorSemanticLint {
                request_id: payload.request_id.clone(),
                cursor_position: payload.cursor_position,
                from: payload.paragraph_from + from,
                to: payload.paragraph_from + to,
                message: format!("{}：{}", chapter_label, message),
                severity: "warning".to_string(),
            });
        }
    }

    let state = app.state::<AppState>();
    let Ok(db) = lock_hermes(&state) else {
        return None;
    };
    let skills = db.get_active_skills().unwrap_or_default();
    drop(db);

    for skill in skills {
        if let Some((from, to, message)) =
            build_lore_conflict_hint(paragraph, &skill.category, &skill.skill)
        {
            return Some(EditorSemanticLint {
                request_id: payload.request_id.clone(),
                cursor_position: payload.cursor_position,
                from: payload.paragraph_from + from,
                to: payload.paragraph_from + to,
                message: format!("{}：{}", chapter_label, message),
                severity: "warning".to_string(),
            });
        }
    }

    None
}

fn find_writer_agent_diagnostic_lint(
    app: &tauri::AppHandle,
    payload: &SemanticLintPayload,
    chapter_label: &str,
) -> Option<EditorSemanticLint> {
    let state = app.state::<AppState>();
    let kernel = state.writer_kernel.lock().ok()?;
    let diagnostics = kernel.diagnose_paragraph(
        &payload.paragraph,
        payload.paragraph_from,
        payload.chapter_title.as_deref().unwrap_or("Chapter-1"),
    );
    drop(kernel);

    let diagnostic = diagnostics.into_iter().next()?;
    let severity = match diagnostic.severity {
        writer_agent::diagnostics::DiagnosticSeverity::Error => "error",
        writer_agent::diagnostics::DiagnosticSeverity::Warning => "warning",
        writer_agent::diagnostics::DiagnosticSeverity::Info => "info",
    };

    Some(EditorSemanticLint {
        request_id: payload.request_id.clone(),
        cursor_position: payload.cursor_position,
        from: diagnostic.from,
        to: diagnostic.to.max(diagnostic.from + 1),
        message: format!("{}：{}", chapter_label, diagnostic.message),
        severity: severity.to_string(),
    })
}

fn abort_editor_prediction_task(
    state: &AppState,
    request_id: Option<&str>,
) -> Result<bool, String> {
    let mut task = lock_editor_prediction(state)?;
    let should_cancel = match (&*task, request_id) {
        (Some(active), Some(request_id)) => active.request_id == request_id,
        (Some(_), None) => true,
        (None, _) => false,
    };

    if should_cancel {
        if let Some(active) = task.take() {
            active.cancel.cancel();
        }
        return Ok(true);
    }

    Ok(false)
}

fn clear_editor_prediction_task(state: &AppState, request_id: &str) -> Result<(), String> {
    let mut task = lock_editor_prediction(state)?;
    if task
        .as_ref()
        .is_some_and(|active| active.request_id == request_id)
    {
        *task = None;
    }
    Ok(())
}

fn clear_editor_prediction_for_output(app: &tauri::AppHandle, request_id: Option<&str>) {
    let Some(request_id) = request_id else {
        return;
    };
    let state = app.state::<AppState>();
    let _ = clear_editor_prediction_task(&state, request_id);
}

#[tauri::command]
fn abort_editor_prediction(
    app: tauri::AppHandle,
    request_id: Option<String>,
) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let aborted = abort_editor_prediction_task(&state, request_id.as_deref())?;
    if aborted {
        if let Some(bus_state) = app.try_state::<Mutex<agent_harness_core::AmbientEventBus>>() {
            if let Ok(mut bus) = bus_state.lock() {
                bus.abort_agent("co-writer");
            }
        }
    }
    Ok(aborted)
}

#[tauri::command]
async fn report_editor_state(
    app: tauri::AppHandle,
    payload: EditorStatePayload,
) -> Result<(), String> {
    if !realtime_cowrite_enabled() {
        return Ok(());
    }

    let prefix = payload.prefix.trim_end();
    if prefix.chars().count() < 12 {
        return Ok(());
    }

    let request_id = payload.request_id.clone();
    let cursor_position = payload.cursor_position;
    let cancel = CancellationToken::new();
    let render_target = EditorGhostRenderTarget {
        request_id: request_id.clone(),
        cursor_position,
    };

    {
        let state = app.state::<AppState>();
        abort_editor_prediction_task(&state, None)?;
        let mut task = lock_editor_prediction(&state)?;
        *task = Some(EditorPredictionTask {
            request_id: request_id.clone(),
            cancel: cancel.clone(),
        });
    }

    let project_id = storage::active_project_id(&app)?;
    let observation = build_writer_observation_from_editor_state(&payload, &project_id);
    let (proposals, context_pack_for_llm) = {
        let state = app.state::<AppState>();
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        refresh_kernel_canon_from_lorebook(&app, &mut kernel);
        let proposals = kernel.observe(observation.clone())?;
        let context_pack = if proposals
            .iter()
            .any(|proposal| proposal.kind == writer_agent::proposal::ProposalKind::Ghost)
            && resolve_api_key().is_some()
        {
            Some(kernel.ghost_context_pack(&observation))
        } else {
            None
        };
        (proposals, context_pack)
    };

    for proposal in &proposals {
        app.emit(events::AGENT_PROPOSAL, proposal.clone())
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    if let Some(proposal) = proposals
        .iter()
        .find(|proposal| proposal.kind == writer_agent::proposal::ProposalKind::Ghost)
    {
        emit_writer_ghost_proposal(
            &app,
            &render_target,
            proposal,
            false,
            context_pack_for_llm.is_none(),
        )?;
    } else {
        emit_editor_ghost_end(&app, &render_target, "complete")?;
    }

    if let Some(context_pack) = context_pack_for_llm {
        spawn_llm_ghost_proposal(app.clone(), observation, context_pack, Some(render_target));
        return Ok(());
    }

    drop(cancel);

    Ok(())
}

#[tauri::command]
async fn report_semantic_lint_state(
    app: tauri::AppHandle,
    payload: SemanticLintPayload,
) -> Result<(), String> {
    if !semantic_lint_enabled() {
        return Ok(());
    }

    let app_clone = app.clone();
    tokio::spawn(async move {
        let _intent = agent_harness_core::Intent::Linter;
        if let Some(lint) = find_semantic_lint(&app_clone, &payload) {
            let _ = app_clone.emit(events::EDITOR_SEMANTIC_LINT, lint);
        }
    });

    Ok(())
}

#[tauri::command]
async fn batch_generate_chapter(
    app: tauri::AppHandle,
    chapter_title: String,
    summary: String,
    frontend_state: Option<FrontendChapterStateSnapshot>,
) -> Result<(), String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let user_profile_entries = collect_user_profile_entries(&app).unwrap_or_default();
    let request_id = chapter_generation::make_request_id("batch");

    let app_clone = app.clone();
    let title_clone = chapter_title.clone();

    tokio::spawn(async move {
        let _ = app_clone.emit(
            events::BATCH_STATUS,
            BatchStatus {
                chapter_title: title_clone.clone(),
                status: "generating".to_string(),
                error: String::new(),
            },
        );

        let payload = GenerateChapterAutonomousPayload {
            request_id: Some(request_id),
            target_chapter_title: Some(title_clone.clone()),
            target_chapter_number: None,
            user_instruction: format!("帮我写《{}》这一章的完整初稿。", title_clone),
            budget: None,
            frontend_state,
            save_mode: SaveMode::ReplaceIfClean,
            chapter_summary_override: Some(summary),
        };
        let user_instruction = payload.user_instruction.clone();
        let trace_app = app_clone.clone();

        let terminal = chapter_generation::run_chapter_generation_pipeline(
            app_clone.clone(),
            settings,
            payload,
            user_profile_entries,
            |event| {
                let _ = app_clone.emit(events::CHAPTER_GENERATION, event);
            },
            move |context| {
                let state = trace_app.state::<AppState>();
                let Ok(mut kernel) = state.writer_kernel.lock() else {
                    return;
                };
                {
                    let packet = chapter_generation::build_chapter_generation_task_packet(
                        &kernel.project_id,
                        &kernel.session_id,
                        context,
                        &user_instruction,
                        agent_runtime::now_ms(),
                    );
                    if let Err(error) = kernel.record_task_packet(
                        context.request_id.clone(),
                        "ChapterGeneration",
                        packet,
                    ) {
                        tracing::warn!(
                            "WriterAgent chapter-generation task packet rejected: {}",
                            error
                        );
                    }
                }
            },
        )
        .await;

        match terminal {
            PipelineTerminal::Completed {
                saved,
                generated_content,
            } => {
                observe_generated_chapter_result(&app_clone, &saved, &generated_content);
                let embed_app = app_clone.clone();
                let embed_title = title_clone.clone();
                tokio::spawn(async move {
                    auto_embed_chapter(&embed_app, &embed_title, &generated_content).await;
                });
                let _ = app_clone.emit(
                    events::BATCH_STATUS,
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "complete".to_string(),
                        error: String::new(),
                    },
                );
            }
            PipelineTerminal::Conflict(conflict) => {
                let _ = app_clone.emit(
                    events::BATCH_STATUS,
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "error".to_string(),
                        error: format!("save conflict: {}", conflict.reason),
                    },
                );
            }
            PipelineTerminal::Failed(error) => {
                let _ = app_clone.emit(
                    events::BATCH_STATUS,
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "error".to_string(),
                        error: error.message,
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn generate_chapter_autonomous(
    app: tauri::AppHandle,
    payload: GenerateChapterAutonomousPayload,
) -> Result<ChapterGenerationStart, String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let user_profile_entries = collect_user_profile_entries(&app).unwrap_or_default();
    let request_id = payload
        .request_id
        .clone()
        .unwrap_or_else(|| chapter_generation::make_request_id("chapter"));
    let payload = GenerateChapterAutonomousPayload {
        request_id: Some(request_id.clone()),
        ..payload
    };
    let app_clone = app.clone();

    tokio::spawn(async move {
        let user_instruction = payload.user_instruction.clone();
        let trace_app = app_clone.clone();
        let terminal = chapter_generation::run_chapter_generation_pipeline(
            app_clone.clone(),
            settings,
            payload,
            user_profile_entries,
            |event: ChapterGenerationEvent| {
                let _ = app_clone.emit(events::CHAPTER_GENERATION, event);
            },
            move |context| {
                let state = trace_app.state::<AppState>();
                let Ok(mut kernel) = state.writer_kernel.lock() else {
                    return;
                };
                {
                    let packet = chapter_generation::build_chapter_generation_task_packet(
                        &kernel.project_id,
                        &kernel.session_id,
                        context,
                        &user_instruction,
                        agent_runtime::now_ms(),
                    );
                    if let Err(error) = kernel.record_task_packet(
                        context.request_id.clone(),
                        "ChapterGeneration",
                        packet,
                    ) {
                        tracing::warn!(
                            "WriterAgent chapter-generation task packet rejected: {}",
                            error
                        );
                    }
                }
            },
        )
        .await;

        if let PipelineTerminal::Completed {
            saved,
            generated_content,
        } = terminal
        {
            observe_generated_chapter_result(&app_clone, &saved, &generated_content);
            let embed_app = app_clone.clone();
            tokio::spawn(async move {
                auto_embed_chapter(&embed_app, &saved.chapter_title, &generated_content).await;
            });
        }
    });

    Ok(ChapterGenerationStart { request_id })
}

async fn auto_embed_chapter(app: &tauri::AppHandle, chapter_title: &str, content: &str) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);

    if let Err(e) = brain_service::embed_chapter(app, &settings, chapter_title, content).await {
        tracing::warn!(
            "Failed to update Project Brain for '{}': {}",
            chapter_title,
            e
        );
    }
}

#[derive(Serialize, Clone)]
struct Epiphany {
    skill: String,
    category: String,
    id: i64,
}

async fn extract_skills_from_recent(app: &tauri::AppHandle) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);

    let state = app.state::<AppState>();
    let recent = {
        let Ok(db) = lock_hermes(&state) else {
            tracing::error!("Failed to lock Hermes memory for recent interactions");
            return;
        };
        db.recent_interactions(20).unwrap_or_default()
    };
    // Guard dropped before any .await

    if recent.len() < 4 {
        return;
    }

    let transcript: String = recent
        .iter()
        .map(|r| format!("[{}]: {}", r.role, r.content))
        .collect::<Vec<_>>()
        .join("\n");

    let parsed = match llm_runtime::chat_json(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": "You are a reflection engine. Analyze the recent interaction transcript and extract 1-2 reusable writing rules or user preferences. Output JSON: {\"skills\": [{\"skill\": \"...\", \"category\": \"style|character|pacing|preference\"}]}. If nothing new, output {\"skills\": []}."}),
            serde_json::json!({"role": "user", "content": format!("Transcript:\n{}", transcript)}),
        ],
        30,
    )
    .await
    {
        Ok(b) => b,
        Err(_) => return,
    };

    let skills = parsed["skills"].as_array();
    if let Some(skills) = skills {
        let Ok(db) = lock_hermes(&state) else {
            tracing::error!("Failed to lock Hermes memory for skill extraction");
            return;
        };
        for s in skills {
            let skill_text = s["skill"].as_str().unwrap_or("").to_string();
            let category = s["category"].as_str().unwrap_or("general").to_string();
            if skill_text.is_empty() || skill_text.len() < 10 {
                continue;
            }
            if let Ok(id) = db.insert_skill(&skill_text, &category) {
                let _ = app.emit(
                    events::AGENT_EPIPHANY,
                    Epiphany {
                        skill: skill_text,
                        category,
                        id,
                    },
                );
            }
        }
        // SimpleMem-inspired consolidation: decay → merge → prune
        let _ = db.consolidate();
        let _ = db.clean_old_sessions();
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

fn budget_items(items: &[String], max_tokens: usize) -> Vec<String> {
    let mut accepted = Vec::new();
    let mut consumed = 0;
    for item in items {
        let cost = estimate_tokens(item);
        if consumed + cost > max_tokens {
            break;
        }
        accepted.push(item.clone());
        consumed += cost;
    }
    accepted
}

fn build_context_injection(app: &tauri::AppHandle, query: &str) -> String {
    let state = app.state::<AppState>();
    let Ok(db) = lock_hermes(&state) else {
        tracing::error!("Failed to lock Hermes memory for context injection");
        return String::new();
    };

    let mut parts = Vec::new();

    // 1. User drift profile
    if let Ok(profiles) = db.get_drift_profiles() {
        if !profiles.is_empty() {
            let profile_text: Vec<String> = profiles
                .iter()
                .map(|p| {
                    format!(
                        "- {}: {} (confidence {:.0}%)",
                        p.key,
                        p.value,
                        p.confidence * 100.0
                    )
                })
                .collect();
            let budgeted = budget_items(&profile_text, 200);
            if !budgeted.is_empty() {
                parts.push(format!(
                    "## User Preferences (learned over time)\n{}\n",
                    budgeted.join("\n")
                ));
            }
        }
    }

    // 2. Relevant skills (keyword match + token budget capped at 300)
    if !query.is_empty() {
        if let Ok(skills) = db.search_skills(query) {
            if !skills.is_empty() {
                let skill_text: Vec<String> = skills
                    .iter()
                    .map(|s| format!("- [{}] {}", s.category, s.skill))
                    .collect();
                let budgeted = budget_items(&skill_text, 300);
                if !budgeted.is_empty() {
                    parts.push(format!(
                        "## Relevant Learned Skills\n{}\n",
                        budgeted.join("\n")
                    ));
                }
            }
        }
    }

    drop(db);

    parts.join("\n")
}

fn collect_user_profile_entries(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let db = lock_hermes(&state)?;
    let profiles = db
        .get_drift_profiles()
        .map_err(|e| format!("Failed to read user profile: {}", e))?;
    Ok(profiles
        .iter()
        .map(|profile| {
            format!(
                "- {}: {} (confidence {:.0}%)",
                profile.key,
                profile.value,
                profile.confidence * 100.0
            )
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReviewItem {
    quote: String,
    #[serde(rename = "type")]
    review_type: String,
    issue: String,
    suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReviewReport {
    reviews: Vec<ReviewItem>,
}

#[tauri::command]
async fn analyze_chapter(
    _app: tauri::AppHandle,
    content: String,
) -> Result<Vec<ReviewItem>, String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);

    let system_prompt = r#"You are a professional novel editor. Analyze the chapter and output a JSON object with a "reviews" array.

Each review must have:
- "quote": exact text from the chapter (copy verbatim, at least 10 characters)
- "type": one of "logic" | "ooc" | "pacing" | "prose"
- "issue": what the problem is
- "suggestion": how to fix it (in Chinese, specific rewrite suggestion)

Output ONLY the JSON object, no explanation outside. Example:
{"reviews":[{"quote":"他走出了房间","type":"prose","issue":"缺乏画面感","suggestion":"他推开吱呀作响的木门，幽暗的走廊里只有自己的脚步声在回荡。"}]}"#;

    let truncated = truncate_context(&content, 8000);
    let body = llm_runtime::chat_json(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": format!("Analyze this chapter:\n\n{}", truncated)}),
        ],
        60,
    )
    .await?;

    let report: ReviewReport =
        serde_json::from_value(body).map_err(|e| format!("Failed to parse review JSON: {}", e))?;

    Ok(report.reviews)
}

#[tauri::command]
async fn ask_project_brain(app: tauri::AppHandle, query: String) -> Result<(), String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);

    brain_service::answer_query(&app, &settings, &query, |content| {
        let _ = app.emit(events::AGENT_STREAM_CHUNK, StreamChunk { content });
        Ok(llm_runtime::StreamControl::Continue)
    })
    .await?;

    let _ = app.emit(
        events::AGENT_STREAM_END,
        StreamEnd {
            reason: "complete".to_string(),
        },
    );
    Ok(())
}

#[tauri::command]
async fn generate_parallel_drafts(
    payload: ParallelDraftPayload,
) -> Result<Vec<ParallelDraft>, String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let chapter = payload
        .chapter_title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or("当前章节");
    let focus = if payload.selected_text.trim().is_empty() {
        payload.paragraph.trim()
    } else {
        payload.selected_text.trim()
    };

    let prompt = format!(
        "你是中文小说共创写手。请顺着用户已有文本，生成三个不同方向的平行草稿。\n\
         输出格式必须严格为：\n\
         A: ...\nB: ...\nC: ...\n\
         每个版本 2-5 句，可以分段；不要解释，不要 Markdown。\n\
         A 偏顺势推进，B 偏冲突加压，C 偏情绪转折。\n\
         ## 章节\n{}\n## 光标前文\n{}\n## 光标后文\n{}\n## 当前焦点\n{}",
        chapter,
        truncate_context(&payload.prefix, 3000),
        truncate_context(&payload.suffix, 1000),
        focus,
    );

    let text = llm_runtime::chat_text(
        &settings,
        vec![serde_json::json!({"role": "user", "content": prompt})],
        false,
        45,
    )
    .await?;
    let drafts = parse_parallel_drafts(&text);
    if drafts.is_empty() {
        let fallback = trim_parallel_draft(&text);
        if fallback.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![ParallelDraft {
            id: "a".to_string(),
            label: "A 顺势推进".to_string(),
            text: fallback,
        }]);
    }
    Ok(drafts)
}

#[derive(Debug, Clone, Serialize)]
struct GraphEntity {
    id: String,
    name: String,
    category: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
struct GraphRelationship {
    source: String,
    target: String,
    label: String,
}

#[derive(Debug, Clone, Serialize)]
struct GraphChapter {
    title: String,
    summary: String,
    status: String,
    word_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectGraphData {
    entities: Vec<GraphEntity>,
    relationships: Vec<GraphRelationship>,
    chapters: Vec<GraphChapter>,
}

#[tauri::command]
fn get_project_graph_data(app: tauri::AppHandle) -> Result<ProjectGraphData, String> {
    let mut entities = Vec::new();
    let mut relationships = Vec::new();
    let mut chapters = Vec::new();

    // 1. Entities from Lorebook
    let lore_entries = storage::load_lorebook(&app)?;
    for entry in lore_entries {
        entities.push(GraphEntity {
            id: format!("lore-{}", entry.id),
            name: entry.keyword.clone(),
            category: "character".to_string(),
            description: entry.content.clone(),
        });
    }

    // 2. Entities from agent_skills (extracted character rules)
    let state = app.state::<AppState>();
    let db = lock_hermes(&state)?;
    if let Ok(skills) = db.get_active_skills() {
        for skill in skills {
            if skill.category == "character" {
                // Extract entity name from skill description
                let name = skill.skill.chars().take(30).collect::<String>();
                entities.push(GraphEntity {
                    id: format!("skill-{}", skill.id),
                    name,
                    category: "character_trait".to_string(),
                    description: skill.skill.clone(),
                });
            }
        }
    }
    drop(db);

    // 3. Chapters from file tree + outline
    let dir = storage::project_dir(&app)?;
    match storage::load_outline(&app) {
        Ok(outline_nodes) => {
            for node in outline_nodes {
                // Count words in chapter file
                let filename =
                    format!("{}.md", node.chapter_title.replace(' ', "-").to_lowercase());
                let path = dir.join(&filename);
                let word_count = if path.exists() {
                    std::fs::read_to_string(&path)
                        .map(|s| s.split_whitespace().count())
                        .unwrap_or(0)
                } else {
                    0
                };
                chapters.push(GraphChapter {
                    title: node.chapter_title.clone(),
                    summary: node.summary.clone(),
                    status: node.status.clone(),
                    word_count,
                });
            }
        }
        Err(e) => {
            tracing::warn!(
                "Project graph skipped outline because it failed to load: {}",
                e
            );
        }
    }

    // If outline is empty, derive chapters from file tree
    if chapters.is_empty() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    let title = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    let word_count = content.split_whitespace().count();
                    chapters.push(GraphChapter {
                        title,
                        summary: String::new(),
                        status: "empty".to_string(),
                        word_count,
                    });
                }
            }
        }
    }

    // 4. Relationships: co-occurrence of entities in same chapter
    let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
    for chapter in &chapters {
        let filename = format!("{}.md", chapter.title.replace(' ', "-").to_lowercase());
        let path = dir.join(&filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let content_lower = content.to_lowercase();
            let found: Vec<&String> = entity_names
                .iter()
                .filter(|name| content_lower.contains(&name.to_lowercase()))
                .collect();
            if found.len() >= 2 {
                for i in 0..found.len() {
                    for j in i + 1..found.len() {
                        let exists = relationships.iter().any(|r: &GraphRelationship| {
                            (r.source == *found[i] && r.target == *found[j])
                                || (r.source == *found[j] && r.target == *found[i])
                        });
                        if !exists {
                            relationships.push(GraphRelationship {
                                source: found[i].clone(),
                                target: found[j].clone(),
                                label: format!("Co-occur in {}", chapter.title),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(ProjectGraphData {
        entities,
        relationships,
        chapters,
    })
}

#[tauri::command]
fn get_agent_tools() -> Result<Vec<AgentToolDescriptor>, String> {
    Ok(agent_runtime::registered_tools())
}

#[tauri::command]
fn get_effective_agent_tool_inventory() -> Result<agent_harness_core::EffectiveToolInventory, String>
{
    Ok(agent_runtime::effective_tool_inventory())
}

#[tauri::command]
fn get_agent_kernel_status() -> Result<AgentKernelStatus, String> {
    let registry = default_writing_tool_registry();
    let tools = registry.list();
    let inventory = agent_runtime::effective_tool_inventory();
    let domain = writing_domain_profile();

    Ok(AgentKernelStatus {
        tool_generation: registry.generation(),
        tool_count: tools.len(),
        effective_tool_count: inventory.allowed.len(),
        blocked_tool_count: inventory.blocked.len(),
        model_callable_tool_count: inventory.openai_callable_allowed_count(),
        approval_required_tool_count: tools.iter().filter(|tool| tool.requires_approval).count(),
        write_tool_count: tools
            .iter()
            .filter(|tool| tool.side_effect_level == agent_harness_core::ToolSideEffectLevel::Write)
            .count(),
        domain_id: domain.id,
        capability_count: domain.capabilities.len(),
        quality_gate_count: domain.quality_gates.len(),
        trace_enabled: true,
    })
}

#[tauri::command]
fn get_agent_domain_profile() -> Result<agent_harness_core::AgentDomainProfile, String> {
    Ok(writing_domain_profile())
}

#[tauri::command]
fn get_project_storage_diagnostics(
    app: tauri::AppHandle,
) -> Result<storage::ProjectStorageDiagnostics, String> {
    storage::project_storage_diagnostics(&app)
}

#[tauri::command]
fn list_file_backups(
    app: tauri::AppHandle,
    target: storage::BackupTarget,
) -> Result<Vec<storage::FileBackupInfo>, String> {
    storage::list_file_backups(&app, target)
}

#[tauri::command]
fn restore_file_backup(
    app: tauri::AppHandle,
    target: storage::BackupTarget,
    backup_id: String,
) -> Result<(), String> {
    let label = backup_target_label(&target);
    storage::restore_file_backup(&app, target, backup_id.clone())?;
    audit_project_file_write(
        &app,
        &label,
        &format!("Backup restored: {}", label),
        "restored_file_backup",
        &format!("Author restored backup '{}' for {}.", backup_id, label),
        &[format!("backup:{}:{}", label, backup_id)],
    );
    Ok(())
}

#[tauri::command]
fn get_writer_agent_status(
    state: tauri::State<'_, AppState>,
) -> Result<writer_agent::WriterAgentStatus, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.status())
}

#[tauri::command]
fn get_writer_agent_ledger(
    state: tauri::State<'_, AppState>,
) -> Result<writer_agent::kernel::WriterAgentLedgerSnapshot, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.ledger_snapshot())
}

#[tauri::command]
fn get_writer_agent_pending_proposals(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<writer_agent::proposal::AgentProposal>, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.pending_proposals())
}

#[tauri::command]
fn get_story_review_queue(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<writer_agent::kernel::StoryReviewQueueEntry>, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.story_review_queue())
}

#[tauri::command]
fn get_story_debt_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<writer_agent::kernel::StoryDebtSnapshot, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.story_debt_snapshot())
}

#[tauri::command]
fn get_writer_agent_trace(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> Result<writer_agent::kernel::WriterAgentTraceSnapshot, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.trace_snapshot(limit.unwrap_or(20).min(100)))
}

#[tauri::command]
fn apply_proposal_feedback(
    state: tauri::State<'_, AppState>,
    feedback: writer_agent::ProposalFeedback,
) -> Result<writer_agent::WriterAgentStatus, String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.apply_feedback(feedback)?;
    Ok(kernel.status())
}

#[tauri::command]
fn record_implicit_ghost_rejection(
    state: tauri::State<'_, AppState>,
    proposal_id: String,
    created_at: u64,
) -> Result<bool, String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.record_implicit_ghost_rejection(&proposal_id, created_at)
}

#[tauri::command]
fn approve_writer_operation(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    operation: writer_agent::operation::WriterOperation,
    current_revision: String,
    approval: Option<writer_agent::operation::OperationApproval>,
) -> Result<writer_agent::operation::OperationResult, String> {
    if let writer_agent::operation::WriterOperation::OutlineUpdate { node_id, patch } = &operation {
        {
            let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
            let preflight = kernel.approve_editor_operation_with_approval(
                operation.clone(),
                &current_revision,
                approval.as_ref(),
            )?;
            if !preflight
                .error
                .as_ref()
                .is_some_and(|error| error.code == "invalid")
            {
                return Ok(preflight);
            }
        }
        let result = approve_outline_update_operation(
            &app,
            operation.clone(),
            node_id,
            patch.clone(),
            approval.as_ref(),
        )?;
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        if !result.success {
            let save_result = result
                .error
                .as_ref()
                .map(|error| format!("{}:{}", error.code, error.message))
                .unwrap_or_else(|| "outline_storage:failed".to_string());
            kernel.record_operation_durable_save(
                approval
                    .as_ref()
                    .and_then(|context| context.proposal_id.clone()),
                operation,
                save_result,
            )?;
            return Ok(result);
        }
        if let Some(context) = approval
            .as_ref()
            .filter(|context| context.is_valid_for_write())
        {
            kernel.record_operation_durable_save(
                context.proposal_id.clone(),
                operation,
                "outline_storage:ok".to_string(),
            )?;
        }
        return Ok(result);
    }

    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.approve_editor_operation_with_approval(operation, &current_revision, approval.as_ref())
}

#[tauri::command]
fn record_writer_operation_durable_save(
    state: tauri::State<'_, AppState>,
    proposal_id: Option<String>,
    operation: writer_agent::operation::WriterOperation,
    save_result: String,
) -> Result<(), String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.record_operation_durable_save(proposal_id, operation, save_result)
}

fn approve_outline_update_operation(
    app: &tauri::AppHandle,
    operation: writer_agent::operation::WriterOperation,
    node_id: &str,
    patch: serde_json::Value,
    approval: Option<&writer_agent::operation::OperationApproval>,
) -> Result<writer_agent::operation::OperationResult, String> {
    if !approval.is_some_and(|context| context.is_valid_for_write()) {
        return Ok(writer_agent::operation::OperationResult {
            success: false,
            operation,
            error: Some(writer_agent::operation::OperationError::approval_required(
                "outline.update requires an explicit surfaced approval context",
            )),
            revision_after: None,
        });
    }

    match storage::patch_outline_node(app, node_id.to_string(), patch) {
        Ok(_) => Ok(writer_agent::operation::OperationResult {
            success: true,
            operation,
            error: None,
            revision_after: None,
        }),
        Err(e) => Ok(writer_agent::operation::OperationResult {
            success: false,
            operation,
            error: Some(writer_agent::operation::OperationError::invalid(&e)),
            revision_after: None,
        }),
    }
}

fn to_writer_observation(
    observation: &AgentObservation,
    project_id: &str,
) -> writer_agent::observation::WriterObservation {
    let reason = match observation.reason {
        agent_runtime::AgentObservationReason::UserTyped => {
            if observation.idle_ms >= 900 {
                writer_agent::observation::ObservationReason::Idle
            } else {
                writer_agent::observation::ObservationReason::Typed
            }
        }
        agent_runtime::AgentObservationReason::SelectionChange => {
            writer_agent::observation::ObservationReason::Selection
        }
        agent_runtime::AgentObservationReason::ChapterSwitch => {
            writer_agent::observation::ObservationReason::ChapterSwitch
        }
        agent_runtime::AgentObservationReason::IdleTick => {
            writer_agent::observation::ObservationReason::Idle
        }
    };

    writer_agent::observation::WriterObservation {
        id: observation.id.clone(),
        created_at: observation.created_at,
        source: writer_agent::observation::ObservationSource::Editor,
        reason,
        project_id: project_id.to_string(),
        chapter_title: observation.chapter_title.clone(),
        chapter_revision: observation.chapter_revision.clone(),
        cursor: Some(writer_agent::observation::TextRange {
            from: observation.cursor_position,
            to: observation.cursor_position,
        }),
        selection: observation.selection.as_ref().map(|selection| {
            writer_agent::observation::TextSelection {
                from: selection.from,
                to: selection.to,
                text: selection.text.clone(),
            }
        }),
        prefix: observation.nearby_text.clone(),
        suffix: String::new(),
        paragraph: observation.current_paragraph.clone(),
        full_text_digest: None,
        editor_dirty: observation.dirty,
    }
}

fn refresh_kernel_canon_from_lorebook(
    app: &tauri::AppHandle,
    kernel: &mut writer_agent::WriterAgentKernel,
) {
    let entries = match storage::load_lorebook(app) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("WriterAgent canon refresh skipped lorebook: {}", e);
            return;
        }
    };

    for entry in entries {
        let keyword = entry.keyword.trim();
        if keyword.is_empty() {
            continue;
        }

        let mut attributes = serde_json::Map::new();
        if let Some(weapon) = extract_weapon_from_lore(&entry.content) {
            attributes.insert("weapon".to_string(), serde_json::Value::String(weapon));
        }

        if attributes.is_empty() {
            continue;
        }

        let summary: String = entry.content.chars().take(240).collect();
        let aliases = Vec::<String>::new();
        let _ = kernel.memory.upsert_canon_entity(
            "character",
            keyword,
            &aliases,
            &summary,
            &serde_json::Value::Object(attributes),
            0.8,
        );
    }
}

fn extract_weapon_from_lore(content: &str) -> Option<String> {
    if !["武器", "惯用", "用刀", "用剑", "佩刀", "佩剑", "兵器"]
        .iter()
        .any(|cue| content.contains(cue))
    {
        return None;
    }

    [
        "寒影刀",
        "长剑",
        "短剑",
        "匕首",
        "弓",
        "枪",
        "棍",
        "鞭",
        "斧",
        "刀",
        "剑",
    ]
    .iter()
    .find(|weapon| content.contains(**weapon))
    .map(|weapon| (*weapon).to_string())
}

pub(crate) fn render_writer_context_pack(
    pack: &writer_agent::context::WritingContextPack,
) -> String {
    writer_agent::kernel::render_context_pack_for_prompt(pack)
}

fn build_writer_observation_from_editor_state(
    payload: &EditorStatePayload,
    project_id: &str,
) -> writer_agent::observation::WriterObservation {
    let cursor = payload
        .text_cursor_position
        .unwrap_or_else(|| payload.prefix.chars().count());
    let paragraph = if payload.paragraph.trim().is_empty() {
        paragraph_hint(&payload.prefix)
    } else {
        payload.paragraph.clone()
    };

    writer_agent::observation::WriterObservation {
        id: format!("fim-{}", payload.request_id),
        created_at: agent_runtime::now_ms(),
        source: writer_agent::observation::ObservationSource::Editor,
        reason: writer_agent::observation::ObservationReason::Idle,
        project_id: project_id.to_string(),
        chapter_title: payload.chapter_title.clone(),
        chapter_revision: payload.chapter_revision.clone(),
        cursor: Some(writer_agent::observation::TextRange {
            from: cursor,
            to: cursor,
        }),
        selection: None,
        prefix: payload.prefix.clone(),
        suffix: payload.suffix.clone(),
        paragraph,
        full_text_digest: Some(storage::content_revision(&format!(
            "{}{}",
            payload.prefix, payload.suffix
        ))),
        editor_dirty: payload.editor_dirty.unwrap_or(true),
    }
}

#[cfg(test)]
fn test_editor_state_payload(
    prefix: &str,
    suffix: &str,
    paragraph: &str,
    cursor_position: usize,
    text_cursor_position: Option<usize>,
) -> EditorStatePayload {
    EditorStatePayload {
        request_id: "test-request".to_string(),
        prefix: prefix.to_string(),
        suffix: suffix.to_string(),
        cursor_position,
        text_cursor_position,
        paragraph: paragraph.to_string(),
        chapter_title: Some("Chapter-1".to_string()),
        chapter_revision: Some("rev-1".to_string()),
        editor_dirty: Some(true),
    }
}

fn writer_agent_ghost_messages(
    observation: &writer_agent::observation::WriterObservation,
    pack: &writer_agent::context::WritingContextPack,
) -> Vec<serde_json::Value> {
    let context = render_writer_context_pack(pack);
    vec![
        serde_json::json!({
            "role": "system",
            "content": "你是一个中文长篇小说写作 Agent，不是聊天助手。你只负责在光标处提供可直接插入正文的一小段续写。必须尊重已给出的设定、伏笔、风格偏好和光标后文。不要解释，不要 Markdown，不要重复光标前文。输出 1-2 句中文正文。"
        }),
        serde_json::json!({
            "role": "user",
            "content": format!(
                "章节: {}\n光标位置: {}\n当前段落:\n{}\n\nContextPack:\n{}\n\n请输出光标处续写正文:",
                observation.chapter_title.as_deref().unwrap_or("current chapter"),
                observation.cursor.as_ref().map(|c| c.to).unwrap_or(0),
                observation.paragraph,
                context
            )
        }),
    ]
}

pub(crate) fn writer_agent_inline_operation_messages(
    message: &str,
    observation: &writer_agent::observation::WriterObservation,
    pack: &writer_agent::context::WritingContextPack,
) -> Vec<serde_json::Value> {
    let context = render_writer_context_pack(pack);
    let selected = observation.selected_text();
    vec![
        serde_json::json!({
            "role": "system",
            "content": "你是 Forge 的 Cursor 式中文小说写作 Agent。你只为当前光标生成可执行的正文改写或插入文本，不聊天，不解释，不输出 Markdown，不输出 XML action 标签。必须尊重 ContextPack、设定、伏笔和光标后文。输出必须是可直接进入小说正文的中文文本。"
        }),
        serde_json::json!({
            "role": "user",
            "content": format!(
                "作者指令: {}\n章节: {}\n光标文本位置: {}\n选中文本:\n{}\n\n光标前文:\n{}\n\n光标后文:\n{}\n\nContextPack:\n{}\n\n请只输出要应用到正文中的文本:",
                message,
                observation.chapter_title.as_deref().unwrap_or("current chapter"),
                observation.cursor.as_ref().map(|c| c.to).unwrap_or(0),
                selected,
                observation.prefix,
                observation.suffix,
                context
            )
        }),
    ]
}

fn ask_agent_request_id(context_payload: Option<&AskAgentContext>) -> String {
    context_payload
        .and_then(|payload| payload.request_id.clone())
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("ask-{}", agent_runtime::now_ms()))
}

async fn run_inline_writer_operation(
    app: tauri::AppHandle,
    message: String,
    context: String,
    paragraph: String,
    selected_text: String,
    context_payload: Option<AskAgentContext>,
) -> Result<(), String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();
    let project_id = storage::active_project_id(&app)?;
    let request_id = ask_agent_request_id(context_payload.as_ref());
    let observation = build_manual_writer_observation(
        &message,
        &context,
        &paragraph,
        &selected_text,
        context_payload.as_ref(),
        &project_id,
    );

    let (context_pack, local_proposals) = {
        let state = app.state::<AppState>();
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        refresh_kernel_canon_from_lorebook(&app, &mut kernel);
        let local_proposals = kernel.observe(observation.clone())?;
        let context_pack = kernel.context_pack_for_default(
            writer_agent::context::AgentTask::InlineRewrite,
            &observation,
        );
        (context_pack, local_proposals)
    };

    let messages = writer_agent_inline_operation_messages(&message, &observation, &context_pack);
    let draft = crate::llm_runtime::chat_text(&settings, messages, false, 30).await?;

    let (proposal, operation) = {
        let state = app.state::<AppState>();
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        let proposal =
            kernel.create_inline_operation_proposal(observation, &message, draft, &model)?;
        let operation =
            proposal.operations.first().cloned().ok_or_else(|| {
                "inline operation proposal did not include an operation".to_string()
            })?;
        (proposal, operation)
    };

    for proposal in local_proposals {
        app.emit(events::AGENT_PROPOSAL, proposal)
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    app.emit(events::AGENT_PROPOSAL, proposal.clone())
        .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    app.emit(
        events::INLINE_WRITER_OPERATION,
        InlineWriterOperationEvent {
            request_id,
            proposal,
            operation,
        },
    )
    .map_err(|e| format!("Failed to emit inline writer operation: {}", e))?;

    Ok(())
}

pub(crate) fn char_tail(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars().rev().take(max_chars).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn split_context_for_cursor(
    context: &str,
    cursor_position: usize,
    prefix_chars: usize,
    suffix_chars: usize,
) -> (String, String) {
    let cursor_position = cursor_position.min(context.chars().count());
    let prefix: String = context.chars().take(cursor_position).collect();
    let suffix: String = context
        .chars()
        .skip(cursor_position)
        .take(suffix_chars)
        .collect();
    (char_tail(&prefix, prefix_chars), suffix)
}

fn selected_text_range(
    context: &str,
    selected_text: &str,
) -> Option<writer_agent::observation::TextSelection> {
    let selected = selected_text.trim();
    if selected.is_empty() {
        return None;
    }
    let (from, to) = find_char_range(context, selected).unwrap_or((0, selected.chars().count()));
    Some(writer_agent::observation::TextSelection {
        from,
        to,
        text: selected.to_string(),
    })
}

fn build_manual_writer_observation(
    message: &str,
    context: &str,
    paragraph: &str,
    selected_text: &str,
    payload: Option<&AskAgentContext>,
    project_id: &str,
) -> writer_agent::observation::WriterObservation {
    let cursor_position = payload
        .and_then(|payload| payload.cursor_position)
        .unwrap_or_else(|| context.chars().count());
    let chapter_title = payload
        .and_then(|payload| payload.chapter_title.clone())
        .filter(|title| !title.trim().is_empty())
        .or_else(|| Some("manual".to_string()));
    let chapter_revision = payload
        .and_then(|payload| payload.chapter_revision.clone())
        .filter(|revision| !revision.trim().is_empty())
        .or_else(|| Some(storage::content_revision(context)));
    let paragraph = if paragraph.trim().is_empty() {
        if selected_text.trim().is_empty() {
            message.to_string()
        } else {
            selected_text.to_string()
        }
    } else {
        paragraph.to_string()
    };
    let (prefix, suffix) = split_context_for_cursor(context, cursor_position, 3_000, 1_000);

    writer_agent::observation::WriterObservation {
        id: format!("manual-{}", agent_runtime::now_ms()),
        created_at: agent_runtime::now_ms(),
        source: writer_agent::observation::ObservationSource::ManualRequest,
        reason: writer_agent::observation::ObservationReason::Explicit,
        project_id: project_id.to_string(),
        chapter_title,
        chapter_revision,
        cursor: Some(writer_agent::observation::TextRange {
            from: cursor_position,
            to: cursor_position,
        }),
        selection: selected_text_range(context, selected_text),
        prefix,
        suffix,
        paragraph,
        full_text_digest: Some(storage::content_revision(context)),
        editor_dirty: payload.and_then(|payload| payload.dirty).unwrap_or(false),
    }
}

fn writer_agent_memory_messages(
    observation: &writer_agent::observation::WriterObservation,
) -> Vec<serde_json::Value> {
    let text = observation.prefix.trim();
    vec![
        serde_json::json!({
            "role": "system",
            "content": "你是中文长篇小说项目的记忆抽取器。只从用户已经写出的正文中抽取值得长期保存的设定 canon 和未回收伏笔 promises。不要发明正文没有的信息。输出严格 JSON，不要 Markdown。JSON schema: {\"canon\":[{\"kind\":\"character|object|place|rule|entity\",\"name\":\"\",\"aliases\":[],\"summary\":\"\",\"attributes\":{},\"confidence\":0.0}],\"promises\":[{\"kind\":\"open_question|object_in_motion|foreshadowing|mystery\",\"title\":\"\",\"description\":\"\",\"introducedChapter\":\"\",\"expectedPayoff\":\"\",\"priority\":0,\"confidence\":0.0}]}。只返回置信度 >=0.55 的条目，最多 canon 5 条、promises 5 条。"
        }),
        serde_json::json!({
            "role": "user",
            "content": format!(
                "章节: {}\n当前段落:\n{}\n\n章节文本摘录:\n{}",
                observation.chapter_title.as_deref().unwrap_or("current chapter"),
                observation.paragraph,
                truncate_context(text, 3_500)
            )
        }),
    ]
}

fn spawn_llm_memory_proposals(
    app: tauri::AppHandle,
    observation: writer_agent::observation::WriterObservation,
) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();
    let messages = writer_agent_memory_messages(&observation);

    tokio::spawn(async move {
        let value = match llm_runtime::chat_json(&settings, messages, 20).await {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!("Writer Agent LLM memory extraction failed: {}", e);
                return;
            }
        };

        let state = app.state::<AppState>();
        let proposals = {
            let mut kernel = match state.writer_kernel.lock() {
                Ok(kernel) => kernel,
                Err(e) => {
                    tracing::error!("Writer kernel lock poisoned: {}", e);
                    return;
                }
            };
            kernel.create_llm_memory_proposals(observation, value, &model)
        };

        for proposal in proposals {
            let _ = app.emit(events::AGENT_PROPOSAL, proposal);
        }
    });
}

fn spawn_llm_ghost_proposal(
    app: tauri::AppHandle,
    observation: writer_agent::observation::WriterObservation,
    context_pack: writer_agent::context::WritingContextPack,
    render_target: Option<EditorGhostRenderTarget>,
) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();
    let messages = writer_agent_ghost_messages(&observation, &context_pack);

    tokio::spawn(async move {
        let target_for_error = render_target.clone();
        let text = match llm_runtime::chat_text(&settings, messages, false, 12).await {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!("Writer Agent LLM ghost proposal failed: {}", e);
                if let Some(target) = target_for_error {
                    let _ = emit_editor_ghost_end(&app, &target, "complete");
                }
                return;
            }
        };

        if text.trim().is_empty() {
            if let Some(target) = target_for_error {
                let _ = emit_editor_ghost_end(&app, &target, "complete");
            }
            return;
        }

        let state = app.state::<AppState>();
        let proposal = {
            let mut kernel = match state.writer_kernel.lock() {
                Ok(kernel) => kernel,
                Err(e) => {
                    tracing::error!("Writer kernel lock poisoned: {}", e);
                    return;
                }
            };
            match kernel.create_llm_ghost_proposal(observation, text, &model) {
                Ok(proposal) => proposal,
                Err(e) => {
                    tracing::warn!("Writer Agent rejected LLM proposal: {}", e);
                    if let Some(target) = target_for_error {
                        let _ = emit_editor_ghost_end(&app, &target, "complete");
                    }
                    return;
                }
            }
        };

        let _ = app.emit(events::AGENT_PROPOSAL, proposal.clone());
        if let Some(target) = render_target {
            let _ = emit_writer_ghost_proposal(&app, &target, &proposal, true, true);
        }
    });
}

#[tauri::command]
fn agent_observe(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    observation: AgentObservation,
) -> Result<AgentObserveResult, String> {
    let request_id = format!("agent-{}", agent_runtime::now_ms());
    let now = agent_runtime::now_ms();
    let decision = agent_runtime::attention_policy(&observation, now);
    let observation_id = observation.id.clone();

    let mut emitted_proposal_id = None;
    if matches!(observation.mode, agent_runtime::AgentMode::Proactive) {
        let project_id = storage::active_project_id(&app)?;
        let writer_observation = to_writer_observation(&observation, &project_id);
        let writer_observation_for_llm = writer_observation.clone();
        let proposals = {
            let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
            refresh_kernel_canon_from_lorebook(&app, &mut kernel);
            kernel.observe(writer_observation)?
        };
        let should_spawn_llm = proposals
            .iter()
            .any(|proposal| proposal.kind == writer_agent::proposal::ProposalKind::Ghost);
        let context_pack_for_llm = if should_spawn_llm && resolve_api_key().is_some() {
            let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
            Some(kernel.ghost_context_pack(&writer_observation_for_llm))
        } else {
            None
        };

        for proposal in proposals {
            emitted_proposal_id.get_or_insert_with(|| proposal.id.clone());
            app.emit(events::AGENT_PROPOSAL, proposal)
                .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
        }

        if let Some(context_pack) = context_pack_for_llm {
            spawn_llm_ghost_proposal(app.clone(), writer_observation_for_llm, context_pack, None);
        }
    }

    if emitted_proposal_id.is_some() {
        return Ok(AgentObserveResult {
            request_id,
            observation_id,
            decision: "writer_proposal".to_string(),
            reason: decision.reason,
            suggestion_id: emitted_proposal_id,
        });
    }

    if !decision.should_suggest {
        return Ok(AgentObserveResult {
            request_id,
            observation_id,
            decision: "noop".to_string(),
            reason: decision.reason,
            suggestion_id: None,
        });
    }

    let outline_summary = observation
        .chapter_title
        .as_ref()
        .and_then(|chapter_title| match storage::load_outline(&app) {
            Ok(nodes) => nodes
                .into_iter()
                .find(|node| &node.chapter_title == chapter_title)
                .map(|node| node.summary)
                .filter(|summary| !summary.trim().is_empty()),
            Err(e) => {
                tracing::warn!("Agent observe skipped outline summary: {}", e);
                None
            }
        });

    let paragraph_lower = observation.current_paragraph.to_lowercase();
    let nearby_lower = observation.nearby_text.to_lowercase();
    let lore_entries = match storage::load_lorebook(&app) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Agent observe skipped lore hits: {}", e);
            Vec::new()
        }
    };
    let lore_hits = lore_entries
        .into_iter()
        .filter(|entry| {
            let keyword = entry.keyword.to_lowercase();
            !keyword.is_empty()
                && (paragraph_lower.contains(&keyword) || nearby_lower.contains(&keyword))
        })
        .map(|entry| (entry.keyword, entry.content))
        .collect::<Vec<_>>();

    let profile_count = collect_user_profile_entries(&app)
        .map(|entries| entries.len())
        .unwrap_or(0);
    let source_summaries = agent_runtime::build_source_summaries(
        &observation,
        outline_summary,
        lore_hits,
        profile_count,
    );
    let suggestion = agent_runtime::build_suggestion(
        &observation,
        request_id.clone(),
        &decision,
        source_summaries,
    );
    let suggestion_id = suggestion.id.clone();
    app.emit(events::AGENT_SUGGESTION, suggestion)
        .map_err(|e| format!("Failed to emit agent suggestion: {}", e))?;

    Ok(AgentObserveResult {
        request_id,
        observation_id,
        decision: "suggestion".to_string(),
        reason: decision.reason,
        suggestion_id: Some(suggestion_id),
    })
}

#[tauri::command]
async fn analyze_pacing(summaries: String) -> Result<String, String> {
    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let text = llm_runtime::chat_text(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": "You are a structural editor. Analyze the chapter sequence for pacing issues, slow sections, abrupt transitions, and unresolved arcs. Be specific and concise."}),
            serde_json::json!({"role": "user", "content": format!("Chapter summaries:\n{}", summaries)}),
        ],
        false,
        60,
    )
    .await?;

    Ok(if text.is_empty() {
        "No analysis generated".to_string()
    } else {
        text
    })
}

#[tauri::command]
fn rename_chapter_file(
    app: tauri::AppHandle,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    storage::rename_chapter_file(&app, old_name.clone(), new_name.clone())?;
    audit_project_file_write(
        &app,
        &new_name,
        &format!("Chapter renamed: {} -> {}", old_name, new_name),
        "renamed_chapter",
        &format!("Author renamed chapter '{}' to '{}'.", old_name, new_name),
        &[
            format!("chapter:{}", old_name),
            format!("chapter:{}", new_name),
        ],
    );
    Ok(())
}

#[tauri::command]
async fn ask_agent(
    app: tauri::AppHandle,
    message: String,
    context: String,
    paragraph: String,
    selected_text: String,
    context_payload: Option<AskAgentContext>,
) -> Result<(), String> {
    if context_payload
        .as_ref()
        .and_then(|payload| payload.mode.as_ref())
        .is_some_and(|mode| *mode == AskAgentMode::InlineOperation)
    {
        return run_inline_writer_operation(
            app,
            message,
            context,
            paragraph,
            selected_text,
            context_payload,
        )
        .await;
    }

    let api_key = require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();

    let state = app.state::<AppState>();
    let truncated_context = truncate_context(&context, 2000).to_string();
    let project_id = storage::active_project_id(&app)?;
    let runtime_manual_history = {
        let history = lock_manual_agent_history(&state)?;
        history.recent_for_project(
            &project_id,
            MANUAL_AGENT_HISTORY_MAX_TURNS,
            MANUAL_AGENT_HISTORY_MAX_CHARS,
        )
    };
    let persisted_manual_history = {
        let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        kernel
            .memory
            .list_manual_agent_turns(&project_id, MANUAL_AGENT_PERSISTED_HISTORY_LOOKBACK)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(ManualAgentTurn::from)
            .collect::<Vec<_>>()
    };
    let manual_history = merge_manual_agent_history(
        &project_id,
        persisted_manual_history,
        runtime_manual_history,
        MANUAL_AGENT_HISTORY_MAX_TURNS,
        MANUAL_AGENT_HISTORY_MAX_CHARS,
    );
    let manual_observation = build_manual_writer_observation(
        &message,
        &context,
        &paragraph,
        &selected_text,
        context_payload.as_ref(),
        &project_id,
    );
    let (mut prepared_run, emitted_proposals, _has_lore, _has_outline) = {
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        refresh_kernel_canon_from_lorebook(&app, &mut kernel);
        let request = writer_agent::kernel::WriterAgentRunRequest {
            task: writer_agent::kernel::WriterAgentTask::ManualRequest,
            observation: manual_observation.clone(),
            user_instruction: message.clone(),
            frontend_state: writer_agent::kernel::WriterAgentFrontendState {
                truncated_context,
                paragraph: paragraph.clone(),
                selected_text: selected_text.clone(),
                memory_context: build_context_injection(&app, &message),
                has_lore: storage::load_lorebook(&app)
                    .map(|l| !l.is_empty())
                    .unwrap_or(false),
                has_outline: storage::load_outline(&app)
                    .map(|o| !o.is_empty())
                    .unwrap_or(false),
            },
            approval_mode: writer_agent::kernel::WriterAgentApprovalMode::SurfaceProposals,
            stream_mode: writer_agent::kernel::WriterAgentStreamMode::Text,
            manual_history: manual_agent_history_messages(&manual_history),
        };
        let has_lore = request.frontend_state.has_lore;
        let has_outline = request.frontend_state.has_outline;
        let provider = std::sync::Arc::new(OpenAiCompatProvider::new(
            &settings.api_base,
            &settings.api_key,
            &settings.model,
        ));
        let prepared = kernel.prepare_task_run(
            request,
            provider,
            tool_bridge::TauriToolBridge { app: app.clone() },
            &model,
        )?;
        let proposals = prepared.proposals().to_vec();
        (prepared, proposals, has_lore, has_outline)
    };

    for proposal in emitted_proposals {
        app.emit(events::AGENT_PROPOSAL, proposal)
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    // Wire events to Tauri frontend
    let app_handle = app.clone();
    prepared_run.set_event_callback(std::sync::Arc::new(move |event| match event {
        AgentLoopEvent::Intent { intent } => {
            let _ = app_handle.emit(
                events::AGENT_CHAIN_OF_THOUGHT,
                serde_json::json!({
                    "step": 1,
                    "total": 3,
                    "description": format!("Intent: {}", intent),
                    "status": "done",
                }),
            );
        }
        AgentLoopEvent::TextChunk { content } => {
            let _ = app_handle.emit(
                events::AGENT_STREAM_CHUNK,
                serde_json::json!({"content": content}),
            );
        }
        AgentLoopEvent::Error { message } => {
            let _ = app_handle.emit(
                events::AGENT_ERROR,
                serde_json::json!({"message": message, "source": "agent_loop"}),
            );
        }
        AgentLoopEvent::Complete { .. } => {
            let _ = app_handle.emit(
                events::AGENT_STREAM_END,
                serde_json::json!({"reason": "complete"}),
            );
        }
        _ => {
            let _ = app_handle.emit("agent-loop-event", serde_json::json!(event));
        }
    }));

    // Log user message to Hermes memory
    {
        let db = lock_hermes(&state)?;
        let _ = db.log_interaction("user", &message);
    }

    let run_request = prepared_run.request().clone();
    match prepared_run.run().await {
        Ok(run_result) => {
            let final_text = run_result.answer.clone();
            // Log assistant response to Hermes memory
            {
                let db = lock_hermes(&state)?;
                let _ = db.log_interaction("assistant", &final_text);
            }
            {
                let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
                kernel.record_run_completion(&run_request, &run_result)?;
            }
            {
                let mut history = lock_manual_agent_history(&state)?;
                history.append(ManualAgentTurn {
                    project_id: project_id.clone(),
                    created_at: agent_runtime::now_ms(),
                    observation_id: manual_observation.id.clone(),
                    chapter_title: manual_observation.chapter_title.clone(),
                    user: message,
                    assistant: final_text,
                    source_refs: run_result.source_refs,
                });
            }

            // Background skill extraction
            let app_clone = app.clone();
            tokio::spawn(async move {
                extract_skills_from_recent(&app_clone).await;
            });

            {
                let mut s = lock_harness_state(&state)?;
                *s = HarnessState::Idle;
            }
            Ok(())
        }
        Err(e) => {
            {
                let mut s = lock_harness_state(&state)?;
                *s = HarnessState::Idle;
            }
            Err(e)
        }
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();

    // Shared context cache for ambient agents
    let cache: std::sync::Arc<tokio::sync::Mutex<ambient_agents::context_fetcher::ContextCache>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(
            ambient_agents::context_fetcher::ContextCache::default(),
        ));

    // Capture clones before the setup closure
    let cache1 = cache.clone();
    let cache_for_state = cache.clone();

    if let Err(e) = tauri::Builder::default()
        .setup(move |app| {
            let hermes_db = open_app_hermes_db(app.handle()).map_err(startup_error)?;
            let writer_kernel = open_app_writer_kernel(app.handle()).map_err(startup_error)?;

            let mut event_bus = agent_harness_core::AmbientEventBus::new(256);
            let ah = app.handle().clone();
            let output_app = ah.clone();
            event_bus.set_output_handler(std::sync::Arc::new(move |output| {
                emit_ambient_output(&output_app, output);
            }));

            event_bus.spawn_agent(std::sync::Arc::new(
                ambient_agents::context_fetcher::ContextFetcherAgent {
                    app: ah.clone(),
                    cache: cache1,
                },
            ));

            app.manage(Mutex::new(event_bus));
            app.manage(AppState {
                harness_state: Mutex::new(HarnessState::Idle),
                hermes_db: Mutex::new(hermes_db),
                editor_prediction: Mutex::new(None),
                writer_kernel: Mutex::new(writer_kernel),
                manual_agent_history: Mutex::new(ManualAgentHistory::default()),
            });
            app.manage(cache_for_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            abort_editor_prediction,
            report_editor_state,
            report_semantic_lint_state,
            ask_agent,
            agent_observe,
            get_agent_domain_profile,
            get_agent_kernel_status,
            get_project_storage_diagnostics,
            list_file_backups,
            restore_file_backup,
            get_writer_agent_status,
            get_writer_agent_ledger,
            get_writer_agent_pending_proposals,
            get_story_review_queue,
            get_story_debt_snapshot,
            get_writer_agent_trace,
            apply_proposal_feedback,
            record_implicit_ghost_rejection,
            approve_writer_operation,
            record_writer_operation_durable_save,
            get_agent_tools,
            get_effective_agent_tool_inventory,
            get_lorebook,
            save_lore_entry,
            delete_lore_entry,
            read_project_dir,
            create_chapter,
            save_chapter,
            load_chapter,
            get_chapter_revision,
            get_outline,
            save_outline_node,
            delete_outline_node,
            update_outline_status,
            reorder_outline_nodes,
            batch_generate_chapter,
            generate_chapter_autonomous,
            analyze_chapter,
            ask_project_brain,
            generate_parallel_drafts,
            get_project_graph_data,
            analyze_pacing,
            rename_chapter_file,
            set_api_key,
            check_api_key,
            export_diagnostic_logs,
            export_writer_agent_trajectory
        ])
        .run(tauri::generate_context!())
    {
        let message = format!("Error while running Tauri application: {}", e);
        tracing::error!("{}", message);
        let _ = msgbox::create("Agent-Writer Error", &message, msgbox::IconType::Error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_parallel_drafts_keeps_multiline_branches() {
        let drafts = parse_parallel_drafts(
            "A: 林墨没有立刻回答。\n他只是把刀压低。\nB：门外忽然传来脚步声。\nC: 她看见他眼里的犹豫。",
        );

        assert_eq!(drafts.len(), 3);
        assert_eq!(drafts[0].id, "a");
        assert!(drafts[0].text.contains("把刀压低"));
        assert_eq!(drafts[1].label, "B 冲突加压");
        assert_eq!(drafts[2].id, "c");
    }

    #[test]
    fn frontend_protocol_does_not_expose_legacy_commands_or_xml_actions() {
        let protocol = include_str!("../../src/protocol.ts");
        assert!(!protocol.contains("harness_echo"));
        assert!(!protocol.contains("harnessEcho"));
        assert!(!protocol.contains("ACTION_INSERT"));
        assert!(!protocol.contains("ACTION_REPLACE"));
        assert!(!protocol.contains("extractActions"));
    }

    fn test_context_pack() -> writer_agent::context::WritingContextPack {
        writer_agent::context::WritingContextPack {
            task: writer_agent::context::AgentTask::InlineRewrite,
            total_chars: 18,
            budget_limit: 32,
            budget_report: writer_agent::context::ContextBudgetReport {
                total_budget: 32,
                used: 18,
                wasted: 14,
                source_reports: vec![
                    writer_agent::context::SourceReport {
                        source: "SelectedText".to_string(),
                        requested: 20,
                        provided: 12,
                        truncated: false,
                        reason:
                            "InlineRewrite required source reserved 400 chars before priority fill."
                                .to_string(),
                        truncation_reason: None,
                    },
                    writer_agent::context::SourceReport {
                        source: "CursorPrefix".to_string(),
                        requested: 20,
                        provided: 6,
                        truncated: true,
                        reason:
                            "InlineRewrite required source reserved 160 chars before priority fill."
                                .to_string(),
                        truncation_reason: Some(
                            "Source content was limited by its per-source budget of 20 chars."
                                .to_string(),
                        ),
                    },
                ],
            },
            sources: vec![
                writer_agent::context::ContextExcerpt {
                    source: writer_agent::context::ContextSource::SelectedText,
                    content: "林墨握紧寒影刀。".to_string(),
                    char_count: 8,
                    truncated: false,
                    priority: 10,
                    evidence_ref: Some("selection".to_string()),
                },
                writer_agent::context::ContextExcerpt {
                    source: writer_agent::context::ContextSource::CursorPrefix,
                    content: "张三退后".to_string(),
                    char_count: 4,
                    truncated: true,
                    priority: 9,
                    evidence_ref: None,
                },
            ],
        }
    }

    #[test]
    fn render_writer_context_pack_includes_budget_report() {
        let rendered = render_writer_context_pack(&test_context_pack());

        assert!(rendered.contains("# ContextPack Budget"));
        assert!(rendered.contains("task: InlineRewrite"));
        assert!(rendered.contains("used/budget: 18/32"));
        assert!(rendered.contains("wasted: 14"));
        assert!(rendered.contains("- CursorPrefix: requested 20, provided 6, truncated true"));
        assert!(rendered.contains("reason: InlineRewrite required source"));
        assert!(rendered.contains("truncation: Source content was limited"));
        assert!(rendered.contains("# ContextPack Sources"));
        assert!(rendered.contains("## SelectedText"));
        assert!(rendered.contains("林墨握紧寒影刀。"));
    }

    #[test]
    fn inline_operation_messages_include_context_budget_report() {
        let observation = build_manual_writer_observation(
            "改写得更紧张",
            "林墨握紧寒影刀。张三退后。",
            "林墨握紧寒影刀。",
            "寒影刀",
            None,
            "novel-a",
        );
        let messages = writer_agent_inline_operation_messages(
            "改写得更紧张",
            &observation,
            &test_context_pack(),
        );
        let user_content = messages[1]["content"].as_str().unwrap();

        assert!(user_content.contains("ContextPack:"));
        assert!(user_content.contains("# ContextPack Budget"));
        assert!(user_content.contains("used/budget: 18/32"));
        assert!(user_content.contains("truncated true"));
    }

    #[test]
    fn trim_parallel_draft_removes_markdown_fence_noise() {
        assert_eq!(
            trim_parallel_draft("```\n林墨停下脚步。\n```"),
            "林墨停下脚步。"
        );
    }

    #[test]
    fn manual_observation_uses_char_cursor_and_selection_range() {
        let context = "林墨拔出寒影刀。\n张三后退一步。";
        let observation =
            build_manual_writer_observation("这段有冲突吗", context, "", "寒影刀", None, "test");

        assert_eq!(
            observation.source,
            writer_agent::observation::ObservationSource::ManualRequest
        );
        assert_eq!(
            observation.reason,
            writer_agent::observation::ObservationReason::Explicit
        );
        assert_eq!(
            observation.cursor.unwrap().to,
            context.chars().count(),
            "cursor must use character index, not UTF-8 bytes"
        );
        let selection = observation.selection.unwrap();
        assert_eq!(selection.from, 4);
        assert_eq!(selection.to, 7);
        assert_eq!(selection.text, "寒影刀");
        assert_eq!(observation.paragraph, "寒影刀");
    }

    #[test]
    fn manual_agent_history_keeps_only_matching_project_recent_first() {
        let mut history = ManualAgentHistory::default();
        history.append(ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 1,
            observation_id: "obs-a-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            user: "第一问".to_string(),
            assistant: "第一答".to_string(),
            source_refs: vec!["Lorebook".to_string()],
        });
        history.append(ManualAgentTurn {
            project_id: "novel-b".to_string(),
            created_at: 2,
            observation_id: "obs-b-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            user: "另一项目".to_string(),
            assistant: "不应出现".to_string(),
            source_refs: Vec::new(),
        });
        history.append(ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 3,
            observation_id: "obs-a-2".to_string(),
            chapter_title: Some("第二章".to_string()),
            user: "第二问".to_string(),
            assistant: "第二答".to_string(),
            source_refs: vec!["Outline".to_string()],
        });

        let recent = history.recent_for_project("novel-a", 8, 12_000);

        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].user, "第一问");
        assert_eq!(recent[1].user, "第二问");
        assert_eq!(recent[1].source_refs, vec!["Outline".to_string()]);
    }

    #[test]
    fn manual_agent_history_applies_turn_and_char_budgets() {
        let mut history = ManualAgentHistory::default();
        for idx in 0..5 {
            history.append(ManualAgentTurn {
                project_id: "novel-a".to_string(),
                created_at: idx,
                observation_id: format!("obs-a-{}", idx),
                chapter_title: None,
                user: format!("问题{}", idx),
                assistant: "答复".repeat(80),
                source_refs: Vec::new(),
            });
        }

        let recent = history.recent_for_project("novel-a", 2, 12_000);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].user, "问题3");
        assert_eq!(recent[1].user, "问题4");

        let budgeted = history.recent_for_project("novel-a", 8, 180);
        assert_eq!(budgeted.len(), 1);
        assert_eq!(budgeted[0].user, "问题4");
    }

    #[test]
    fn manual_agent_history_messages_restore_dialog_roles() {
        let messages = manual_agent_history_messages(&[ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 42,
            observation_id: "obs-a-42".to_string(),
            chapter_title: Some("第三章".to_string()),
            user: "上一轮怎么处理张三？".to_string(),
            assistant: "先让张三隐瞒玉佩。".to_string(),
            source_refs: vec!["PromiseLedger".to_string()],
        }]);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert!(messages[0]
            .content
            .as_ref()
            .unwrap()
            .contains("上一轮怎么处理张三"));
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .contains("PromiseLedger"));
    }

    #[test]
    fn merge_manual_agent_history_dedupes_persisted_and_runtime_turns() {
        let persisted = vec![ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 1,
            observation_id: "obs-a-1".to_string(),
            chapter_title: None,
            user: "已持久化问题".to_string(),
            assistant: "已持久化答复".to_string(),
            source_refs: Vec::new(),
        }];
        let runtime = vec![
            ManualAgentTurn {
                project_id: "novel-a".to_string(),
                created_at: 1,
                observation_id: "obs-a-1".to_string(),
                chapter_title: None,
                user: "重复问题".to_string(),
                assistant: "重复答复".to_string(),
                source_refs: Vec::new(),
            },
            ManualAgentTurn {
                project_id: "novel-a".to_string(),
                created_at: 2,
                observation_id: "obs-a-2".to_string(),
                chapter_title: None,
                user: "新问题".to_string(),
                assistant: "新答复".to_string(),
                source_refs: Vec::new(),
            },
        ];

        let merged = merge_manual_agent_history("novel-a", persisted, runtime, 8, 12_000);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].user, "已持久化问题");
        assert_eq!(merged[1].user, "新问题");
    }

    #[test]
    fn split_context_for_cursor_keeps_suffix_after_cursor() {
        let (prefix, suffix) = split_context_for_cursor("甲乙丙丁戊", 3, 2, 2);
        assert_eq!(prefix, "乙丙");
        assert_eq!(suffix, "丁戊");
    }

    #[test]
    fn html_to_plain_text_keeps_editor_text_without_tags() {
        let text = html_to_plain_text(
            "<p>林墨&nbsp;拔出寒影刀。</p><p>张三说：&quot;别动&quot;&amp;后退。</p>",
        );

        assert_eq!(text, "林墨 拔出寒影刀。\n张三说：\"别动\"&后退。");
    }

    #[test]
    fn last_meaningful_paragraph_ignores_short_trailing_lines() {
        let paragraph = last_meaningful_paragraph("第一段很长。\n短\n最后一段足够长。").unwrap();
        assert_eq!(paragraph, "最后一段足够长。");
    }

    #[test]
    fn editor_state_observation_uses_text_cursor_for_kernel() {
        let payload = test_editor_state_payload("林墨拔出", "寒影刀", "林墨拔出", 99, Some(4));
        let observation = build_writer_observation_from_editor_state(&payload, "test");

        assert_eq!(
            observation.cursor.unwrap().to,
            4,
            "WriterAgent coordinates must be plain-text character indexes"
        );
        assert_eq!(observation.prefix, "林墨拔出");
        assert_eq!(observation.suffix, "寒影刀");
        assert_eq!(
            observation.reason,
            writer_agent::observation::ObservationReason::Idle
        );
    }

    #[test]
    fn editor_state_observation_falls_back_to_prefix_char_count() {
        let payload = test_editor_state_payload("林墨拔出", "", "", 99, None);
        let observation = build_writer_observation_from_editor_state(&payload, "test");

        assert_eq!(observation.cursor.unwrap().to, "林墨拔出".chars().count());
        assert!(observation.paragraph.contains("Current paragraph"));
    }

    #[test]
    fn migrate_legacy_db_copies_when_target_is_missing() {
        let root = std::env::temp_dir().join(format!(
            "forge-db-migrate-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy_path = root.join("legacy.db");
        let target_path = root.join("app-data").join("writer_memory.db");

        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&legacy_path, b"legacy-memory").unwrap();

        migrate_legacy_db_if_needed(&target_path, Some(legacy_path.clone())).unwrap();

        assert_eq!(std::fs::read(&target_path).unwrap(), b"legacy-memory");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migrate_legacy_db_does_not_overwrite_existing_target() {
        let root = std::env::temp_dir().join(format!(
            "forge-db-migrate-existing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy_path = root.join("legacy.db");
        let target_dir = root.join("app-data");
        let target_path = target_dir.join("writer_memory.db");

        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(&legacy_path, b"legacy-memory").unwrap();
        std::fs::write(&target_path, b"current-memory").unwrap();

        migrate_legacy_db_if_needed(&target_path, Some(legacy_path)).unwrap();

        assert_eq!(std::fs::read(&target_path).unwrap(), b"current-memory");
        let _ = std::fs::remove_dir_all(&root);
    }
}
