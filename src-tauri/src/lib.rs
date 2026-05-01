use std::sync::{Mutex, MutexGuard};

use agent_harness_core::{classify_intent, hermes_memory::HermesDB};

mod brain_service;
mod agent_runtime;
mod chapter_generation;
mod llm_runtime;
mod storage;
use agent_runtime::{AgentObserveResult, AgentObservation, AgentToolDescriptor};
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
    pub const AGENT_SEARCH_STATUS: &str = "agent-search-status";
    pub const AGENT_STREAM_CHUNK: &str = "agent-stream-chunk";
    pub const AGENT_STREAM_END: &str = "agent-stream-end";
    pub const BATCH_STATUS: &str = "batch-status";
    pub const CHAPTER_GENERATION: &str = "chapter-generation";
    pub const EDITOR_GHOST_CHUNK: &str = "editor-ghost-chunk";
    pub const EDITOR_GHOST_END: &str = "editor-ghost-end";
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
fn get_api_key(provider: String) -> Result<String, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &provider)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .get_password()
        .map_err(|e| format!("Get error: {}", e))
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
    Thinking,
    Streaming,
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

#[derive(Serialize, Clone)]
struct SearchStatus {
    keyword: String,
    round: u32,
}

use agent_harness_core::actions::extract_search_action as extract_action_search;
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

#[derive(Serialize, Clone)]
struct AgentError {
    message: String,
    source: String,
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
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorGhostEnd {
    request_id: String,
    cursor_position: usize,
    reason: String,
}

fn emit_error(app: &tauri::AppHandle, message: &str, source: &str) {
    let _ = app.emit(
        events::AGENT_ERROR,
        AgentError {
            message: message.to_string(),
            source: source.to_string(),
        },
    );
}

fn realtime_cowrite_enabled() -> bool {
    std::env::var("AGENT_WRITER_REALTIME_COWRITE")
        .map(|value| {
            let normalized = value.trim().to_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "disabled")
        })
        .unwrap_or(true)
}

fn build_fim_messages(prefix: &str, suffix: &str, chapter_title: Option<&str>) -> Vec<serde_json::Value> {
    let title = chapter_title.unwrap_or("current chapter");
    let system_prompt = "You are a low-latency fill-in-the-middle writing engine for a novelist. \
Complete only the text that belongs exactly at the cursor. Return 1-3 short Chinese sentences at most. \
Do not explain, do not quote the prompt, do not wrap the answer in markdown, and stop at a natural boundary.";
    let user_prompt = format!(
        "<|fim_prefix|>\n# {}\n{}\n<|fim_suffix|>\n{}\n<|fim_middle|>",
        title, prefix, suffix
    );

    vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_prompt}),
    ]
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

