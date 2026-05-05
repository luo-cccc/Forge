//! Evidence-first conflict explanations for writer beliefs.
//!
//! This module does not decide which source wins. It collects source-labelled
//! evidence and explains why two or more beliefs cannot all be true at once.

use agent_harness_core::Chunk;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::memory::{
    CanonEntitySummary, ChapterMissionSummary, PlotPromiseSummary, StoryContractQuality,
    StoryContractSummary, WriterMemory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefSource {
    StoryContract,
    ChapterMission,
    Canon,
    PromiseLedger,
    ProjectBrain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeliefEvidence {
    pub source: BeliefSource,
    pub reference: String,
    pub snippet: String,
    pub confidence: f64,
    #[serde(default)]
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefConflictKind {
    ForbiddenReveal,
    FactContradiction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeliefConflictExplanation {
    pub id: String,
    pub kind: BeliefConflictKind,
    pub summary: String,
    pub rationale: String,
    pub confidence: f64,
    pub evidence: Vec<BeliefEvidence>,
    pub resolution_hint: String,
}

#[derive(Debug, Clone)]
struct GuardBelief<'a> {
    evidence: &'a BeliefEvidence,
    terms: Vec<String>,
    signal: GuardSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuardSignal {
    Forbidden,
    DeferredPayoff,
}

#[derive(Debug, Clone)]
struct FactBelief<'a> {
    subject: String,
    predicate: String,
    object: String,
    evidence: &'a BeliefEvidence,
}

pub fn project_brain_chunk_belief_evidence(chunk: &Chunk, confidence: f64) -> BeliefEvidence {
    let mut signals = Vec::new();
    if let Some(source_ref) = chunk.source_ref.as_deref() {
        signals.push(format!("source_ref={source_ref}"));
    }
    if let Some(source_revision) = chunk.source_revision.as_deref() {
        signals.push(format!("source_revision={source_revision}"));
    }
    BeliefEvidence {
        source: BeliefSource::ProjectBrain,
        reference: format!("project_brain:{}", chunk.id),
        snippet: snippet(&chunk.text, 260),
        confidence: clamp_confidence(confidence),
        signals,
    }
}

pub fn explain_memory_belief_conflicts(
    memory: &WriterMemory,
    project_id: &str,
    chapter_title: Option<&str>,
    project_brain_evidence: &[BeliefEvidence],
) -> rusqlite::Result<Vec<BeliefConflictExplanation>> {
    let mut evidence = Vec::new();

    if let Some(contract) = memory.get_story_contract(project_id)? {
        evidence.extend(story_contract_evidence(&contract));
    }
    if let Some(chapter_title) = chapter_title {
        if let Some(mission) = memory.get_chapter_mission(project_id, chapter_title)? {
            evidence.extend(chapter_mission_evidence(&mission));
        }
    }
    evidence.extend(canon_evidence(&memory.list_canon_entities()?));
    evidence.extend(promise_evidence(&memory.get_open_promise_summaries()?));
    evidence.extend(
        project_brain_evidence
            .iter()
            .filter(|item| item.source == BeliefSource::ProjectBrain)
            .cloned(),
    );

    Ok(explain_belief_conflicts(&evidence))
}

pub fn explain_belief_conflicts(evidence: &[BeliefEvidence]) -> Vec<BeliefConflictExplanation> {
    let mut conflicts = Vec::new();
    conflicts.extend(explain_forbidden_reveals(evidence));
    conflicts.extend(explain_fact_contradictions(evidence));
    dedupe_conflicts(conflicts)
}

fn story_contract_evidence(contract: &StoryContractSummary) -> Vec<BeliefEvidence> {
    let confidence = match contract.quality() {
        StoryContractQuality::Strong => 0.94,
        StoryContractQuality::Usable => 0.86,
        StoryContractQuality::Vague => 0.62,
        StoryContractQuality::Missing => 0.3,
    };
    let mut evidence = Vec::new();
    push_evidence(
        &mut evidence,
        BeliefSource::StoryContract,
        "story_contract:reader_promise",
        &contract.reader_promise,
        confidence,
    );
    push_evidence(
        &mut evidence,
        BeliefSource::StoryContract,
        "story_contract:main_conflict",
        &contract.main_conflict,
        confidence,
    );
    push_evidence(
        &mut evidence,
        BeliefSource::StoryContract,
        "story_contract:structural_boundary",
        &contract.structural_boundary,
        confidence,
    );
    evidence
}

fn chapter_mission_evidence(mission: &ChapterMissionSummary) -> Vec<BeliefEvidence> {
    let confidence = match mission.status.as_str() {
        "active" | "draft" | "completed" => 0.9,
        "needs_review" | "drifted" => 0.72,
        "blocked" | "retired" => 0.55,
        _ => 0.68,
    };
    let prefix = format!("chapter_mission:{}", mission.chapter_title);
    let mut evidence = Vec::new();
    push_evidence(
        &mut evidence,
        BeliefSource::ChapterMission,
        &format!("{prefix}:mission"),
        &mission.mission,
        confidence,
    );
    push_evidence(
        &mut evidence,
        BeliefSource::ChapterMission,
        &format!("{prefix}:must_include"),
        &mission.must_include,
        confidence,
    );
    push_evidence(
        &mut evidence,
        BeliefSource::ChapterMission,
        &format!("{prefix}:must_not"),
        &mission.must_not,
        confidence,
    );
    push_evidence(
        &mut evidence,
        BeliefSource::ChapterMission,
        &format!("{prefix}:expected_ending"),
        &mission.expected_ending,
        confidence,
    );
    evidence
}

fn canon_evidence(entities: &[CanonEntitySummary]) -> Vec<BeliefEvidence> {
    let mut evidence = Vec::new();
    for entity in entities {
        push_evidence(
            &mut evidence,
            BeliefSource::Canon,
            &format!("canon:{}:summary", entity.name),
            &entity.summary,
            entity.confidence,
        );
        if let Some(attributes) = entity.attributes.as_object() {
            for (key, value) in attributes {
                let value = match value {
                    serde_json::Value::String(value) => value.trim().to_string(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                };
                if value.trim().is_empty() {
                    continue;
                }
                push_evidence(
                    &mut evidence,
                    BeliefSource::Canon,
                    &format!("canon:{}:{key}", entity.name),
                    &format!("{} {key}={value}", entity.name),
                    entity.confidence,
                );
            }
        }
    }
    evidence
}

fn promise_evidence(promises: &[PlotPromiseSummary]) -> Vec<BeliefEvidence> {
    let mut evidence = Vec::new();
    for promise in promises {
        let confidence = (0.64 + (promise.priority.clamp(0, 10) as f64 * 0.025)).min(0.9);
        push_evidence(
            &mut evidence,
            BeliefSource::PromiseLedger,
            &format!("promise:{}:description", promise.id),
            &format!("{}: {}", promise.title, promise.description),
            confidence,
        );
        push_evidence(
            &mut evidence,
            BeliefSource::PromiseLedger,
            &format!("promise:{}:expected_payoff", promise.id),
            &format!(
                "{} expected payoff: {}",
                promise.title, promise.expected_payoff
            ),
            confidence,
        );
    }
    evidence
}

fn push_evidence(
    evidence: &mut Vec<BeliefEvidence>,
    source: BeliefSource,
    reference: &str,
    snippet_text: &str,
    confidence: f64,
) {
    let snippet_text = snippet_text.trim();
    if snippet_text.is_empty() {
        return;
    }
    evidence.push(BeliefEvidence {
        source,
        reference: reference.to_string(),
        snippet: snippet(snippet_text, 260),
        confidence: clamp_confidence(confidence),
        signals: Vec::new(),
    });
}

fn explain_forbidden_reveals(evidence: &[BeliefEvidence]) -> Vec<BeliefConflictExplanation> {
    let guards = evidence
        .iter()
        .filter_map(classify_guard)
        .collect::<Vec<_>>();
    let mut conflicts = Vec::new();

    for reveal in evidence.iter().filter(|item| is_reveal_claim(item)) {
        let matched_guards = guards
            .iter()
            .filter(|guard| guard.evidence.reference != reveal.reference)
            .filter(|guard| terms_overlap(&guard.terms, &reveal.snippet))
            .collect::<Vec<_>>();
        if matched_guards.is_empty() {
            continue;
        }

        let mut conflict_evidence = Vec::new();
        for guard in matched_guards {
            let mut item = guard.evidence.clone();
            item.signals.push(match guard.signal {
                GuardSignal::Forbidden => "guard=forbidden_reveal".to_string(),
                GuardSignal::DeferredPayoff => "guard=deferred_payoff".to_string(),
            });
            conflict_evidence.push(item);
        }
        let mut reveal_item = reveal.clone();
        reveal_item
            .signals
            .push("claim=revealed_or_resolved".to_string());
        conflict_evidence.push(reveal_item);
        let confidence = conflict_confidence(&conflict_evidence);
        let summary = format!(
            "Reveal claim conflicts with {} guarded belief source(s).",
            conflict_evidence.len().saturating_sub(1)
        );

        conflicts.push(BeliefConflictExplanation {
            id: stable_conflict_id(BeliefConflictKind::ForbiddenReveal, &conflict_evidence),
            kind: BeliefConflictKind::ForbiddenReveal,
            summary,
            rationale:
                "A source says this information is forbidden or deferred, while another source says it has already been revealed or resolved."
                    .to_string(),
            confidence,
            evidence: conflict_evidence,
            resolution_hint:
                "Ask the author to confirm whether to update the guard, move the reveal later, or archive the stale source."
                    .to_string(),
        });
    }

    conflicts
}

fn classify_guard(evidence: &BeliefEvidence) -> Option<GuardBelief<'_>> {
    let text = evidence.snippet.trim();
    if text.is_empty() {
        return None;
    }
    if has_forbid_signal(text) {
        let terms = guard_terms(text);
        if !terms.is_empty() {
            return Some(GuardBelief {
                evidence,
                terms,
                signal: GuardSignal::Forbidden,
            });
        }
    }
    if evidence.source == BeliefSource::PromiseLedger && has_deferred_payoff_signal(text) {
        let terms = guard_terms(text);
        if !terms.is_empty() {
            return Some(GuardBelief {
                evidence,
                terms,
                signal: GuardSignal::DeferredPayoff,
            });
        }
    }
    None
}

fn explain_fact_contradictions(evidence: &[BeliefEvidence]) -> Vec<BeliefConflictExplanation> {
    let facts = evidence
        .iter()
        .flat_map(extract_facts)
        .collect::<Vec<FactBelief<'_>>>();
    let mut conflicts = Vec::new();

    for left_index in 0..facts.len() {
        for right_index in (left_index + 1)..facts.len() {
            let left = &facts[left_index];
            let right = &facts[right_index];
            if left.evidence.reference == right.evidence.reference
                || left.subject != right.subject
                || left.predicate != right.predicate
                || !objects_conflict(&left.object, &right.object)
            {
                continue;
            }
            let mut conflict_evidence = vec![left.evidence.clone(), right.evidence.clone()];
            for item in &mut conflict_evidence {
                item.signals
                    .push(format!("fact={}:{}", left.subject, left.predicate));
            }
            conflicts.push(BeliefConflictExplanation {
                id: stable_conflict_id(BeliefConflictKind::FactContradiction, &conflict_evidence),
                kind: BeliefConflictKind::FactContradiction,
                summary: format!("Conflicting facts for {} {}.", left.subject, left.predicate),
                rationale: format!(
                    "One source says '{}', while another source says '{}'.",
                    left.object, right.object
                ),
                confidence: conflict_confidence(&conflict_evidence),
                evidence: conflict_evidence,
                resolution_hint:
                    "Keep both sources visible until the author confirms which fact is current."
                        .to_string(),
            });
        }
    }

    conflicts
}

fn extract_facts(evidence: &BeliefEvidence) -> Vec<FactBelief<'_>> {
    let mut facts = Vec::new();
    if evidence.source == BeliefSource::Canon {
        if let Some((subject, predicate)) = canon_subject_predicate(&evidence.reference) {
            if let Some(object) = value_after_equals(&evidence.snippet) {
                facts.push(FactBelief {
                    subject,
                    predicate,
                    object,
                    evidence,
                });
            }
        }
    }

    for predicate in ["来源", "身份", "下落", "位置"] {
        if let Some((subject, object)) = infer_chinese_fact(&evidence.snippet, predicate) {
            facts.push(FactBelief {
                subject,
                predicate: predicate.to_string(),
                object,
                evidence,
            });
        }
    }

    facts
}

fn canon_subject_predicate(reference: &str) -> Option<(String, String)> {
    let mut parts = reference.split(':');
    if parts.next()? != "canon" {
        return None;
    }
    let subject = parts.next()?.trim();
    let predicate = parts.next()?.trim();
    if subject.is_empty() || predicate.is_empty() || predicate == "summary" {
        return None;
    }
    Some((subject.to_string(), predicate.to_string()))
}

fn infer_chinese_fact(text: &str, predicate: &str) -> Option<(String, String)> {
    let predicate_pos = text.find(predicate)?;
    let subject = subject_before(text, predicate_pos)?;
    let after_predicate = &text[predicate_pos + predicate.len()..];
    let object = if let Some(value) = value_after_equals(after_predicate) {
        value
    } else if let Some(object) = object_after_marker(after_predicate, &["来自", "是", "为", "在"])
    {
        object
    } else {
        object_after_marker(text, &["来自", "是", "为", "在"])?
    };
    if subject == object || object.chars().count() < 1 {
        return None;
    }
    Some((subject, object))
}

fn subject_before(text: &str, byte_pos: usize) -> Option<String> {
    let prefix = &text[..byte_pos];
    let mut chars = Vec::new();
    for ch in prefix.chars().rev() {
        if is_boundary_char(ch) {
            break;
        }
        chars.push(ch);
        if chars.len() >= 16 {
            break;
        }
    }
    chars.reverse();
    let subject = trim_fact_edge_words(&chars.into_iter().collect::<String>());
    if subject.chars().count() >= 2 {
        Some(subject)
    } else {
        None
    }
}

fn value_after_equals(text: &str) -> Option<String> {
    let split_at = text
        .find('=')
        .or_else(|| text.find(':'))
        .or_else(|| text.find('：'))?;
    let value = trim_fact_edge_words(&take_until_boundary(&text[split_at + 1..]));
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn object_after_marker(text: &str, markers: &[&str]) -> Option<String> {
    let (marker_pos, marker) = markers
        .iter()
        .filter_map(|marker| text.find(marker).map(|pos| (pos, *marker)))
        .min_by_key(|(pos, _)| *pos)?;
    let value = trim_fact_edge_words(&take_until_boundary(&text[marker_pos + marker.len()..]));
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn take_until_boundary(text: &str) -> String {
    text.chars()
        .take_while(|ch| !is_boundary_char(*ch))
        .collect::<String>()
}

fn trim_fact_edge_words(text: &str) -> String {
    let mut value = text
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\'' | '`' | '“' | '”' | '「' | '」' | '《' | '》' | '-' | '_' | '='
                )
        })
        .trim()
        .to_string();
    for prefix in ["已经", "已", "仍", "还", "会", "将", "就是", "其实", "原来"] {
        value = value.trim_start_matches(prefix).trim().to_string();
    }
    for suffix in ["已经揭示", "已揭示", "已经确认", "已确认"] {
        value = value.trim_end_matches(suffix).trim().to_string();
    }
    value
}

fn objects_conflict(left: &str, right: &str) -> bool {
    let left = normalize_fact_value(left);
    let right = normalize_fact_value(right);
    if left.is_empty() || right.is_empty() || left == right {
        return false;
    }
    if is_unknown_value(&left) != is_unknown_value(&right) {
        return true;
    }
    !left.contains(&right) && !right.contains(&left)
}

fn normalize_fact_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '。' | '，' | ',' | '.' | ';' | '；'))
        .collect::<String>()
        .to_lowercase()
}

