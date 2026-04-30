mod vector_db;

use std::sync::Mutex;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use vector_db::{chunk_text, Chunk, VectorDB};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
enum HarnessState {
    Idle,
    Thinking,
    Streaming,
}

struct AppState {
    harness_state: Mutex<HarnessState>,
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

fn truncate_context(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        return text;
    }
    let start = text.len().saturating_sub(max_chars);
    // Find nearest char boundary and space
    let slice = &text[start..];
    // Skip to first complete character after a space for cleaner truncation
    if let Some(idx) = slice.find(' ') {
        &slice[idx + 1..]
    } else {
        slice
    }
}

fn extract_action_search(text: &str) -> Option<String> {
    let tag = "<ACTION_SEARCH>";
    let end_tag = "</ACTION_SEARCH>";
    if let Some(start) = text.find(tag) {
        let content_start = start + tag.len();
        if let Some(end) = text[content_start..].find(end_tag) {
            return Some(text[content_start..content_start + end].trim().to_string());
        }
    }
    None
}

#[tauri::command]
fn harness_echo(message: String) -> String {
    format!("Harness Received: {}", message)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoreEntry {
    id: String,
    keyword: String,
    content: String,
}

fn lorebook_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(dir.join("lorebook.json"))
}

fn load_lorebook(app: &tauri::AppHandle) -> Result<Vec<LoreEntry>, String> {
    let path = lorebook_path(app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).unwrap_or_else(|_| Ok(vec![]))
}

fn save_lorebook(app: &tauri::AppHandle, entries: &[LoreEntry]) -> Result<(), String> {
    let path = lorebook_path(app)?;
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_lorebook(app: tauri::AppHandle) -> Result<Vec<LoreEntry>, String> {
    load_lorebook(&app)
}

#[tauri::command]
fn save_lore_entry(
    app: tauri::AppHandle,
    keyword: String,
    content: String,
) -> Result<Vec<LoreEntry>, String> {
    let mut entries = load_lorebook(&app)?;
    if let Some(entry) = entries.iter_mut().find(|e| e.keyword == keyword) {
        entry.content = content;
    } else {
        let id = (entries.len() + 1).to_string();
        entries.push(LoreEntry {
            id,
            keyword,
            content,
        });
    }
    save_lorebook(&app, &entries)?;
    Ok(entries)
}

#[tauri::command]
fn delete_lore_entry(app: tauri::AppHandle, id: String) -> Result<Vec<LoreEntry>, String> {
    let mut entries = load_lorebook(&app)?;
    entries.retain(|e| e.id != id);
    save_lorebook(&app, &entries)?;
    Ok(entries)
}

fn project_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?
        .join("project");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create project dir: {}", e))?;
    Ok(dir)
}

#[derive(Debug, Clone, Serialize)]
struct ChapterInfo {
    title: String,
    filename: String,
}

#[tauri::command]
fn read_project_dir(app: tauri::AppHandle) -> Result<Vec<ChapterInfo>, String> {
    let dir = project_dir(&app)?;
    let mut chapters = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let title = stem.replace('-', " ");
            chapters.push(ChapterInfo {
                filename: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                title,
            });
        }
    }
    chapters.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(chapters)
}

#[tauri::command]
fn create_chapter(app: tauri::AppHandle, title: String) -> Result<ChapterInfo, String> {
    let dir = project_dir(&app)?;
    let filename = format!("{}.md", title.replace(' ', "-").to_lowercase());
    let path = dir.join(&filename);
    if !path.exists() {
        std::fs::write(&path, "").map_err(|e| e.to_string())?;
    }
    Ok(ChapterInfo { title, filename })
}

#[tauri::command]
fn save_chapter(app: tauri::AppHandle, title: String, content: String) -> Result<(), String> {
    let dir = project_dir(&app)?;
    let filename = format!("{}.md", title.replace(' ', "-").to_lowercase());
    let path = dir.join(&filename);
    atomic_write(&path, &content)?;

    // Background auto-embed
    let app_clone = app.clone();
    let title_clone = title.clone();
    let content_clone = content.clone();
    tokio::spawn(async move {
        auto_embed_chapter(&app_clone, &title_clone, &content_clone).await;
    });

    Ok(())
}

