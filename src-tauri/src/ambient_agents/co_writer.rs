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
    fn name(&self) -> &str {
        "co-writer"
    }

    fn subscribed_events(&self) -> Vec<String> {
        vec!["idle_tick".into()]
    }

    async fn process(
        &self,
        event: EditorEvent,
        cancel: CancellationToken,
    ) -> Option<AgentOutput> {
        if let EditorEvent::IdleTick {
            idle_ms,
            chapter,
            paragraph,
            cursor_position,
        } = event
        {
            if idle_ms < 500 {
                return None;
            }

            let cache = self.cache.lock().await;
            let lore_context: String = cache
                .lore_entries
                .values()
                .take(3)
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            let outline = cache
                .outline_map
                .get(&chapter)
                .cloned()
                .unwrap_or_default();

            let prompt = format!(
                "你是中文小说写作助手。根据上下文从光标处续写，只输出续写文本。\n\
                 ## 大纲\n{}\n## 设定\n{}\n## 前文\n{}\n## 续写",
                outline, lore_context, paragraph,
            );

            let api_key = match crate::resolve_api_key() {
                Some(k) => k,
                None => return None,
            };
            let settings = crate::llm_runtime::settings(api_key);
            let messages = vec![serde_json::json!({"role": "user", "content": prompt})];

            let mut ghost = String::new();
            let result = crate::llm_runtime::stream_chat_cancellable(
                &settings,
                messages,
                8,
                cancel.clone(),
                |content| {
                    ghost.push_str(&content);
                    Ok(crate::llm_runtime::StreamControl::Continue)
                },
            )
            .await;

            if cancel.is_cancelled() {
                return None;
            }

            if result.is_ok() && ghost.len() > 2 {
                return Some(AgentOutput::GhostText {
                    text: ghost,
                    position: cursor_position,
                });
            }
        }
        None
    }
}
