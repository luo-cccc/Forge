//! Writing relevance ranking for context ledger slices.

use std::collections::HashSet;

use super::memory::{CanonEntitySummary, CreativeDecisionSummary, PlotPromiseSummary};
use super::observation::WriterObservation;

pub(crate) struct WritingRelevance {
    cursor_text: String,
    foundation_text: String,
    cursor_terms: Vec<String>,
    foundation_terms: Vec<String>,
}

#[derive(Default)]
pub(crate) struct RelevanceScore {
    pub score: i32,
    pub reasons: Vec<String>,
}

impl RelevanceScore {
    fn add(&mut self, score: i32, reason: impl Into<String>) {
        self.score += score;
        let reason = reason.into();
        if !reason.trim().is_empty() && !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }
}

impl WritingRelevance {
    pub(crate) fn new(
        observation: &WriterObservation,
        chapter_mission: &str,
        next_beat: &str,
        result_feedback: &str,
        decision_slice: &str,
    ) -> Self {
        let cursor_text = [
            observation.prefix.as_str(),
            observation.paragraph.as_str(),
            observation.suffix.as_str(),
            observation.selected_text(),
        ]
        .join("\n");
        let foundation_text = [chapter_mission, next_beat, result_feedback, decision_slice]
            .into_iter()
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            cursor_terms: relevance_terms(&cursor_text),
            foundation_terms: relevance_terms(&foundation_text),
            cursor_text,
            foundation_text,
        }
    }

    fn cursor_contains(&self, needle: &str) -> bool {
        !needle.trim().is_empty() && self.cursor_text.contains(needle)
    }

    fn foundation_contains(&self, needle: &str) -> bool {
        !needle.trim().is_empty() && self.foundation_text.contains(needle)
    }
}

