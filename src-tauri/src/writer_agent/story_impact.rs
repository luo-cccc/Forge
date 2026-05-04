//! Story Impact Radius — graph-shaped context assembly for Writer Agent.
//!
//! Instead of stuffing context by source priority alone, this module computes
//! which story facts (canon, promises, missions, Project Brain chunks) are in
//! the "blast radius" of the current writing task and assembles a budgeted,
//! distance-weighted context report.

use serde::{Deserialize, Serialize};

use super::context::{ContextSource, WritingContextPack};
use super::memory::WriterMemory;
use super::observation::WriterObservation;

const MAX_TRAVERSAL_DEPTH: usize = 4;
const DEFAULT_IMPACT_BUDGET_CHARS: usize = 2_400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WriterStoryGraphNode {
    pub id: String,
    pub kind: StoryNodeKind,
    pub label: String,
    pub source_ref: String,
    pub source_revision: Option<String>,
    pub chapter: Option<String>,
    pub confidence: f32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StoryNodeKind {
    CanonEntity,
    CanonRule,
    PlotPromise,
    ChapterMission,
    ResultFeedback,
    ProjectBrainChunk,
    Decision,
    StoryContract,
    SeedTask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WriterStoryGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: StoryEdgeKind,
    pub evidence_ref: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StoryEdgeKind {
    MentionsEntity,
    UpdatesPromise,
    SupportsMission,
    ContradictsCanon,
    DependsOnResult,
    SameSourceRevision,
    SharedKeyword,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WriterStoryImpactRadius {
    pub seed_nodes: Vec<WriterStoryGraphNode>,
    pub impacted_nodes: Vec<WriterStoryGraphNode>,
    pub impacted_sources: Vec<String>,
    pub edges: Vec<WriterStoryGraphEdge>,
    pub risk: StoryImpactRisk,
    pub truncated: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StoryImpactRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoryImpactBudgetReport {
    pub budget_limit: usize,
    pub requested_chars: usize,
    pub provided_chars: usize,
    pub truncated_node_count: usize,
    pub dropped_high_risk_sources: Vec<String>,
    pub reasons: Vec<String>,
}

impl StoryNodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StoryNodeKind::CanonEntity => "canon_entity",
            StoryNodeKind::CanonRule => "canon_rule",
            StoryNodeKind::PlotPromise => "plot_promise",
            StoryNodeKind::ChapterMission => "chapter_mission",
            StoryNodeKind::ResultFeedback => "result_feedback",
            StoryNodeKind::ProjectBrainChunk => "project_brain_chunk",
            StoryNodeKind::Decision => "decision",
            StoryNodeKind::StoryContract => "story_contract",
            StoryNodeKind::SeedTask => "seed_task",
        }
    }
}

impl StoryEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StoryEdgeKind::MentionsEntity => "mentions_entity",
            StoryEdgeKind::UpdatesPromise => "updates_promise",
            StoryEdgeKind::SupportsMission => "supports_mission",
            StoryEdgeKind::ContradictsCanon => "contradicts_canon",
            StoryEdgeKind::DependsOnResult => "depends_on_result",
            StoryEdgeKind::SameSourceRevision => "same_source_revision",
            StoryEdgeKind::SharedKeyword => "shared_keyword",
        }
    }
}

