use agent_harness_core::ambient::{AgentOutput, AmbientAgent, EditorEvent};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub struct ContinuityWatcher {
    pub app: tauri::AppHandle,
}

#[async_trait]
impl AmbientAgent for ContinuityWatcher {
    fn name(&self) -> &str {
        "continuity-watcher"
    }

    fn subscribed_events(&self) -> Vec<String> {
        vec!["idle_tick".into()]
    }

    async fn process(&self, event: EditorEvent, _cancel: CancellationToken) -> Option<AgentOutput> {
        let EditorEvent::IdleTick {
            chapter,
            paragraph,
            cursor_position,
            ..
        } = event
        else {
            return None;
        };

        if paragraph.chars().count() < 20 {
            return None;
        }

        if let Ok(entries) = crate::storage::load_lorebook(&self.app) {
            for entry in entries {
                if !entry.keyword.trim().is_empty()
                    && paragraph.contains(&entry.keyword)
                    && paragraph.contains("矛盾")
                {
                    let from = cursor_position.saturating_sub(paragraph.chars().count());
                    return Some(AgentOutput::SemanticLint {
                        message: format!(
                            "{}：{} 相关段落出现矛盾提示，请核对设定。",
                            chapter, entry.keyword
                        ),
                        from,
                        to: cursor_position,
                        severity: "warning".to_string(),
                    });
                }
            }
        }

        if ["明明", "不可能", "从未", "却又"]
            .iter()
            .any(|marker| paragraph.contains(marker))
        {
            let from = cursor_position.saturating_sub(paragraph.chars().count());
            return Some(AgentOutput::HoverHint {
                message: format!("{}：这里可能需要检查前后因果或人物状态。", chapter),
                from,
                to: cursor_position,
            });
        }

        None
    }
}
