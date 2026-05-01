use agent_harness_core::ambient::{AgentOutput, AmbientAgent, EditorEvent, GhostCandidate};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::context_fetcher::ContextCache;

pub struct CoWriterAgent {
    pub cache: Arc<Mutex<ContextCache>>,
}

#[derive(Debug, Clone, Copy)]
enum WritingIntent {
    Dialogue,
    Instruction,
    Scenery,
    Action,
    Continuation,
}

struct LocalIntentClassifier {
    scenery_markers: &'static [&'static str],
    action_markers: &'static [&'static str],
}

const PREFIX_CONTEXT_CHARS: usize = 3000;
const SUFFIX_CONTEXT_CHARS: usize = 1000;
const LOCAL_INTENT_CLASSIFIER: LocalIntentClassifier = LocalIntentClassifier {
    scenery_markers: &["风景", "景色", "夜色", "街道", "房间", "描写", "环境"],
    action_markers: &["拔出", "冲向", "躲开", "挥", "追", "打", "砍", "跑"],
};

impl WritingIntent {
    fn as_str(self) -> &'static str {
        match self {
            WritingIntent::Dialogue => "dialogue",
            WritingIntent::Instruction => "instruction",
            WritingIntent::Scenery => "scenery",
            WritingIntent::Action => "action",
            WritingIntent::Continuation => "continuation",
        }
    }
}

impl LocalIntentClassifier {
    fn classify(&self, paragraph: &str) -> WritingIntent {
        let trimmed = paragraph.trim();
        if trimmed.ends_with("说道：\"")
            || trimmed.ends_with("说道：“")
            || trimmed.ends_with("问道：\"")
            || trimmed.ends_with("问道：“")
            || trimmed.ends_with('“')
        {
            return WritingIntent::Dialogue;
        }

        if (trimmed.starts_with('(') && trimmed.ends_with(')'))
            || (trimmed.starts_with('（') && trimmed.ends_with('）'))
        {
            return WritingIntent::Instruction;
        }

        if self
            .scenery_markers
            .iter()
            .any(|marker| trimmed.contains(marker))
        {
            return WritingIntent::Scenery;
        }

        if self
            .action_markers
            .iter()
            .any(|marker| trimmed.contains(marker))
        {
            return WritingIntent::Action;
        }

        WritingIntent::Continuation
    }
}

fn trim_chars_from_end(text: &str, max_chars: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(max_chars);
    chars[start..].iter().collect()
}

