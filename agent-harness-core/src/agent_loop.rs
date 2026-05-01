use std::sync::Arc;

use crate::provider::{LlmMessage, LlmRequest, Provider, StreamEvent};
use crate::router::{classify_intent, Intent};
use crate::tool_executor::{ToolExecution, ToolExecutor, ToolHandler};
use crate::tool_registry::{ToolFilter, ToolRegistry, ToolSideEffectLevel};

/// Events emitted during agent loop execution to the UI layer.
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
    ToolCallStart {
        tool: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_call_end")]
    ToolCallEnd {
        tool: String,
        result: ToolExecution,
    },
    #[serde(rename = "doom_loop_warning")]
    DoomLoopWarning { tool: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "complete")]
    Complete {
        rounds: u32,
        tool_calls: u32,
        tokens_used: u64,
    },
}

/// Configuration for agent loop execution.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_rounds: u32,
    pub system_prompt: String,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_rounds: 10,
            system_prompt: String::new(),
        }
    }
}

/// Event callback type — the Tauri layer provides this to emit events to the frontend.
pub type EventCallback = Arc<dyn Fn(AgentLoopEvent) + Send + Sync>;

/// The core agent execution loop.
/// Generic over Provider and ToolHandler — fully testable with mocks.
/// Ported from Claw Code `ConversationRuntime<C,T>` pattern.
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

    /// Build the available tools list filtered by intent.
    pub fn build_tools(&self, intent: &Intent) -> Vec<serde_json::Value> {
        let filter = ToolFilter {
            intent: Some(intent.clone()),
            max_side_effect_level: Some(ToolSideEffectLevel::Write),
            include_requires_approval: true,
            include_disabled: false,
            required_tags: Vec::new(),
        };
        let registry = self.executor.registry.blocking_lock();
        registry.to_openai_tools(&filter)
    }

    /// The main execution loop.
    ///
    /// 1. Classify intent → filter tools
    /// 2. While rounds < max: call LLM with streaming → execute tool calls → append results
    /// 3. Return the final text from the last assistant response
    pub async fn run(
        &mut self,
        user_message: &str,
        has_lorebook: bool,
        has_outline: bool,
    ) -> Result<String, String> {
        // Phase 1: Classify intent
        let intent = classify_intent(user_message, has_lorebook, has_outline);
        self.emit(AgentLoopEvent::Intent {
            intent: format!("{:?}", intent),
        });

        // Phase 2: Build tools for this intent
        let tools = self.build_tools(&intent);
        let has_tools = !tools.is_empty();

        // Phase 3: Execution rounds
        let mut rounds = 0u32;
        let mut total_tool_calls = 0u32;
        let mut final_text = String::new();

        self.emit(AgentLoopEvent::Thinking);

        while rounds < self.config.max_rounds {
            // Build LLM request
            let request = LlmRequest {
                messages: self.messages.clone(),
                tools: if has_tools { Some(tools.clone()) } else { None },
                temperature: Some(0.7),
                max_tokens: Some(4096),
                system: Some(self.config.system_prompt.clone()),
                stream: true,
            };

            // Call LLM with streaming — forward text chunks to UI
            let event_cb = self.on_event.clone();
            let response = self
                .provider
                .stream_call(
                    request,
                    Box::new(move |ev| {
                        if let (StreamEvent::TextDelta { content }, Some(ref cb)) =
                            (&ev, &event_cb)
                        {
                            cb(AgentLoopEvent::TextChunk {
                                content: content.clone(),
                            });
                        }
                    }),
                )
                .await
                .map_err(|e| {
                    self.emit(AgentLoopEvent::Error {
                        message: e.clone(),
                    });
                    e
                })?;

            let response_tool_calls = response.tool_calls.unwrap_or_default();

            // No tool calls → done
            if response_tool_calls.is_empty() {
                final_text = response.content.unwrap_or_default();
                self.messages.push(LlmMessage {
                    role: "assistant".into(),
                    content: Some(final_text.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
                break;
            }

            // Execute each tool call
            let mut assistant_tool_calls = Vec::new();
            for tc in &response_tool_calls {
                total_tool_calls += 1;

                let args: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);

                self.emit(AgentLoopEvent::ToolCallStart {
                    tool: tc.function.name.clone(),
                    args: args.clone(),
                });

                let execution = self.executor.execute(&tc.function.name, args).await;

                // Check for doom loop
                if execution
                    .error
                    .as_ref()
                    .map(|e| e.contains("DOOM LOOP"))
                    .unwrap_or(false)
                {
                    self.emit(AgentLoopEvent::DoomLoopWarning {
                        tool: tc.function.name.clone(),
                    });
                }

                self.emit(AgentLoopEvent::ToolCallEnd {
                    tool: tc.function.name.clone(),
                    result: execution.clone(),
                });

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

            // Add assistant message (the one that requested the tool calls)
            self.messages.push(LlmMessage {
                role: "assistant".into(),
                content: match response.content {
                    Some(ref c) if !c.is_empty() => Some(c.clone()),
                    _ => None,
                },
                tool_calls: Some(assistant_tool_calls),
                tool_call_id: None,
                name: None,
            });

            rounds += 1;
        }

        // Check max rounds exceeded
        if rounds >= self.config.max_rounds && final_text.is_empty() {
            let msg = format!(
                "Reached max rounds ({}) without final response",
                self.config.max_rounds
            );
            self.emit(AgentLoopEvent::Error {
                message: msg.clone(),
            });
            return Err(msg);
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
    use crate::tool_registry::default_writing_tool_registry;
    use async_trait::async_trait;

    /// Mock tool handler for testing.
    struct MockToolHandler;
    #[async_trait]
    impl ToolHandler for MockToolHandler {
        async fn execute(
            &self,
            tool_name: &str,
            args: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "tool": tool_name,
                "args": args,
                "result": "mock"
            }))
        }
    }

    fn make_agent() -> AgentLoop<OpenAiCompatProvider, MockToolHandler> {
        let provider = Arc::new(OpenAiCompatProvider::new(
            "https://api.openai.com/v1",
            "sk-test",
            "gpt-4o-mini",
        ));
        let registry = default_writing_tool_registry();
        AgentLoop::new(
            AgentLoopConfig {
                max_rounds: 3,
                system_prompt: "You are a test agent.".into(),
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
        assert_eq!(agent.messages[0].content, Some("hello".into()));
    }

    #[test]
    fn test_estimate_tokens() {
        let mut agent = make_agent();
        agent.add_user_message("你好世界".repeat(50));
        let tokens = agent.estimate_tokens();
        // ~200 CJK chars / 3 ≈ 67 tokens + overhead + system prompt overhead
        assert!(tokens > 50);
    }

    #[test]
    fn test_build_tools_returns_valid_schema() {
        let agent = make_agent();
        let tools = agent.build_tools(&Intent::RetrieveKnowledge);
        // Tools without input_schema are filtered out; we check the return type
        for tool in &tools {
            assert_eq!(tool["type"], "function");
            assert!(tool["function"]["name"].is_string());
        }
    }

    #[test]
    fn test_event_callback() {
        let mut agent = make_agent();
        let emitted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let emitted_clone = emitted.clone();
        agent.set_event_callback(Arc::new(move |ev| {
            emitted_clone.lock().unwrap().push(format!("{:?}", ev));
        }));
        agent.emit(AgentLoopEvent::Thinking);
        let events = emitted.lock().unwrap();
        assert!(!events.is_empty());
    }
}
