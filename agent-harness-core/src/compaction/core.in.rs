/// Configuration for context compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Number of most recent messages to preserve uncompacted.
    pub preserve_recent: usize,
    /// Trigger compaction when token estimate exceeds this fraction of context limit.
    pub trigger_fraction: f64,
    /// Maximum tokens for the compaction summary.
    pub max_summary_tokens: u64,
    /// Context window limit for the model being used.
    pub context_limit_tokens: u64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            preserve_recent: 6,
            trigger_fraction: 0.70,
            max_summary_tokens: 800,
            context_limit_tokens: 120_000,
        }
    }
}

/// Kind of compaction applied.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum CompactionKind {
    #[default]
    Full,
    Microcompact,
    OverflowRecovery,
}

/// Result of a compaction operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompactionResult {
    /// The structured summary injected as a system message.
    pub summary: String,
    /// Number of messages that were compacted.
    pub compacted_count: usize,
    /// Number of messages preserved at the tail.
    pub preserved_count: usize,
    /// Estimated tokens before compaction.
    pub tokens_before: u64,
    /// Estimated tokens after compaction.
    pub tokens_after: u64,
    /// Which compaction kind was applied.
    #[serde(default)]
    pub kind: CompactionKind,
    /// Structured checkpoints recorded during compaction.
    #[serde(default)]
    pub checkpoints: Vec<CompactionCheckpoint>,
    /// Tokens saved by truncating old tool results (microcompact).
    #[serde(default)]
    pub tokens_saved_by_tool_truncation: u64,
    /// Human-readable summary of the compaction boundary.
    #[serde(default)]
    pub boundary_summary: String,
    /// Recovery level applied (overflow scenarios).
    #[serde(default)]
    pub recovery_level: Option<String>,
}

/// A checkpoint recorded during compaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompactionCheckpoint {
    pub name: String,
    pub message_count: usize,
    pub token_count: u64,
    pub metadata: Option<String>,
}

/// Structured summary template — mirrors Claw Code's SUMMARY_TEMPLATE.
const SUMMARY_TEMPLATE: &str = r#"Summarize the conversation so far. Be concise but comprehensive.

## Goal
What is the user trying to accomplish?

## Progress
- **Done:** What has been completed?
- **In Progress:** What is currently being worked on?
- **Blocked:** What is blocked and why?

## Key Decisions
What important decisions were made?

## Key Context
- Files/chapters referenced
- Characters involved
- Plot threads touched

## Next Steps
What should happen next?"#;

/// Walk back the compaction boundary to avoid splitting tool-use/tool-result pairs.
/// This prevents "orphaned tool result" errors with OpenAI-compatible providers.
/// Ported from Claw Code `compact.rs` lines 119-158.
pub fn find_safe_boundary(messages: &[LlmMessage], desired_cut: usize) -> usize {
    let boundary = desired_cut.min(messages.len());

    // Track pending tool call IDs that straddle the boundary.
    // If an assistant before the boundary has a tool_call, and the corresponding
    // tool result is after the boundary, extend boundary to include the result.
    let mut pending_ids: Vec<String> = Vec::new();

    for (i, msg) in messages.iter().enumerate() {
        if i < boundary {
            if msg.role == "assistant" {
                if let Some(ref tc) = msg.tool_calls {
                    for call in tc {
                        pending_ids.push(call.id.clone());
                    }
                }
            } else if msg.role == "tool" {
                if let Some(ref call_id) = msg.tool_call_id {
                    pending_ids.retain(|id| id != call_id);
                }
            }
        }
    }

    // Extend boundary past any orphaned tool results
    let mut extended = boundary;
    for msg in messages.iter().skip(boundary) {
        if msg.role == "tool" {
            if let Some(ref call_id) = msg.tool_call_id {
                if pending_ids.contains(call_id) {
                    extended += 1;
                    pending_ids.retain(|id| id != call_id);
                    continue;
                }
            }
        }
        if pending_ids.is_empty() {
            break;
        }
    }

    extended.min(messages.len())
}

/// Keep the latest user request in the preserved tail so compaction never turns
/// the active task into passive summary background.
pub fn anchor_latest_user_message(messages: &[LlmMessage], cut: usize) -> usize {
    let boundary = cut.min(messages.len());
    let Some(last_user_index) = messages.iter().rposition(|message| message.role == "user") else {
        return boundary;
    };

    if last_user_index >= boundary {
        boundary
    } else {
        last_user_index
    }
}