fn is_unknown_value(value: &str) -> bool {
    ["未知", "不明", "未揭示", "unknown", "unrevealed"]
        .iter()
        .any(|marker| value.contains(marker))
}

fn has_forbid_signal(text: &str) -> bool {
    text_contains_any(
        text,
        &[
            "不得",
            "不要",
            "不能",
            "禁止",
            "不许",
            "避免",
            "do not",
            "must not",
            "forbid",
            "forbidden",
        ],
    )
}

fn has_deferred_payoff_signal(text: &str) -> bool {
    text_contains_any(
        text,
        &[
            "expected payoff",
            "payoff",
            "later",
            "defer",
            "第",
            "后",
            "再",
            "延后",
            "回收",
            "兑现",
            "揭示",
        ],
    )
}

fn has_reveal_signal(text: &str) -> bool {
    text_contains_any(
        text,
        &[
            "已经",
            "已",
            "真相",
            "来自",
            "揭示",
            "揭露",
            "揭开",
            "说出",
            "确认",
            "兑现",
            "回收",
            "resolved",
            "revealed",
            "confirmed",
            "paid off",
        ],
    )
}

fn is_reveal_claim(evidence: &BeliefEvidence) -> bool {
    let text = evidence.snippet.as_str();
    !(!has_reveal_signal(text)
        || has_forbid_signal(text)
        || (evidence.source == BeliefSource::PromiseLedger && has_deferred_payoff_signal(text))
        || text_contains_any(
            text,
            &["未知", "不明", "未揭示", "仍是悬念", "保持悬念", "保留"],
        ))
}

