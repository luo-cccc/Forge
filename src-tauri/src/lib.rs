use std::sync::{Mutex, MutexGuard};

use agent_harness_core::{
    ambient::EditorEvent, default_writing_tool_registry, hermes_memory::HermesDB,
    provider::openai_compat::OpenAiCompatProvider, writing_domain_profile, AgentLoop,
    AgentLoopConfig, AgentLoopEvent,
};

mod agent_runtime;
mod ambient_agents;
mod brain_service;
mod chapter_generation;
mod llm_runtime;
mod storage;
mod tool_bridge;
pub mod writer_agent;
use agent_runtime::{AgentObservation, AgentObserveResult, AgentToolDescriptor};
use chapter_generation::{
    ChapterGenerationEvent, GenerateChapterAutonomousPayload, PipelineTerminal, SaveMode,
};
use storage::{ChapterInfo, LoreEntry, OutlineNode};

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
    pub const STORYBOARD_MARKER: &str = "storyboard-marker";
}

#[tauri::command]
fn set_api_key(provider: String, key: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .set_password(&key)
        .map_err(|e| format!("Set error: {}", e))
}

#[tauri::command]
fn check_api_key(provider: String) -> Result<bool, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)
        .map_err(|e| format!("Keyring error: {}", e))?;
    Ok(entry.get_password().is_ok())
}

fn load_api_key_from_keychain() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "openai").ok()?;
    entry.get_password().ok()
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
    Ok(writer_agent::WriterAgentKernel::new(&project_id, memory))
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

use agent_harness_core::truncate_context;

#[tauri::command]
fn harness_echo(message: String) -> String {
    format!("Harness Received: {}", message)
}

#[tauri::command]
fn get_lorebook(app: tauri::AppHandle) -> Result<Vec<LoreEntry>, String> {
    storage::load_lorebook(&app)
}

#[tauri::command]
fn save_lore_entry(
    app: tauri::AppHandle,
    keyword: String,
    content: String,
) -> Result<Vec<LoreEntry>, String> {
    storage::upsert_lore_entry(&app, keyword, content)
}

#[tauri::command]
fn delete_lore_entry(app: tauri::AppHandle, id: String) -> Result<Vec<LoreEntry>, String> {
    storage::remove_lore_entry(&app, id)
}

#[tauri::command]
fn read_project_dir(app: tauri::AppHandle) -> Result<Vec<ChapterInfo>, String> {
    storage::read_project_dir(&app)
}

#[tauri::command]
fn create_chapter(app: tauri::AppHandle, title: String) -> Result<ChapterInfo, String> {
    storage::create_chapter(&app, title)
}