/// Estimate tokens in a collection of messages.
/// Rough heuristic: 1 token ≈ 3 chars for CJK-heavy text + 8 token per-message overhead.
pub fn estimate_message_tokens(messages: &[LlmMessage]) -> u64 {
    messages
        .iter()
        .map(|m| {
            let content_chars = m.content.as_ref().map(|c| c.chars().count()).unwrap_or(0) as u64;
            content_chars / 3 + 8
        })
        .sum()
}

/// Determine if compaction should be triggered.
pub fn should_compact(
    messages: &[LlmMessage],
    system_prompt: &str,
    config: &CompactionConfig,
) -> bool {
    let msg_tokens = estimate_message_tokens(messages);
    let sys_tokens = (system_prompt.chars().count() as u64) / 3;
    let total = msg_tokens + sys_tokens;
    let threshold = (config.context_limit_tokens as f64 * config.trigger_fraction) as u64;
    total > threshold
}

/// Build the compaction prompt for the LLM.
pub fn build_compaction_prompt(messages_to_compact: &[LlmMessage]) -> String {
    let conversation_text: String = messages_to_compact
        .iter()
        .map(|m| format!("[{}]: {}", m.role, m.content.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{}\n\n## Conversation to Summarize\n\n{}\n\nProvide the structured summary below.",
        SUMMARY_TEMPLATE, conversation_text,
    )
}

/// Perform full compaction: LLM summarizes old messages, summary injected as system message,
/// preserved messages kept at the tail.
/// Returns the compacted message list and a report.
pub async fn compact_messages<P: Provider>(
    messages: &[LlmMessage],
    config: &CompactionConfig,
    provider: &P,
) -> Result<(Vec<LlmMessage>, CompactionResult), String> {
    let total = messages.len();
    if total <= config.preserve_recent {
        return Ok((
            messages.to_vec(),
            CompactionResult {
                summary: String::new(),
                compacted_count: 0,
                preserved_count: total,
                tokens_before: estimate_message_tokens(messages),
                tokens_after: estimate_message_tokens(messages),
                kind: CompactionKind::Microcompact,
                checkpoints: Vec::new(),
                tokens_saved_by_tool_truncation: 0,
                boundary_summary: String::new(),
                recovery_level: None,
            },
        ));
    }

    let cut = total.saturating_sub(config.preserve_recent);
    let anchored_cut = anchor_latest_user_message(messages, cut);
    let safe_cut = find_safe_boundary(messages, anchored_cut);

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
        system: Some(
            "You are a conversation summarizer. Be concise but thorough. Follow the template exactly."
                .into(),
        ),
        stream: false,
    };

    let response = provider
        .call(request)
        .await
        .map_err(|e| format!("Compaction LLM call failed: {}", e))?;

    let summary = response.content.unwrap_or_default();

    // Build new message list: compaction system message + preserved
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

    Ok((
        new_messages,
        CompactionResult {
            summary,
            compacted_count: to_compact.len(),
            preserved_count: preserved.len(),
            tokens_before,
            tokens_after,
            kind: CompactionKind::Full,
            checkpoints: vec![CompactionCheckpoint {
                name: "compacted".to_string(),
                message_count: to_compact.len(),
                token_count: tokens_before.saturating_sub(tokens_after),
                metadata: Some(format!("cut at message {}", safe_cut)),
            }],
            tokens_saved_by_tool_truncation: 0,
            boundary_summary: format!(
                "compacted {} messages, preserved {} messages",
                to_compact.len(),
                preserved.len()
            ),
            recovery_level: None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    fn make_msg(role: &str, content: &str) -> LlmMessage {
        LlmMessage {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    fn make_tool_call_msg(
        role: &str,
        tool_calls: Option<Vec<crate::provider::ToolCall>>,
        call_id: Option<&str>,
    ) -> LlmMessage {
        LlmMessage {
            role: role.into(),
            content: Some("tool content".into()),
            tool_calls,
            tool_call_id: call_id.map(|s| s.into()),
            name: None,
        }
    }

    struct SummaryProvider;

    #[async_trait]
    impl Provider for SummaryProvider {
        fn name(&self) -> &str {
            "summary-provider"
        }

        fn models(&self) -> Vec<String> {
            vec!["summary-model".to_string()]
        }

        async fn stream_call(
            &self,
            _request: LlmRequest,
            _on_event: Box<dyn Fn(crate::provider::StreamEvent) + Send + Sync>,
        ) -> Result<crate::provider::LlmResponse, String> {
            self.call(_request).await
        }

        async fn call(&self, _request: LlmRequest) -> Result<crate::provider::LlmResponse, String> {
            Ok(crate::provider::LlmResponse {
                content: Some("summary".to_string()),
                tool_calls: None,
                finish_reason: "stop".to_string(),
                usage: None,
            })
        }

        async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
            Ok(Vec::new())
        }

        fn estimate_tokens(&self, messages: &[LlmMessage]) -> u64 {
            estimate_message_tokens(messages)
        }

        async fn health_check(&self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_safe_boundary_no_tool_calls() {
        let messages = vec![
            make_msg("user", "hello"),
            make_msg("assistant", "hi"),
            make_msg("user", "how are you"),
            make_msg("assistant", "good"),
        ];
        assert_eq!(find_safe_boundary(&messages, 2), 2);
    }

    #[test]
    fn test_safe_boundary_protects_tool_pair() {
        let tool_call = crate::provider::ToolCall {
            id: "call_1".into(),
            call_type: "function".into(),
            function: crate::provider::ToolCallFunction {
                name: "search".into(),
                arguments: r#"{"q":"test"}"#.into(),
            },
        };
        let messages = vec![
            make_msg("user", "search for X"),
            make_tool_call_msg("assistant", Some(vec![tool_call]), None),
            make_tool_call_msg("tool", None, Some("call_1")),
            make_msg("assistant", "found it"),
        ];
        // Cutting at 2 would split the assistant(tool_call) from tool(result)
        let boundary = find_safe_boundary(&messages, 2);
        assert!(
            boundary >= 3,
            "boundary should extend past the tool result, got {}",
            boundary
        );
    }

    #[test]
    fn latest_user_anchor_moves_cut_back_when_active_task_would_be_summarized() {
        let messages = vec![
            make_msg("user", "old request"),
            make_msg("assistant", "old answer"),
            make_msg("user", "ACTIVE TASK"),
            make_msg("assistant", "working"),
            make_msg("assistant", "still working"),
        ];

        assert_eq!(anchor_latest_user_message(&messages, 4), 2);
        assert_eq!(anchor_latest_user_message(&messages, 2), 2);
    }

    #[tokio::test]
    async fn compaction_preserves_latest_user_request_in_tail() {
        let messages = vec![
            make_msg("user", "old request"),
            make_msg("assistant", "old answer"),
            make_msg("user", "ACTIVE TASK: continue chapter 7"),
            make_msg("assistant", "I am checking context"),
            make_msg("assistant", "I am drafting now"),
        ];

        let (compacted, report) = compact_messages(
            &messages,
            &CompactionConfig {
                preserve_recent: 1,
                trigger_fraction: 0.70,
                max_summary_tokens: 200,
                context_limit_tokens: 8_000,
            },
            &SummaryProvider,
        )
        .await
        .expect("compaction succeeds");

        assert_eq!(report.compacted_count, 2);
        assert!(compacted.iter().any(|message| {
            message.role == "user"
                && message
                    .content
                    .as_deref()
                    .is_some_and(|content| content.contains("ACTIVE TASK"))
        }));
    }

    #[test]
    fn test_should_compact_below_threshold() {
        let config = CompactionConfig::default();
        let messages = vec![make_msg("user", "hi")];
        assert!(!should_compact(&messages, "short", &config));
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_message_tokens(&[]), 0);
    }

    #[test]
    fn test_compaction_prompt_includes_template() {
        let messages = vec![make_msg("user", "write chapter 3")];
        let prompt = build_compaction_prompt(&messages);
        assert!(prompt.contains("## Goal"));
        assert!(prompt.contains("write chapter 3"));
    }

    #[test]
    fn test_estimate_tokens_cjk() {
        let messages = vec![make_msg(
            "user",
            &"主角在破庙里发现了一把古老的剑。".repeat(10),
        )];
        let tokens = estimate_message_tokens(&messages);
        // ~170 CJK chars / 3 ≈ 57 + 8 overhead ≈ 65 tokens
        assert!(tokens > 30 && tokens < 200);
    }
}
