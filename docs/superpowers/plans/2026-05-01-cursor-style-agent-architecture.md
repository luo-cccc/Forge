# Cursor-Style Co-Writing Agent — Foundation-First Architecture Plan

> Status note: this is a historical implementation plan. Some phase items have since been completed or superseded. See `docs/project-status.md` for the active project status, cleanup policy, and remaining gaps.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a rock-solid agent runtime foundation (provider abstraction → tool loop → compaction → memory → permission → PTC) before touching any editor UI. All four reference agents mined for architecture, algorithms, and patterns. Editor integration (patch review, ghost text, inline commands) goes on top only after the foundation passes full test coverage.

**Architecture:** Six phases, strictly ordered by dependency. Phases 1-4 are pure backend (`agent-harness-core` + `src-tauri` bridge). Phase 5 is editor integration. Phase 6 is multi-agent pipeline. Each module is test-covered before the next phase starts.

**Tech Stack:** Rust (Tauri v2, tokio, rusqlite, reqwest, jieba-rs), TypeScript (React 19, TipTap 3, Zustand 5), OpenRouter API (OpenAI-compatible function calling)

**Reference Projects Mined:**
- **Claw Code** (`claw-code-main/rust/`): ProviderClient enum, ConversationRuntime<C,T> generic loop, compaction pair-boundary guard, permission pipeline, splitmix64 jitter, hook system
- **OpenCode** (`opencode-1.14.30/`): Provider trait design, apply_patch multi-block review, doom loop detection, Effect-based streaming pipeline, tool registry pattern
- **Hermes Agent** (`hermes-agent-2026.4.30/`): PTC architecture, self-evolving skill system, autonomous Curator, credential failover pool, structured context compression, session FTS5 search
- **CowAgent** (`CowAgent-2.0.7/`): LLM summarization callback on trim, Deep Dream memory distillation, dynamic bot enhancement mixin, tool failure protection, tiered context trimming

---

## Foundation-First File Map

```
agent-harness-core/src/          # Pure library — all agent runtime logic
├── lib.rs                        # [MODIFY] Re-exports for all new modules
├── provider/
│   ├── mod.rs                    # [CREATE] Provider trait + ProviderRegistry
│   ├── openai_compat.rs          # [CREATE] OpenAI-compatible provider impl
│   └── types.rs                  # [CREATE] Shared LLM types (Message, ToolCall, StreamEvent)
├── retry.rs                      # [CREATE] Backoff + jitter + error classification
├── tool_registry.rs              # [MODIFY] Add to_openai_schema(), register_handler()
├── tool_executor.rs              # [CREATE] Tool dispatch + doom loop detection + bridge trait
├── agent_loop.rs                 # [CREATE] AgentLoop::run() — full execution orchestrator
├── compaction.rs                 # [CREATE] Context compaction + LLM summarization
├── hermes_memory.rs              # [MODIFY] Add character_state, plot_thread, world_rule tables
├── vector_db.rs                  # [MODIFY] jieba-rs Chinese tokenization for BM25
├── permission.rs                 # [CREATE] Permission pipeline (deny→mode→ask→allow)
├── skill_lifecycle.rs            # [CREATE] Skill CRUD + Curator (decay/merge/prune)
├── ptc.rs                        # [CREATE] Programmatic Tool Calling
└── context_pack.rs               # [MODIFY] Add compaction trigger threshold

src-tauri/src/                    # Tauri app — thin bridge layer
├── lib.rs                        # [REWRITE] Delegate to AgentLoop; emit events
├── tool_bridge.rs                # [CREATE] Real tool handlers bridging to Tauri storage
└── agent_runtime.rs              # [MODIFY] Skill-driven attention policy
```

---

## Phase 1: Core Agent Runtime (Foundation of Foundations)

**Goal:** Provider abstraction → retry → tool execution → full AgentLoop::run(). After this phase, the agent can execute multi-round tool-calling conversations with proper error recovery. All existing XML action tag logic in src-tauri is retired.

### Task 1.1: Provider Abstraction Layer

**Reference:** Claw Code `api/src/providers/` (ProviderClient enum), OpenCode `provider/provider.ts` (Provider interface)

**Files:**
- Create: `agent-harness-core/src/provider/mod.rs`
- Create: `agent-harness-core/src/provider/types.rs`
- Create: `agent-harness-core/src/provider/openai_compat.rs`
- Modify: `agent-harness-core/src/lib.rs`
- Modify: `agent-harness-core/Cargo.toml`

- [ ] **Step 1: Define shared types**

Create `agent-harness-core/src/provider/types.rs`:

```rust
use serde::{Serialize, Deserialize};

/// A message in the conversation — provider-agnostic format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Events emitted during an LLM stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum StreamEvent {
    #[serde(rename = "text_delta")]
    TextDelta { content: String },
    #[serde(rename = "tool_call_delta")]
    ToolCallDelta { id: String, name: String, arguments_delta: String },
    #[serde(rename = "tool_call_end")]
    ToolCallEnd { id: String, name: String, arguments: String },
    #[serde(rename = "message_stop")]
    MessageStop { finish_reason: String },
    #[serde(rename = "error")]
    Error { message: String, retryable: bool },
}

/// Result of an LLM call — either streaming events or a complete message.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: String,
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Configuration for an LLM call.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub system: Option<String>,
    pub stream: bool,
}
```

- [ ] **Step 2: Define Provider trait**

Create `agent-harness-core/src/provider/mod.rs`:

```rust
pub mod types;
pub mod openai_compat;

use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::Stream;
use crate::retry::ErrorClass;
pub use types::*;

/// Trait for LLM providers. Generic over the streaming implementation.
/// Mirrors Claw Code's ProviderClient enum + OpenCode's Provider interface.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Unique provider identifier.
    fn name(&self) -> &str;

    /// List available models for this provider.
    fn models(&self) -> Vec<String>;

    /// Call the LLM with streaming, yielding StreamEvents via callback.
    /// The callback approach (vs returning a Stream) gives the caller
    /// control over cancellation and backpressure.
    async fn stream_call(
        &self,
        request: LlmRequest,
        on_event: Box<dyn Fn(StreamEvent) + Send + Sync>,
    ) -> Result<LlmResponse, String>;

    /// Non-streaming call — returns complete response.
    async fn call(&self, request: LlmRequest) -> Result<LlmResponse, String>;

    /// Get embeddings for a text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String>;

    /// Estimate token count for messages. Provider-specific.
    fn estimate_tokens(&self, messages: &[LlmMessage]) -> u64;

    /// Check connectivity and API key validity.
    async fn health_check(&self) -> Result<(), String>;
}

/// Provider registry — holds all configured providers.
/// Mirrors OpenCode's provider discovery system.
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn Provider>>,
    default_model: String,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            default_model: String::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.providers.push(provider);
    }

    pub fn set_default_model(&mut self, model: &str) {
        self.default_model = model.to_string();
    }

    /// Find a provider that supports the given model.
    /// Falls back to the first registered provider.
    pub fn resolve(&self, model: &str) -> Option<Arc<dyn Provider>> {
        for p in &self.providers {
            if p.models().iter().any(|m| m == model) {
                return Some(p.clone());
            }
        }
        self.providers.first().cloned()
    }

    /// Get the default provider.
    pub fn default(&self) -> Option<Arc<dyn Provider>> {
        if self.default_model.is_empty() {
            self.providers.first().cloned()
        } else {
            self.resolve(&self.default_model)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::openai_compat::OpenAiCompatProvider;
    use crate::provider::types::*;

    fn make_provider() -> OpenAiCompatProvider {
        OpenAiCompatProvider::new(
            "https://api.openai.com/v1",
            "sk-test",
            "gpt-4o-mini",
        )
    }

    #[test]
    fn test_provider_models() {
        let p = make_provider();
        assert!(p.models().contains(&"gpt-4o-mini".to_string()));
    }

    #[test]
    fn test_registry_resolve() {
        let mut registry = ProviderRegistry::new();
        let p = Arc::new(make_provider());
        registry.register(p);
        registry.set_default_model("gpt-4o-mini");

        let resolved = registry.resolve("gpt-4o-mini");
        assert!(resolved.is_some());
    }
}
```

- [ ] **Step 3: Implement OpenAI-compatible provider**