fn abort_editor_prediction_task(state: &AppState, request_id: Option<&str>) -> Result<bool, String> {
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

#[tauri::command]
fn abort_editor_prediction(
    app: tauri::AppHandle,
    request_id: Option<String>,
) -> Result<bool, String> {
    let state = app.state::<AppState>();
    abort_editor_prediction_task(&state, request_id.as_deref())
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

    let settings = llm_runtime::settings(api_key);
    let mut prefix = payload.prefix.clone();
    prefix.push_str(&paragraph_hint(&payload.paragraph));
    let messages = build_fim_messages(&prefix, &payload.suffix, payload.chapter_title.as_deref());
    let app_clone = app.clone();

    tokio::spawn(async move {
        let mut emitted_chars = 0usize;
        let stream_result = llm_runtime::stream_chat_cancellable(
            &settings,
            messages,
            8,
            cancel.clone(),
            |content| {
                let content = trim_ghost_completion(&content);
                if content.is_empty() {
                    return Ok(llm_runtime::StreamControl::Continue);
                }

                emitted_chars += content.chars().count();
                let _ = app_clone.emit(
                    events::EDITOR_GHOST_CHUNK,
                    EditorGhostChunk {
                        request_id: request_id.clone(),
                        cursor_position,
                        content,
                    },
                );

                if emitted_chars >= 180 {
                    Ok(llm_runtime::StreamControl::Stop)
                } else {
                    Ok(llm_runtime::StreamControl::Continue)
                }
            },
        )
        .await;

        let reason = match stream_result {
            Ok(_) => "complete",
            Err(ref e) if e == "cancelled" || cancel.is_cancelled() => "cancelled",
            Err(e) => {
                tracing::warn!("Ghost completion failed: {}", e);
                "error"
            }
        }
        .to_string();

        let _ = app_clone.emit(
            events::EDITOR_GHOST_END,
            EditorGhostEnd {
                request_id: request_id.clone(),
                cursor_position,
                reason,
            },
        );

        let state = app_clone.state::<AppState>();
        let _ = clear_editor_prediction_task(&state, &request_id);
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
struct CoTEvent {
    step: u32,
    total: u32,
    description: String,
    status: String,
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

    let outline_summary = observation.chapter_title.as_ref().and_then(|chapter_title| {
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
    let source_summaries =
        agent_runtime::build_source_summaries(&observation, outline_summary, lore_hits, profile_count);
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
    let settings = llm_runtime::settings(api_key);

    let state = app.state::<AppState>();

    let truncated_context = truncate_context(&context, 2000);

    // Semantic router: classify intent
    let has_lore = storage::load_lorebook(&app)
        .map(|l| !l.is_empty())
        .unwrap_or(false);
    let has_outline = storage::load_outline(&app)
        .map(|o| !o.is_empty())
        .unwrap_or(false);
    let intent = classify_intent(&message, has_lore, has_outline);

    // Emit CoT: intent detection
    let _ = app.emit(
        events::AGENT_CHAIN_OF_THOUGHT,
        CoTEvent {
            step: 0,
            total: 1,
            description: format!("Intent: {:?}", intent),
            status: "done".to_string(),
        },
    );

    // Log user message to Hermes memory
    {
        let state = app.state::<AppState>();
        let db = lock_hermes(&state)?;
        let _ = db.log_interaction("user", &message);
    }

    // Build context injection from learned memory
    let memory_context = build_context_injection(&app, &message);

    let system_prompt = format!(
        "You are a creative writing assistant helping the user write a novel.\n\
{}\n\
Current draft (last ~2000 chars):\n\
\"\"\"\n\
{}\n\
\"\"\"\n\
\n\
Current paragraph the user is focused on:\n\
\"\"\"\n\
{}\n\
\"\"\"\n\
\n\
Selected text (user wants to rewrite this):\n\
\"\"\"\n\
{}\n\
\"\"\"\n\
\n\
## Rules\n\
1. Respond conversationally to the user's requests about their writing.\n\
2. When you want to write NEW content into the editor, use:\n\
   <ACTION_INSERT>your text here</ACTION_INSERT>\n\
3. When the user provides selected text and asks you to rewrite, polish, or modify it, output ONLY the rewritten version wrapped in:\n\
   <ACTION_REPLACE>rewritten text</ACTION_REPLACE>\n\
   Do NOT include the original text in your response. Do NOT add explanations inside the tags.\n\
4. You may use multiple ACTION_INSERT or ACTION_REPLACE blocks in a single response.\n\
5. Do NOT wrap normal conversation in action tags — only content meant for the editor.\n\
6. Action tags will be intercepted automatically; the user will NOT see them in chat.\n\
7. If you need to know details about a character, location, or world setting that may exist in the lorebook, use:\n\
   <ACTION_SEARCH>keyword</ACTION_SEARCH>\n\
   The system will search the lorebook and return matching entries. Always search before inventing new details about named characters or settings.",
        memory_context, truncated_context, paragraph, selected_text
    );

    let mut messages: Vec<serde_json::Value> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": message}),
    ];

    let max_rounds = 3u32;

    for round in 0..max_rounds {
        {
            let mut s = lock_harness_state(&state)?;
            *s = HarnessState::Thinking;
        }

        {
            let mut s = lock_harness_state(&state)?;
            *s = HarnessState::Streaming;
        }

        let mut raw_buffer = String::new();
        let mut found_search = false;
        let mut search_keyword = String::new();

        let stream_result = llm_runtime::stream_chat(&settings, messages.clone(), 60, |content| {
            raw_buffer.push_str(&content);

            if let Some(keyword) = extract_action_search(&raw_buffer) {
                found_search = true;
                search_keyword = keyword;
                return Ok(llm_runtime::StreamControl::Stop);
            }

            let _ = app.emit(events::AGENT_STREAM_CHUNK, StreamChunk { content });
            Ok(llm_runtime::StreamControl::Continue)
        })
        .await;

        if let Err(e) = stream_result {
            emit_error(&app, &e, "stream");
            {
                let mut s = lock_harness_state(&state)?;
                *s = HarnessState::Idle;
            }
            let _ = app.emit(
                events::AGENT_STREAM_END,
                StreamEnd {
                    reason: "error".to_string(),
                },
            );
            return Err(e);
        }

        if found_search {
            let keyword = search_keyword;
            let _ = app.emit(
                events::AGENT_SEARCH_STATUS,
                SearchStatus {
                    keyword: keyword.clone(),
                    round: round + 1,
                },
            );

            messages.push(serde_json::json!({"role": "assistant", "content": raw_buffer.clone()}));

            let entries = storage::load_lorebook(&app)?;
            let results: Vec<&LoreEntry> = entries
                .iter()
                .filter(|e| {
                    e.keyword.to_lowercase().contains(&keyword.to_lowercase())
                        || keyword.to_lowercase().contains(&e.keyword.to_lowercase())
                })
                .collect();

            let search_result = if results.is_empty() {
                format!("No lorebook entries found for '{}'.", keyword)
            } else {
                results
                    .iter()
                    .map(|e| format!("[{}]: {}", e.keyword, e.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            messages.push(serde_json::json!({"role": "user", "content": format!(
                "SYSTEM SEARCH RESULT for '{}':\n{}\n\nContinue based on this information.",
                keyword, search_result
            )}));
        }

        if !found_search {
            // Natural completion — no search needed
            {
                let mut s = lock_harness_state(&state)?;
                *s = HarnessState::Idle;
            }
            {
                // Log assistant response to Hermes memory
                let state = app.state::<AppState>();
                let db = lock_hermes(&state)?;
                let _ = db.log_interaction("assistant", &raw_buffer);
            }

            // Trigger background skill extraction (Hermes pattern)
            let app_clone = app.clone();
            tokio::spawn(async move { extract_skills_from_recent(&app_clone).await });

            let _ = app.emit(
                events::AGENT_STREAM_END,
                StreamEnd {
                    reason: "complete".to_string(),
                },
            );
            return Ok(());
        }
    }

    // Max rounds exhausted
    {
        let mut s = lock_harness_state(&state)?;
        *s = HarnessState::Idle;
    }
    let _ = app.emit(
        events::AGENT_STREAM_END,
        StreamEnd {
            reason: "max_rounds".to_string(),
        },
    );

    Ok(())
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

    if let Err(e) = tauri::Builder::default()
        .manage(AppState {
            harness_state: Mutex::new(HarnessState::Idle),
            hermes_db: Mutex::new(hermes_db),
            editor_prediction: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            abort_editor_prediction,
            harness_echo,
            report_editor_state,
            ask_agent,
            agent_observe,
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
            get_project_graph_data,
            analyze_pacing,
            rename_chapter_file,
            set_api_key,
            get_api_key,
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
