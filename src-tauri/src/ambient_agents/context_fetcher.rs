use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use agent_harness_core::ambient::{AmbientAgent, AgentOutput, EditorEvent};
use async_trait::async_trait;

/// Shared context cache readable by all ambient agents.
/// Populated silently by ContextFetcherAgent on keyword/character detection.
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
    fn name(&self) -> &str {
        "context-fetcher"
    }

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
                    if cache.lore_entries.contains_key(kw) {
                        continue;
                    }
                    if let Ok(entries) = crate::storage::load_lorebook(&self.app) {
                        let matches: Vec<String> = entries
                            .iter()
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
                    .unwrap()
                    .as_millis() as u64;
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
        None // Pure background caching — no UI output
    }
}