fn guard_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for term in meaningful_terms(text) {
        if is_guard_stop_term(&term) {
            continue;
        }
        push_unique_term(&mut terms, &term);
    }

    let chars = text.chars().collect::<Vec<_>>();
    for pair in chars.windows(2) {
        let term = pair.iter().collect::<String>();
        if term.chars().all(is_term_char) && !is_guard_stop_term(&term) {
            push_unique_term(&mut terms, &term);
        }
    }
    terms
}

fn meaningful_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if is_term_char(ch) {
            current.push(ch);
        } else {
            push_unique_term(&mut terms, &current);
            current.clear();
        }
    }
    push_unique_term(&mut terms, &current);
    terms
}

fn push_unique_term(terms: &mut Vec<String>, term: &str) {
    let term = term.trim();
    if term.chars().count() < 2 || is_guard_stop_term(term) {
        return;
    }
    if !terms.iter().any(|existing| existing == term) {
        terms.push(term.to_string());
    }
}

fn terms_overlap(terms: &[String], text: &str) -> bool {
    terms.iter().any(|term| text.contains(term))
}

fn is_guard_stop_term(term: &str) -> bool {
    const STOP_TERMS: &[&str] = &[
        "不得",
        "不要",
        "不能",
        "禁止",
        "不许",
        "避免",
        "提前",
        "泄露",
        "揭露",
        "揭示",
        "揭开",
        "来源",
        "身份",
        "真相",
        "expected",
        "payoff",
        "later",
        "defer",
        "must",
        "not",
        "forbidden",
    ];
    STOP_TERMS
        .iter()
        .any(|stop| term.eq_ignore_ascii_case(stop) || term.contains(stop))
}