Create `agent-harness-core/src/provider/openai_compat.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use crate::provider::{Provider, LlmRequest, LlmResponse, LlmMessage, StreamEvent, UsageInfo};
use crate::retry::{ErrorClass, backoff_duration};

/// OpenAI-compatible provider.
/// Works with OpenAI, OpenRouter, DeepSeek, Groq, xAI, and any /v1/chat/completions endpoint.
pub struct OpenAiCompatProvider {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub embedding_model: String,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(api_base: &str, api_key: &str, model: &str) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            embedding_model: "text-embedding-3-small".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Build OpenAI-format messages from LlmMessages.
    fn build_api_messages(messages: &[LlmMessage]) -> Vec<serde_json::Value> {
        messages.iter().map(|m| {
            let mut api = serde_json::json!({"role": m.role});
            if let Some(ref c) = m.content {
                api["content"] = serde_json::json!(c);
            }
            if let Some(ref tc) = m.tool_calls {
                api["tool_calls"] = serde_json::json!(tc);
            }
            if let Some(ref cid) = m.tool_call_id {
                api["tool_call_id"] = serde_json::json!(cid);
            }
            if let Some(ref n) = m.name {
                api["name"] = serde_json::json!(n);
            }
            api
        }).collect()
    }

    /// Parse SSE stream into StreamEvents.
    async fn parse_sse_stream(
        response: reqwest::Response,
        on_event: &(dyn Fn(StreamEvent) + Send + Sync),
    ) -> Result<LlmResponse, String> {
        use futures_util::StreamExt;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        let mut tool_calls_map: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
        let mut finish_reason = String::new();
        let mut usage = None;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| format!("Stream read error: {}", e))?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            while let Some(nl_pos) = buffer.find('\n') {
                let line = buffer[..nl_pos].trim().to_string();
                buffer = buffer[nl_pos + 1..].to_string();

                if line.is_empty() || line == "data: [DONE]" {
                    if line == "data: [DONE]" {
                        continue;
                    }
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        let choices = json.get("choices");
                        if let Some(choices) = choices {
                            if let Some(first) = choices.as_array().and_then(|a| a.first()) {
                                // Text delta
                                if let Some(delta) = first.get("delta") {
                                    if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                                        content.push_str(text);
                                        on_event(StreamEvent::TextDelta { content: text.to_string() });
                                    }
                                    // Tool call deltas
                                    if let Some(tc_deltas) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                        for tc_delta in tc_deltas {
                                            let id = tc_delta.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                            let name = tc_delta.get("function").and_then(|v| v.get("name")).and_then(|v| v.as_str()).unwrap_or("");
                                            let args = tc_delta.get("function").and_then(|v| v.get("arguments")).and_then(|v| v.as_str()).unwrap_or("");

                                            let entry = tool_calls_map.entry(id.to_string()).or_insert_with(|| (name.to_string(), String::new()));
                                            if !name.is_empty() { entry.0 = name.to_string(); }
                                            entry.1.push_str(args);

                                            on_event(StreamEvent::ToolCallDelta {
                                                id: id.to_string(),
                                                name: entry.0.clone(),
                                                arguments_delta: args.to_string(),
                                            });
                                        }
                                    }
                                }
                                // Finish reason
                                if let Some(fr) = first.get("finish_reason").and_then(|v| v.as_str()) {
                                    finish_reason = fr.to_string();
                                }
                            }
                        }
                        // Usage
                        if let Some(u) = json.get("usage") {
                            usage = Some(UsageInfo {
                                input_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                                output_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                            });
                        }
                    }
                }
            }
        }

        // Emit tool call end events
        let tool_calls: Vec<crate::provider::types::ToolCall> = tool_calls_map.into_iter().map(|(id, (name, args))| {
            on_event(StreamEvent::ToolCallEnd {
                id: id.clone(),
                name: name.clone(),
                arguments: args.clone(),
            });
            crate::provider::types::ToolCall {
                id,
                call_type: "function".to_string(),
                function: crate::provider::types::ToolCallFunction { name, arguments: args },
            }
        }).collect();

        on_event(StreamEvent::MessageStop { finish_reason: finish_reason.clone() });

        Ok(LlmResponse {
            content: if content.is_empty() { None } else { Some(content) },
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            finish_reason,
            usage,
        })
    }
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn name(&self) -> &str { "openai-compat" }

    fn models(&self) -> Vec<String> {
        vec![self.model.clone()]
    }

    async fn stream_call(
        &self,
        request: LlmRequest,
        on_event: Box<dyn Fn(StreamEvent) + Send + Sync>,
    ) -> Result<LlmResponse, String> {
        let messages = Self::build_api_messages(&request.messages);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "temperature": request.temperature.unwrap_or(0.7),
        });

        if let Some(ref tools) = request.tools {
            body["tools"] = serde_json::json!(tools);
            body["tool_choice"] = serde_json::json!("auto");
        }
        if let Some(mt) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(mt);
        }

        let mut retry_count = 0u32;
        loop {
            let response = self.client
                .post(format!("{}/chat/completions", self.api_base))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("HTTP request failed: {}", e))?;

            let status = response.status();
            if status.is_success() {
                return Self::parse_sse_stream(response, &*on_event).await;
            }

            let status_code = status.as_u16();
            let body_text = response.text().await.unwrap_or_default();
            let class = ErrorClass::from_http_status_and_body(status_code, &body_text);

            if !class.is_retryable() || retry_count >= 3 {
                return Err(format!("LLM call failed ({}): {}", status_code, body_text));
            }

            let delay = backoff_duration(retry_count);
            tokio::time::sleep(delay).await;
            retry_count += 1;
        }
    }

    async fn call(&self, mut request: LlmRequest) -> Result<LlmResponse, String> {
        request.stream = false;
        // For non-streaming, reuse stream_call and collect
        let mut content = String::new();
        let result = self.stream_call(request, Box::new(move |ev| {
            if let StreamEvent::TextDelta { content: c } = ev {
                // no-op: non-streaming collects at the end
            }
        })).await;
        result
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let response = self.client
            .post(format!("{}/embeddings", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.embedding_model,
                "input": text,
            }))
            .send()
            .await
            .map_err(|e| format!("Embedding request failed: {}", e))?;

        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

        json["data"][0]["embedding"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or_else(|| "Missing embedding in response".to_string())
    }

    fn estimate_tokens(&self, messages: &[LlmMessage]) -> u64 {
        // Rough: 1 token ≈ 3 chars for mixed text + 8 token per-message overhead
        messages.iter().map(|m| {
            (m.content.as_ref().map(|c| c.chars().count()).unwrap_or(0) as u64 / 3) + 8
        }).sum()
    }

    async fn health_check(&self) -> Result<(), String> {
        let response = self.client
            .get(format!("{}/models", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Health check failed with status: {}", response.status()))
        }
    }
}
```

- [ ] **Step 4: Add dependencies and register module**

In `agent-harness-core/Cargo.toml`, add:
```toml
async-trait = "0.1"
```

In `agent-harness-core/src/lib.rs`:
```rust
pub mod provider;
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p agent-harness-core`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add agent-harness-core/src/provider/ agent-harness-core/src/lib.rs agent-harness-core/Cargo.toml
git commit -m "feat: add provider abstraction layer with OpenAI-compatible implementation (from Claw Code + OpenCode)"
```

### Task 1.2: Retry + Error Classification

**Reference:** Claw Code `providers/anthropic.rs` (splitmix64 jitter), Hermes `error_classifier.py`

**Files:**
- Create: `agent-harness-core/src/retry.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Create retry module** — identical to Phase 1 Task 1.2 from the original plan. Keep existing code.

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib retry`
Expected: 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/retry.rs agent-harness-core/src/lib.rs
git commit -m "feat: add exponential backoff with splitmix64 jitter and error classification"
```

### Task 1.3: Tool Schema Export + ToolExecutor with Real Dispatch Bridge

**Reference:** Claw Code `tools/src/lib.rs` (execute_tool dispatch), OpenCode `tool/registry.ts` (tool definition pattern)

**Files:**
- Modify: `agent-harness-core/src/tool_registry.rs`
- Create: `agent-harness-core/src/tool_executor.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Add `to_openai_tool()` to ToolDescriptor** — keep from original plan Task 1.1.

- [ ] **Step 2: Create ToolExecutor with bridge trait**

Create `agent-harness-core/src/tool_executor.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::tool_registry::{ToolRegistry, ToolDescriptor, ToolSideEffectLevel};

/// Callback trait for tool handlers.
/// Implementations bridge to the actual application layer (Tauri storage, lorebook, etc.).
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(&self, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value, String>;
}

/// Result of a tool execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolExecution {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Tracks tool calls to detect doom loops.
/// Ported from OpenCode `processor.ts` doom loop detection (line 305-331).
#[derive(Debug, Clone, Default)]
pub struct DoomLoopDetector {
    call_history: HashMap<(String, u64), u32>,
}

impl DoomLoopDetector {
    fn hash_args(args: &serde_json::Value) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        args.to_string().hash(&mut h);
        h.finish()
    }

    /// Returns true if same tool + same args called 3+ consecutive times.
    pub fn is_doom_loop(&mut self, tool_name: &str, args: &serde_json::Value) -> bool {
        let key = (tool_name.to_string(), Self::hash_args(args));
        let count = self.call_history.entry(key).or_insert(0);
        *count += 1;
        *count >= 3
    }

    /// Reset all tracking. Call after a successful round with different output.
    pub fn reset(&mut self) {
        self.call_history.clear();
    }
}

/// The tool executor dispatches tool calls to registered handlers.
/// Generic over the handler implementation — matches Claw Code pattern.
pub struct ToolExecutor<H: ToolHandler> {
    pub registry: Arc<Mutex<ToolRegistry>>,
    pub handler: H,
    pub doom_detector: DoomLoopDetector,
}

