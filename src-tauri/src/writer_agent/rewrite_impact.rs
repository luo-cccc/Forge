//! Rewrite Impact Preview — before accepting a rewrite, preview which story
//! facts (canon, promises, missions, style) will be affected.
//!
//! Reuses Story Impact Radius for bidirectional graph analysis.

use serde::{Deserialize, Serialize};

use super::memory::WriterMemory;
use super::observation::WriterObservation;
use super::story_impact::{
    compute_story_impact, StoryImpactBudgetReport, StoryImpactRisk, WriterStoryImpactRadius,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RewriteImpactPreview {
    pub observation_id: String,
    pub impacted_canon: Vec<ImpactedEntity>,
    pub impacted_promises: Vec<ImpactedPromise>,
    pub impacted_missions: Vec<ImpactedMission>,
    pub style_signals: Vec<ImpactedStyleSignal>,
    pub risk: String,
    pub truncated_high_risk_sources: Vec<String>,
    pub recommend_planning_review: bool,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedEntity {
    pub name: String,
    pub affected_attributes: Vec<String>,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedPromise {
    pub title: String,
    pub kind: String,
    pub impact: String, // "payoff_opportunity" | "contradiction_risk" | "progress"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedMission {
    pub chapter: String,
    pub clause: String,
    pub impact: String, // "supports" | "contradicts" | "drifts"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedStyleSignal {
    pub signal: String,
    pub direction: String, // "aligns" | "diverges"
}

/// Compute a rewrite impact preview using Story Impact Radius.
/// Read-only — does not modify text or ledger.
pub fn compute_rewrite_impact_preview(
    observation: &WriterObservation,
    memory: &WriterMemory,
) -> RewriteImpactPreview {
    let dummy_pack = super::context::assemble_observation_context_with_default_budget(
        super::context::AgentTask::GhostWriting,
        observation,
        memory,
    );

    let (radius, budget) = compute_story_impact(observation, &dummy_pack, memory, None);

    let impacted_canon: Vec<ImpactedEntity> = radius
        .impacted_nodes
        .iter()
        .filter(|n| matches!(n.kind, super::story_impact::StoryNodeKind::CanonEntity))
        .map(|n| ImpactedEntity {
            name: n.label.clone(),
            affected_attributes: Vec::new(),
            risk: if n.confidence < 0.6 { "high" } else { "low" }.to_string(),
        })
        .collect();

    let impacted_promises: Vec<ImpactedPromise> = radius
        .impacted_nodes
        .iter()
        .filter(|n| matches!(n.kind, super::story_impact::StoryNodeKind::PlotPromise))
        .map(|n| ImpactedPromise {
            title: n.label.clone(),
            kind: String::new(),
            impact: if n.confidence < 0.6 {
                "contradiction_risk"
            } else {
                "payoff_opportunity"
            }
            .to_string(),
        })
        .collect();

    let impacted_missions: Vec<ImpactedMission> = radius
        .impacted_nodes
        .iter()
        .filter(|n| matches!(n.kind, super::story_impact::StoryNodeKind::ChapterMission))
        .map(|n| ImpactedMission {
            chapter: n.chapter.clone().unwrap_or_default(),
            clause: n.summary.clone(),
            impact: "supports".to_string(),
        })
        .collect();

    let style_signals: Vec<ImpactedStyleSignal> = Vec::new();

    let recommend_planning_review =
        matches!(radius.risk, StoryImpactRisk::High) || budget.truncated_node_count > 3;

    RewriteImpactPreview {
        observation_id: observation.id.clone(),
        impacted_canon,
        impacted_promises,
        impacted_missions,
        style_signals,
        risk: format!("{:?}", radius.risk),
        truncated_high_risk_sources: budget.dropped_high_risk_sources,
        recommend_planning_review,
        evidence_refs: radius.impacted_sources,
    }
}
