//! Writing relevance ranking for context ledger slices.

use std::collections::HashSet;

use super::memory::{CanonEntitySummary, CreativeDecisionSummary, PlotPromiseSummary};
use super::observation::WriterObservation;

pub(crate) struct WritingRelevance {
    cursor_text: String,
    foundation_text: String,
    cursor_terms: Vec<String>,
    foundation_terms: Vec<String>,
    cursor_scene_types: Vec<WritingSceneType>,
    foundation_scene_types: Vec<WritingSceneType>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WritingSceneType {
    Dialogue,
    Action,
    Description,
    EmotionalBeat,
    ConflictEscalation,
    Reveal,
    SetupPayoff,
    Exposition,
    Transition,
}

impl WritingSceneType {
    fn label(self) -> &'static str {
        match self {
            WritingSceneType::Dialogue => "dialogue",
            WritingSceneType::Action => "action",
            WritingSceneType::Description => "description",
            WritingSceneType::EmotionalBeat => "emotional_beat",
            WritingSceneType::ConflictEscalation => "conflict_escalation",
            WritingSceneType::Reveal => "reveal",
            WritingSceneType::SetupPayoff => "setup_payoff",
            WritingSceneType::Exposition => "exposition",
            WritingSceneType::Transition => "transition",
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
            cursor_scene_types: infer_scene_types(&cursor_text),
            foundation_scene_types: infer_scene_types(&foundation_text),
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

pub fn score_text_for_writing_focus(writing_focus: &str, text: &str) -> (f32, Vec<String>) {
    let focus = WritingRelevanceFocus::new(writing_focus);
    let relevance = score_text_chunk(&focus, text);
    (relevance.score as f32, relevance.reasons)
}

pub fn format_text_chunk_relevance(reasons: &[String]) -> String {
    let reason = if reasons.is_empty() {
        "semantic similarity".to_string()
    } else {
        relevance_reason_text(reasons)
    };
    format!("WHY writing_relevance: {}", reason)
}

pub fn writing_scene_types(text: &str) -> Vec<String> {
    infer_scene_types(text)
        .into_iter()
        .map(|scene_type| scene_type.label().to_string())
        .collect()
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
    let entity_scene_types = infer_scene_types(&entity_text);
    add_scene_type_scores(
        &mut score,
        &relevance.cursor_scene_types,
        &entity_scene_types,
        14,
        "cursor",
    );
    add_scene_type_scores(
        &mut score,
        &relevance.foundation_scene_types,
        &entity_scene_types,
        10,
        "foundation",
    );

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
    let mut promise_scene_types = infer_scene_types(&promise_text);
    for scene_type in promise_kind_scene_types(&promise.kind) {
        if !promise_scene_types.contains(&scene_type) {
            promise_scene_types.push(scene_type);
        }
    }
    add_scene_type_scores(
        &mut score,
        &relevance.cursor_scene_types,
        &promise_scene_types,
        16,
        "cursor",
    );
    add_scene_type_scores(
        &mut score,
        &relevance.foundation_scene_types,
        &promise_scene_types,
        12,
        "foundation",
    );
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
    negative_terms: Vec<String>,
    scene_types: Vec<WritingSceneType>,
}

impl WritingRelevanceFocus {
    fn new(text: &str) -> Self {
        let negative_terms = negative_relevance_terms(text);
        let terms = relevance_terms(text)
            .into_iter()
            .filter(|term| !negative_terms.contains(term))
            .collect();
        Self {
            raw_text: text.to_string(),
            terms,
            negative_terms,
            scene_types: infer_scene_types(text),
        }
    }
}

fn score_text_chunk(focus: &WritingRelevanceFocus, text: &str) -> RelevanceScore {
    let mut score = RelevanceScore::default();
    let chunk_scene_types = infer_scene_types(text);
    add_scene_type_scores(
        &mut score,
        &focus.scene_types,
        &chunk_scene_types,
        28,
        "focus",
    );
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
    for term in focus
        .negative_terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .take(3)
    {
        score.add(-72, format!("avoid term {}", term));
    }
    score
}

fn add_scene_type_scores(
    score: &mut RelevanceScore,
    focus_scene_types: &[WritingSceneType],
    candidate_scene_types: &[WritingSceneType],
    points: i32,
    prefix: &str,
) {
    for scene_type in focus_scene_types
        .iter()
        .filter(|scene_type| candidate_scene_types.contains(scene_type))
        .take(2)
    {
        score.add(
            points,
            format!("{} scene type {}", prefix, scene_type.label()),
        );
    }
}

fn promise_kind_scene_types(kind: &str) -> Vec<WritingSceneType> {
    match kind {
        "mystery_clue" => vec![WritingSceneType::Reveal, WritingSceneType::SetupPayoff],
        "object_whereabouts" => vec![WritingSceneType::SetupPayoff],
        "emotional_debt" => vec![WritingSceneType::EmotionalBeat],
        "character_commitment" => vec![WritingSceneType::Dialogue, WritingSceneType::SetupPayoff],
        "relationship_tension" => {
            vec![WritingSceneType::Dialogue, WritingSceneType::EmotionalBeat]
        }
        _ => Vec::new(),
    }
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

fn infer_scene_types(text: &str) -> Vec<WritingSceneType> {
    let mut scene_types = Vec::new();
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::Dialogue,
        &[
            "「", "」", "\"", "说", "问", "回答", "低声", "低语", "喃喃", "喊",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::Action,
        &[
            "拔", "挥", "冲", "扑", "闪", "避", "刺", "劈", "砍", "追", "打斗", "交锋",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::Description,
        &[
            "雾", "风", "月光", "烛", "气味", "潮湿", "冷意", "雪", "雨", "颜色", "影子",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::EmotionalBeat,
        &[
            "沉默", "心跳", "愤怒", "恐惧", "悲伤", "颤抖", "握紧", "犹豫", "后悔",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::ConflictEscalation,
        &[
            "突然",
            "然而",
            "不料",
            "没想到",
            "更糟",
            "危机",
            "追杀",
            "背叛",
            "阻止",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::Reveal,
        &[
            "真相", "揭开", "揭露", "发现", "原来", "秘密", "身份", "来源", "线索",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::SetupPayoff,
        &[
            "伏笔", "回收", "兑现", "下落", "承诺", "誓言", "代价", "结果", "收束",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::Exposition,
        &[
            "解释", "说明", "背景", "来历", "规则", "设定", "宗门", "朝堂", "历史",
        ],
    );
    add_scene_type_if(
        &mut scene_types,
        text,
        WritingSceneType::Transition,
        &[
            "翌日",
            "后来",
            "与此同时",
            "转眼",
            "回到",
            "离开",
            "抵达",
            "路上",
        ],
    );
    scene_types
}

fn add_scene_type_if(
    scene_types: &mut Vec<WritingSceneType>,
    text: &str,
    scene_type: WritingSceneType,
    cues: &[&str],
) {
    if cues.iter().any(|cue| text.contains(cue)) && !scene_types.contains(&scene_type) {
        scene_types.push(scene_type);
    }
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

fn negative_relevance_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for segment in text.split(|ch| matches!(ch, '\n' | '。' | '；' | ';' | '.')) {
        for cue in NEGATIVE_CUES {
            if let Some(cue_start) = segment.find(cue) {
                let cue_tail = &segment[cue_start..];
                for term in relevance_terms(cue_tail) {
                    if seen.insert(term.clone()) {
                        terms.push(term);
                    }
                }
            }
        }
    }
    terms
}

const NEGATIVE_CUES: &[&str] = &["不要", "不得", "禁止", "避免", "不能"];

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