/// Extract seed nodes from the current observation and context pack.
pub fn extract_seed_nodes(
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
    memory: &WriterMemory,
) -> Vec<WriterStoryGraphNode> {
    let mut seeds = Vec::new();

    seeds.push(WriterStoryGraphNode {
        id: format!("task:{}", observation.id),
        kind: StoryNodeKind::SeedTask,
        label: "当前写作任务".to_string(),
        source_ref: format!("observation:{}", observation.id),
        source_revision: observation.chapter_revision.clone(),
        chapter: observation.chapter_title.clone(),
        confidence: 1.0,
        summary: format!("{:?} observation", observation.source),
    });

    // Current chapter mission.
    if let Some(chapter) = &observation.chapter_title {
        if let Ok(Some(mission)) = memory.get_chapter_mission(&observation.project_id, chapter) {
            seeds.push(WriterStoryGraphNode {
                id: format!("mission:{}", mission.chapter_title),
                kind: StoryNodeKind::ChapterMission,
                label: format!("章节任务: {}", mission.chapter_title),
                source_ref: format!("chapter_mission:{}", mission.chapter_title),
                source_revision: None,
                chapter: Some(mission.chapter_title.clone()),
                confidence: 0.9,
                summary: mission.mission,
            });
        }
    }

    // Open promises (top 3 by priority).
    if let Ok(open) = memory.get_open_promises() {
        for (kind, title, description, introduced) in open.iter().take(3) {
            seeds.push(WriterStoryGraphNode {
                id: format!("promise:{}:{}", kind, title),
                kind: StoryNodeKind::PlotPromise,
                label: title.clone(),
                source_ref: format!("promise:{}:{}", kind, title),
                source_revision: None,
                chapter: Some(introduced.clone()),
                confidence: 0.85,
                summary: description.clone(),
            });
        }
    }

    // Canon entities mentioned in cursor / selected text.
    if let Ok(entity_names) = memory.get_canon_entity_names() {
        for source in &context_pack.sources {
            if matches!(
                source.source,
                ContextSource::CursorPrefix | ContextSource::SelectedText
            ) {
                let text_lower = source.content.to_lowercase();
                for name in entity_names.iter().take(5) {
                    if text_lower.contains(&name.to_lowercase()) {
                        seeds.push(WriterStoryGraphNode {
                            id: format!("canon:{}", name),
                            kind: StoryNodeKind::CanonEntity,
                            label: name.clone(),
                            source_ref: format!("canon:{}", name),
                            source_revision: None,
                            chapter: None,
                            confidence: 0.8,
                            summary: format!("实体: {}", name),
                        });
                    }
                }
                break;
            }
        }
    }

    seeds
}

