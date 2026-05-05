//! Project Intake Report — read-only manuscript import analysis.
//!
//! When an author imports existing chapters, Forge reads first
//! and produces a structured report before any writing happens.

use serde::{Deserialize, Serialize};

use super::memory::WriterMemory;
use super::observation::WriterObservation;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIntakeReport {
    pub project_id: String,
    pub chapter_count: usize,
    pub chapter_map: Vec<IntakeChapterSummary>,
    pub identified_characters: Vec<IntakeEntitySummary>,
    pub identified_canon: Vec<IntakeCanonCandidate>,
    pub open_promises: Vec<IntakePromiseCandidate>,
    pub style_fingerprint: IntakeStyleFingerprint,
    pub conflicts: Vec<IntakeConflict>,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakeChapterSummary {
    pub title: String,
    pub word_count_estimate: usize,
    pub main_events: Vec<String>,
    pub characters_introduced: Vec<String>,
    pub promises_introduced: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakeEntitySummary {
    pub name: String,
    pub kind: String,
    pub first_seen_chapter: String,
    pub attributes: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakeCanonCandidate {
    pub entity_name: String,
    pub attribute_key: String,
    pub attribute_value: String,
    pub source_chapter: String,
    pub confidence: f64,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakePromiseCandidate {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub introduced_chapter: String,
    pub expected_payoff_chapter: Option<String>,
    pub confidence: f64,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakeStyleFingerprint {
    pub avg_sentence_length: f64,
    pub dialogue_ratio: f64,
    pub pov_type: String,
    pub common_phrases: Vec<String>,
    pub taboo_signals: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntakeConflict {
    pub kind: String,
    pub description: String,
    pub sources: Vec<String>,
    pub severity: String,
}

/// Build a project intake report from existing memory data.
/// This is a read-only computation — it does not write to Canon, Promise, or Story Bible.
pub fn build_project_intake_report(
    project_id: &str,
    observations: &[WriterObservation],
    memory: &WriterMemory,
) -> ProjectIntakeReport {
    let mut chapter_map = Vec::new();
    let mut all_characters: Vec<IntakeEntitySummary> = Vec::new();
    let mut canon_candidates: Vec<IntakeCanonCandidate> = Vec::new();
    let mut promise_candidates: Vec<IntakePromiseCandidate> = Vec::new();
    let mut conflicts: Vec<IntakeConflict> = Vec::new();
    let mut evidence_refs = Vec::new();

    // Chapter map from observations.
    for obs in observations {
        if let Some(ref title) = obs.chapter_title {
            let word_count = obs.paragraph.chars().filter(|c| c.is_whitespace()).count() + 1;
            chapter_map.push(IntakeChapterSummary {
                title: title.clone(),
                word_count_estimate: word_count,
                main_events: Vec::new(),
                characters_introduced: Vec::new(),
                promises_introduced: Vec::new(),
            });
            evidence_refs.push(format!("observation:{}", obs.id));
        }
    }

    // Extract characters from canon entities.
    if let Ok(entities) = memory.list_canon_entities() {
        for entity in &entities {
            let attrs: Vec<String> = entity
                .attributes
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| format!("{}: {}", k, v)).collect())
                .unwrap_or_default();
            all_characters.push(IntakeEntitySummary {
                name: entity.name.clone(),
                kind: entity.kind.clone(),
                first_seen_chapter: String::new(),
                attributes: attrs,
                confidence: entity.confidence,
            });
            evidence_refs.push(format!("canon:{}", entity.name));
        }
    }

    // Extract open promises.
    if let Ok(promises) = memory.get_open_promise_summaries() {
        for p in &promises {
            promise_candidates.push(IntakePromiseCandidate {
                kind: p.kind.clone(),
                title: p.title.clone(),
                description: p.description.clone(),
                introduced_chapter: p.introduced_chapter.clone(),
                expected_payoff_chapter: if p.expected_payoff.is_empty() {
                    None
                } else {
                    Some(p.expected_payoff.clone())
                },
                confidence: 0.7,
                evidence: format!("promise_id:{}", p.id),
            });
            evidence_refs.push(format!("promise:{}", p.id));
        }
    }

    // Canon candidates from entity attributes.
    for entity in all_characters.iter() {
        for attr in &entity.attributes {
            if let Some((key, value)) = attr.split_once(':') {
                canon_candidates.push(IntakeCanonCandidate {
                    entity_name: entity.name.clone(),
                    attribute_key: key.trim().to_string(),
                    attribute_value: value.trim().to_string(),
                    source_chapter: entity.first_seen_chapter.clone(),
                    confidence: entity.confidence,
                    evidence: format!("canon_entity:{}", entity.name),
                });
            }
        }
    }

    // Detect conflicts: entities with same name but different kind.
    let mut seen_names: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for entity in &all_characters {
        if let Some(prev_kind) = seen_names.get(entity.name.as_str()) {
            if *prev_kind != entity.kind {
                conflicts.push(IntakeConflict {
                    kind: "entity_kind_conflict".to_string(),
                    description: format!(
                        "Entity '{}' appears as both '{}' and '{}'",
                        entity.name, prev_kind, entity.kind
                    ),
                    sources: vec![
                        format!("canon:{}", entity.name),
                        format!("canon:{}", entity.name),
                    ],
                    severity: "medium".to_string(),
                });
            }
        }
        seen_names.insert(&entity.name, &entity.kind);
    }

    // Style fingerprint (basic heuristics from memory).
    let style_fp = if let Ok(prefs) = memory.list_style_preferences(20) {
        let phrases: Vec<String> = prefs.iter().map(|p| p.key.clone()).collect();
        IntakeStyleFingerprint {
            avg_sentence_length: 18.0,
            dialogue_ratio: 0.3,
            pov_type: "third_person".to_string(),
            common_phrases: phrases,
            taboo_signals: Vec::new(),
            confidence: 0.5,
        }
    } else {
        IntakeStyleFingerprint {
            avg_sentence_length: 0.0,
            dialogue_ratio: 0.0,
            pov_type: String::new(),
            common_phrases: Vec::new(),
            taboo_signals: Vec::new(),
            confidence: 0.0,
        }
    };

    let confidence = if chapter_map.is_empty() { 0.0 } else { 0.6 };

    let mut recommendations = Vec::new();
    if !promise_candidates.is_empty() {
        recommendations.push(format!(
            "发现 {} 个开放伏笔，建议审查 Promise Ledger",
            promise_candidates.len()
        ));
    }
    if !conflicts.is_empty() {
        recommendations.push(format!(
            "发现 {} 个潜在设定冲突，建议审查 Canon",
            conflicts.len()
        ));
    }

    ProjectIntakeReport {
        project_id: project_id.to_string(),
        chapter_count: chapter_map.len(),
        chapter_map,
        identified_characters: all_characters,
        identified_canon: canon_candidates,
        open_promises: promise_candidates,
        style_fingerprint: style_fp,
        conflicts,
        confidence,
        evidence_refs,
        recommendations,
    }
}