fn trim_chars_from_start(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn classify_writing_intent(paragraph: &str) -> WritingIntent {
    LOCAL_INTENT_CLASSIFIER.classify(paragraph)
}

fn branch_labels(intent: WritingIntent) -> [&'static str; 3] {
    match intent {
        WritingIntent::Dialogue => ["A 直接表态", "B 言语试探", "C 压住情绪"],
        WritingIntent::Instruction => ["A 完整落段", "B 克制短写", "C 氛围强化"],
        WritingIntent::Scenery => ["A 感官描写", "B 情绪映射", "C 动作带景"],
        WritingIntent::Action => ["A 快节奏", "B 细节拆解", "C 反转打断"],
        WritingIntent::Continuation => ["A 顺势推进", "B 内心转折", "C 外部打断"],
    }
}

#[async_trait]
impl AmbientAgent for CoWriterAgent {
    fn name(&self) -> &str {
        "co-writer"
    }

    fn subscribed_events(&self) -> Vec<String> {
        vec!["idle_tick".into()]
    }

    async fn process(&self, event: EditorEvent, cancel: CancellationToken) -> Option<AgentOutput> {
        if let EditorEvent::IdleTick {
            request_id,
            idle_ms,
            chapter,
            paragraph,
            prefix,
            suffix,
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
            let outline = cache.outline_map.get(&chapter).cloned().unwrap_or_default();

            let intent = classify_writing_intent(&paragraph);
            let labels = branch_labels(intent);
            let prefix_context = if prefix.trim().is_empty() {
                paragraph.clone()
            } else {
                trim_chars_from_end(&prefix, PREFIX_CONTEXT_CHARS)
            };
            let suffix_context = trim_chars_from_start(&suffix, SUFFIX_CONTEXT_CHARS);
            let prompt = format!(
                "你是中文小说写作助手。根据上下文从光标处生成三条不同方向的幽灵续写候选。\n\
                 当前意图：{}\n\
                 输出格式必须严格为三行：\n\
                 A: ...\nB: ...\nC: ...\n\
                 每条 1-2 句中文，不解释，不使用 Markdown。\n\
                 不要重复已存在的光标前文，也不要和光标后文冲突。\n\
                 ## 大纲\n{}\n## 设定\n{}\n## 光标前文\n{}\n## 光标后文\n{}\n## 当前段落\n{}\n## 候选",
                intent.as_str(),
                outline,
                lore_context,
                prefix_context,
                suffix_context,
                paragraph,
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
                let mut candidates = parse_candidates(&ghost, labels);
                if candidates.is_empty() {
                    candidates.push(GhostCandidate {
                        id: "a".to_string(),
                        label: labels[0].to_string(),
                        text: ghost.trim().to_string(),
                    });
                }
                return Some(AgentOutput::MultiGhost {
                    request_id,
                    position: cursor_position,
                    intent: intent.as_str().to_string(),
                    candidates,
                });
            }

            return Some(AgentOutput::GhostEnd {
                request_id,
                position: cursor_position,
                reason: if result.is_ok() { "complete" } else { "error" }.to_string(),
            });
        }
        None
    }
}

fn parse_candidates(raw: &str, labels: [&str; 3]) -> Vec<GhostCandidate> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some((head, body)) = trimmed.split_once(':').or_else(|| trimmed.split_once('：'))
        else {
            continue;
        };
        let idx = match head.trim().chars().next().map(|c| c.to_ascii_uppercase()) {
            Some('A') => 0,
            Some('B') => 1,
            Some('C') => 2,
            _ => continue,
        };
        let text = body.trim();
        if text.is_empty() {
            continue;
        }
        out.push(GhostCandidate {
            id: ["a", "b", "c"][idx].to_string(),
            label: labels[idx].to_string(),
            text: text.to_string(),
        });
    }
    out.truncate(3);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(paragraph: &str) -> &'static str {
        classify_writing_intent(paragraph).as_str()
    }

    #[test]
    fn classify_dialogue_prompt() {
        assert_eq!(intent("林墨深吸一口气，说道：“"), "dialogue");
        assert_eq!(intent("她停在门前，问道：\""), "dialogue");
    }

    #[test]
    fn classify_instruction_and_scenery() {
        assert_eq!(intent("（这里补充一段风景描写）"), "instruction");
        assert_eq!(intent("夜色压在街道尽头"), "scenery");
    }

    #[test]
    fn classify_action_and_continuation() {
        assert_eq!(intent("林墨拔出寒影刀，冲向门外"), "action");
        assert_eq!(intent("林墨沉默了很久"), "continuation");
    }

    #[test]
    fn parse_three_labeled_candidates() {
        let labels = branch_labels(WritingIntent::Dialogue);
        let candidates = parse_candidates(
            "A: “我不会走。”\nB：他垂下眼，只问了一句。\nC: 他把话咽回去。",
            labels,
        );

        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].id, "a");
        assert_eq!(candidates[0].label, "A 直接表态");
        assert_eq!(candidates[0].text, "“我不会走。”");
        assert_eq!(candidates[1].id, "b");
        assert_eq!(candidates[2].id, "c");
    }

    #[test]
    fn trim_context_windows_keep_cursor_edges() {
        let prefix = "一二三四五";
        let suffix = "六七八九十";

        assert_eq!(trim_chars_from_end(prefix, 3), "三四五");
        assert_eq!(trim_chars_from_start(suffix, 3), "六七八");
    }
}
