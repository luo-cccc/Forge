pub(crate) fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '\n') {
            let trimmed = current.trim();
            if trimmed.chars().count() >= 6 {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    let trimmed = current.trim();
    if trimmed.chars().count() >= 6 {
        sentences.push(trimmed.to_string());
    }
    sentences
}

fn extract_name_after(sentence: &str, cue: &str) -> Option<String> {
    let cue_byte = sentence.find(cue)?;
    let after = &sentence[cue_byte + cue.len()..];
    let name: String = after
        .chars()
        .skip_while(|c| c.is_whitespace() || matches!(c, '“' | '"' | '\'' | '：' | ':'))
        .take_while(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(c))
        .take(6)
        .collect();
    let count = name.chars().count();
    if (2..=6).contains(&count) {
        Some(name)
    } else {
        None
    }
}

fn should_keep_entity(name: &str, known: &[String], existing: &[CanonEntityOp]) -> bool {
    let name = name.trim();
    !name.is_empty()
        && !known.iter().any(|item| item == name)
        && !existing.iter().any(|item| item.name == name)
}

fn extract_related_entities(sentence: &str) -> Vec<String> {
    let mut entities = Vec::new();
    for marker in [
        "林墨",
        "张三",
        "玉佩",
        "长刀",
        "戒指",
        "密信",
        "钥匙",
        "令牌",
        "黑衣人",
        "旧门",
    ] {
        if sentence.contains(marker) {
            entities.push(marker.to_string());
        }
    }
    if entities.is_empty() {
        entities.push("unknown".to_string());
    }
    entities.truncate(3);
    entities
}

fn contains_promise_cue(sentence: &str) -> bool {
    [
        "还没",
        "尚未",
        "迟早",
        "总有一天",
        "秘密",
        "谜",
        "真相",
        "下落",
        "没有说出口",
        "没有告诉",
        "约定",
        "承诺",
        "发誓",
        "一定会",
        "等着",
        "留给",
        "交给",
        "带走",
        "失踪",
        "不见",
        "消失",
        "藏",
        "隐瞒",
    ]
    .iter()
    .any(|cue| sentence.contains(cue))
}

fn promise_kind_from_cues(sentence: &str) -> PromiseKind {
    let s = sentence;
    if s.contains("下落") || s.contains("不见") || s.contains("消失") || s.contains("带走")
    {
        PromiseKind::ObjectWhereabouts
    } else if s.contains("秘密") || s.contains("谜") || s.contains("真相") || s.contains("隐瞒")
    {
        PromiseKind::MysteryClue
    } else if s.contains("约定") || s.contains("承诺") || s.contains("发誓") || s.contains("等着")
    {
        PromiseKind::CharacterCommitment
    } else if s.contains("没有说出口") || s.contains("没有告诉") || s.contains("藏") {
        PromiseKind::EmotionalDebt
    } else {
        PromiseKind::PlotPromise
    }
}

fn promise_title(sentence: &str) -> String {
    for marker in [
        "玉佩", "密信", "钥匙", "令牌", "真相", "秘密", "下落", "戒指", "剑", "刀", "信物", "地图",
        "药", "毒",
    ] {
        if sentence.contains(marker) {
            return marker.to_string();
        }
    }
    sentence
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(12)
        .collect()
}

pub(crate) fn sentence_snippet(sentence: &str, limit: usize) -> String {
    sentence
        .trim_matches(|c: char| c.is_whitespace())
        .chars()
        .take(limit)
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum MemoryCandidateQuality {
    Acceptable,
    Vague {
        reason: String,
    },
    Duplicate {
        existing_name: String,
    },
    MergeableAttributes {
        existing_name: String,
        attributes: Vec<(String, String)>,
    },
    Conflict {
        existing_name: String,
        reason: String,
    },
}

pub fn validate_canon_candidate(candidate: &CanonEntityOp) -> MemoryCandidateQuality {
    let name = candidate.name.trim();
    if name.chars().count() < 2 {
        return MemoryCandidateQuality::Vague {
            reason: "entity name too short (min 2 chars)".to_string(),
        };
    }
    let summary = candidate.summary.trim();
    if summary.chars().count() < 8 {
        return MemoryCandidateQuality::Vague {
            reason: format!(
                "entity summary too short ({} chars, min 8)",
                summary.chars().count()
            ),
        };
    }
    MemoryCandidateQuality::Acceptable
}

pub fn validate_canon_candidate_with_memory(
    candidate: &CanonEntityOp,
    memory: &WriterMemory,
) -> MemoryCandidateQuality {
    let quality = validate_canon_candidate(candidate);
    if quality != MemoryCandidateQuality::Acceptable {
        return quality;
    }

    let Some(existing) = find_existing_canon_entity(candidate, memory) else {
        return MemoryCandidateQuality::Acceptable;
    };

    if existing.kind.trim() != candidate.kind.trim() {
        return MemoryCandidateQuality::Conflict {
            existing_name: existing.name.clone(),
            reason: format!(
                "kind differs for existing canon '{}': existing={}, candidate={}",
                existing.name, existing.kind, candidate.kind
            ),
        };
    }

    if let Some((attribute, existing_value, candidate_value)) =
        conflicting_canon_attribute(candidate, &existing)
    {
        return MemoryCandidateQuality::Conflict {
            existing_name: existing.name.clone(),
            reason: format!(
                "{}.{} conflicts: existing={}, candidate={}",
                existing.name, attribute, existing_value, candidate_value
            ),
        };
    }

    let mergeable_attributes = mergeable_canon_attributes(candidate, &existing);
    if !mergeable_attributes.is_empty() {
        return MemoryCandidateQuality::MergeableAttributes {
            existing_name: existing.name,
            attributes: mergeable_attributes,
        };
    }

    MemoryCandidateQuality::Duplicate {
        existing_name: existing.name,
    }
}

fn find_existing_canon_entity(
    candidate: &CanonEntityOp,
    memory: &WriterMemory,
) -> Option<CanonEntitySummary> {
    let mut names = Vec::with_capacity(candidate.aliases.len() + 1);
    names.push(candidate.name.trim().to_string());
    names.extend(
        candidate
            .aliases
            .iter()
            .map(|alias| alias.trim().to_string()),
    );

    let resolved = names
        .into_iter()
        .filter(|name| !name.is_empty())
        .filter_map(|name| memory.resolve_canon_entity_name(&name).ok().flatten())
        .collect::<HashSet<_>>();
    if resolved.is_empty() {
        return None;
    }

    memory
        .list_canon_entities()
        .ok()?
        .into_iter()
        .find(|entity| resolved.contains(&entity.name))
}

fn conflicting_canon_attribute(
    candidate: &CanonEntityOp,
    existing: &CanonEntitySummary,
) -> Option<(String, String, String)> {
    let candidate_attributes = candidate.attributes.as_object()?;
    let existing_attributes = existing.attributes.as_object()?;

    for (attribute, candidate_value) in candidate_attributes {
        let Some(candidate_text) = canon_attribute_value(candidate_value) else {
            continue;
        };
        let Some(existing_text) = existing_attributes
            .get(attribute)
            .and_then(canon_attribute_value)
        else {
            continue;
        };
        if existing_text != candidate_text {
            return Some((attribute.clone(), existing_text, candidate_text));
        }
    }

    None
}

fn mergeable_canon_attributes(
    candidate: &CanonEntityOp,
    existing: &CanonEntitySummary,
) -> Vec<(String, String)> {
    let Some(candidate_attributes) = candidate.attributes.as_object() else {
        return Vec::new();
    };
    let existing_attributes = existing.attributes.as_object().cloned().unwrap_or_default();

    candidate_attributes
        .iter()
        .filter_map(|(attribute, candidate_value)| {
            let candidate_text = canon_attribute_value(candidate_value)?;
            if existing_attributes.contains_key(attribute) {
                return None;
            }
            Some((attribute.clone(), candidate_text))
        })
        .collect()
}

fn canon_attribute_value(value: &serde_json::Value) -> Option<String> {
    let text = match value {
        serde_json::Value::Null => return None,
        serde_json::Value::String(value) => value.trim().to_string(),
        serde_json::Value::Array(values) if values.is_empty() => return None,
        serde_json::Value::Object(values) if values.is_empty() => return None,
        other => other.to_string(),
    };

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn validate_promise_candidate(candidate: &PlotPromiseOp) -> MemoryCandidateQuality {
    let title = candidate.title.trim();
    if title.chars().count() < 2 {
        return MemoryCandidateQuality::Vague {
            reason: "promise title too short (min 2 chars)".to_string(),
        };
    }
    let description = candidate.description.trim();
    if description.chars().count() < 8 {
        return MemoryCandidateQuality::Vague {
            reason: format!(
                "promise description too short ({} chars, min 8)",
                description.chars().count()
            ),
        };
    }
    MemoryCandidateQuality::Acceptable
}

pub fn validate_promise_candidate_with_dedup(
    candidate: &PlotPromiseOp,
    memory: &WriterMemory,
) -> MemoryCandidateQuality {
    let quality = validate_promise_candidate(candidate);
    if quality != MemoryCandidateQuality::Acceptable {
        return quality;
    }
    if let Ok(existing) = memory.get_open_promise_summaries() {
        if existing
            .iter()
            .any(|p| p.title.trim() == candidate.title.trim())
        {
            return MemoryCandidateQuality::Duplicate {
                existing_name: candidate.title.clone(),
            };
        }
    }
    MemoryCandidateQuality::Acceptable
}

pub fn validate_style_preference(key: &str, value: &str) -> MemoryCandidateQuality {
    let key = key.trim();
    if key.chars().count() < 3 {
        return MemoryCandidateQuality::Vague {
            reason: "style key too short (min 3 chars)".to_string(),
        };
    }
    let value = value.trim();
    let slot = style_preference_slot(key, value);
    if is_vague_style_key(key) && slot.is_none() && !is_feedback_style_key(key) {
        return MemoryCandidateQuality::Vague {
            reason: format!("style key '{}' is too generic", key),
        };
    }

    let value_chars = value.chars().count();
    if value_chars < 6 {
        return MemoryCandidateQuality::Vague {
            reason: format!("style preference too short ({} chars, min 6)", value_chars),
        };
    }
    if is_vague_style_value(value) {
        return MemoryCandidateQuality::Vague {
            reason: format!("style preference '{}' is too generic", value),
        };
    }

    MemoryCandidateQuality::Acceptable
}

pub fn style_preference_taxonomy_slot(key: &str, value: &str) -> Option<String> {
    style_preference_slot(key.trim(), value.trim()).map(|slot| slot.label())
}

pub fn validate_style_preference_with_memory(
    key: &str,
    value: &str,
    memory: &WriterMemory,
) -> MemoryCandidateQuality {
    let quality = validate_style_preference(key, value);
    if quality != MemoryCandidateQuality::Acceptable {
        return quality;
    }

    let key = key.trim();
    let value = value.trim();
    let existing_preferences = memory.list_style_preferences(200).unwrap_or_default();
    if let Some(existing) = existing_preferences
        .iter()
        .find(|preference| preference.key.trim().eq_ignore_ascii_case(key))
    {
        return classify_existing_style_preference(existing, value);
    }

    let Some(candidate_slot) = comparable_style_preference_slot(key, value) else {
        return MemoryCandidateQuality::Acceptable;
    };
    if let Some(existing) = existing_preferences.iter().find(|preference| {
        comparable_style_preference_slot(&preference.key, &preference.value)
            .is_some_and(|existing_slot| existing_slot == candidate_slot)
    }) {
        return classify_existing_style_taxonomy_preference(existing, value, candidate_slot);
    }

    MemoryCandidateQuality::Acceptable
}

fn classify_existing_style_preference(
    existing: &StylePreferenceSummary,
    value: &str,
) -> MemoryCandidateQuality {
    if existing.value.trim() == value {
        MemoryCandidateQuality::Duplicate {
            existing_name: existing.key.clone(),
        }
    } else if style_preference_polarity(&existing.value) == style_preference_polarity(value) {
        MemoryCandidateQuality::Acceptable
    } else {
        MemoryCandidateQuality::Conflict {
            existing_name: existing.key.clone(),
            reason: format!(
                "style preference '{}' conflicts: existing={}, candidate={}",
                existing.key, existing.value, value
            ),
        }
    }
}

fn classify_existing_style_taxonomy_preference(
    existing: &StylePreferenceSummary,
    value: &str,
    slot: StylePreferenceSlot,
) -> MemoryCandidateQuality {
    if existing.value.trim() == value {
        MemoryCandidateQuality::Duplicate {
            existing_name: existing.key.clone(),
        }
    } else if style_preference_polarity(&existing.value) == style_preference_polarity(value) {
        MemoryCandidateQuality::Acceptable
    } else {
        MemoryCandidateQuality::Conflict {
            existing_name: existing.key.clone(),
            reason: format!(
                "style taxonomy slot '{}' already has a preference: existing={}, candidate={}",
                slot.label(),
                existing.value,
                value
            ),
        }
    }
}

pub fn style_preference_memory_key(key: &str, value: &str) -> String {
    comparable_style_preference_slot(key, value)
        .map(|slot| format!("style:{}", slot.label()))
        .unwrap_or_else(|| key.trim().to_string())
}

fn is_vague_style_key(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    matches!(
        key.as_str(),
        "style" | "tone" | "voice" | "writing" | "preference" | "good" | "bad" | "风格" | "语气"
    )
}

fn is_feedback_style_key(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    key.starts_with("accepted_")
        || key.starts_with("ignored_")
        || key.starts_with("rejected_")
        || key.starts_with("memory_extract:")
}

fn is_vague_style_value(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "good" | "bad" | "better" | "nice" | "ok" | "accepted" | "rejected" | "not my style"
    ) {
        return true;
    }
    matches!(
        value.trim(),
        "好" | "不好" | "更好" | "不错" | "可以" | "喜欢" | "不喜欢" | "有感觉" | "没感觉"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StylePreferencePolarity {
    Prefer,
    Avoid,
    Neutral,
}

fn style_preference_polarity(value: &str) -> StylePreferencePolarity {
    let normalized = value.trim().to_ascii_lowercase();
    if contains_any(
        &normalized,
        &[
            "避免", "不要", "少", "减少", "降低", "拒绝", "别", "不再", "别再", "少用", "克制",
            "avoid", "less", "reduce", "reject", "without", "no ",
        ],
    ) {
        return StylePreferencePolarity::Avoid;
    }
    if contains_any(
        &normalized,
        &[
            "偏",
            "优先",
            "保留",
            "增加",
            "更多",
            "多",
            "强化",
            "强调",
            "喜欢",
            "倾向",
            "prefer",
            "more",
            "keep",
            "increase",
            "emphasize",
        ],
    ) {
        return StylePreferencePolarity::Prefer;
    }
    StylePreferencePolarity::Neutral
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StylePreferenceSlot {
    dimension: &'static str,
    axis: &'static str,
    comparable: bool,
}

impl StylePreferenceSlot {
    fn label(self) -> String {
        format!("{}.{}", self.dimension, self.axis)
    }
}

fn comparable_style_preference_slot(key: &str, value: &str) -> Option<StylePreferenceSlot> {
    style_preference_slot(key, value).filter(|slot| slot.comparable)
}

fn style_preference_slot(key: &str, value: &str) -> Option<StylePreferenceSlot> {
    let key_lower = key.trim().to_ascii_lowercase();
    if is_feedback_style_key(&key_lower) {
        return Some(StylePreferenceSlot {
            dimension: "feedback",
            axis: "proposal",
            comparable: false,
        });
    }

    let value_lower = value.trim().to_ascii_lowercase();
    let combined = format!("{} {}", key_lower, value_lower);

    if contains_any(&combined, &["dialogue", "dialog", "对话", "台词", "对白"])
        && contains_any(
            &combined,
            &[
                "subtext",
                "潜台词",
                "留白",
                "解释情绪",
                "真实情绪",
                "情绪解释",
                "direct emotion",
                "explain emotion",
            ],
        )
    {
        return Some(StylePreferenceSlot {
            dimension: "dialogue",
            axis: "subtext",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &[
            "sentence_length",
            "sentence length",
            "short sentence",
            "long sentence",
            "短句",
            "长句",
            "句长",
            "断句",
        ],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "prose",
            axis: "sentence_length",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &[
            "exposition",
            "info_dump",
            "infodump",
            "information density",
            "说明",
            "信息量",
            "背景交代",
            "解释设定",
        ],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "exposition",
            axis: "density",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &[
            "description",
            "sensory",
            "image",
            "描写",
            "感官",
            "气味",
            "触感",
            "画面",
        ],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "description",
            axis: "sensory_detail",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &[
            "pov",
            "point_of_view",
            "point of view",
            "视角",
            "内心",
            "旁白",
        ],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "pov",
            axis: "distance",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &["action", "combat", "fight", "动作", "打斗", "追逐", "交锋"],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "action",
            axis: "clarity",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &[
            "chapter_hook",
            "hook",
            "cliffhanger",
            "悬念",
            "钩子",
            "章尾",
            "转折",
        ],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "structure",
            axis: "hook",
            comparable: true,
        });
    }

    if contains_any(
        &combined,
        &[
            "tone", "voice", "语气", "风格", "冷峻", "克制", "轻松", "幽默", "吐槽",
        ],
    ) {
        return Some(StylePreferenceSlot {
            dimension: "tone",
            axis: "voice",
            comparable: true,
        });
    }

    None
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}