pub fn rerank_text_chunks<'a, T, F>(
    chunks: Vec<(f32, T)>,
    writing_focus: &str,
    text_for_chunk: F,
) -> Vec<(f32, Vec<String>, T)>
where
    F: Fn(&T) -> String,
{
    let focus = WritingRelevanceFocus::new(writing_focus);
    let mut scored = chunks
        .into_iter()
        .map(|(base_score, chunk)| {
            let text = text_for_chunk(&chunk);
            let relevance = score_text_chunk(&focus, &text);
            let combined = base_score + relevance.score as f32;
            (combined, relevance.reasons, chunk)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

pub fn format_text_chunk_relevance(reasons: &[String]) -> String {
    let reason = if reasons.is_empty() {
        "semantic similarity".to_string()
    } else {
        relevance_reason_text(reasons)
    };
    format!("WHY writing_relevance: {}", reason)
}

pub(crate) fn score_canon_entity(
    entity: &CanonEntitySummary,
    observation: &WriterObservation,
    relevance: &WritingRelevance,
    open_promises: &[PlotPromiseSummary],
) -> RelevanceScore {
    let mut score = RelevanceScore::default();
    let entity_text = canon_entity_text(entity);
    if relevance.cursor_contains(&entity.name) {
        score.add(90, format!("cursor mentions entity {}", entity.name));
    }
    if relevance.foundation_contains(&entity.name) {
        score.add(
            70,
            format!("mission/result mentions entity {}", entity.name),
        );
    }

    for term in relevance
        .cursor_terms
        .iter()
        .filter(|term| entity_text.contains(term.as_str()))
        .take(3)
    {
        score.add(16, format!("cursor term {}", term));
    }
    for term in relevance
        .foundation_terms
        .iter()
        .filter(|term| entity_text.contains(term.as_str()))
        .take(3)
    {
        score.add(12, format!("foundation term {}", term));
    }

    for promise in open_promises.iter().take(12) {
        let promise_text = promise_text(promise);
        if promise_text.contains(&entity.name) || entity_text.contains(&promise.title) {
            score.add(24, format!("linked open promise {}", promise.title));
        }
    }

    if score.score == 0 && observation.paragraph.contains(&entity.summary) {
        score.add(8, "cursor overlaps entity summary");
    }
    score
}

pub(crate) fn score_promise(
    promise: &PlotPromiseSummary,
    observation: &WriterObservation,
    relevance: &WritingRelevance,
    decisions: &[CreativeDecisionSummary],
) -> RelevanceScore {
    let mut score = RelevanceScore::default();
    score.add(
        promise.priority.clamp(0, 10),
        format!("ledger priority {}", promise.priority),
    );
    if relevance.cursor_contains(&promise.title) {
        score.add(90, format!("cursor mentions promise {}", promise.title));
    }
    if relevance.foundation_contains(&promise.title) {
        score.add(
            70,
            format!("mission/result mentions promise {}", promise.title),
        );
    }
    if observation
        .chapter_title
        .as_deref()
        .is_some_and(|chapter| !chapter.is_empty() && promise.expected_payoff.contains(chapter))
    {
        score.add(42, "current chapter is expected payoff");
    }

    let promise_text = promise_text(promise);
    for term in relevance
        .cursor_terms
        .iter()
        .filter(|term| promise_text.contains(term.as_str()))
        .take(4)
    {
        score.add(15, format!("cursor term {}", term));
    }
    for term in relevance
        .foundation_terms
        .iter()
        .filter(|term| promise_text.contains(term.as_str()))
        .take(4)
    {
        score.add(12, format!("foundation term {}", term));
    }
    for decision in decisions.iter().take(6) {
        let decision_text = format!("{} {}", decision.title, decision.rationale);
        if decision_text.contains(&promise.title)
            || promise_text_contains_terms(&promise_text, &decision_text)
        {
            score.add(18, format!("recent decision {}", decision.title));
        }
    }

    score
}

pub(crate) fn format_canon_line(entity: &CanonEntitySummary, reasons: &[String]) -> String {
    let attrs = canon_attributes_text(entity);
    format!(
        "WHY writing_relevance: {} | {} [{}] {} {}",
        relevance_reason_text(reasons),
        entity.name,
        entity.kind,
        entity.summary,
        attrs
    )
}

pub(crate) fn format_promise_line(promise: &PlotPromiseSummary, reasons: &[String]) -> String {
    let mut line = format!(
        "WHY writing_relevance: {} | {} [{}]: {} -> {}",
        relevance_reason_text(reasons),
        promise.title,
        promise.kind,
        promise.description,
        promise.expected_payoff
    );
    if !promise.last_seen_chapter.trim().is_empty() {
        line.push_str(&format!(" | last seen: {}", promise.last_seen_chapter));
    }
    line
}

fn promise_text_contains_terms(promise_text: &str, decision_text: &str) -> bool {
    relevance_terms(decision_text)
        .into_iter()
        .take(6)
        .any(|term| promise_text.contains(&term))
}

struct WritingRelevanceFocus {
    raw_text: String,
    terms: Vec<String>,
}

impl WritingRelevanceFocus {
    fn new(text: &str) -> Self {
        Self {
            raw_text: text.to_string(),
            terms: relevance_terms(text),
        }
    }
}

fn score_text_chunk(focus: &WritingRelevanceFocus, text: &str) -> RelevanceScore {
    let mut score = RelevanceScore::default();
    for term in focus
        .terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
    {
        let weight = if focus.raw_text.contains(term.as_str()) {
            1
        } else {
            0
        };
        let points = 18 + (term.chars().count().min(8) as i32 * 2) + weight;
        score.add(points, format!("writing term {}", term));
        if score.reasons.len() >= 5 {
            break;
        }
    }
    if focus.raw_text.contains("不要") || focus.raw_text.contains("不得") {
        for term in relevance_terms(&focus.raw_text)
            .into_iter()
            .filter(|term| text.contains(term))
            .take(2)
        {
            score.add(10, format!("constraint term {}", term));
        }
    }
    score
}

fn relevance_reason_text(reasons: &[String]) -> String {
    if reasons.is_empty() {
        "ledger priority".to_string()
    } else {
        reasons
            .iter()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn canon_attributes_text(entity: &CanonEntitySummary) -> String {
    entity
        .attributes
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(key, value)| format!("{}={}", key, value))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn canon_entity_text(entity: &CanonEntitySummary) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        entity.name,
        entity.kind,
        entity.summary,
        canon_attributes_text(entity)
    )
}

fn promise_text(promise: &PlotPromiseSummary) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        promise.title,
        promise.kind,
        promise.description,
        promise.introduced_chapter,
        promise.last_seen_chapter,
        promise.expected_payoff
    )
}

fn relevance_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for marker in [
        "寒玉戒指",
        "寒影刀",
        "黑衣人",
        "北境林墨",
        "南境林墨",
        "玉佩",
        "密信",
        "钥匙",
        "令牌",
        "真相",
        "秘密",
        "下落",
        "来源",
        "林墨",
        "张三",
        "北境",
        "南境",
        "宗门",
        "朝堂",
        "旧门",
        "戒指",
        "长剑",
        "信任",
        "怀疑",
        "关系",
        "承诺",
        "誓言",
        "冲突",
        "危机",
    ] {
        if text.contains(marker) && seen.insert(marker.to_string()) {
            terms.push(marker.to_string());
        }
    }

    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&ch) {
            current.push(ch);
        } else {
            push_relevance_term(&mut terms, &mut seen, &current);
            current.clear();
        }
    }
    push_relevance_term(&mut terms, &mut seen, &current);
    terms
}

fn push_relevance_term(terms: &mut Vec<String>, seen: &mut HashSet<String>, raw: &str) {
    let term = raw.trim();
    let count = term.chars().count();
    if !(2..=10).contains(&count) || is_relevance_stopword(term) {
        return;
    }
    if seen.insert(term.to_string()) {
        terms.push(term.to_string());
    }
}

fn is_relevance_stopword(term: &str) -> bool {
    [
        "章节", "本章", "任务", "目标", "当前", "需要", "继续", "处理", "保持", "推进", "不要",
        "不得", "后续", "解释", "结果", "摘要", "状态", "变化", "新的", "明确", "作者", "accepted",
    ]
    .iter()
    .any(|stopword| term == *stopword)
}