#[tauri::command]
fn load_chapter(app: tauri::AppHandle, title: String) -> Result<String, String> {
    let dir = project_dir(&app)?;
    let filename = format!("{}.md", title.replace(' ', "-").to_lowercase());
    let path = dir.join(&filename);
    if !path.exists() {
        return Err(format!("Chapter '{}' not found", title));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutlineNode {
    chapter_title: String,
    summary: String,
    status: String,
}

fn outline_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(dir.join("outline.json"))
}

fn load_outline(app: &tauri::AppHandle) -> Result<Vec<OutlineNode>, String> {
    let path = outline_path(app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).unwrap_or_else(|_| Ok(vec![]))
}

fn brain_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(dir.join("project_brain.json"))
}

fn save_outline(app: &tauri::AppHandle, nodes: &[OutlineNode]) -> Result<(), String> {
    let path = outline_path(app)?;
    let json = serde_json::to_string_pretty(nodes).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_outline(app: tauri::AppHandle) -> Result<Vec<OutlineNode>, String> {
    load_outline(&app)
}

#[tauri::command]
fn save_outline_node(
    app: tauri::AppHandle,
    chapter_title: String,
    summary: String,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(&app)?;
    let status = if let Some(existing) = nodes.iter().find(|n| n.chapter_title == chapter_title) {
        existing.status.clone()
    } else {
        "empty".to_string()
    };
    if let Some(node) = nodes.iter_mut().find(|n| n.chapter_title == chapter_title) {
        node.summary = summary;
    } else {
        nodes.push(OutlineNode {
            chapter_title,
            summary,
            status,
        });
    }
    save_outline(&app, &nodes)?;
    Ok(nodes)
}

#[tauri::command]
fn delete_outline_node(
    app: tauri::AppHandle,
    chapter_title: String,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(&app)?;
    nodes.retain(|n| n.chapter_title != chapter_title);
    save_outline(&app, &nodes)?;
    Ok(nodes)
}

#[tauri::command]
fn update_outline_status(
    app: tauri::AppHandle,
    chapter_title: String,
    status: String,
) -> Result<Vec<OutlineNode>, String> {
    let mut nodes = load_outline(&app)?;
    if let Some(node) = nodes.iter_mut().find(|n| n.chapter_title == chapter_title) {
        node.status = status;
    }
    save_outline(&app, &nodes)?;
    Ok(nodes)
}

#[derive(Serialize, Clone)]
struct BatchStatus {
    chapter_title: String,
    status: String,
    error: String,
}

#[derive(Serialize, Clone)]
struct AgentError {
    message: String,
    source: String,
}

fn emit_error(app: &tauri::AppHandle, message: &str, source: &str) {
    let _ = app.emit(
        "agent-error",
        AgentError {
            message: message.to_string(),
            source: source.to_string(),
        },
    );
}

#[tauri::command]
async fn batch_generate_chapter(
    app: tauri::AppHandle,
    chapter_title: String,
    summary: String,
) -> Result<(), String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not set".to_string())?;
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("OPENAI_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let app_clone = app.clone();
    let title_clone = chapter_title.clone();

    tokio::spawn(async move {
        let _ = app_clone.emit(
            "batch-status",
            BatchStatus {
                chapter_title: title_clone.clone(),
                status: "generating".to_string(),
                error: String::new(),
            },
        );

        // Gather context
        let lorebook = load_lorebook(&app_clone).unwrap_or_default();
        let lore_context = if lorebook.is_empty() {
            String::from("No lorebook entries.")
        } else {
            lorebook
                .iter()
                .map(|e| format!("[{}]: {}", e.keyword, e.content))
                .collect::<Vec<_>>()
                .join("\n")
        };

        // Get previous 2 chapter summaries from outline (sliding window)
        let outline = load_outline(&app_clone).unwrap_or_default();
        let prev_idx = outline
            .iter()
            .position(|n| n.chapter_title == title_clone);
        let prev_summaries: Vec<&OutlineNode> = if let Some(idx) = prev_idx {
            let start = idx.saturating_sub(2);
            outline[start..idx].iter().collect()
        } else {
            // Chapter not in outline yet; take last 2
            let start = outline.len().saturating_sub(2);
            outline[start..].iter().collect()
        };
        let prev_context = if prev_summaries.is_empty() {
            "None (first chapter)".to_string()
        } else {
            prev_summaries
                .iter()
                .map(|n| format!("[{}]: {}", n.chapter_title, n.summary))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let system_prompt = format!(
            "You are a professional novelist. Write a complete chapter based on the beat sheet.\n\n\
             ## Lorebook (world setting)\n{}\n\n\
             ## Previous chapters (last 2)\n{}\n\n\
             ## Current chapter beat\n{}\n\n\
             Write this chapter in full prose. Do NOT use any action tags. \
             Write naturally in Chinese. Output ONLY the chapter content, no meta-commentary.",
            lore_context, prev_context, summary
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");
        let resp = client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": format!(
                        "Write the full chapter for: {}", summary
                    )}
                ],
                "stream": false
            }))
            .send()
            .await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    let body: serde_json::Value = r.json().await.unwrap_or_default();
                    let content = body["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    if !content.is_empty() {
                        // Write to chapter file
                        let _ = crate::save_chapter_internal(&app_clone, &title_clone, &content);

                        // Update outline status
                        let _ = crate::update_outline_status(
                            app_clone.clone(),
                            title_clone.clone(),
                            "generated".to_string(),
                        );
                    }

                    let _ = app_clone.emit(
                        "batch-status",
                        BatchStatus {
                            chapter_title: title_clone,
                            status: "complete".to_string(),
                            error: String::new(),
                        },
                    );
                } else {
                    let _ = app_clone.emit(
                        "batch-status",
                        BatchStatus {
                            chapter_title: title_clone,
                            status: "error".to_string(),
                            error: format!("HTTP {}", r.status().as_u16()),
                        },
                    );
                }
            }
            Err(e) => {
                let _ = app_clone.emit(
                    "batch-status",
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "error".to_string(),
                        error: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(())
}

async fn embed_text(text: &str) -> Result<Vec<f32>, String> {
    let api_key =
        std::env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY not set".to_string())?;
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let resp = client
        .post(format!("{}/embeddings", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "text-embedding-3-small",
            "input": text
        }))
        .send()
        .await
        .map_err(|e| format!("Embed request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Embed API error: {}", resp.status()));
    }

    let body: serde_json::Value =
        resp.json().await.map_err(|e| format!("JSON parse: {}", e))?;
    let embedding: Vec<f32> = body["data"][0]["embedding"]
        .as_array()
        .ok_or("Missing embedding in response")?
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0) as f32)
        .collect();

    Ok(embedding)
}