/// Build an in-memory story graph from memory ledger data.
pub fn build_story_graph(
    memory: &WriterMemory,
    project_id: &str,
) -> (Vec<WriterStoryGraphNode>, Vec<WriterStoryGraphEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Canon entities.
    if let Ok(entities) = memory.list_canon_entities() {
        for entity in &entities {
            let node_id = format!("canon:{}", entity.name);
            let attrs_str = entity
                .attributes
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| format!("{}: {}", k, v))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();

            nodes.push(WriterStoryGraphNode {
                id: node_id.clone(),
                kind: StoryNodeKind::CanonEntity,
                label: entity.name.clone(),
                source_ref: format!("canon:{}", entity.name),
                source_revision: None,
                chapter: None,
                confidence: entity.confidence as f32,
                summary: if attrs_str.is_empty() {
                    entity.summary.clone()
                } else {
                    attrs_str
                },
            });
        }
    }

    // Canon rules.
    if let Ok(rules) = memory.list_canon_rules(20) {
        for rule in &rules {
            let node_id = format!("canon_rule:{}", rule.rule);
            nodes.push(WriterStoryGraphNode {
                id: node_id,
                kind: StoryNodeKind::CanonRule,
                label: rule.rule.clone(),
                source_ref: format!("canon_rule:{}", rule.rule),
                source_revision: None,
                chapter: None,
                confidence: rule.priority as f32 / 10.0,
                summary: format!("类别: {}, 状态: {}", rule.category, rule.status),
            });
        }
    }

    // Open promises (with full summaries).
    if let Ok(promises) = memory.get_open_promise_summaries() {
        for p in &promises {
            let node_id = format!("promise:{}", p.id);
            nodes.push(WriterStoryGraphNode {
                id: node_id.clone(),
                kind: StoryNodeKind::PlotPromise,
                label: p.title.clone(),
                source_ref: format!("promise:{}", p.id),
                source_revision: None,
                chapter: Some(p.introduced_chapter.clone()),
                confidence: if p.risk == "high" { 0.5 } else { 0.85 },
                summary: format!(
                    "kind={} payoff={} risk={} desc={}",
                    p.kind, p.expected_payoff, p.risk, p.description
                ),
            });

            // Edge: promise ↔ entity by name overlap.
            for other in &nodes {
                if other.id != node_id {
                    let label_lower = p.title.to_lowercase();
                    if label_lower.contains(&other.label.to_lowercase())
                        || other.label.to_lowercase().contains(&label_lower)
                    {
                        edges.push(WriterStoryGraphEdge {
                            from: node_id.clone(),
                            to: other.id.clone(),
                            kind: StoryEdgeKind::UpdatesPromise,
                            evidence_ref: format!("promise_entity_overlap:{}:{}", p.id, other.id),
                            confidence: 0.6,
                        });
                    }
                }
            }
        }
    }

    // Chapter missions.
    if let Ok(missions) = memory.list_chapter_missions(project_id, 20) {
        for m in &missions {
            let node_id = format!("mission:{}", m.chapter_title);
            nodes.push(WriterStoryGraphNode {
                id: node_id.clone(),
                kind: StoryNodeKind::ChapterMission,
                label: format!("章节任务: {}", m.chapter_title),
                source_ref: format!("chapter_mission:{}", m.chapter_title),
                source_revision: None,
                chapter: Some(m.chapter_title.clone()),
                confidence: 0.9,
                summary: m.mission.clone(),
            });

            // Mission → entity edges.
            let mission_text = format!(
                "{} {} {} {}",
                m.mission, m.must_include, m.must_not, m.expected_ending
            )
            .to_lowercase();
            for other in &nodes {
                if other.id != node_id && mission_text.contains(&other.label.to_lowercase()) {
                    edges.push(WriterStoryGraphEdge {
                        from: node_id.clone(),
                        to: other.id.clone(),
                        kind: StoryEdgeKind::SupportsMission,
                        evidence_ref: format!("mission_mentions:{}:{}", m.chapter_title, other.id),
                        confidence: 0.75,
                    });
                }
            }
        }
    }

    // Recent chapter result feedback.
    if let Ok(results) = memory.list_recent_chapter_results(project_id, 5) {
        for r in &results {
            let node_id = format!("feedback:{}", r.id);
            nodes.push(WriterStoryGraphNode {
                id: node_id.clone(),
                kind: StoryNodeKind::ResultFeedback,
                label: format!("章节反馈: {}", r.chapter_title),
                source_ref: format!("result_feedback:{}", r.id),
                source_revision: Some(r.chapter_revision.clone()),
                chapter: Some(r.chapter_title.clone()),
                confidence: 0.8,
                summary: r.summary.clone(),
            });

            // Feedback → entity edges via promise_updates / canon_updates.
            for update in r.promise_updates.iter().chain(r.canon_updates.iter()) {
                for other in &nodes {
                    if other.id != node_id
                        && update.to_lowercase().contains(&other.label.to_lowercase())
                    {
                        edges.push(WriterStoryGraphEdge {
                            from: node_id.clone(),
                            to: other.id.clone(),
                            kind: StoryEdgeKind::DependsOnResult,
                            evidence_ref: format!("feedback_update:{}:{}", r.id, other.id),
                            confidence: 0.7,
                        });
                    }
                }
            }
        }
    }

    // Story contract.
    if let Ok(Some(contract)) = memory.get_story_contract(project_id) {
        nodes.push(WriterStoryGraphNode {
            id: "story_contract".to_string(),
            kind: StoryNodeKind::StoryContract,
            label: "故事合同".to_string(),
            source_ref: "story_contract".to_string(),
            source_revision: None,
            chapter: None,
            confidence: 1.0,
            summary: format!("{}: {}", contract.title, contract.reader_promise),
        });
    }

    // Recent creative decisions.
    if let Ok(decisions) = memory.list_recent_decisions(10) {
        for d in &decisions {
            let node_id = format!("decision:{}:{}", d.scope, d.title);
            nodes.push(WriterStoryGraphNode {
                id: node_id,
                kind: StoryNodeKind::Decision,
                label: d.title.clone(),
                source_ref: format!("decision:{}:{}", d.scope, d.title),
                source_revision: None,
                chapter: Some(d.scope.clone()),
                confidence: 0.85,
                summary: d.rationale.clone(),
            });
        }
    }

    // ContradictsCanon edges: canon rule ↔ entity where the rule restricts the entity.
    let canon_entity_ids: Vec<String> = nodes
        .iter()
        .filter(|n| matches!(n.kind, StoryNodeKind::CanonEntity))
        .map(|n| n.id.clone())
        .collect();
    let canon_rule_ids: Vec<String> = nodes
        .iter()
        .filter(|n| matches!(n.kind, StoryNodeKind::CanonRule))
        .map(|n| n.id.clone())
        .collect();
    for rule_id in &canon_rule_ids {
        if let Some(rule_node) = nodes.iter().find(|n| &n.id == rule_id) {
            let rule_lower = rule_node.label.to_lowercase();
            for entity_id in &canon_entity_ids {
                if let Some(entity_node) = nodes.iter().find(|n| &n.id == entity_id) {
                    let entity_label_lower = entity_node.label.to_lowercase();
                    // Edge if rule text mentions the entity, or entity summary mentions rule category.
                    if rule_lower.contains(&entity_label_lower)
                        || entity_node.summary.to_lowercase().contains(&rule_lower)
                    {
                        edges.push(WriterStoryGraphEdge {
                            from: rule_id.clone(),
                            to: entity_id.clone(),
                            kind: StoryEdgeKind::ContradictsCanon,
                            evidence_ref: format!(
                                "canon_rule_entity:{}:{}",
                                rule_node.label, entity_node.label
                            ),
                            confidence: 0.65,
                        });
                    }
                }
            }
        }
    }

    // SameSourceRevision edges: nodes sharing the same chapter or source revision.
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            let a = &nodes[i];
            let b = &nodes[j];
            let share_chapter =
                a.chapter.is_some() && b.chapter.is_some() && a.chapter == b.chapter;
            let share_revision = a.source_revision.is_some()
                && b.source_revision.is_some()
                && a.source_revision == b.source_revision;
            if share_chapter || share_revision {
                edges.push(WriterStoryGraphEdge {
                    from: a.id.clone(),
                    to: b.id.clone(),
                    kind: StoryEdgeKind::SameSourceRevision,
                    evidence_ref: format!(
                        "same_source:{}:{}",
                        a.chapter.as_deref().unwrap_or("?"),
                        b.chapter.as_deref().unwrap_or("?")
                    ),
                    confidence: 0.5,
                });
            }
        }
    }

    (nodes, edges)
}

