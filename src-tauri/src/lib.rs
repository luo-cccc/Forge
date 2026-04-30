use std::sync::Mutex;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

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

    let system_prompt = format!(
        "You are a creative writing assistant helping the user write a novel.\n\
\n\
Current full draft:\n\
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
        context, paragraph, selected_text
    );

    let mut messages: Vec<serde_json::Value> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": message}),
    ];

    let client = reqwest::Client::new();
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
            let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
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
            delete_lore_entry
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
