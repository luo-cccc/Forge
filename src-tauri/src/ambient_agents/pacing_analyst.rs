use agent_harness_core::ambient::{AgentOutput, AmbientAgent, EditorEvent};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct PacingAnalyst {
    pub app: tauri::AppHandle,
}

#[async_trait]
impl AmbientAgent for PacingAnalyst {
    fn name(&self) -> &str {
        "pacing-analyst"
    }

    fn subscribed_events(&self) -> Vec<String> {
        vec!["chapter_saved".into()]
    }

    async fn process(&self, event: EditorEvent, _cancel: CancellationToken) -> Option<AgentOutput> {
        let EditorEvent::ChapterSaved {
            chapter,
            content_length,
            ..
        } = event
        else {
            return None;
        };

        let outline_status = crate::storage::load_outline(&self.app)
            .ok()
            .and_then(|nodes| {
                nodes
                    .into_iter()
                    .find(|node| node.chapter_title == chapter)
                    .map(|node| node.status)
            })
            .unwrap_or_else(|| "unknown".to_string());

        let (message, level) = if content_length < 500 {
            (
                format!("{} 篇幅偏短，当前大纲状态为 {}。", chapter, outline_status),
                "warning",
            )
        } else if content_length > 16_000 {
            (
                format!("{} 篇幅偏长，可能需要拆分场景或章节。", chapter),
                "warning",
            )
        } else {
            return None;
        };

        Some(AgentOutput::StoryboardMarker {
            chapter,
            message,
            level: level.to_string(),
        })
    }
}
