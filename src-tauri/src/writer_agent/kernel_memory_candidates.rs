//! Memory candidate extraction and proposal construction helpers.

use std::collections::HashSet;

use super::kernel_chapters::proposal_id;
use super::kernel_memory_feedback::{
    memory_candidate_slot_for_canon, memory_candidate_slot_for_promise, MemoryCandidate,
    MemoryExtractionFeedback,
};
use super::memory::{CanonEntitySummary, PromiseKind, StylePreferenceSummary, WriterMemory};
use super::observation::WriterObservation;
use super::operation::{CanonEntityOp, PlotPromiseOp, WriterOperation};
use super::proposal::{AgentProposal, EvidenceRef, EvidenceSource, ProposalKind, ProposalPriority};

pub(crate) fn memory_candidates_from_observation(
    observation: &WriterObservation,
    memory: &WriterMemory,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
) -> Vec<AgentProposal> {
    let mut proposals = Vec::new();
    let mut known = memory.get_canon_entity_names().unwrap_or_default();
    known.sort();
    known.dedup();

    let feedback = MemoryExtractionFeedback::from_memory(memory);

    for mut entity in extract_new_canon_entities(&observation.paragraph, &known)
        .into_iter()
        .take(3)
    {
        let slot = memory_candidate_slot_for_canon(&entity);
        if feedback.is_suppressed(&slot) {
            continue;
        }
        if feedback.is_preferred(&slot) {
            entity.confidence = (entity.confidence + 0.08).min(0.92);
        }
        match validate_canon_candidate_with_memory(&entity, memory) {
            MemoryCandidateQuality::Acceptable => proposals.push(canon_candidate_proposal(
                observation,
                observation_id,
                proposal_counter,
                session_id,
                entity,
                CandidateSource::Local,
            )),
            MemoryCandidateQuality::Conflict {
                existing_name,
                reason,
            } => proposals.push(canon_conflict_candidate_proposal(
                observation,
                observation_id,
                proposal_counter,
                session_id,
                entity,
                existing_name,
                reason,
                CandidateSource::Local,
            )),
            MemoryCandidateQuality::MergeableAttributes {
                existing_name,
                attributes,
            } => proposals.push(canon_attribute_merge_candidate_proposal(
                observation,
                observation_id,
                proposal_counter,
                session_id,
                existing_name,
                attributes,
                entity.confidence,
                CandidateSource::Local,
            )),
            MemoryCandidateQuality::Vague { .. } | MemoryCandidateQuality::Duplicate { .. } => {}
        }
    }

    for mut promise in extract_plot_promises(&observation.paragraph, observation)
        .into_iter()
        .take(3)
    {
        let slot = memory_candidate_slot_for_promise(&promise);
        if feedback.is_suppressed(&slot) {
            continue;
        }
        if feedback.is_preferred(&slot) {
            promise.priority = (promise.priority + 1).min(10);
        }
        if validate_promise_candidate_with_dedup(&promise, memory)
            == MemoryCandidateQuality::Acceptable
        {
            proposals.push(promise_candidate_proposal(
                observation,
                observation_id,
                proposal_counter,
                session_id,
                promise,
                CandidateSource::Local,
            ));
        }
    }

    proposals
}

pub(crate) fn canon_candidate_proposal(
    observation: &WriterObservation,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
    entity: CanonEntityOp,
    source: CandidateSource,
) -> AgentProposal {
    let id = proposal_id(session_id, *proposal_counter);
    *proposal_counter += 1;
    let preview = format!("沉淀设定: {} - {}", entity.name, entity.summary);
    let snippet = entity.summary.clone();
    let (rationale, confidence, risks) = source.canon_metadata();
    AgentProposal {
        id,
        observation_id: observation_id.to_string(),
        kind: ProposalKind::CanonUpdate,
        priority: ProposalPriority::Ambient,
        target: observation.cursor.clone(),
        preview,
        operations: vec![WriterOperation::CanonUpsertEntity { entity }],
        rationale,
        evidence: vec![EvidenceRef {
            source: EvidenceSource::ChapterText,
            reference: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            snippet,
        }],
        risks,
        alternatives: vec![],
        confidence,
        expires_at: None,
    }
}

pub(crate) fn promise_candidate_proposal(
    observation: &WriterObservation,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
    promise: PlotPromiseOp,
    source: CandidateSource,
) -> AgentProposal {
    let id = proposal_id(session_id, *proposal_counter);
    *proposal_counter += 1;
    let preview = format!("登记伏笔: {} - {}", promise.title, promise.description);
    let snippet = promise.description.clone();
    let (rationale, confidence, risks) = source.promise_metadata();
    AgentProposal {
        id,
        observation_id: observation_id.to_string(),
        kind: ProposalKind::PlotPromise,
        priority: ProposalPriority::Ambient,
        target: observation.cursor.clone(),
        preview,
        operations: vec![WriterOperation::PromiseAdd { promise }],
        rationale,
        evidence: vec![EvidenceRef {
            source: EvidenceSource::ChapterText,
            reference: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            snippet,
        }],
        risks,
        alternatives: vec![],
        confidence,
        expires_at: None,
    }
}

