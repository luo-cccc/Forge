//! Chapter Settlement Queue — post-save settlement workflow.
//!
//! After a chapter is saved, this queue groups reviewable updates by
//! Canon / Promise / Mission / Style / Continuity risk, keeping the
//! author in control via explicit approval.

use serde::{Deserialize, Serialize};

use super::memory::WriterMemory;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSettlementQueue {
    pub chapter_title: String,
    pub chapter_revision: String,
    pub canon_updates: Vec<SettlementItem>,
    pub promise_updates: Vec<SettlementItem>,
    pub mission_suggestions: Vec<SettlementItem>,
    pub style_notes: Vec<SettlementItem>,
    pub continuity_risks: Vec<SettlementItem>,
    pub high_priority_count: usize,
    pub requires_author_approval: bool,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettlementItem {
    pub id: String,
    pub category: String, // "canon" | "promise" | "mission" | "style" | "continuity"
    pub title: String,
    pub description: String,
    pub priority: String, // "high" | "medium" | "low"
    pub requires_approval: bool,
    pub evidence_source: String,
    pub suggested_action: String,
}

/// Build a settlement queue from post-save memory state.
/// All items are proposals — nothing is written automatically.
pub fn build_chapter_settlement_queue(
    chapter_title: &str,
    chapter_revision: &str,
    memory: &WriterMemory,
    project_id: &str,
) -> ChapterSettlementQueue {
    let mut canon_updates = Vec::new();
    let mut promise_updates = Vec::new();
    let mut mission_suggestions = Vec::new();
    let mut continuity_risks = Vec::new();
    let mut evidence_refs = Vec::new();

    // Canon: flag entities with low confidence.
    if let Ok(entities) = memory.list_canon_entities() {
        for entity in &entities {
            if entity.confidence < 0.6 {
                canon_updates.push(SettlementItem {
                    id: format!("canon:{}", entity.name),
                    category: "canon".to_string(),
                    title: format!("低置信度实体: {}", entity.name),
                    description: format!(
                        "Entity '{}' (kind={}) has confidence {}",
                        entity.name, entity.kind, entity.confidence
                    ),
                    priority: "high".to_string(),
                    requires_approval: true,
                    evidence_source: format!("canon_fact:{}", entity.name),
                    suggested_action: "审查并确认或修正此实体属性".to_string(),
                });
                evidence_refs.push(format!("canon:{}", entity.name));
            }
        }
    }

    // Promise: flag at-risk or stale promises.
    if let Ok(promises) = memory.get_open_promise_summaries() {
        for p in &promises {
            if p.risk == "high" {
                promise_updates.push(SettlementItem {
                    id: format!("promise:{}", p.id),
                    category: "promise".to_string(),
                    title: format!("高风险伏笔: {}", p.title),
                    description: format!(
                        "Promise '{}' (kind={}) is at high risk; expected payoff: {}",
                        p.title, p.kind, p.expected_payoff
                    ),
                    priority: "high".to_string(),
                    requires_approval: true,
                    evidence_source: format!("promise:{}", p.id),
                    suggested_action: "审查伏笔状态，确认是否延期、兑现或放弃".to_string(),
                });
                evidence_refs.push(format!("promise:{}", p.id));
            }
        }
    }

    // Mission: check current chapter mission status.
    if let Ok(Some(mission)) = memory.get_chapter_mission(project_id, chapter_title) {
        if mission.status == "draft" || mission.status == "needs_review" {
            mission_suggestions.push(SettlementItem {
                id: format!("mission:{}", mission.chapter_title),
                category: "mission".to_string(),
                title: format!("章节任务需审查: {}", mission.chapter_title),
                description: format!(
                    "Mission status is '{}'; must_include: {}, must_not: {}",
                    mission.status, mission.must_include, mission.must_not
                ),
                priority: "medium".to_string(),
                requires_approval: true,
                evidence_source: format!("chapter_mission:{}", mission.chapter_title),
                suggested_action: "标记为 completed / drifted / needs_review".to_string(),
            });
            evidence_refs.push(format!("mission:{}", mission.chapter_title));
        }
    }

    // Style notes from recent feedback.
    if let Ok(feedback_entries) = memory.list_memory_feedback(5) {
        for entry in &feedback_entries {
            if entry.action == "correction" {
                continuity_risks.push(SettlementItem {
                    id: format!("style:{}", entry.slot),
                    category: "continuity".to_string(),
                    title: format!("风格纠错: {}", entry.slot),
                    description: entry.reason.clone().unwrap_or_default(),
                    priority: "medium".to_string(),
                    requires_approval: false,
                    evidence_source: format!("memory_feedback:{}", entry.slot),
                    suggested_action: "确认风格约束已更新".to_string(),
                });
            }
        }
    }

    let high_priority = canon_updates
        .iter()
        .filter(|i| i.priority == "high")
        .count()
        + promise_updates
            .iter()
            .filter(|i| i.priority == "high")
            .count();

    ChapterSettlementQueue {
        chapter_title: chapter_title.to_string(),
        chapter_revision: chapter_revision.to_string(),
        canon_updates,
        promise_updates,
        mission_suggestions,
        style_notes: Vec::new(),
        continuity_risks,
        high_priority_count: high_priority,
        requires_author_approval: high_priority > 0,
        evidence_refs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer_agent::memory::WriterMemory;
    use std::path::Path;

    #[test]
    fn settlement_requires_approval_for_ledger() {
        let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
        memory
            .ensure_story_contract_seed("eval", "T", "fantasy", "p", "j", "")
            .unwrap();
        memory
            .upsert_canon_entity(
                "character",
                "林墨",
                &[],
                "主角",
                &serde_json::json!({"weapon":"sword"}),
                0.3,
            )
            .ok();
        let queue = build_chapter_settlement_queue("Ch3", "rev-1", &memory, "eval");
        assert!(queue.requires_author_approval);
        assert!(queue.high_priority_count > 0);
    }

    #[test]
    fn settlement_groups_by_category() {
        let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
        memory
            .ensure_story_contract_seed("eval", "T", "fantasy", "p", "j", "")
            .unwrap();
        memory
            .add_promise("plot_promise", "戒指", "遗物", "Ch1", "Ch5", 4)
            .unwrap();
        memory
            .upsert_canon_entity("character", "林墨", &[], "x", &serde_json::json!({}), 0.3)
            .ok();
        let queue = build_chapter_settlement_queue("Ch3", "rev-1", &memory, "eval");
        assert!(
            !queue.canon_updates.is_empty() || !queue.promise_updates.is_empty(),
            "should have at least one category populated"
        );
    }
}