/// Compute the story impact radius from seeds through the graph.
pub fn compute_story_impact_radius(
    seeds: &[WriterStoryGraphNode],
    graph_nodes: &[WriterStoryGraphNode],
    graph_edges: &[WriterStoryGraphEdge],
    budget_limit: usize,
) -> WriterStoryImpactRadius {
    let mut impacted_ids = std::collections::HashSet::new();
    let mut impacted_edges = Vec::new();
    let mut reasons = Vec::new();
    let mut total_chars: usize = 0;
    let mut truncated = false;

    for seed in seeds {
        impacted_ids.insert(seed.id.clone());
        total_chars += seed.summary.chars().count();
        reasons.push(format!("种子节点: {}", seed.label));
    }

    // Build adjacency index: node_id → outgoing edges (avoids O(n²) scan).
    let mut adjacency: std::collections::HashMap<&str, Vec<&WriterStoryGraphEdge>> =
        std::collections::HashMap::new();
    for edge in graph_edges {
        adjacency.entry(edge.from.as_str()).or_default().push(edge);
    }
    // Build node lookup: node_id → &node.
    let node_lookup: std::collections::HashMap<&str, &WriterStoryGraphNode> =
        graph_nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // BFS traversal from seeds with adjacency-indexed edges.
    let mut frontier: Vec<String> = seeds.iter().map(|s| s.id.clone()).collect();
    let mut depth = 0;

    while depth < MAX_TRAVERSAL_DEPTH && !frontier.is_empty() {
        let mut next_frontier = Vec::new();

        for node_id in &frontier {
            let outgoing = adjacency.get(node_id.as_str());
            for edge in outgoing.into_iter().flatten() {
                if impacted_ids.contains(edge.to.as_str()) {
                    continue;
                }
                if let Some(target) = node_lookup.get(edge.to.as_str()) {
                    let node_chars = target.summary.chars().count();
                    if total_chars + node_chars > budget_limit {
                        truncated = true;
                        if target.confidence > 0.7
                            || matches!(
                                target.kind,
                                StoryNodeKind::PlotPromise | StoryNodeKind::CanonEntity
                            )
                        {
                            reasons.push(format!(
                                "预算截断: {} (需要 {} 字符, 剩余 {} 字符)",
                                target.label,
                                node_chars,
                                budget_limit.saturating_sub(total_chars)
                            ));
                        }
                        continue;
                    }
                    total_chars += node_chars;
                    impacted_ids.insert(edge.to.clone());
                    next_frontier.push(edge.to.clone());
                    impacted_edges.push((*edge).clone());
                    reasons.push(format!(
                        "距离 {}: {} -> {} ({})",
                        depth + 1,
                        node_id,
                        target.label,
                        edge.kind.as_str()
                    ));
                }
            }
        }

        frontier = next_frontier;
        depth += 1;
    }

    if !frontier.is_empty() {
        truncated = true;
        reasons.push(format!(
            "遍历深度限制 ({}) {} 个节点未展开",
            MAX_TRAVERSAL_DEPTH,
            frontier.len()
        ));
    }

    let impacted_nodes: Vec<WriterStoryGraphNode> = graph_nodes
        .iter()
        .filter(|n| impacted_ids.contains(&n.id))
        .cloned()
        .collect();

    let impacted_sources: Vec<String> = impacted_nodes
        .iter()
        .map(|n| n.source_ref.clone())
        .collect();

    let has_high_risk = impacted_nodes.iter().any(|n| n.confidence < 0.6)
        || impacted_nodes
            .iter()
            .any(|n| matches!(n.kind, StoryNodeKind::PlotPromise));
    let has_medium_risk = truncated
        || impacted_nodes
            .iter()
            .any(|n| matches!(n.kind, StoryNodeKind::CanonEntity));
    let risk = if has_high_risk {
        StoryImpactRisk::High
    } else if has_medium_risk {
        StoryImpactRisk::Medium
    } else {
        StoryImpactRisk::Low
    };

    WriterStoryImpactRadius {
        seed_nodes: seeds.to_vec(),
        impacted_nodes,
        impacted_sources,
        edges: impacted_edges,
        risk,
        truncated,
        reasons,
    }
}

