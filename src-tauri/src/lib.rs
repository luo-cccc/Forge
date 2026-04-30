use std::sync::Mutex;

use futures_util::StreamExt;
use serde::Serialize;
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

#[tauri::command]
fn harness_echo(message: String) -> String {
    format!("Harness Received: {}", message)
}

#[tauri::command]
async fn ask_agent(app: tauri::AppHandle, message: String) -> Result<(), String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not set in .env".to_string())?;
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("OPENAI_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let state = app.state::<AppState>();
    {
        let mut s = state.harness_state.lock().map_err(|e| e.to_string())?;
        *s = HarnessState::Thinking;
    }

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": message}
        ],
        "stream": true
    });

    let resp = client
        .post(format!("{}/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
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
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let data = if let Some(d) = line.strip_prefix("data: ") {
                d
            } else {
                continue;
            };

            if data == "[DONE]" {
                break;
            }

            let parsed: serde_json::Value =
                serde_json::from_str(data).map_err(|e| format!("JSON parse error: {}", e))?;

            let content = parsed["choices"][0]["delta"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if !content.is_empty() {
                let _ = app.emit(
                    "agent-stream-chunk",
                    StreamChunk {
                        content,
                    },
                );
            }
        }
    }

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

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            harness_state: Mutex::new(HarnessState::Idle),
        })
        .invoke_handler(tauri::generate_handler![harness_echo, ask_agent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
