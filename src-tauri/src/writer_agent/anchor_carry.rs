//! Anchor carry scoring for real-author calibration.
//!
//! Mention rate only proves that a draft repeated a term. Carry scoring checks
//! whether named anchors participate in scene action, dialogue, consequence, or
//! payoff pressure. It is a deterministic heuristic signal, not a replacement
//! for author review.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnchorCarryReport {
    pub anchor_count: u64,
    pub mentioned_count: u64,
    pub carried_count: u64,
    pub mention_rate: f64,
    pub carry_rate: f64,
    pub items: Vec<AnchorCarryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnchorCarryItem {
    pub anchor: String,
    pub mentioned: bool,
    pub carried: bool,
    pub carry_modes: Vec<String>,
    pub supporting_terms: Vec<String>,
}

const ACTION_TERMS: &[&str] = &[
    "拔",
    "握",
    "递",
    "交",
    "救",
    "追",
    "挡",
    "打开",
    "藏",
    "拿",
    "看",
    "盯",
    "亮出",
    "压",
    "斩",
    "换",
    "插",
    "转",
    "抢",
    "护",
    "逼问",
    "承认",
    "选择",
    "摊",
    "展开",
    "翻",
    "读",
    "放回",
    "递出去",
    "逼",
    "追问",
];

const DIALOGUE_TERMS: &[&str] = &[
    "\"", "“", "”", "说", "问", "喊", "答", "承认", "道", "低声", "开口", "接话",
];

const CONSEQUENCE_TERMS: &[&str] = &[
    "因此",
    "于是",
    "导致",
    "逼得",
    "只好",
    "选择",
    "决定",
    "代价",
    "后果",
    "失去",
    "换来",
    "发现",
    "意识到",
    "确认",
    "暴露",
    "牵出",
    "重新",
    "不敢",
    "被迫",
];

const PAYOFF_PRESSURE_TERMS: &[&str] = &[
    "要还", "还债", "偿还", "清算", "兑现", "伏笔", "真相", "账册", "代价", "承诺", "线索", "缺页",
    "入口", "交易", "选择", "信任", "背叛", "道歉", "秘密", "谜底", "追债",
];

pub fn score_anchor_carry(text: &str, anchors: &[String]) -> AnchorCarryReport {
    let sentences = split_sentences(text);
    let items = anchors
        .iter()
        .filter_map(|anchor| {
            let anchor = anchor.trim();
            if anchor.is_empty() {
                return None;
            }
            Some(score_anchor(anchor, &sentences))
        })
        .collect::<Vec<_>>();

    let anchor_count = items.len() as u64;
    let mentioned_count = items.iter().filter(|item| item.mentioned).count() as u64;
    let carried_count = items.iter().filter(|item| item.carried).count() as u64;

    AnchorCarryReport {
        anchor_count,
        mentioned_count,
        carried_count,
        mention_rate: ratio(mentioned_count, anchor_count),
        carry_rate: ratio(carried_count, anchor_count),
        items,
    }
}

fn score_anchor(anchor: &str, sentences: &[String]) -> AnchorCarryItem {
    let mut carry_modes = Vec::new();
    let mut supporting_terms = Vec::new();
    let mut mentioned = false;

    for sentence in sentences
        .iter()
        .filter(|sentence| sentence.contains(anchor))
    {
        mentioned = true;
        collect_mode(
            sentence,
            "action",
            ACTION_TERMS,
            &mut carry_modes,
            &mut supporting_terms,
        );
        collect_mode(
            sentence,
            "dialogue",
            DIALOGUE_TERMS,
            &mut carry_modes,
            &mut supporting_terms,
        );
        collect_mode(
            sentence,
            "consequence",
            CONSEQUENCE_TERMS,
            &mut carry_modes,
            &mut supporting_terms,
        );
        collect_mode(
            sentence,
            "payoff_pressure",
            PAYOFF_PRESSURE_TERMS,
            &mut carry_modes,
            &mut supporting_terms,
        );
    }

    carry_modes.sort();
    carry_modes.dedup();
    supporting_terms.sort();
    supporting_terms.dedup();

    AnchorCarryItem {
        anchor: anchor.to_string(),
        mentioned,
        carried: !carry_modes.is_empty(),
        carry_modes,
        supporting_terms,
    }
}

fn collect_mode(
    sentence: &str,
    mode: &str,
    terms: &[&str],
    carry_modes: &mut Vec<String>,
    supporting_terms: &mut Vec<String>,
) {
    let matched = terms
        .iter()
        .filter(|term| sentence.contains(**term))
        .take(3)
        .copied()
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return;
    }
    carry_modes.push(mode.to_string());
    supporting_terms.extend(matched.into_iter().map(str::to_string));
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '!' | '?' | ';' | '；' | '\n') {
            push_sentence(&mut sentences, &mut current);
        }
    }
    push_sentence(&mut sentences, &mut current);
    sentences
}

fn push_sentence(sentences: &mut Vec<String>, current: &mut String) {
    let sentence = current.trim();
    if !sentence.is_empty() {
        sentences.push(sentence.to_string());
    }
    current.clear();
}

fn ratio(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchors() -> Vec<String> {
        ["寒影刀", "张三", "镜中墟", "旧债"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn carry_score_distinguishes_bare_mentions() {
        let report = score_anchor_carry("本章出现寒影刀、张三、镜中墟和旧债。", &anchors());

        assert_eq!(report.mentioned_count, 4);
        assert_eq!(report.carried_count, 0);
        assert_eq!(report.mention_rate, 1.0);
        assert_eq!(report.carry_rate, 0.0);
    }

    #[test]
    fn carry_score_detects_action_dialogue_and_payoff_pressure() {
        let report = score_anchor_carry(
            "林墨拔出寒影刀逼问张三：“旧债今天要还。”镜中墟的门因此重新打开。",
            &anchors(),
        );

        assert_eq!(report.mentioned_count, 4);
        assert!(report.carried_count >= 3);
        assert!(report.carry_rate >= 0.75);
        assert!(report.items.iter().any(
            |item| item.anchor == "寒影刀" && item.carry_modes.contains(&"action".to_string())
        ));
    }
}