pub(crate) fn canon_conflict_candidate_proposal(
    observation: &WriterObservation,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
    entity: CanonEntityOp,
    existing_name: String,
    reason: String,
    source: CandidateSource,
) -> AgentProposal {
    let id = proposal_id(session_id, *proposal_counter);
    *proposal_counter += 1;
    let preview = format!("设定冲突需确认: {} - {}", entity.name, reason);
    let source_label = source.label();
    AgentProposal {
        id,
        observation_id: observation_id.to_string(),
        kind: ProposalKind::ContinuityWarning,
        priority: ProposalPriority::Urgent,
        target: observation.cursor.clone(),
        preview,
        operations: vec![],
        rationale: format!(
            "{} 记忆候选与现有 canon 冲突，必须由作者明确确认后再进入长期记忆。",
            source_label
        ),
        evidence: vec![
            EvidenceRef {
                source: EvidenceSource::ChapterText,
                reference: observation
                    .chapter_title
                    .clone()
                    .unwrap_or_else(|| "current chapter".to_string()),
                snippet: entity.summary.clone(),
            },
            EvidenceRef {
                source: EvidenceSource::Canon,
                reference: existing_name,
                snippet: reason,
            },
        ],
        risks: vec![
            "未自动写入长期 canon；请先确认是正文临场描述、误抽取，还是需要修改既有设定。"
                .to_string(),
        ],
        alternatives: vec![],
        confidence: match source {
            CandidateSource::Local => 0.7,
            CandidateSource::Llm(_) => 0.82,
        },
        expires_at: None,
    }
}

pub(crate) fn canon_attribute_merge_candidate_proposal(
    observation: &WriterObservation,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
    existing_name: String,
    attributes: Vec<(String, String)>,
    confidence: f64,
    source: CandidateSource,
) -> AgentProposal {
    let id = proposal_id(session_id, *proposal_counter);
    *proposal_counter += 1;
    let source_label = source.label();
    let attribute_text = attributes
        .iter()
        .map(|(key, value)| format!("{}.{} = {}", existing_name, key, value))
        .collect::<Vec<_>>()
        .join("; ");
    let operations = attributes
        .iter()
        .map(|(attribute, value)| WriterOperation::CanonUpdateAttribute {
            entity: existing_name.clone(),
            attribute: attribute.clone(),
            value: value.clone(),
            confidence,
        })
        .collect::<Vec<_>>();
    AgentProposal {
        id,
        observation_id: observation_id.to_string(),
        kind: ProposalKind::CanonUpdate,
        priority: ProposalPriority::Ambient,
        target: observation.cursor.clone(),
        preview: format!("补充设定属性: {}", attribute_text),
        operations,
        rationale: format!(
            "{} 记忆候选命中既有 canon，只补充缺失属性；需作者确认后合并。",
            source_label
        ),
        evidence: vec![
            EvidenceRef {
                source: EvidenceSource::ChapterText,
                reference: observation
                    .chapter_title
                    .clone()
                    .unwrap_or_else(|| "current chapter".to_string()),
                snippet: attribute_text.clone(),
            },
            EvidenceRef {
                source: EvidenceSource::Canon,
                reference: existing_name,
                snippet: "existing entity; missing non-conflicting attributes only".to_string(),
            },
        ],
        risks: vec!["仅补充缺失属性，不覆盖既有 canon；请确认该属性是长期设定。".to_string()],
        alternatives: vec![],
        confidence: match source {
            CandidateSource::Local => 0.64,
            CandidateSource::Llm(_) => 0.8,
        },
        expires_at: None,
    }
}

pub(crate) enum CandidateSource {
    Local,
    Llm(String),
}

impl CandidateSource {
    fn label(&self) -> String {
        match self {
            CandidateSource::Local => "本地记忆抽取".to_string(),
            CandidateSource::Llm(model) => format!("LLM增强记忆抽取: {}.", model),
        }
    }

    fn canon_metadata(&self) -> (String, f64, Vec<String>) {
        match self {
            CandidateSource::Local => (
                "章节保存后发现可复用人物/物件设定，建议写入长期 canon。".to_string(),
                0.62,
                vec!["自动抽取可能误把普通名词当设定，请确认后接受。".to_string()],
            ),
            CandidateSource::Llm(model) => (
                format!("LLM增强记忆抽取: {}. 建议写入长期 canon。", model),
                0.78,
                vec!["LLM 抽取仍需人工确认，避免把临场描述误记成长期设定。".to_string()],
            ),
        }
    }

    fn promise_metadata(&self) -> (String, f64, Vec<String>) {
        match self {
            CandidateSource::Local => (
                "章节保存后发现未回收信息，建议加入伏笔 ledger 以便后续提醒。".to_string(),
                0.66,
                vec!["请确认这是真伏笔，而不是只在当前场景内解决的信息。".to_string()],
            ),
            CandidateSource::Llm(model) => (
                format!("LLM增强记忆抽取: {}. 建议加入伏笔 ledger。", model),
                0.8,
                vec!["请确认这是真伏笔，而不是 LLM 过度解读。".to_string()],
            ),
        }
    }
}