fn is_term_char(ch: char) -> bool {
    ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn is_boundary_char(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '。' | '，'
                | ','
                | '.'
                | ';'
                | '；'
                | ':'
                | '：'
                | '!'
                | '！'
                | '?'
                | '？'
                | '\n'
                | '\r'
        )
}

fn text_contains_any(text: &str, needles: &[&str]) -> bool {
    let lower = text.to_lowercase();
    needles
        .iter()
        .any(|needle| lower.contains(&needle.to_lowercase()))
}

fn conflict_confidence(evidence: &[BeliefEvidence]) -> f64 {
    if evidence.is_empty() {
        return 0.0;
    }
    let average = evidence.iter().map(|item| item.confidence).sum::<f64>() / evidence.len() as f64;
    let source_bonus = unique_sources(evidence).len().saturating_sub(2) as f64 * 0.03;
    clamp_confidence(average + source_bonus)
}

fn unique_sources(evidence: &[BeliefEvidence]) -> BTreeSet<BeliefSource> {
    evidence.iter().map(|item| item.source).collect()
}

fn stable_conflict_id(kind: BeliefConflictKind, evidence: &[BeliefEvidence]) -> String {
    let mut refs = evidence
        .iter()
        .map(|item| item.reference.as_str())
        .collect::<Vec<_>>();
    refs.sort_unstable();
    let kind = match kind {
        BeliefConflictKind::ForbiddenReveal => "forbidden_reveal",
        BeliefConflictKind::FactContradiction => "fact_contradiction",
    };
    format!("belief_conflict:{kind}:{}", refs.join("|"))
}

