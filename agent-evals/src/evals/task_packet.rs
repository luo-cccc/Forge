use super::*;

use agent_harness_core::provider::{
    LlmMessage, LlmRequest, LlmResponse, Provider, StreamEvent, UsageInfo,
};

struct StaticDiagnosticProvider {
    answer: String,
    model: String,
}

impl StaticDiagnosticProvider {
    fn new(answer: &str) -> Self {
        Self {
            answer: answer.to_string(),
            model: "gpt-4o-mini".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Provider for StaticDiagnosticProvider {
    fn name(&self) -> &str {
        "eval-static-diagnostic"
    }

    fn models(&self) -> Vec<String> {
        vec![self.model.clone()]
    }

    async fn stream_call(
        &self,
        _request: LlmRequest,
        on_event: Box<dyn Fn(StreamEvent) + Send + Sync>,
    ) -> Result<LlmResponse, String> {
        on_event(StreamEvent::TextDelta {
            content: self.answer.clone(),
        });
        Ok(LlmResponse {
            content: Some(self.answer.clone()),
            tool_calls: None,
            finish_reason: "stop".to_string(),
            usage: Some(UsageInfo {
                input_tokens: 512,
                output_tokens: self.answer.chars().count() as u64 / 3,
            }),
        })
    }

    async fn call(&self, request: LlmRequest) -> Result<LlmResponse, String> {
        self.stream_call(request, Box::new(|_| {})).await
    }

    async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
        Ok(vec![0.0; 4])
    }

    fn estimate_tokens(&self, messages: &[LlmMessage]) -> u64 {
        messages
            .iter()
            .map(|message| {
                message
                    .content
                    .as_ref()
                    .map(|content| content.chars().count() as u64 / 3 + 8)
                    .unwrap_or(8)
            })
            .sum()
    }

    fn context_window_tokens(&self) -> u64 {
        128_000
    }

    async fn health_check(&self) -> Result<(), String> {
        Ok(())
    }
}

include!("task_packet/part_a.in.rs");
include!("task_packet/part_b.in.rs");
include!("task_packet/part_c.in.rs");