async fn auto_embed_chapter(app: &tauri::AppHandle, chapter_title: &str, content: &str) {
    let chunks = chunk_text(content, 500);
    if chunks.is_empty() {
        return;
    }

    let path = match brain_path(app) {
        Ok(p) => p,
        Err(_) => return,
    };
    let mut db = VectorDB::load(&path).unwrap_or_else(|_| VectorDB::new());
    db.remove_chapter(chapter_title);

    for (i, chunk_text) in chunks.iter().enumerate() {
        if chunk_text.trim().len() < 20 {
            continue;
        }
        let embedding = match embed_text(chunk_text).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        db.upsert(Chunk {
            id: format!("{}-{}", chapter_title, i),
            chapter: chapter_title.to_string(),
            text: chunk_text.to_string(),
            embedding,
        });
    }

    let _ = db.save(&path);
}

fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("Write tmp failed: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Atomic rename failed: {}", e))
}

fn save_chapter_internal(app: &tauri::AppHandle, title: &str, content: &str) -> Result<(), String> {
    let dir = project_dir(app)?;
    let filename = format!("{}.md", title.replace(' ', "-").to_lowercase());
    let path = dir.join(&filename);
    atomic_write(&path, content)
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
async fn analyze_chapter(_app: tauri::AppHandle, content: String) -> Result<Vec<ReviewItem>, String> {
    let api_key =
        std::env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY not set".to_string())?;
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("OPENAI_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let system_prompt = r#"You are a professional novel editor. Analyze the chapter and output a JSON object with a "reviews" array.

Each review must have:
- "quote": exact text from the chapter (copy verbatim, at least 10 characters)
- "type": one of "logic" | "ooc" | "pacing" | "prose"
- "issue": what the problem is
- "suggestion": how to fix it (in Chinese, specific rewrite suggestion)

Output ONLY the JSON object, no explanation outside. Example:
{"reviews":[{"quote":"他走出了房间","type":"prose","issue":"缺乏画面感","suggestion":"他推开吱呀作响的木门，幽暗的走廊里只有自己的脚步声在回荡。"}]}"#;

    let truncated = truncate_context(&content, 8000);
    let resp = client
        .post(format!("{}/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": format!("Analyze this chapter:\n\n{}", truncated)}
            ],
            "stream": false,
            "response_format": {"type": "json_object"}
        }))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status.as_u16(), text));
    }

    let body: serde_json::Value =
        resp.json().await.map_err(|e| format!("JSON parse: {}", e))?;
    let text = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    let report: ReviewReport =
        serde_json::from_str(text).map_err(|e| format!("Failed to parse review JSON: {}", e))?;

    Ok(report.reviews)
}

