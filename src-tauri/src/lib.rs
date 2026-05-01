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
fn export_diagnostic_logs() -> Result<String, String> {
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
    paragraph: String,
    chapter_title: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorGhostChunk {
    request_id: String,
    cursor_position: usize,
    content: String,
    intent: Option<String>,
    candidates: Vec<EditorGhostCandidate>,
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

fn extract_keywords_for_ambient(paragraph: &str, app: &tauri::AppHandle) -> Vec<String> {
    let mut keywords = Vec::new();

    if let Ok(entries) = storage::load_lorebook(app) {
        for entry in entries {
            let keyword = entry.keyword.trim();
            if !keyword.is_empty() && paragraph.contains(keyword) {
                keywords.push(keyword.to_string());
            }
            if keywords.len() >= 8 {
                break;
            }
        }
    }

    if keywords.is_empty() {
        keywords.extend(
            agent_harness_core::extract_keywords(paragraph)
                .into_iter()
                .take(4),
        );
    }

    keywords
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
                    cursor_position: position,
                    content,
                    intent: None,
                    candidates: Vec::new(),
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
                    cursor_position: position,
                    content,
                    intent: Some(intent),
                    candidates,
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

    let lore_entries = storage::load_lorebook(app).unwrap_or_default();
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

    let Some(api_key) = resolve_api_key() else {
        return Ok(());
    };

    let request_id = payload.request_id.clone();
    let cursor_position = payload.cursor_position;
    let cancel = CancellationToken::new();

    {
        let state = app.state::<AppState>();
        abort_editor_prediction_task(&state, None)?;
        let mut task = lock_editor_prediction(&state)?;
        *task = Some(EditorPredictionTask {
            request_id: request_id.clone(),
            cancel: cancel.clone(),
        });
    }

    let chapter = payload
        .chapter_title
        .clone()
        .unwrap_or_else(|| "current chapter".to_string());
    let paragraph = if payload.paragraph.trim().is_empty() {
        paragraph_hint(&payload.prefix)
    } else {
        payload.paragraph.clone()
    };

    let Some(bus_state) = app.try_state::<Mutex<agent_harness_core::AmbientEventBus>>() else {
        return Ok(());
    };

    let cache = app
        .state::<std::sync::Arc<tokio::sync::Mutex<ambient_agents::context_fetcher::ContextCache>>>(
        )
        .inner()
        .clone();
    let keywords = extract_keywords_for_ambient(&paragraph, &app);
    if let Ok(mut bus) = bus_state.lock() {
        bus.abort_agent("co-writer");
        bus.spawn_agent(std::sync::Arc::new(
            ambient_agents::co_writer::CoWriterAgent { cache },
        ));
        if !keywords.is_empty() {
            let _ = bus.publish(EditorEvent::KeywordDetected {
                keywords,
                chapter: chapter.clone(),
                paragraph: paragraph.clone(),
            });
        }
        let _ = bus.publish(EditorEvent::IdleTick {
            request_id: Some(request_id),
            idle_ms: 500,
            chapter,
            paragraph,
            prefix: payload.prefix,
            suffix: payload.suffix,
            cursor_position,
        });
    }

    drop(api_key);
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
    if let Ok(outline_nodes) = storage::load_outline(&app) {
        for node in outline_nodes {
            // Count words in chapter file
            let filename = format!("{}.md", node.chapter_title.replace(' ', "-").to_lowercase());
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
fn agent_observe(
    app: tauri::AppHandle,
    observation: AgentObservation,
) -> Result<AgentObserveResult, String> {
    let request_id = format!("agent-{}", agent_runtime::now_ms());
    let now = agent_runtime::now_ms();
    let decision = agent_runtime::attention_policy(&observation, now);

    if !decision.should_suggest {
        return Ok(AgentObserveResult {
            request_id,
            observation_id: observation.id,
            decision: "noop".to_string(),
            reason: decision.reason,
            suggestion_id: None,
        });
    }

    let outline_summary = observation
        .chapter_title
        .as_ref()
        .and_then(|chapter_title| {
            storage::load_outline(&app).ok().and_then(|nodes| {
                nodes
                    .into_iter()
                    .find(|node| &node.chapter_title == chapter_title)
                    .map(|node| node.summary)
                    .filter(|summary| !summary.trim().is_empty())
            })
        });

    let paragraph_lower = observation.current_paragraph.to_lowercase();
    let nearby_lower = observation.nearby_text.to_lowercase();
    let lore_hits = storage::load_lorebook(&app)
        .unwrap_or_default()
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
        observation_id: observation.id,
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
) -> Result<(), String> {
    let api_key = require_api_key()?;
    let api_base =
        std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://openrouter.ai/api/v1".into());
    let model =
        std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "deepseek/deepseek-v4-flash".into());

    let state = app.state::<AppState>();
    let truncated_context = truncate_context(&context, 2000);

    // Check lore / outline availability for intent routing
    let has_lore = storage::load_lorebook(&app)
        .map(|l| !l.is_empty())
        .unwrap_or(false);
    let has_outline = storage::load_outline(&app)
        .map(|o| !o.is_empty())
        .unwrap_or(false);

    // Build context injection from learned memory
    let memory_context = build_context_injection(&app, &message);

    let system_prompt = format!(
        "You are a creative writing assistant helping the user write a novel.\n\
{}\n\
Current draft (last ~2000 chars):\n\
\"\"\"\n{}\n\"\"\"\n\
\n\
Current paragraph the user is focused on:\n\
\"\"\"\n{}\n\"\"\"\n\
\n\
Selected text:\n\
\"\"\"\n{}\n\"\"\"\n\
\n\
Use the available tools to retrieve information about characters, settings, \
and lore before inventing new details. Search the lorebook for named entities.",
        memory_context, truncated_context, paragraph, selected_text
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
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("hermes_memory.db");
    let hermes_db = match HermesDB::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            let message = format!("Failed to open Hermes memory DB: {}", e);
            tracing::error!("{}", message);
            let _ = msgbox::create("Agent-Writer Error", &message, msgbox::IconType::Error);
            return;
        }
    };

    // Shared context cache for ambient agents
    let cache: std::sync::Arc<tokio::sync::Mutex<ambient_agents::context_fetcher::ContextCache>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(
            ambient_agents::context_fetcher::ContextCache::default(),
        ));

    // Capture clones before the setup closure
    let cache1 = cache.clone();

    if let Err(e) = tauri::Builder::default()
        .setup(move |app| {
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
            event_bus.spawn_agent(std::sync::Arc::new(
                ambient_agents::continuity_watcher::ContinuityWatcher { app: ah.clone() },
            ));
            event_bus.spawn_agent(std::sync::Arc::new(
                ambient_agents::pacing_analyst::PacingAnalyst { app: ah.clone() },
            ));

            app.manage(Mutex::new(event_bus));
            Ok(())
        })
        .manage(AppState {
            harness_state: Mutex::new(HarnessState::Idle),
            hermes_db: Mutex::new(hermes_db),
            editor_prediction: Mutex::new(None),
        })
        .manage(cache)
        .invoke_handler(tauri::generate_handler![
            abort_editor_prediction,
            harness_echo,
            report_editor_state,
            report_semantic_lint_state,
            ask_agent,
            agent_observe,
            get_agent_domain_profile,
            get_agent_kernel_status,
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
}