fn dedupe_conflicts(conflicts: Vec<BeliefConflictExplanation>) -> Vec<BeliefConflictExplanation> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for conflict in conflicts {
        if seen.insert(conflict.id.clone()) {
            deduped.push(conflict);
        }
    }
    deduped.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
    deduped
}

fn clamp_confidence(confidence: f64) -> f64 {
    confidence.clamp(0.0, 1.0)
}

fn snippet(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_forbidden_reveal_without_choosing_winner() {
        let evidence = vec![
            BeliefEvidence {
                source: BeliefSource::StoryContract,
                reference: "story_contract:structural_boundary".to_string(),
                snippet: "不得提前泄露寒玉戒指来源".to_string(),
                confidence: 0.9,
                signals: Vec::new(),
            },
            BeliefEvidence {
                source: BeliefSource::ProjectBrain,
                reference: "project_brain:ring".to_string(),
                snippet: "寒玉戒指来源已经揭示，来自皇宫禁库。".to_string(),
                confidence: 0.85,
                signals: Vec::new(),
            },
        ];

        let conflicts = explain_belief_conflicts(&evidence);
        assert!(conflicts.iter().any(|conflict| {
            conflict.kind == BeliefConflictKind::ForbiddenReveal
                && conflict.evidence.len() == 2
                && conflict
                    .evidence
                    .iter()
                    .any(|item| item.source == BeliefSource::StoryContract)
                && conflict
                    .evidence
                    .iter()
                    .any(|item| item.source == BeliefSource::ProjectBrain)
        }));
    }
}