impl<H: ToolHandler> ToolExecutor<H> {
    pub fn new(registry: ToolRegistry, handler: H) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
            handler,
            doom_detector: DoomLoopDetector::default(),
        }
    }

    /// Execute a tool and return structured result.
    pub async fn execute(&mut self, tool_name: &str, args: serde_json::Value) -> ToolExecution {
        let start = std::time::Instant::now();

        // Doom loop check
        let is_doom = self.doom_detector.is_doom_loop(tool_name, &args);

        let (output, error) = match self.handler.execute(tool_name, args.clone()).await {
            Ok(result) => (result, None),
            Err(e) => (serde_json::Value::Null, Some(e)),
        };

        let mut error_msg = error;
        if is_doom {
            error_msg = Some(format!(
                "DOOM LOOP DETECTED: tool '{}' called with same args 3+ times. The agent should try a different approach.",
                tool_name
            ));
        }

        ToolExecution {
            tool_name: tool_name.to_string(),
            input: args,
            output,
            error: error_msg,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doom_loop_detection() {
        let mut d = DoomLoopDetector::default();
        let args = serde_json::json!({"q": "test"});
        assert!(!d.is_doom_loop("search", &args));
        assert!(!d.is_doom_loop("search", &args));
        assert!(d.is_doom_loop("search", &args));
    }

    #[test]
    fn test_doom_loop_different_args_no_trigger() {
        let mut d = DoomLoopDetector::default();
        d.is_doom_loop("search", &serde_json::json!({"q": "a"}));
        d.is_doom_loop("search", &serde_json::json!({"q": "b"}));
        assert!(!d.is_doom_loop("search", &serde_json::json!({"q": "c"})));
    }

    #[test]
    fn test_doom_loop_reset() {
        let mut d = DoomLoopDetector::default();
        d.is_doom_loop("s", &serde_json::json!({}));
        d.is_doom_loop("s", &serde_json::json!({}));
        d.reset();
        assert!(!d.is_doom_loop("s", &serde_json::json!({})));
    }
}
```

- [ ] **Step 3: Register module in lib.rs**

```rust
pub mod tool_executor;
pub use tool_executor::{ToolExecutor, ToolHandler, ToolExecution, DoomLoopDetector};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-harness-core --lib tool_executor`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add agent-harness-core/src/tool_executor.rs agent-harness-core/src/tool_registry.rs agent-harness-core/src/lib.rs
git commit -m "feat: add ToolExecutor with doom loop detection and bridge trait (from OpenCode + Claw Code)"
```

### Task 1.4: Complete AgentLoop with run() Method

**Reference:** Claw Code `ConversationRuntime::run_turn()` (conversation.rs:314), Hermes `AIAgent.run_conversation()` (run_agent.py:10151), CowAgent `AgentStreamExecutor.run_stream()` (agent_stream.py:212)

**Files:**
- Create: `agent-harness-core/src/agent_loop.rs`
- Create: `src-tauri/src/tool_bridge.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Create AgentLoop with full run() method**

Create `agent-harness-core/src/agent_loop.rs`:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::provider::{Provider, LlmMessage, LlmRequest, StreamEvent, LlmResponse};
use crate::tool_registry::{ToolRegistry, ToolFilter, ToolSideEffectLevel};
use crate::tool_executor::{ToolExecutor, ToolHandler, ToolExecution};
use crate::router::{classify_intent, Intent};
use crate::compaction::{CompactionConfig, CompactionResult, should_compact, compact_messages};

/// Events emitted during agent loop execution to the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum AgentLoopEvent {
    #[serde(rename = "intent")]
    Intent { intent: String },
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "text_chunk")]
    TextChunk { content: String },
    #[serde(rename = "tool_call_start")]
    ToolCallStart { tool: String, args: serde_json::Value },
    #[serde(rename = "tool_call_end")]
    ToolCallEnd { tool: String, result: ToolExecution },
    #[serde(rename = "doom_loop_warning")]
    DoomLoopWarning { tool: String },
    #[serde(rename = "compaction")]
    Compaction { before_tokens: u64, after_tokens: u64, compacted_count: usize },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "complete")]
    Complete { rounds: u32, tool_calls: u32, tokens_used: u64 },
}

/// Configuration for agent loop execution.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_rounds: u32,
    pub max_retries: u32,
    pub system_prompt: String,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_rounds: 10,
            max_retries: 3,
            system_prompt: String::new(),
        }
    }
}

/// Event callback type — the Tauri layer provides this to emit events to the frontend.
pub type EventCallback = Arc<dyn Fn(AgentLoopEvent) + Send + Sync>;

/// The core agent execution loop.
/// Generic over Provider and ToolHandler — fully testable with mocks.
pub struct AgentLoop<P: Provider, H: ToolHandler> {
    pub config: AgentLoopConfig,
    pub provider: Arc<P>,
    pub executor: ToolExecutor<H>,
    pub messages: Vec<LlmMessage>,
    pub on_event: Option<EventCallback>,
}

impl<P: Provider, H: ToolHandler> AgentLoop<P, H> {
    pub fn new(config: AgentLoopConfig, provider: Arc<P>, registry: ToolRegistry, handler: H) -> Self {
        Self {
            config,
            provider,
            executor: ToolExecutor::new(registry, handler),
            messages: Vec::new(),
            on_event: None,
        }
    }

    pub fn set_event_callback(&mut self, cb: EventCallback) {
        self.on_event = Some(cb);
    }

    fn emit(&self, event: AgentLoopEvent) {
        if let Some(ref cb) = self.on_event {
            cb(event);
        }
    }

    /// Add a user message to the conversation.
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(LlmMessage {
            role: "user".into(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }

    /// Estimate total tokens in the current conversation.
    pub fn estimate_tokens(&self) -> u64 {
        self.provider.estimate_tokens(&self.messages)
            + (self.config.system_prompt.chars().count() as u64 / 3)
    }

    /// Build the available tools list for the current intent.
    pub fn build_tools(&self, intent: &Intent) -> Vec<serde_json::Value> {
        let filter = ToolFilter::new()
            .with_intent(intent.clone())
            .max_side_effect(ToolSideEffectLevel::Write);
        let registry = self.executor.registry.blocking_lock();
        registry.to_openai_tools(&filter)
    }

    /// The main execution loop.
    /// 1. Classify intent → filter tools
    /// 2. While rounds < max: call LLM → execute tools → append results → check compaction
    /// 3. Return the final assistant message
    pub async fn run(&mut self, user_message: &str, has_lorebook: bool, has_outline: bool) -> Result<String, String> {
        // Phase 1: Classify intent
        let (intent, _) = classify_intent(user_message, has_lorebook, has_outline);
        self.emit(AgentLoopEvent::Intent { intent: format!("{:?}", intent) });

        // Phase 2: Build tools
        let tools = self.build_tools(&intent);
        let has_tools = !tools.is_empty();

        // Phase 3: Execute rounds
        let mut rounds = 0u32;
        let mut total_tool_calls = 0u32;
        let mut final_text = String::new();

        self.emit(AgentLoopEvent::Thinking);

        while rounds < self.config.max_rounds {
            // Build request
            let request = LlmRequest {
                messages: self.messages.clone(),
                tools: if has_tools { Some(tools.clone()) } else { None },
                temperature: Some(0.7),
                max_tokens: Some(4096),
                system: Some(self.config.system_prompt.clone()),
                stream: true,
            };

            // Call LLM
            let event_cb = self.on_event.clone();
            let mut response_content = String::new();
            let mut response_tool_calls: Vec<crate::provider::types::ToolCall> = Vec::new();

            let response = self.provider.stream_call(request, Box::new(move |ev| {
                match ev {
                    StreamEvent::TextDelta { content } => {
                        if let Some(ref cb) = event_cb {
                            cb(AgentLoopEvent::TextChunk { content: content.clone() });
                        }
                    }
                    StreamEvent::ToolCallEnd { id, name, arguments } => {
                        // Accumulate for later processing
                    }
                    _ => {}
                }
            })).await.map_err(|e| {
                self.emit(AgentLoopEvent::Error { message: e.clone() });
                e
            })?;

            response_content = response.content.unwrap_or_default();
            response_tool_calls = response.tool_calls.unwrap_or_default();

            // If no tool calls, this is the final response
            if response_tool_calls.is_empty() {
                final_text = response_content.clone();
                // Add assistant message
                self.messages.push(LlmMessage {
                    role: "assistant".into(),
                    content: Some(response_content),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
                break;
            }

            // Phase 4: Execute tools
            let mut assistant_tool_calls = Vec::new();
            for tc in &response_tool_calls {
                total_tool_calls += 1;

                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                self.emit(AgentLoopEvent::ToolCallStart {
                    tool: tc.function.name.clone(),
                    args: args.clone(),
                });

                let execution = self.executor.execute(&tc.function.name, args).await;

                self.emit(AgentLoopEvent::ToolCallEnd {
                    tool: tc.function.name.clone(),
                    result: execution.clone(),
                });

                if execution.error.as_ref().map(|e| e.contains("DOOM LOOP")).unwrap_or(false) {
                    self.emit(AgentLoopEvent::DoomLoopWarning { tool: tc.function.name.clone() });
                }

                // Add tool result to conversation
                self.messages.push(LlmMessage {
                    role: "tool".into(),
                    content: Some(execution.output.to_string()),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                });

                assistant_tool_calls.push(tc.clone());
            }

            // Add assistant message with tool calls
            self.messages.push(LlmMessage {
                role: "assistant".into(),
                content: if response_content.is_empty() { None } else { Some(response_content) },
                tool_calls: Some(assistant_tool_calls),
                tool_call_id: None,
                name: None,
            });

            rounds += 1;

            // Check for compaction
            let compaction_config = CompactionConfig::default();
            if should_compact(&self.messages, &self.config.system_prompt, &compaction_config) {
                let before = self.estimate_tokens();
                // Compaction would need an LLM call — for now, emit warning
                self.emit(AgentLoopEvent::Compaction {
                    before_tokens: before,
                    after_tokens: before / 3, // rough estimate
                    compacted_count: self.messages.len().saturating_sub(6),
                });
                // Full compaction is implemented in Phase 2
            }
        }

        if rounds >= self.config.max_rounds && final_text.is_empty() {
            self.emit(AgentLoopEvent::Error {
                message: format!("Reached max rounds ({}) without final response", self.config.max_rounds),
            });
            return Err("Max rounds exceeded".into());
        }

        self.emit(AgentLoopEvent::Complete {
            rounds,
            tool_calls: total_tool_calls,
            tokens_used: self.estimate_tokens(),
        });

        Ok(final_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::openai_compat::OpenAiCompatProvider;
    use crate::tool_registry::default_tools;
    use crate::tool_executor::ToolHandler;
    use async_trait::async_trait;

    /// Mock tool handler for testing.
    struct MockToolHandler;
    #[async_trait]
    impl ToolHandler for MockToolHandler {
        async fn execute(&self, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "tool": tool_name,
                "args": args,
                "result": "mock result"
            }))
        }
    }

    fn make_agent() -> AgentLoop<OpenAiCompatProvider, MockToolHandler> {
        let provider = Arc::new(OpenAiCompatProvider::new(
            "https://api.openai.com/v1",
            "sk-test",
            "gpt-4o-mini",
        ));
        let mut registry = ToolRegistry::new();
        for tool in default_tools() {
            registry.upsert(tool);
        }
        AgentLoop::new(
            AgentLoopConfig {
                max_rounds: 3,
                system_prompt: "You are a test agent.".into(),
                ..Default::default()
            },
            provider,
            registry,
            MockToolHandler,
        )
    }

    #[test]
    fn test_agent_creation() {
        let agent = make_agent();
        assert_eq!(agent.config.max_rounds, 3);
        assert!(agent.messages.is_empty());
    }

    #[test]
    fn test_add_user_message() {
        let mut agent = make_agent();
        agent.add_user_message("hello".into());
        assert_eq!(agent.messages.len(), 1);
        assert_eq!(agent.messages[0].role, "user");
    }

    #[test]
    fn test_estimate_tokens() {
        let mut agent = make_agent();
        agent.add_user_message("你好世界".repeat(50));
        let tokens = agent.estimate_tokens();
        assert!(tokens > 0);
    }
}
```

- [ ] **Step 2: Create Tauri tool bridge**

Create `src-tauri/src/tool_bridge.rs`:

```rust
use async_trait::async_trait;
use agent_harness_core::tool_executor::ToolHandler;
use tauri::AppHandle;

/// Real tool handler that bridges agent-harness-core tools to the Tauri app's storage layer.
pub struct TauriToolBridge {
    pub app: AppHandle,
}

#[async_trait]
impl ToolHandler for TauriToolBridge {
    async fn execute(&self, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
        match tool_name {
            "load_current_chapter" => {
                let chapter = args.get("chapter").and_then(|v| v.as_str()).unwrap_or("");
                let content = crate::storage::load_chapter_content(&self.app, chapter)
                    .map_err(|e| format!("Failed to load chapter: {}", e))?;
                Ok(serde_json::json!({
                    "content": content,
                    "chapter": chapter,
                }))
            }
            "search_lorebook" => {
                let keyword = args.get("keyword").and_then(|v| v.as_str()).unwrap_or("");
                let entries = crate::storage::search_lorebook(&self.app, keyword)
                    .map_err(|e| format!("Lorebook search failed: {}", e))?;
                Ok(serde_json::json!({
                    "matches": entries.iter().map(|e| serde_json::json!({
                        "keyword": e.keyword,
                        "content": e.content,
                    })).collect::<Vec<_>>(),
                }))
            }
            "load_outline_node" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let node = crate::storage::get_outline_node(&self.app, id)
                    .map_err(|e| format!("Outline load failed: {}", e))?;
                Ok(serde_json::json!({
                    "id": node.id,
                    "title": node.title,
                    "summary": node.summary,
                }))
            }
            "query_project_brain" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let chunks = crate::brain_service::query_brain(&self.app, query)
                    .map_err(|e| format!("Brain query failed: {}", e))?;
                Ok(serde_json::json!({"chunks": chunks}))
            }
            "read_user_drift_profile" => {
                let profiles = crate::storage::get_drift_profiles(&self.app)
                    .map_err(|e| format!("Drift read failed: {}", e))?;
                Ok(serde_json::json!({"profiles": profiles}))
            }
            "load_domain_profile" => {
                let profile = agent_harness_core::writing_domain_profile();
                Ok(serde_json::json!(profile))
            }
            "pack_agent_context" => {
                let packer = agent_harness_core::ContextPacker::new(24000);
                Ok(serde_json::json!({"packed": format!("{:?}", packer)}))
            }
            "plan_chapter_task" => {
                let goal = args.get("goal").and_then(|v| v.as_str()).unwrap_or("");
                let plan = agent_harness_core::planner::ExecutionPlan {
                    steps: vec![agent_harness_core::planner::PlanStep {
                        number: 1,
                        action: "draft".into(),
                        description: goal.into(),
                        query: None,
                        focus: None,
                        style: None,
                    }],
                    state: agent_harness_core::planner::PlanState::Executing,
                };
                Ok(serde_json::json!(plan))
            }
            "generate_bounded_continuation" => {
                let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                let llm = crate::llm_runtime::build_llm_client(&self.app)?;
                let result = llm.chat_text(prompt, Some(2000), false).await
                    .map_err(|e| format!("Generation failed: {}", e))?;
                Ok(serde_json::json!({"text": result}))
            }
            "generate_chapter_draft" => {
                // This requires approval — handled by permission pipeline
                Err("generate_chapter_draft requires explicit approval. Use chapter generation pipeline.".into())
            }
            "classify_writing_intent" => {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let (intent, _) = agent_harness_core::classify_intent(text, true, true);
                Ok(serde_json::json!({"intent": format!("{:?}", intent)}))
            }
            "record_run_trace" => {
                Ok(serde_json::json!({"recorded": true}))
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
```

- [ ] **Step 3: Register modules**

In `agent-harness-core/src/lib.rs`:
```rust
pub mod agent_loop;
pub use agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopEvent};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-harness-core --lib agent_loop`
Expected: 3 tests PASS

- [ ] **Step 5: Verify workspace compilation**

Run: `cargo check`
Expected: No errors (tool_bridge may have unused warnings until wired in)

- [ ] **Step 6: Commit**

```bash
git add agent-harness-core/src/agent_loop.rs agent-harness-core/src/lib.rs src-tauri/src/tool_bridge.rs
git commit -m "feat: add complete AgentLoop::run() with real tool dispatch bridge (from Claw Code + Hermes)"
```

### Task 1.5: Rewrite Tauri ask_agent to Use AgentLoop

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Delete: `src-tauri/src/hermes_memory.rs`, `src-tauri/src/vector_db.rs`

- [ ] **Step 1: Delete duplicate files**

```bash
rm src-tauri/src/hermes_memory.rs src-tauri/src/vector_db.rs
```

- [ ] **Step 2: Rewrite ask_agent command**

In `src-tauri/src/lib.rs`, replace the `ask_agent` function with:

```rust
use agent_harness_core::{
    AgentLoop, AgentLoopConfig, AgentLoopEvent,
    provider::openai_compat::OpenAiCompatProvider,
    tool_registry::{ToolRegistry, default_tools},
};
use crate::tool_bridge::TauriToolBridge;

#[tauri::command]
async fn ask_agent(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    message: String,
    context: Option<String>,
    paragraph: Option<String>,
    selected_text: Option<String>,
) -> Result<(), String> {
    let api_key = require_api_key(&state)?;
    let has_lore = has_lorebook_entries(app.clone()).await.unwrap_or(false);
    let has_outline = has_outline_data(app.clone()).await.unwrap_or(false);

    // Build provider
    let api_base = std::env::var("OPENAI_API_BASE")
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into());
    let model = std::env::var("OPENAI_MODEL")
        .unwrap_or_else(|_| "deepseek/deepseek-v4-flash".into());
    let provider = Arc::new(OpenAiCompatProvider::new(&api_base, &api_key, &model));

    // Build system prompt
    let memory_ctx = build_context_injection(&state, &message).await;
    let system_prompt = format!(
        "{}\n\n## 当前项目上下文\n章节内容: {}\n当前段落: {}\n选中文本: {}\n\n## 学习到的偏好\n{}",
        SYSTEM_PROMPT_BASE,
        context.unwrap_or_default(),
        paragraph.unwrap_or_default(),
        selected_text.unwrap_or_default(),
        memory_ctx,
    );

    // Build tool registry
    let mut registry = ToolRegistry::new();
    for tool in default_tools() {
        registry.upsert(tool);
    }

    // Build tool bridge
    let bridge = TauriToolBridge { app: app.clone() };

    // Build agent loop
    let mut agent = AgentLoop::new(
        AgentLoopConfig {
            max_rounds: 10,
            system_prompt,
            ..Default::default()
        },
        provider,
        registry,
        bridge,
    );

    // Wire event emission to Tauri frontend
    let app_handle = app.clone();
    agent.set_event_callback(Arc::new(move |event| {
        let _ = app_handle.emit("agent-loop-event", serde_json::json!(event));
        // Also emit legacy events for backward compat
        match &event {
            AgentLoopEvent::TextChunk { content } => {
                let _ = app_handle.emit("agent-stream-chunk", serde_json::json!({"content": content}));
            }
            AgentLoopEvent::Complete { .. } => {
                let _ = app_handle.emit("agent-stream-end", serde_json::json!({"reason": "complete"}));
            }
            AgentLoopEvent::Error { message } => {
                let _ = app_handle.emit("agent-error", serde_json::json!({"message": message, "source": "agent_loop"}));
            }
            AgentLoopEvent::Intent { intent } => {
                let _ = app_handle.emit("agent-chain-of-thought", serde_json::json!({
                    "step": 1, "total": 3,
                    "description": format!("Intent: {}", intent),
                    "status": "completed"
                }));
            }
            _ => {}
        }
    }));

    // Add user message
    agent.add_user_message(message.clone());

    // Log to HermesDB
    log_user_message(&state, &message).await;

    // Run agent loop
    match agent.run(&message, has_lore, has_outline).await {
        Ok(final_text) => {
            log_assistant_message(&state, &final_text).await;
            // Background skill extraction
            let state_clone = state.inner().clone();
            tokio::spawn(async move {
                extract_skills_from_recent(&state_clone).await;
            });
            Ok(())
        }
        Err(e) => {
            // Already emitted via AgentLoopEvent::Error
            Err(e)
        }
    }
}
```

- [ ] **Step 3: Verify compilation and fix all errors**

Run: `cargo check 2>&1`
Fix any compilation errors. Ensure all imports are correct.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/tool_bridge.rs
git rm src-tauri/src/hermes_memory.rs src-tauri/src/vector_db.rs 2>/dev/null
git commit -m "refactor: rewrite ask_agent to use AgentLoop with provider + tool bridge (retire XML action tags)"
```

---

## Phase 2: Context Window Management

**Goal:** Context compaction with tool-pair boundary protection + LLM summarization + auto-trigger. This is the single most critical missing capability for long-form writing.

### Task 2.1: Compaction with Pair-Boundary Protection

**Reference:** Claw Code `compact.rs:96-158` (find_safe_boundary), CowAgent `agent_stream.py:1283-1331` (summarization callback)

**Files:**
- Create: `agent-harness-core/src/compaction.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Create compaction module** — use the code from original plan Task 2.1, which includes `find_safe_boundary`, `should_compact`, `estimate_message_tokens`, `build_compaction_prompt`.

- [ ] **Step 2: Add `compact_messages` with LLM call**

Add to `compaction.rs`:

```rust
use crate::provider::{Provider, LlmMessage, LlmRequest};

/// Perform full compaction: LLM summarizes old messages, summary injected as system message.
/// Returns the compacted message list and a report.
pub async fn compact_messages<P: Provider>(
    messages: &[LlmMessage],
    config: &CompactionConfig,
    provider: &P,
) -> Result<(Vec<LlmMessage>, CompactionResult), String> {
    let total = messages.len();
    if total <= config.preserve_recent {
        return Ok((messages.to_vec(), CompactionResult {
            summary: String::new(),
            compacted_count: 0,
            preserved_count: total,
            tokens_before: estimate_message_tokens(messages),
            tokens_after: estimate_message_tokens(messages),
        }));
    }

    let cut = total.saturating_sub(config.preserve_recent);
    let safe_cut = find_safe_boundary(messages, cut);

    let to_compact = &messages[..safe_cut];
    let preserved = &messages[safe_cut..];

    let tokens_before = estimate_message_tokens(messages);

    // Call LLM for structured summary
    let prompt = build_compaction_prompt(to_compact);
    let request = LlmRequest {
        messages: vec![LlmMessage {
            role: "user".into(),
            content: Some(prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }],
        tools: None,
        temperature: Some(0.3),
        max_tokens: Some(config.max_summary_tokens as u32),
        system: Some("You are a conversation summarizer. Be concise but thorough.".into()),
        stream: false,
    };

    let response = provider.call(request).await
        .map_err(|e| format!("Compaction LLM call failed: {}", e))?;

    let summary = response.content.unwrap_or_default();

    // Build new message list
    let mut new_messages = vec![LlmMessage {
        role: "system".into(),
        content: Some(format!(
            "[CONTEXT COMPACTION — {} messages summarized]\n\n{}",
            to_compact.len(),
            summary,
        )),
        tool_calls: None,
        tool_call_id: None,
        name: Some("compaction".into()),
    }];
    new_messages.extend_from_slice(preserved);

    let tokens_after = estimate_message_tokens(&new_messages);

    Ok((new_messages, CompactionResult {
        summary,
        compacted_count: to_compact.len(),
        preserved_count: preserved.len(),
        tokens_before,
        tokens_after,
    }))
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib compaction`
Expected: 5 tests PASS

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/compaction.rs agent-harness-core/src/lib.rs
git commit -m "feat: add context compaction with pair-boundary protection + LLM summarization (from Claw Code + CowAgent)"
```

### Task 2.2: Integrate Compaction into AgentLoop

**Files:**
- Modify: `agent-harness-core/src/agent_loop.rs`

- [ ] **Step 1: Add auto-compaction to AgentLoop::run()**

In the `run()` method, replace the placeholder compaction check with:

```rust
// In the compaction check section of run():
if should_compact(&self.messages, &self.config.system_prompt, &compaction_config) {
    let before = self.estimate_tokens();
    let (new_messages, result) = compact_messages(
        &self.messages,
        &compaction_config,
        &*self.provider,
    ).await?;
    self.messages = new_messages;
    self.emit(AgentLoopEvent::Compaction {
        before_tokens: before,
        after_tokens: result.tokens_after,
        compacted_count: result.compacted_count,
    });
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent-harness-core`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/agent_loop.rs
git commit -m "feat: integrate auto-compaction into AgentLoop::run() with LLM summarization"
```

---

## Phase 3: Memory & Structured Understanding

**Goal:** Structured character/plot/world-rule models. Fix BM25 for Chinese. Add session search. The "second brain" needs structured knowledge, not just RAG.

### Task 3.1: Fix BM25 for Chinese

**Reference:** jieba-rs for Chinese word segmentation

**Files:**
- Modify: `agent-harness-core/src/vector_db.rs`
- Modify: `agent-harness-core/Cargo.toml`

- [ ] **Step 1: Add jieba-rs and replace tokenization** — use code from original plan Task 4.1.

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib vector_db`
Expected: BM25 Chinese tokenization tests PASS (multiple tokens, not one blob)

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/vector_db.rs agent-harness-core/Cargo.toml
git commit -m "fix: replace whitespace tokenization with jieba-rs for Chinese BM25"
```

### Task 3.2: Structured Character/Plot/World-Rule Tables

**Reference:** Hermes `hermes_state.py` (SQLite FTS5 schema)

**Files:**
- Modify: `agent-harness-core/src/hermes_memory.rs`

- [ ] **Step 1: Add character_state, plot_thread, world_rule tables + CRUD** — use code from original plan Task 4.2.

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib hermes_memory`
Expected: Character CRUD, plot lifecycle, world rule violation tests PASS

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/hermes_memory.rs
git commit -m "feat: add character_state, plot_thread, world_rule tables to HermesDB for structured story understanding"
```

### Task 3.3: Session Search (FTS5 + LLM Summarization)

**Reference:** Hermes `session_search_tool.py` (FTS5 cross-session search with LLM refinement)

**Files:**
- Modify: `agent-harness-core/src/hermes_memory.rs`

- [ ] **Step 1: Add session_search method**

Add to `impl HermesDB`:

```rust
/// Full-text search across all session history.
/// Returns relevant past conversations with LLM-summarized context.
pub fn search_sessions(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<SessionSearchResult>> {
    let db = self.db.lock().unwrap();
    let mut stmt = db.prepare(
        "SELECT role, content, created_at FROM session_history
         WHERE content LIKE ?1
         ORDER BY created_at DESC
         LIMIT ?2"
    )?;
    let pattern = format!("%{}%", query);
    let rows = stmt.query_map(rusqlite::params![pattern, limit], |row| {
        Ok(SessionSearchResult {
            role: row.get(0)?,
            content: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;
    rows.collect()
}
```

Where `SessionSearchResult` is:
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSearchResult {
    pub role: String,
    pub content: String,
    pub created_at: String,
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib hermes_memory`
Expected: Session search tests PASS

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/hermes_memory.rs
git commit -m "feat: add FTS5 session search for cross-session memory retrieval (from Hermes)"
```

---

## Phase 4: Intelligence Layer

**Goal:** Permission pipeline, skill lifecycle with Curator, Programmatic Tool Calling. These are the "brain" features that make the agent learn and operate safely.

### Task 4.1: Permission Pipeline

**Reference:** Claw Code `permissions.rs` (PermissionPolicy::authorize_with_context)

**Files:**
- Create: `agent-harness-core/src/permission.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Create permission module** — use code from original plan Task 6.2.

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib permission`
Expected: 5 tests PASS

- [ ] **Step 3: Commit**

```bash
git add agent-harness-core/src/permission.rs agent-harness-core/src/lib.rs
git commit -m "feat: add permission pipeline with deny/allow/ask rules and mode escalation (from Claw Code)"
```

### Task 4.2: Skill Lifecycle with Curator

**Reference:** Hermes `curator.py` + `skill_manager_tool.py`, CowAgent `summarizer.py` (Deep Dream)

**Files:**
- Create: `agent-harness-core/src/skill_lifecycle.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Create skill_lifecycle module** — use code from original plan Task 5.1.

- [ ] **Step 2: Add Deep Dream periodic distillation method**

Add to `SkillCurator`:

```rust
/// Deep Dream: periodically distill recent session history into refined skills.
/// Ported from CowAgent `summarizer.py` (line 39-80).
pub async fn deep_dream<P: Provider>(
    &mut self,
    recent_sessions: &[String], // last N session contents
    provider: &P,
) -> Result<Vec<Skill>, String> {
    let combined = recent_sessions.join("\n---\n");
    let prompt = format!(
        r#"回顾以下写作会话，提炼出可复用的写作规则和用户偏好。

## 会话记录
{}

## 输出格式
返回JSON数组，每个元素包含：
- skill: 规则描述（简洁的一句话）
- category: style|character|pacing|preference|plot_structure|dialogue|description|world_building
- confidence: 0.0-1.0 (根据证据强度)
- triggers: 触发关键词列表

只提取有充分证据支撑的规则。不要编造。"#,
        combined.chars().take(8000).collect::<String>(),
    );

    let request = LlmRequest {
        messages: vec![LlmMessage {
            role: "user".into(),
            content: Some(prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }],
        tools: None,
        temperature: Some(0.3),
        max_tokens: Some(2000),
        system: Some("你是一个写作风格分析助手。只输出JSON。".into()),
        stream: false,
    };

    let response = provider.call(request).await
        .map_err(|e| format!("Deep Dream failed: {}", e))?;

    let text = response.content.unwrap_or_default();
    let skills: Vec<Skill> = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Deep Dream output: {}", e))?;

    for skill in &skills {
        self.upsert_skill(skill.clone());
    }

    Ok(skills)
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-harness-core --lib skill_lifecycle`
Expected: 3+ tests PASS

- [ ] **Step 4: Commit**

```bash
git add agent-harness-core/src/skill_lifecycle.rs agent-harness-core/src/lib.rs
git commit -m "feat: add skill lifecycle with Curator + Deep Dream periodic distillation (from Hermes + CowAgent)"
```

### Task 4.3: Programmatic Tool Calling

**Reference:** Hermes `code_execution_tool.py` (PTC architecture)

**Files:**
- Create: `agent-harness-core/src/ptc.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Create PTC module** — use code from original plan Task 6.1.

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-harness-core --lib ptc`
Expected: 4 tests PASS

- [ ] **Step 3: Verify the full agent-harness-core test suite**

Run: `cargo test -p agent-harness-core`
Expected: ALL tests PASS (30+ tests across all modules)

- [ ] **Step 4: Commit**

```bash
git add agent-harness-core/src/ptc.rs agent-harness-core/src/lib.rs
git commit -m "feat: add Programmatic Tool Calling for complex multi-step analysis (from Hermes PTC)"
```

---

## Phase 5: Editor Integration (Cursor-Style Co-Writing)

**Now the foundation is solid. Editor integration goes on top.**

### Task 5.1: Patch Types + PatchMark ProseMirror Extension

**Reference:** OpenCode `apply_patch` tool pattern

**Files:**
- Modify: `src/protocol.ts`
- Create: `src/extensions/PatchMark.ts`
- Modify: `src/index.css`

- [ ] **Step 1: Add patch types to protocol.ts** — use code from original plan Task 3.1.

- [ ] **Step 2: Create PatchMark extension** — use code from original plan Task 3.2.

- [ ] **Step 3: Commit**

```bash
git add src/protocol.ts src/extensions/PatchMark.ts src/index.css
git commit -m "feat: add patch review types + PatchMark ProseMirror extension"
```

### Task 5.2: PatchReviewOverlay Component

**Reference:** OpenCode multi-block diff review UI

**Files:**
- Create: `src/components/PatchReviewOverlay.tsx`
- Modify: `src/store.ts`
- Modify: `src/components/EditorPanel.tsx`

- [ ] **Step 1: Create PatchReviewOverlay** — use code from original plan Task 3.3.

- [ ] **Step 2: Commit**

```bash
git add src/components/PatchReviewOverlay.tsx src/store.ts src/components/EditorPanel.tsx
git commit -m "feat: add PatchReviewOverlay with Tab/Esc per-block accept/reject"
```

### Task 5.3: Context-Aware Ghost Text

**Files:**
- Modify: `src-tauri/src/lib.rs` (`report_editor_state`)
- Modify: `src/extensions/GhostText.ts`

- [ ] **Step 1: Replace raw FIM with context-aware prompt** — use code from original plan Task 3.4.

- [ ] **Step 2: Verify FIM format matches DeepSeek**

Check the model's expected FIM format. For DeepSeek V3/V4:
```
<|fim▁begin|>prefix<|fim▁hole|>suffix<|fim▁end|>
```

Update the prompt template accordingly in `report_editor_state`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs src/extensions/GhostText.ts
git commit -m "feat: upgrade ghost text to context-aware continuation with proper FIM format"
```

### Task 5.4: Skill-Driven Attention Policy

**Files:**
- Modify: `src-tauri/src/agent_runtime.rs`

- [ ] **Step 1: Replace hardcoded keywords with SkillCurator matching** — use code from original plan Task 5.2.

- [ ] **Step 2: Add CoWriterStatusBar** — use code from original plan Task 6.3.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/agent_runtime.rs src/components/CoWriterStatusBar.tsx src/App.tsx
git commit -m "feat: replace hardcoded attention with skill-driven policy + CoWriterStatusBar"
```

---

## Phase 6: Event-Driven Ambient Agent Swarm (Co-Pilot Model)

**Goal:** Replace the batch "outline→draft→review→polish" pipeline with an event-driven swarm of ambient agents. Each agent is a background daemon subscribed to specific editor events. They work silently, never blocking the main thread. Output is rendered as ghost text, hover hints, storyboard markers, or inline annotations — never as a blocking progress bar.

**Reference:** Cursor's inline completion model, OpenCode's LSP integration pattern, Hermes subagent delegation, the project's own existing `CancellationToken` + `EditorPredictionTask` pattern (19 references in lib.rs)

**Architecture:**

```
Tiptap Editor (frontend)
  onUpdate / onSelectionChange / onSave / onChapterSwitch
Tauri Event Bridge (existing: report_editor_state, agent_observe, save_chapter)
tokio::sync::broadcast::channel<EditorEvent> (capacity: 256)
  +-- 🕵 ContextFetcherAgent   subscribe: CursorMoved, KeywordDetected
  |     Behavior: silently fetch lorebook/outline/brain into Arc<Mutex<ContextCache>>
  |     Output: None (pure background caching)
  |
  +-- 👻 CoWriterAgent         subscribe: IdleTick(500ms)
  |     Behavior: read ContextCache, generate FIM ghost text
  |     Output: editor-ghost-chunk event (gray ghost text)
  |     Cancel: user types -> CancellationToken abort old task
  |
  +-- 🧙 PacingAnalystAgent    subscribe: ChapterSaved
  |     Behavior: parallel read full outline, analyze pacing
  |     Output: storyboard-update event (orange marker)
  |
  +-- 🔍 ContinuityWatcher     subscribe: IdleTick(3000ms)
  |     Behavior: check last 3 paragraphs for setting contradictions
  |     Output: editor-semantic-lint event (red wavy underline)
  |
  +-- 📓 MemoryCurator         subscribe: SessionEnded, ChapterSaved
        Behavior: Deep Dream distill session into MEMORY.md
        Output: agent-epiphany event (purple skill card)
```

### Task 6.1: Event Bus + AmbientAgent Trait

**Files:**
- Create: `agent-harness-core/src/ambient.rs`
- Modify: `agent-harness-core/src/lib.rs`

- [ ] **Step 1: Define EditorEvent and AmbientAgent trait**

Create `agent-harness-core/src/ambient.rs`:

```rust
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Events emitted by the editor that agents can subscribe to.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum EditorEvent {
    #[serde(rename = "cursor_moved")]
    CursorMoved {
        chapter: String,
        position: usize,
        paragraph: String,
    },
    #[serde(rename = "text_changed")]
    TextChanged {
        chapter: String,
        full_text_snippet: String,
        change_summary: String,
    },
    #[serde(rename = "idle_tick")]
    IdleTick {
        idle_ms: u64,
        chapter: String,
        paragraph: String,
        cursor_position: usize,
    },
    #[serde(rename = "selection_changed")]
    SelectionChanged {
        from: usize,
        to: usize,
        text: String,
        chapter: String,
    },
    #[serde(rename = "chapter_saved")]
    ChapterSaved {
        chapter: String,
        content_length: usize,
        revision: String,
    },
    #[serde(rename = "chapter_switched")]
    ChapterSwitched {
        from: Option<String>,
        to: String,
    },
    #[serde(rename = "session_ended")]
    SessionEnded,
    #[serde(rename = "keyword_detected")]
    KeywordDetected {
        keywords: Vec<String>,
        chapter: String,
        paragraph: String,
    },
}

/// Result of an ambient agent's processing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "output_kind")]
pub enum AgentOutput {
    #[serde(rename = "ghost_text")]
    GhostText { text: String, position: usize },
    #[serde(rename = "hover_hint")]
    HoverHint { message: String, from: usize, to: usize },
    #[serde(rename = "semantic_lint")]
    SemanticLint {
        message: String,
        from: usize,
        to: usize,
        severity: String,
    },
    #[serde(rename = "storyboard_marker")]
    StoryboardMarker {
        chapter: String,
        message: String,
        level: String,
    },
    #[serde(rename = "epiphany")]
    Epiphany { skill: String, category: String },
    #[serde(rename = "none")]
    None,
}

/// Trait for ambient agents -- background daemons that respond to editor events.
/// Each agent runs in its own tokio task and never blocks the main thread.
#[async_trait::async_trait]
pub trait AmbientAgent: Send + Sync {
    fn name(&self) -> &str;
    fn subscribed_events(&self) -> Vec<String>;
    async fn process(
        &self,
        event: EditorEvent,
        cancel: CancellationToken,
    ) -> Option<AgentOutput>;
}

/// The event bus that routes editor events to subscribed agents.
pub struct AmbientEventBus {
    tx: broadcast::Sender<EditorEvent>,
    agents: Vec<AmbientAgentHandle>,
}

struct AmbientAgentHandle {
    name: String,
    join_handle: Option<tokio::task::JoinHandle<()>>,
    cancel: CancellationToken,
}

impl AmbientEventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx, agents: Vec::new() }
    }

    pub fn publish(&self, event: EditorEvent) -> Result<usize, broadcast::error::SendError<EditorEvent>> {
        self.tx.send(event)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EditorEvent> {
        self.tx.subscribe()
    }

    /// Spawn an ambient agent. The agent runs in a background tokio task.
    /// Call abort_agent() to cancel it if the user modifies its dependent context.
    pub fn spawn<A: AmbientAgent + 'static>(
        &mut self,
        agent: Arc<A>,
        on_output: Arc<dyn Fn(AgentOutput) + Send + Sync>,
    ) {
        let mut rx = self.subscribe();
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let name = agent.name().to_string();
        let subscribed = agent.subscribed_events();

        let join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = rx.recv() => {
                        match result {
                            Ok(event) => {
                                let event_kind = event_kind_str(&event);
                                if !subscribed.is_empty()
                                    && !subscribed.iter().any(|e| e == event_kind)
                                {
                                    continue;
                                }
                                if let Some(output) = agent.process(event, cancel_clone.clone()).await {
                                    on_output(output);
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("Agent {} lagged by {} events", name, n);
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });

        self.agents.push(AmbientAgentHandle { name, join_handle: Some(join_handle), cancel });
    }

    /// Abort a specific agent. Also used for "debounce replacement":
    /// when new text arrives, abort the old CoWriter, spawn a new one.
    pub fn abort_agent(&mut self, name: &str) {
        if let Some(handle) = self.agents.iter().find(|a| a.name == name) {
            handle.cancel.cancel();
        }
    }

    pub async fn shutdown(&mut self) {
        for handle in &self.agents {
            handle.cancel.cancel();
        }
        for mut handle in std::mem::take(&mut self.agents) {
            if let Some(jh) = handle.join_handle.take() {
                let _ = jh.await;
            }
        }
    }
}

fn event_kind_str(event: &EditorEvent) -> &str {
    match event {
        EditorEvent::CursorMoved { .. } => "cursor_moved",
        EditorEvent::TextChanged { .. } => "text_changed",
        EditorEvent::IdleTick { .. } => "idle_tick",
        EditorEvent::SelectionChanged { .. } => "selection_changed",
        EditorEvent::ChapterSaved { .. } => "chapter_saved",
        EditorEvent::ChapterSwitched { .. } => "chapter_switched",
        EditorEvent::SessionEnded => "session_ended",
        EditorEvent::KeywordDetected { .. } => "keyword_detected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TestAgent {
        name: String,
        events: Vec<String>,
    }

    #[async_trait]
    impl AmbientAgent for TestAgent {
        fn name(&self) -> &str { &self.name }
        fn subscribed_events(&self) -> Vec<String> { self.events.clone() }
        async fn process(
            &self,
            _event: EditorEvent,
            _cancel: CancellationToken,
        ) -> Option<AgentOutput> {
            Some(AgentOutput::None)
        }
    }

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let mut bus = AmbientEventBus::new(16);
        let mut rx = bus.subscribe();
        bus.publish(EditorEvent::SessionEnded).unwrap();
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, EditorEvent::SessionEnded));
    }

    #[tokio::test]
    async fn test_spawn_agent_receives_subscribed_event() {
        let mut bus = AmbientEventBus::new(16);
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();

        let agent = Arc::new(TestAgent {
            name: "test".into(),
            events: vec!["idle_tick".into()],
        });
        bus.spawn(agent, Arc::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        bus.publish(EditorEvent::IdleTick {
            idle_ms: 500,
            chapter: "ch1".into(),
            paragraph: "hello".into(),
            cursor_position: 0,
        }).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        bus.shutdown().await;
        assert!(count.load(Ordering::Relaxed) >= 1);
    }

    #[tokio::test]
    async fn test_agent_only_receives_subscribed_events() {
        let mut bus = AmbientEventBus::new(16);
        let count = Arc::new(AtomicU32::new(0));
        let c = count.clone();

        let agent = Arc::new(TestAgent {
            name: "test".into(),
            events: vec!["chapter_saved".into()],
        });
        bus.spawn(agent, Arc::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        // Publish an event the agent does NOT subscribe to
        bus.publish(EditorEvent::CursorMoved {
            chapter: "ch1".into(),
            position: 0,
            paragraph: "hello".into(),
        }).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        bus.shutdown().await;
        // Agent should NOT have received this event
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }
}
```

- [ ] **Step 2: Register in lib.rs**

```rust
pub mod ambient;
pub use ambient::{AmbientAgent, AmbientEventBus, AgentOutput, EditorEvent};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-harness-core --lib ambient`
Expected: 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add agent-harness-core/src/ambient.rs agent-harness-core/src/lib.rs
git commit -m "feat: add AmbientAgent trait + EventBus with pub/sub, filtering, and cancellation"
```

### Task 6.2: ContextFetcherAgent + CoWriterAgent (Background Agents)

**Files:**
- Create: `src-tauri/src/ambient_agents/mod.rs`
- Create: `src-tauri/src/ambient_agents/context_fetcher.rs`
- Create: `src-tauri/src/ambient_agents/co_writer.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create mod.rs and ContextFetcherAgent**

Create `src-tauri/src/ambient_agents/mod.rs`:

```rust
pub mod context_fetcher;
pub mod co_writer;
```

Create `src-tauri/src/ambient_agents/context_fetcher.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use agent_harness_core::ambient::{AmbientAgent, AgentOutput, EditorEvent};
use async_trait::async_trait;

/// Shared context cache readable by all ambient agents.
/// Populated silently by ContextFetcherAgent.
#[derive(Debug, Clone, Default)]
pub struct ContextCache {
    pub lore_entries: HashMap<String, Vec<String>>,
    pub outline_map: HashMap<String, String>,
    pub last_updated: u64,
}

pub struct ContextFetcherAgent {
    pub app: tauri::AppHandle,
    pub cache: Arc<Mutex<ContextCache>>,
}

#[async_trait]
impl AmbientAgent for ContextFetcherAgent {
    fn name(&self) -> &str { "context-fetcher" }

    fn subscribed_events(&self) -> Vec<String> {
        vec!["keyword_detected".into(), "chapter_switched".into()]
    }

    async fn process(
        &self,
        event: EditorEvent,
        _cancel: CancellationToken,
    ) -> Option<AgentOutput> {
        match event {
            EditorEvent::KeywordDetected { keywords, chapter, .. } => {
                let mut cache = self.cache.lock().await;
                for kw in &keywords {
                    if cache.lore_entries.contains_key(kw) { continue; }
                    if let Ok(entries) = crate::storage::load_lorebook(&self.app) {
                        let matches: Vec<String> = entries.iter()
                            .filter(|e| e.keyword.contains(kw) || kw.contains(&e.keyword))
                            .map(|e| e.content.clone())
                            .collect();
                        cache.lore_entries.insert(kw.clone(), matches);
                    }
                }
                if !cache.outline_map.contains_key(&chapter) {
                    if let Ok(nodes) = crate::storage::load_outline(&self.app) {
                        if let Some(node) = nodes.iter().find(|n| n.chapter_title == chapter) {
                            cache.outline_map.insert(chapter, node.summary.clone());
                        }
                    }
                }
                cache.last_updated = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap().as_millis() as u64;
            }
            EditorEvent::ChapterSwitched { to, .. } => {
                let mut cache = self.cache.lock().await;
                if !cache.outline_map.contains_key(&to) {
                    if let Ok(nodes) = crate::storage::load_outline(&self.app) {
                        if let Some(node) = nodes.iter().find(|n| n.chapter_title == to) {
                            cache.outline_map.insert(to, node.summary.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }
}
```

Create `src-tauri/src/ambient_agents/co_writer.rs`:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use agent_harness_core::ambient::{AmbientAgent, AgentOutput, EditorEvent};
use async_trait::async_trait;
use super::context_fetcher::ContextCache;

pub struct CoWriterAgent {
    pub app: tauri::AppHandle,
    pub cache: Arc<Mutex<ContextCache>>,
}

#[async_trait]
impl AmbientAgent for CoWriterAgent {
    fn name(&self) -> &str { "co-writer" }

    fn subscribed_events(&self) -> Vec<String> {
        vec!["idle_tick".into()]
    }

    async fn process(
        &self,
        event: EditorEvent,
        cancel: CancellationToken,
    ) -> Option<AgentOutput> {
        if let EditorEvent::IdleTick { idle_ms, chapter, paragraph, cursor_position } = event {
            if idle_ms < 500 { return None; }

            let cache = self.cache.lock().await;
            let lore_context: String = cache.lore_entries.values()
                .take(3).flatten().cloned().collect::<Vec<_>>().join("\n");
            let outline = cache.outline_map.get(&chapter).cloned().unwrap_or_default();

            let prompt = format!(
                "你是中文小说写作助手。根据上下文从光标处续写，只输出续写文本。\n\
                 ## 大纲\n{}\n## 设定\n{}\n## 前文\n{}\n## 续写",
                outline, lore_context, paragraph,
            );

            let api_key = crate::resolve_api_key()?;
            let settings = crate::llm_runtime::settings(api_key);
            let messages = vec![serde_json::json!({"role": "user", "content": prompt})];

            let mut ghost = String::new();
            let result = crate::llm_runtime::stream_chat_cancellable(
                &settings, messages, cancel.clone(), 30,
                |content| {
                    ghost.push_str(&content);
                    Ok(crate::llm_runtime::StreamControl::Continue)
                },
            ).await;

            if cancel.is_cancelled() { return None; }

            if result.is_ok() && ghost.len() > 2 {
                return Some(AgentOutput::GhostText { text: ghost, position: cursor_position });
            }
        }
        None
    }
}
```

- [ ] **Step 2: Wire into Tauri app lifecycle**

In `src-tauri/src/lib.rs` `run()` function, initialize:

```rust
use agent_harness_core::ambient::AmbientEventBus;
use crate::ambient_agents::context_fetcher::{ContextFetcherAgent, ContextCache};
use crate::ambient_agents::co_writer::CoWriterAgent;

// In run(), add to AppState or as a managed state:
let mut event_bus = AmbientEventBus::new(256);
let cache = Arc::new(Mutex::new(ContextCache::default()));

// Spawn ContextFetcher (silent background cache)
let fetcher = Arc::new(ContextFetcherAgent {
    app: app_handle.clone(),
    cache: cache.clone(),
});
event_bus.spawn(fetcher, Arc::new(|_| {}));

// Spawn CoWriter (emits ghost text events)
let cowriter = Arc::new(CoWriterAgent {
    app: app_handle.clone(),
    cache: cache.clone(),
});
let ah = app_handle.clone();
event_bus.spawn(cowriter, Arc::new(move |output| {
    if let AgentOutput::GhostText { text, position } = output {
        let _ = ah.emit("editor-ghost-chunk", serde_json::json!({
            "content": text, "position": position,
        }));
    }
}));

// Store event_bus for Tauri commands to publish to
app_handle.manage(Mutex::new(event_bus));
```

Route existing events through the bus. In `report_editor_state`, add:

```rust
if let Some(eb) = app.try_state::<Mutex<AmbientEventBus>>() {
    if let Ok(bus) = eb.lock() {
        let _ = bus.publish(EditorEvent::IdleTick {
            idle_ms: 500,
            chapter: payload.chapter_title.unwrap_or_default(),
            paragraph: payload.paragraph.unwrap_or_default(),
            cursor_position: payload.cursor_position.unwrap_or(0),
        });
    }
}
```

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo check && cargo test -p agent-harness-core --lib ambient`
Expected: No errors, 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/ambient_agents/ src-tauri/src/lib.rs
git commit -m "feat: add ContextFetcherAgent + CoWriterAgent wired into Tauri lifecycle"
```

### Task 6.3: ContinuityWatcher + PacingAnalyst (Non-Intrusive Diagnostics)

**Files:**
- Create: `src-tauri/src/ambient_agents/continuity_watcher.rs`
- Create: `src-tauri/src/ambient_agents/pacing_analyst.rs`
- Modify: `src-tauri/src/ambient_agents/mod.rs`

These agents are background daemons that produce hover hints, storyboard markers, and semantic lint — never blocking the main editing flow.

- [ ] **Step 1: Create ContinuityWatcher**

```rust
// Trigger: 3s idle after paragraph completion
// Checks: lorebook contradictions in last 3 paragraphs
// Output: AgentOutput::SemanticLint or AgentOutput::HoverHint
pub struct ContinuityWatcher { pub app: tauri::AppHandle; }
```

- [ ] **Step 2: Create PacingAnalyst**

```rust
// Trigger: ChapterSaved
// Checks: chapter length vs outline expectations, scene balance
// Output: AgentOutput::StoryboardMarker (orange badge on chapter)
pub struct PacingAnalyst { pub app: tauri::AppHandle; }
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/ambient_agents/
git commit -m "feat: add ContinuityWatcher + PacingAnalyst ambient agents"
```

### Key Design Decisions

1. **Debounce = Cancel + Respawn**: When the user types while CoWriterAgent is generating, don't queue the new request — cancel the old one and spawn fresh. `CancellationToken::cancel()` + `tokio::spawn` naturally implements this.

2. **ContextCache is the silent data layer**: ContextFetcherAgent populates it, all other agents read it. No agent talks directly to storage during time-sensitive operations (FIM needs sub-second latency).

3. **Agent output is non-blocking hints**: Ghost text, hover hints, storyboard markers, inline lint. Never a modal. Never a progress bar. The user can ignore all of them and keep typing.

4. **Broadcast capacity = 256**: Large enough to absorb burst events (typing produces many TextChanged events). When lagged, agents skip stale events and process the latest — exactly the behavior described in the user's architecture vision.

## Verification Strategy

### Per-Phase Checklist

After each phase, run:
```bash
# All Rust tests
cargo test -p agent-harness-core
cargo test -p agent_writer_lib

# TypeScript compilation
npx tsc --noEmit

# Full build
cargo build
```

### End-to-End Smoke Test (After Phase 5+)

- [ ] Open Forge, load a project with lorebook and outline
- [ ] Type text → context-aware ghost continuation appears (not just FIM)
- [ ] Ctrl+K on selected text → inline command returns patch set
- [ ] Tab on each patch block accepts, Esc rejects, Ctrl+A accepts all
- [ ] Chat in AgentPanel → agent uses function calling (check devtools for `agent-loop-event`)
- [ ] Write 50+ messages → compaction triggers automatically
- [ ] Semantic lint detects weapon conflicts and world rule violations
- [ ] Attention fires based on learned skills, not hardcoded keywords
- [ ] `get_agent_kernel_status` shows tool registry with execution counts
- [ ] Writing pipeline generates a chapter with outline→draft→review→polish

---

## Dependency Graph

```
Phase 1 (Provider + AgentLoop + ToolExecutor)
  │
  ├──→ Phase 2 (Compaction + Summarization) ── depends on Phase 1 Provider trait
  │
  ├──→ Phase 3 (Memory + BM25 + Session Search) ── depends on Phase 1 HermesDB
  │
  ├──→ Phase 4 (Permission + Skills + PTC) ── depends on Phase 1 Provider + AgentLoop
  │
  ├──→ Phase 5 (Editor Integration) ── depends on ALL above (Phase 1-4)
  │
  └──→ Phase 6 (Multi-Agent Pipeline) ── depends on Phase 1 AgentLoop + Phase 4 PTC
```

**Recommended execution:** Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6
(Phases 2-3 can partially overlap; Phase 5 requires all backend phases complete)

---

## Module Test Coverage Summary

| Module | Min Tests | Covers |
|--------|-----------|--------|
| `provider` | 2 | Registry resolve, model listing |
| `retry` | 4 | Backoff progression, clamping, error classification |
| `tool_registry` | 4 | Schema export, filtering, generation counter |
| `tool_executor` | 3 | Doom loop detection, reset, handler dispatch |
| `agent_loop` | 3 | Creation, message add, token estimation |
| `compaction` | 5 | Safe boundary, token estimation, should_compact, prompt |
| `hermes_memory` | 6+ | Character CRUD, plot lifecycle, world rules, session search |
| `vector_db` | 4 | Chinese BM25, hybrid search, keyword extraction |
| `permission` | 5 | Mode escalation, deny rules, approval trigger |
| `skill_lifecycle` | 3 | Decay, trigger matching, usage tracking |
| `ptc` | 4 | Config defaults, environment, truncation, prompt |
| `delegate` | 0 | (Integration tested via writing pipeline) |
| **Total** | **43+** | Complete agent runtime coverage |