/// Compute impact radius and produce a budget report.
pub fn compute_story_impact(
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
    memory: &WriterMemory,
    budget_limit: Option<usize>,
) -> (WriterStoryImpactRadius, StoryImpactBudgetReport) {
    let budget = budget_limit.unwrap_or(DEFAULT_IMPACT_BUDGET_CHARS);
    let seeds = extract_seed_nodes(observation, context_pack, memory);
    let (graph_nodes, graph_edges) = build_story_graph(memory, &observation.project_id);

    let radius = compute_story_impact_radius(&seeds, &graph_nodes, &graph_edges, budget);

    let provided_chars: usize = radius
        .impacted_nodes
        .iter()
        .map(|n| n.summary.chars().count())
        .sum();
    let requested_chars: usize = graph_nodes.iter().map(|n| n.summary.chars().count()).sum();

    let dropped_high_risk: Vec<String> = radius
        .reasons
        .iter()
        .filter(|r| r.contains("预算截断"))
        .cloned()
        .collect();

    let report = StoryImpactBudgetReport {
        budget_limit: budget,
        requested_chars,
        provided_chars,
        truncated_node_count: if radius.truncated {
            graph_nodes
                .len()
                .saturating_sub(radius.impacted_nodes.len())
        } else {
            0
        },
        dropped_high_risk_sources: dropped_high_risk,
        reasons: radius.reasons.clone(),
    };

    (radius, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(
        id: &str,
        kind: StoryNodeKind,
        label: &str,
        confidence: f32,
        summary: &str,
    ) -> WriterStoryGraphNode {
        WriterStoryGraphNode {
            id: id.to_string(),
            kind,
            label: label.to_string(),
            source_ref: format!("src:{}", id),
            source_revision: None,
            chapter: None,
            confidence,
            summary: summary.to_string(),
        }
    }

    fn make_edge(from: &str, to: &str, kind: StoryEdgeKind) -> WriterStoryGraphEdge {
        WriterStoryGraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
            evidence_ref: format!("edge:{}->{}", from, to),
            confidence: 0.8,
        }
    }

    #[test]
    fn includes_impacted_promise_under_budget() {
        let seeds = vec![make_node(
            "task:1",
            StoryNodeKind::SeedTask,
            "当前任务",
            1.0,
            "写作第三章",
        )];
        let nodes = vec![
            seeds[0].clone(),
            make_node(
                "promise:1",
                StoryNodeKind::PlotPromise,
                "寒玉戒指去向",
                0.85,
                "伏笔: 寒玉戒指的去向",
            ),
            make_node(
                "canon:1",
                StoryNodeKind::CanonEntity,
                "林墨",
                0.9,
                "角色: 林墨",
            ),
        ];
        let edges = vec![
            make_edge("task:1", "promise:1", StoryEdgeKind::UpdatesPromise),
            make_edge("task:1", "canon:1", StoryEdgeKind::MentionsEntity),
        ];

        let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);

        assert!(
            radius
                .impacted_nodes
                .iter()
                .any(|n| n.kind == StoryNodeKind::PlotPromise),
            "Promise should be in impact radius"
        );
        assert!(!radius.truncated);
    }

    #[test]
    fn excludes_semantic_distractor() {
        let seeds = vec![make_node(
            "task:1",
            StoryNodeKind::SeedTask,
            "当前任务",
            1.0,
            "写作第三章：寒玉戒指",
        )];
        let nodes = vec![
            seeds[0].clone(),
            make_node(
                "canon:1",
                StoryNodeKind::CanonEntity,
                "霜铃塔",
                0.9,
                "地点: 霜铃塔",
            ),
            make_node(
                "canon:2",
                StoryNodeKind::CanonEntity,
                "旧门传闻",
                0.3,
                "无关: old_gate_rumor",
            ),
        ];
        let edges = vec![make_edge(
            "task:1",
            "canon:1",
            StoryEdgeKind::MentionsEntity,
        )];
        // canon:2 has NO edge — should be excluded.

        let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);

        assert!(
            radius.impacted_nodes.iter().any(|n| n.id == "canon:1"),
            "Connected canon should be included"
        );
        assert!(
            !radius.impacted_nodes.iter().any(|n| n.id == "canon:2"),
            "Disconnected distractor should be excluded"
        );
    }

    #[test]
    fn reports_truncated_sources() {
        let seeds = vec![make_node(
            "task:1",
            StoryNodeKind::SeedTask,
            "当前任务",
            1.0,
            "写作",
        )];
        let mut nodes = vec![seeds[0].clone()];
        let mut edges = Vec::new();
        for i in 1..=20 {
            nodes.push(make_node(
                &format!("promise:{}", i),
                StoryNodeKind::PlotPromise,
                &format!("伏笔{}", i),
                0.8,
                "这是一个需要大量字符的总结文本用来模拟长内容数据",
            ));
            edges.push(make_edge(
                "task:1",
                &format!("promise:{}", i),
                StoryEdgeKind::UpdatesPromise,
            ));
        }

        let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 200);

        assert!(radius.truncated, "Should be truncated under tight budget");
        assert!(
            radius
                .reasons
                .iter()
                .any(|r| r.contains("预算截断") || r.contains("深度")),
            "Should report truncation reason: {:?}",
            radius.reasons
        );
        assert!(
            radius.impacted_nodes.len() < nodes.len(),
            "Should have fewer impacted nodes than total ({} < {})",
            radius.impacted_nodes.len(),
            nodes.len()
        );
    }

    #[test]
    fn maps_operation_to_story_nodes() {
        let seeds = vec![
            make_node(
                "task:1",
                StoryNodeKind::SeedTask,
                "inline rewrite",
                1.0,
                "重写第三段",
            ),
            make_node(
                "mission:ch3",
                StoryNodeKind::ChapterMission,
                "第三章任务",
                0.9,
                "推进寒玉戒指线索",
            ),
        ];
        let nodes = vec![
            seeds[0].clone(),
            seeds[1].clone(),
            make_node(
                "promise:1",
                StoryNodeKind::PlotPromise,
                "寒玉戒指去向",
                0.85,
                "伏笔: 寒玉戒指去向",
            ),
        ];
        let edges = vec![make_edge(
            "mission:ch3",
            "promise:1",
            StoryEdgeKind::SupportsMission,
        )];

        let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);

        assert!(
            radius.impacted_nodes.len() >= 2,
            "Should include mission + promise"
        );
        assert!(
            radius
                .impacted_sources
                .iter()
                .any(|s| s.contains("promise:1")),
            "Promise should be in impacted sources"
        );
    }

    #[test]
    fn small_change_stays_minimal() {
        let seeds = vec![make_node(
            "task:1",
            StoryNodeKind::SeedTask,
            "光标微调",
            1.0,
            "fix typo",
        )];
        let nodes = vec![
            seeds[0].clone(),
            make_node("canon:1", StoryNodeKind::CanonEntity, "林墨", 0.9, "角色"),
        ];
        let edges = vec![];

        let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);

        assert_eq!(radius.impacted_nodes.len(), 1);
        assert!(!radius.truncated);
        assert!(matches!(radius.risk, StoryImpactRisk::Low));
    }
}