#[tauri::command]
async fn ask_project_brain(app: tauri::AppHandle, query: String) -> Result<(), String> {
    let api_key =
        std::env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY not set".to_string())?;
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("OPENAI_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    // 1. Embed the query
    let query_embedding = embed_text(&query).await.map_err(|e| format!("Embed error: {}", e))?;

    // 2. Search vector DB for top 5 chunks
    let brain_path = brain_path(&app)?;
    let db = VectorDB::load(&brain_path).unwrap_or_else(|_| VectorDB::new());
    let results = db.search(&query_embedding, 5);

    // 3. Build context from top chunks
    let context = if results.is_empty() {
        "No relevant chunks found in the book.".to_string()
    } else {
        results
            .iter()
            .enumerate()
            .map(|(i, (score, chunk))| {
                format!(
                    "[Chunk {} · {} · score {:.3}]\n{}",
                    i + 1,
                    chunk.chapter,
                    score,
                    chunk.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    // 4. Stream LLM response
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build client: {}", e))?;

    let messages = vec![
        serde_json::json!({"role": "system", "content": format!(
            "You are an expert on this novel. Answer the user's question using ONLY the provided book excerpts. \
             If the excerpts don't contain relevant information, say so honestly.\n\nBook excerpts:\n{}",
            context
        )}),
        serde_json::json!({"role": "user", "content": query}),
    ];

    let resp = client
        .post(format!("{}/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true
        }))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status.as_u16(), text));
    }

    let mut stream = resp.bytes_stream();
    let mut sse_buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        sse_buffer.push_str(&text);

        while let Some(line_end) = sse_buffer.find('\n') {
            let line = sse_buffer[..line_end].trim().to_string();
            sse_buffer = sse_buffer[line_end + 1..].to_string();
            if line.is_empty() { continue; }
            let data = if let Some(d) = line.strip_prefix("data: ") { d } else { continue };
            if data == "[DONE]" { continue; }
            let parsed: serde_json::Value =
                serde_json::from_str(data).unwrap_or_default();
            let content = parsed["choices"][0]["delta"]["content"]
                .as_str().unwrap_or("").to_string();
            if !content.is_empty() {
                let _ = app.emit("agent-stream-chunk", StreamChunk { content });
            }
        }
    }

    let _ = app.emit("agent-stream-end", StreamEnd { reason: "complete".to_string() });
    Ok(())
}