#[tauri::command]
fn save_chapter(app: tauri::AppHandle, title: String, content: String) -> Result<String, String> {
    let revision = storage::save_chapter_content_and_revision(&app, &title, &content)?;
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

#[tauri::command]
fn load_chapter(app: tauri::AppHandle, title: String) -> Result<String, String> {
    storage::load_chapter(&app, title)
}

#[tauri::command]
fn get_chapter_revision(app: tauri::AppHandle, title: String) -> Result<String, String> {
    storage::chapter_revision(&app, &title)
}

#[tauri::command]
fn get_outline(app: tauri::AppHandle) -> Result<Vec<OutlineNode>, String> {
    storage::load_outline(&app)
}

#[tauri::command]
fn save_outline_node(
    app: tauri::AppHandle,
    chapter_title: String,
    summary: String,
) -> Result<Vec<OutlineNode>, String> {
    storage::upsert_outline_node(&app, chapter_title, summary)
}

#[tauri::command]
fn delete_outline_node(
    app: tauri::AppHandle,
    chapter_title: String,
) -> Result<Vec<OutlineNode>, String> {
    storage::remove_outline_node(&app, chapter_title)
}

#[tauri::command]
fn update_outline_status(
    app: tauri::AppHandle,
    chapter_title: String,
    status: String,
) -> Result<Vec<OutlineNode>, String> {
    storage::update_outline_status(&app, chapter_title, status)
}

#[tauri::command]
fn reorder_outline_nodes(
    app: tauri::AppHandle,
    ordered_titles: Vec<String>,
) -> Result<Vec<OutlineNode>, String> {
    storage::reorder_outline_nodes(&app, ordered_titles)
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
            frontend_state: None,
            save_mode: SaveMode::ReplaceIfClean,
            chapter_summary_override: Some(summary),
        };

        let terminal = chapter_generation::run_chapter_generation_pipeline(
            app_clone.clone(),
            settings,
            payload,
            user_profile_entries,
            |event| {
                let _ = app_clone.emit(events::CHAPTER_GENERATION, event);
            },
        )
        .await;

        match terminal {
            PipelineTerminal::Completed {
                generated_content, ..
            } => {
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
        let terminal = chapter_generation::run_chapter_generation_pipeline(
            app_clone.clone(),
            settings,
            payload,
            user_profile_entries,
            |event: ChapterGenerationEvent| {
                let _ = app_clone.emit(events::CHAPTER_GENERATION, event);
            },
        )
        .await;

        if let PipelineTerminal::Completed {
            saved,
            generated_content,
        } = terminal
        {
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
fn get_agent_kernel_status() -> Result<AgentKernelStatus, String> {
    let registry = default_writing_tool_registry();
    let tools = registry.list();
    let domain = writing_domain_profile();

    Ok(AgentKernelStatus {
        tool_generation: registry.generation(),
        tool_count: tools.len(),
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
    storage::restore_file_backup(&app, target, backup_id)
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
    state: tauri::State<'_, AppState>,
    operation: writer_agent::operation::WriterOperation,
    current_revision: String,
) -> Result<writer_agent::operation::OperationResult, String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.approve_editor_operation(operation, &current_revision)
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

fn render_writer_context_pack(pack: &writer_agent::context::WritingContextPack) -> String {
    pack.sources
        .iter()
        .map(|source| {
            format!(
                "## {:?} (priority {}, {} chars{})\n{}",
                source.source,
                source.priority,
                source.char_count,
                if source.truncated { ", truncated" } else { "" },
                source.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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

fn source_refs_from_context_pack(pack: &writer_agent::context::WritingContextPack) -> Vec<String> {
    pack.sources
        .iter()
        .map(|source| format!("{:?}", source.source))
        .collect()
}

fn render_writer_ledger_snapshot(
    snapshot: &writer_agent::kernel::WriterAgentLedgerSnapshot,
) -> String {
    let canon = snapshot
        .canon_entities
        .iter()
        .take(8)
        .map(|entity| {
            format!(
                "- {} [{}]: {} {}",
                entity.name, entity.kind, entity.summary, entity.attributes
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let promises = snapshot
        .open_promises
        .iter()
        .take(8)
        .map(|promise| {
            format!(
                "- {} [{}]: {} -> {}",
                promise.title, promise.kind, promise.description, promise.expected_payoff
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let decisions = snapshot
        .recent_decisions
        .iter()
        .take(8)
        .map(|decision| {
            format!(
                "- {} / {}: {} ({})",
                decision.scope, decision.title, decision.decision, decision.rationale
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    [
        ("Canon entities", canon),
        ("Open promises", promises),
        ("Recent creative decisions", decisions),
    ]
    .into_iter()
    .filter_map(|(label, content)| {
        if content.trim().is_empty() {
            None
        } else {
            Some(format!("## {}\n{}", label, content))
        }
    })
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn char_tail(text: &str, max_chars: usize) -> String {
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

fn render_manual_agent_system_prompt(
    memory_context: &str,
    writer_context: &str,
    ledger_context: &str,
    truncated_context: &str,
    paragraph: &str,
    selected_text: &str,
) -> String {
    format!(
        "你是 Forge 的中文长篇小说写作 Agent，是作家的第二大脑和并肩创作伙伴，不是普通聊天助手，也不是只会补全文字的写作工具。\n\
你的任务是理解作者当前意图，结合项目长期记忆、设定、伏笔、风格偏好和当前稿件，给出可执行、可直接用于创作推进的回答。\n\
如果信息不足，先说明缺口；如果涉及人物、设定、时间线或伏笔，必须优先尊重 WriterAgent ContextPack 与 Ledger，不要随意发明冲突设定。\n\
回答要具体、短而有用；需要写正文时直接给可用正文，需要分析时给明确判断和下一步。\n\n\
{}\n\n\
# WriterAgent ContextPack\n{}\n\n\
# WriterAgent Ledgers\n{}\n\n\
# Current draft tail\n\
\"\"\"\n{}\n\"\"\"\n\n\
# Focused paragraph\n\
\"\"\"\n{}\n\"\"\"\n\n\
# Selected text\n\
\"\"\"\n{}\n\"\"\"\n\n\
可使用工具检索 lorebook、outline 和章节资料；在虚构新信息前先查设定。",
        memory_context,
        writer_context,
        ledger_context,
        truncated_context,
        paragraph,
        selected_text
    )
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
    storage::rename_chapter_file(&app, old_name, new_name)
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
    let api_key = require_api_key()?;
    let api_base =
        std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://openrouter.ai/api/v1".into());
    let model =
        std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "deepseek/deepseek-v4-flash".into());

    let state = app.state::<AppState>();
    let truncated_context = truncate_context(&context, 2000);
    let project_id = storage::active_project_id(&app)?;
    let manual_observation = build_manual_writer_observation(
        &message,
        &context,
        &paragraph,
        &selected_text,
        context_payload.as_ref(),
        &project_id,
    );
    let (writer_context_pack, writer_context, ledger_context, source_refs) = {
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        refresh_kernel_canon_from_lorebook(&app, &mut kernel);
        let proposals = kernel.observe(manual_observation.clone())?;
        for proposal in proposals {
            app.emit(events::AGENT_PROPOSAL, proposal)
                .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
        }

        let writer_context_pack = kernel.context_pack_for(
            writer_agent::context::AgentTask::ManualRequest,
            &manual_observation,
            4_500,
        );
        let writer_context = render_writer_context_pack(&writer_context_pack);
        let ledger_context = render_writer_ledger_snapshot(&kernel.ledger_snapshot());
        let source_refs = source_refs_from_context_pack(&writer_context_pack);
        (
            writer_context_pack,
            writer_context,
            ledger_context,
            source_refs,
        )
    };

    // Check lore / outline availability for intent routing
    let has_lore = storage::load_lorebook(&app)
        .map(|l| !l.is_empty())
        .unwrap_or(false);
    let has_outline = storage::load_outline(&app)
        .map(|o| !o.is_empty())
        .unwrap_or(false);

    // Build context injection from learned memory
    let memory_context = build_context_injection(&app, &message);

    let system_prompt = render_manual_agent_system_prompt(
        &memory_context,
        &writer_context,
        &ledger_context,
        truncated_context,
        &paragraph,
        &selected_text,
    );
    tracing::debug!(
        "Manual WriterAgent context: {} sources, {}/{} chars",
        writer_context_pack.sources.len(),
        writer_context_pack.total_chars,
        writer_context_pack.budget_limit
    );

    // Build provider
    let provider = std::sync::Arc::new(OpenAiCompatProvider::new(&api_base, &api_key, &model));

    // Build tool registry + bridge
    let registry = default_writing_tool_registry();
    let bridge = tool_bridge::TauriToolBridge { app: app.clone() };

    // Build agent loop
    let mut agent = AgentLoop::new(
        AgentLoopConfig {
            max_rounds: 10,
            system_prompt,
        },
        provider,
        registry,
        bridge,
    );

    // Wire events to Tauri frontend
    let app_handle = app.clone();
    agent.set_event_callback(std::sync::Arc::new(move |event| match event {
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

    agent.add_user_message(message.clone());

    // Run the agent loop
    match agent.run(&message, has_lore, has_outline).await {
        Ok(final_text) => {
            // Log assistant response to Hermes memory
            {
                let db = lock_hermes(&state)?;
                let _ = db.log_interaction("assistant", &final_text);
            }
            {
                let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
                kernel.record_manual_exchange(
                    &manual_observation,
                    &message,
                    &final_text,
                    &source_refs,
                );
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
            });
            app.manage(cache_for_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            abort_editor_prediction,
            harness_echo,
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
            get_agent_tools,
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
            export_diagnostic_logs
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
