/// 标准化 Action 类型 — 前后端双向通信协议
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Insert { content: String },
    Replace { content: String },
    Search { keyword: String },
}

/// 从流式文本中提取所有 Action 标签 (XML 格式)
/// 支持: <ACTION_INSERT>, <ACTION_REPLACE>, <ACTION_SEARCH>
pub fn parse_actions(text: &str) -> (Vec<Action>, String) {
    let mut actions = Vec::new();
    let mut clean = String::with_capacity(text.len());
    let mut remaining = text;
    let tags = ["ACTION_INSERT", "ACTION_REPLACE", "ACTION_SEARCH"];

    while !remaining.is_empty() {
        let mut earliest: Option<(usize, &str, usize)> = None;
        for tag in &tags {
            let open = format!("<{}>", tag);
            if let Some(pos) = remaining.find(&open) {
                if earliest.is_none_or(|(p, _, _)| pos < p) {
                    earliest = Some((pos, tag, open.len()));
                }
            }
        }

        match earliest {
            None => {
                clean.push_str(remaining);
                break;
            }
            Some((pos, tag, open_len)) => {
                clean.push_str(&remaining[..pos]);
                let after_open = &remaining[pos + open_len..];
                let close = format!("</{}>", tag);
                if let Some(end) = after_open.find(&close) {
                    let content = after_open[..end].to_string();
                    match tag {
                        "ACTION_INSERT" => actions.push(Action::Insert { content }),
                        "ACTION_REPLACE" => actions.push(Action::Replace { content }),
                        "ACTION_SEARCH" => actions.push(Action::Search { keyword: content }),
                        _ => {}
                    }
                    remaining = &after_open[end + close.len()..];
                } else {
                    // Incomplete tag — treat as literal text
                    clean.push_str(&remaining[pos..pos + open_len]);
                    remaining = after_open;
                }
            }
        }
    }

    (actions, clean)
}

/// 从文本中提取第一个 ACTION_SEARCH 关键词 (用于流中断检测)
pub fn extract_search_action(text: &str) -> Option<String> {
    let tag = "<ACTION_SEARCH>";
    let end_tag = "</ACTION_SEARCH>";
    if let Some(start) = text.find(tag) {
        let content_start = start + tag.len();
        if let Some(end) = text[content_start..].find(end_tag) {
            return Some(text[content_start..content_start + end].trim().to_string());
        }
    }
    None
}