#[tauri::command]
async fn ask_agent(
    app: tauri::AppHandle,
    message: String,
    context: String,
    paragraph: String,
    selected_text: String,
) -> Result<(), String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not set in .env".to_string())?;
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("OPENAI_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let state = app.state::<AppState>();

    let truncated_context = truncate_context(&context, 2000);

    let system_prompt = format!(
        "You are a creative writing assistant helping the user write a novel.\n\
\n\
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
        truncated_context, paragraph, selected_text
    );

    let mut messages: Vec<serde_json::Value> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": message}),
    ];

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let max_rounds = 3u32;

    for round in 0..max_rounds {
        {
            let mut s = state.harness_state.lock().map_err(|e| e.to_string())?;
            *s = HarnessState::Thinking;
        }

        let resp = client
            .post(format!("{}/chat/completions", api_base))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": true
            }))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status.as_u16(), text));
        }

        {
            let mut s = state.harness_state.lock().map_err(|e| e.to_string())?;
            *s = HarnessState::Streaming;
        }

        let mut stream = resp.bytes_stream();
        let mut raw_buffer = String::new();
        let mut sse_buffer = String::new();
        let mut found_search = false;

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    emit_error(&app, &format!("Stream error: {}", e), "stream");
                    {
                        let mut s = state.harness_state.lock().unwrap_or_else(|_| panic!("lock"));
                        *s = HarnessState::Idle;
                    }
                    let _ = app.emit(
                        "agent-stream-end",
                        StreamEnd {
                            reason: "error".to_string(),
                        },
                    );
                    return Err(format!("Stream error: {}", e));
                }
            };
            let text = String::from_utf8_lossy(&chunk);
            sse_buffer.push_str(&text);

            while let Some(line_end) = sse_buffer.find('\n') {
                let line = sse_buffer[..line_end].trim().to_string();
                sse_buffer = sse_buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                let data = if let Some(d) = line.strip_prefix("data: ") {
                    d
                } else {
                    continue;
                };

                if data == "[DONE]" {
                    continue;
                }

                let parsed: serde_json::Value =
                    serde_json::from_str(data).map_err(|e| format!("JSON parse error: {}", e))?;

                let content = parsed["choices"][0]["delta"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                if content.is_empty() {
                    continue;
                }

                raw_buffer.push_str(&content);

                // Check for complete ACTION_SEARCH tag
                if let Some(keyword) = extract_action_search(&raw_buffer) {
                    found_search = true;

                    let _ = app.emit(
                        "agent-search-status",
                        SearchStatus {
                            keyword: keyword.clone(),
                            round: round + 1,
                        },
                    );

                    // Add assistant response (including the SEARCH tag) to history
                    let clean_response = raw_buffer.clone();
                    messages.push(serde_json::json!({"role": "assistant", "content": clean_response}));

                    // Search lorebook
                    let entries = load_lorebook(&app)?;
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

                    break; // exit stream loop, enter next round
                }

                // Stream content to frontend
                let _ = app.emit("agent-stream-chunk", StreamChunk { content });
            }

            if found_search {
                break; // exit outer while loop
            }
        }

        if !found_search {
            // Natural completion — no search needed
            {
                let mut s = state.harness_state.lock().map_err(|e| e.to_string())?;
                *s = HarnessState::Idle;
            }
            let _ = app.emit(
                "agent-stream-end",
                StreamEnd {
                    reason: "complete".to_string(),
                },
            );
            return Ok(());
        }
    }

    // Max rounds exhausted
    {
        let mut s = state.harness_state.lock().map_err(|e| e.to_string())?;
        *s = HarnessState::Idle;
    }
    let _ = app.emit(
        "agent-stream-end",
        StreamEnd {
            reason: "max_rounds".to_string(),
        },
    );

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();
    tauri::Builder::default()
        .manage(AppState {
            harness_state: Mutex::new(HarnessState::Idle),
        })
        .invoke_handler(tauri::generate_handler![
            harness_echo,
            ask_agent,
            get_lorebook,
            save_lore_entry,
            delete_lore_entry,
            read_project_dir,
            create_chapter,
            save_chapter,
            load_chapter,
            get_outline,
            save_outline_node,
            delete_outline_node,
            update_outline_status,
            batch_generate_chapter,
            analyze_chapter,
            ask_project_brain
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