pub(crate) fn extract_new_canon_entities(text: &str, known: &[String]) -> Vec<CanonEntityOp> {
    let mut entities = Vec::new();
    for sentence in split_sentences(text) {
        for cue in ["名叫", "叫做", "名为", "代号"] {
            if let Some(name) = extract_name_after(&sentence, cue) {
                if should_keep_entity(&name, known, &entities) {
                    entities.push(CanonEntityOp {
                        kind: "character".to_string(),
                        name: name.clone(),
                        aliases: vec![],
                        summary: sentence_snippet(&sentence, 120),
                        attributes: serde_json::json!({}),
                        confidence: 0.62,
                    });
                }
            }
        }

        for marker in ["寒影刀", "玉佩", "密信", "钥匙", "令牌"] {
            if sentence.contains(marker) && should_keep_entity(marker, known, &entities) {
                entities.push(CanonEntityOp {
                    kind: "object".to_string(),
                    name: marker.to_string(),
                    aliases: vec![],
                    summary: sentence_snippet(&sentence, 120),
                    attributes: serde_json::json!({ "category": "story_object" }),
                    confidence: 0.58,
                });
            }
        }
    }
    entities
}

pub fn extract_plot_promises(text: &str, observation: &WriterObservation) -> Vec<PlotPromiseOp> {
    let mut promises = Vec::new();
    for sentence in split_sentences(text) {
        if !contains_promise_cue(&sentence) {
            continue;
        }
        let title = promise_title(&sentence);
        if title.is_empty() || promises.iter().any(|p: &PlotPromiseOp| p.title == title) {
            continue;
        }
        let kind = promise_kind_from_cues(&sentence);
        let priority = match kind {
            PromiseKind::ObjectWhereabouts | PromiseKind::MysteryClue => 5,
            PromiseKind::CharacterCommitment | PromiseKind::EmotionalDebt => 4,
            _ => 3,
        };
        let related = extract_related_entities(&sentence);
        promises.push(PlotPromiseOp {
            kind: kind.as_kind_str().to_string(),
            title,
            description: sentence_snippet(&sentence, 140),
            introduced_chapter: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            expected_payoff: "后续章节回收或解释".to_string(),
            priority,
            related_entities: related,
        });
    }
    promises
}

pub(crate) fn llm_memory_candidates_from_value(
    value: serde_json::Value,
    observation: &WriterObservation,
    _model: &str,
) -> Vec<MemoryCandidate> {
    let mut candidates = Vec::new();

    if let Some(canon) = value.get("canon").and_then(|v| v.as_array()) {
        for item in canon.iter().take(5) {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if name.chars().count() < 2 || name.chars().count() > 16 {
                continue;
            }
            let summary = item
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if summary.chars().count() < 6 {
                continue;
            }
            let kind = item
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("entity")
                .trim();
            let confidence = item
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.75)
                .clamp(0.0, 1.0);
            if confidence < 0.55 {
                continue;
            }
            let aliases = item
                .get("aliases")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|alias| alias.as_str())
                        .map(str::trim)
                        .filter(|alias| !alias.is_empty())
                        .take(6)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let attributes = item
                .get("attributes")
                .cloned()
                .filter(|value| value.is_object())
                .unwrap_or_else(|| serde_json::json!({}));
            candidates.push(MemoryCandidate::Canon(CanonEntityOp {
                kind: if kind.is_empty() {
                    "entity".to_string()
                } else {
                    kind.to_string()
                },
                name: name.to_string(),
                aliases,
                summary: sentence_snippet(summary, 180),
                attributes,
                confidence,
            }));
        }
    }

    if let Some(promises) = value.get("promises").and_then(|v| v.as_array()) {
        for item in promises.iter().take(5) {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if title.chars().count() < 2 || description.chars().count() < 6 {
                continue;
            }
            let confidence = item
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.75)
                .clamp(0.0, 1.0);
            if confidence < 0.55 {
                continue;
            }
            candidates.push(MemoryCandidate::Promise(PlotPromiseOp {
                kind: item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open_question")
                    .trim()
                    .to_string(),
                title: sentence_snippet(title, 40),
                description: sentence_snippet(description, 180),
                introduced_chapter: item
                    .get("introducedChapter")
                    .or_else(|| item.get("introduced_chapter"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        observation
                            .chapter_title
                            .as_deref()
                            .unwrap_or("current chapter")
                    })
                    .trim()
                    .to_string(),
                expected_payoff: item
                    .get("expectedPayoff")
                    .or_else(|| item.get("expected_payoff"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("后续章节回收或解释")
                    .trim()
                    .to_string(),
                priority: item
                    .get("priority")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3)
                    .clamp(0, 10) as i32,
                related_entities: vec![],
            }));
        }
    }

    dedupe_memory_candidates(candidates)
}

fn dedupe_memory_candidates(candidates: Vec<MemoryCandidate>) -> Vec<MemoryCandidate> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        let key = match &candidate {
            MemoryCandidate::Canon(entity) => format!("canon:{}", entity.name),
            MemoryCandidate::Promise(promise) => format!("promise:{}", promise.title),
        };
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}

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
