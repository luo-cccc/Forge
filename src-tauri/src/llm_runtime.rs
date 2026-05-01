use futures_util::StreamExt;
use serde::Deserialize;

#[derive(Clone)]
pub struct LlmSettings {
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub embedding_model: String,
}

pub enum StreamControl {
    Continue,
}

pub fn settings(api_key: String) -> LlmSettings {
    LlmSettings {
        api_key,
        api_base: std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
        model: std::env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "deepseek/deepseek-v4-flash".to_string()),
        embedding_model: std::env::var("OPENAI_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-3-small".to_string()),
    }
}

fn endpoint(api_base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        api_base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

pub fn client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

pub async fn chat_text(
    settings: &LlmSettings,
    messages: Vec<serde_json::Value>,
    json_mode: bool,
    timeout_secs: u64,
) -> Result<String, String> {
    let client = client(timeout_secs)?;
    let mut payload = serde_json::json!({
        "model": settings.model,
        "messages": messages,
        "stream": false
    });

    if json_mode {
        payload["response_format"] = serde_json::json!({"type": "json_object"});
    }

    let resp = client
        .post(endpoint(&settings.api_base, "chat/completions"))
        .header("Authorization", format!("Bearer {}", settings.api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status.as_u16(), text));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse: {}", e))?;
    Ok(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

pub async fn embed(
    settings: &LlmSettings,
    input: &str,
    timeout_secs: u64,
) -> Result<Vec<f32>, String> {
    let client = client(timeout_secs)?;
    let resp = client
        .post(endpoint(&settings.api_base, "embeddings"))
        .header("Authorization", format!("Bearer {}", settings.api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": settings.embedding_model,
            "input": input
        }))
        .send()
        .await
        .map_err(|e| format!("Embed request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Embed API error {}: {}", status.as_u16(), text));
    }

    let body: EmbeddingResponse = resp
        .json()
        .await
        .map_err(|e| format!("Embedding JSON parse: {}", e))?;

    body.data
        .into_iter()
        .next()
        .map(|data| data.embedding)
        .filter(|embedding| !embedding.is_empty())
        .ok_or_else(|| "Missing embedding in response".to_string())
}

pub async fn chat_json(
    settings: &LlmSettings,
    messages: Vec<serde_json::Value>,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    let text = chat_text(settings, messages, true, timeout_secs).await?;
    serde_json::from_str(&text).map_err(|e| format!("Failed to parse JSON response: {}", e))
}

pub async fn stream_chat(
    settings: &LlmSettings,
    messages: Vec<serde_json::Value>,
    timeout_secs: u64,
    mut on_delta: impl FnMut(String) -> Result<StreamControl, String>,
) -> Result<String, String> {
    let client = client(timeout_secs)?;
    let resp = client
        .post(endpoint(&settings.api_base, "chat/completions"))
        .header("Authorization", format!("Bearer {}", settings.api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": settings.model,
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
    let mut full = String::new();

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

            full.push_str(&content);
            on_delta(content)?;
        }
    }

    Ok(full)
}
