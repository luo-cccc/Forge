#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::context::AgentTask;
use agent_writer_lib::writer_agent::kernel::{
    WriterAgentApprovalMode, WriterAgentFrontendState, WriterAgentRunRequest,
    WriterAgentStreamMode, WriterAgentTask,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::story_impact::{
    build_story_graph, compute_story_impact, compute_story_impact_radius, extract_seed_nodes,
    StoryEdgeKind, StoryImpactRisk, StoryNodeKind, WriterStoryGraphEdge, WriterStoryGraphNode,
};
use agent_writer_lib::writer_agent::WriterAgentKernel;

fn make_si_node(
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

fn make_si_edge(from: &str, to: &str, kind: StoryEdgeKind) -> WriterStoryGraphEdge {
    WriterStoryGraphEdge {
        from: from.to_string(),
        to: to.to_string(),
        kind,
        evidence_ref: format!("edge:{}->{}", from, to),
        confidence: 0.8,
    }
}

pub fn run_story_impact_radius_includes_impacted_promise_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "task:1",
        StoryNodeKind::SeedTask,
        "",
        1.0,
        "writing ch3",
    )];
    let nodes = vec![
        seeds[0].clone(),
        make_si_node(
            "promise:1",
            StoryNodeKind::PlotPromise,
            "jade ring",
            0.85,
            "promise: jade ring",
        ),
        make_si_node(
            "canon:1",
            StoryNodeKind::CanonEntity,
            "Lin Mo",
            0.9,
            "char: Lin Mo",
        ),
    ];
    let edges = vec![
        make_si_edge("task:1", "promise:1", StoryEdgeKind::UpdatesPromise),
        make_si_edge("task:1", "canon:1", StoryEdgeKind::MentionsEntity),
    ];
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);
    if !radius
        .impacted_nodes
        .iter()
        .any(|n| n.kind == StoryNodeKind::PlotPromise)
    {
        errors.push("promise should be in impact radius".to_string());
    }
    if radius.truncated {
        errors.push("should not be truncated".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_includes_impacted_promise_under_budget",
        format!(
            "impacted={} truncated={}",
            radius.impacted_nodes.len(),
            radius.truncated
        ),
        errors,
    )
}

pub fn run_story_impact_radius_excludes_semantic_distractor_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "task:1",
        StoryNodeKind::SeedTask,
        "",
        1.0,
        "writing ch3",
    )];
    let nodes = vec![
        seeds[0].clone(),
        make_si_node(
            "canon:1",
            StoryNodeKind::CanonEntity,
            "frost tower",
            0.9,
            "place",
        ),
        make_si_node(
            "canon:2",
            StoryNodeKind::CanonEntity,
            "old rumor",
            0.3,
            "noise",
        ),
    ];
    let edges = vec![make_si_edge(
        "task:1",
        "canon:1",
        StoryEdgeKind::MentionsEntity,
    )];
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);
    if !radius.impacted_nodes.iter().any(|n| n.id == "canon:1") {
        errors.push("connected canon should be included".to_string());
    }
    if radius.impacted_nodes.iter().any(|n| n.id == "canon:2") {
        errors.push("disconnected distractor should be excluded".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_excludes_semantic_distractor",
        format!(
            "impacted={} excluded={}",
            radius.impacted_nodes.len(),
            !radius.impacted_nodes.iter().any(|n| n.id == "canon:2")
        ),
        errors,
    )
}

pub fn run_story_impact_radius_reports_truncated_sources_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "task:1",
        StoryNodeKind::SeedTask,
        "",
        1.0,
        "writing",
    )];
    let mut nodes = vec![seeds[0].clone()];
    let mut edges = Vec::new();
    for i in 1..=20 {
        nodes.push(make_si_node(
            &format!("p:{}", i),
            StoryNodeKind::PlotPromise,
            "p",
            0.8,
            "long summary text for budget testing purposes xyz",
        ));
        edges.push(make_si_edge(
            "task:1",
            &format!("p:{}", i),
            StoryEdgeKind::UpdatesPromise,
        ));
    }
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 200);
    if !radius.truncated {
        errors.push("should be truncated".to_string());
    }
    if !radius.reasons.iter().any(|r| {
        r.contains("trunc")
            || r.contains("depth")
            || r.contains("limit")
            || r.contains("预算")
            || r.contains("深度")
            || r.contains("遍历")
    }) {
        errors.push(format!("no truncation reason: {:?}", radius.reasons));
    }
    if radius.impacted_nodes.len() >= nodes.len() {
        errors.push("too many nodes".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_reports_truncated_sources",
        format!(
            "truncated={} impacted={}/{}",
            radius.truncated,
            radius.impacted_nodes.len(),
            nodes.len()
        ),
        errors,
    )
}

pub fn run_story_impact_radius_maps_operation_to_story_nodes_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![
        make_si_node("task:1", StoryNodeKind::SeedTask, "", 1.0, "rewrite"),
        make_si_node(
            "mission:ch3",
            StoryNodeKind::ChapterMission,
            "ch3 mission",
            0.9,
            "advance jade ring",
        ),
    ];
    let nodes = vec![
        seeds[0].clone(),
        seeds[1].clone(),
        make_si_node(
            "promise:1",
            StoryNodeKind::PlotPromise,
            "jade ring",
            0.85,
            "promise",
        ),
    ];
    let edges = vec![make_si_edge(
        "mission:ch3",
        "promise:1",
        StoryEdgeKind::SupportsMission,
    )];
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);
    if radius.impacted_nodes.len() < 2 {
        errors.push("too few impacted nodes".to_string());
    }
    if !radius
        .impacted_sources
        .iter()
        .any(|s| s.contains("promise:1"))
    {
        errors.push("promise not in sources".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_maps_operation_to_story_nodes",
        format!(
            "impacted={} sources={}",
            radius.impacted_nodes.len(),
            radius.impacted_sources.len()
        ),
        errors,
    )
}

pub fn run_story_impact_radius_traverses_reverse_edges_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "canon:1",
        StoryNodeKind::CanonEntity,
        "jade ring",
        0.9,
        "entity: jade ring",
    )];
    let nodes = vec![
        seeds[0].clone(),
        make_si_node(
            "mission:ch3",
            StoryNodeKind::ChapterMission,
            "ch3 mission",
            0.9,
            "advance jade ring clue",
        ),
    ];
    let edges = vec![make_si_edge(
        "mission:ch3",
        "canon:1",
        StoryEdgeKind::SupportsMission,
    )];
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);
    if !radius.impacted_nodes.iter().any(|n| n.id == "mission:ch3") {
        errors.push("reverse traversal should include dependent mission".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_traverses_reverse_edges",
        format!(
            "impacted={} reverseIncluded={}",
            radius.impacted_nodes.len(),
            radius.impacted_nodes.iter().any(|n| n.id == "mission:ch3")
        ),
        errors,
    )
}

pub fn run_story_impact_radius_memory_seed_ids_align_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指",
            "寒玉戒指被黑衣人夺走，必须在第五章回收。",
            "Chapter-2",
            "Chapter-5",
            8,
        )
        .unwrap();
    let observation = observation_in_chapter("林墨摸了摸空荡荡的手指。", "Chapter-3");
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(AgentTask::GhostWriting, &observation);
    let seeds = extract_seed_nodes(&observation, &pack, &kernel.memory);
    let (nodes, _edges) = build_story_graph(&kernel.memory, "eval");
    let expected_id = format!("promise:{}", promise_id);
    if !seeds.iter().any(|n| n.id == expected_id) {
        errors.push(format!("seed missing expected promise id {}", expected_id));
    }
    if !nodes.iter().any(|n| n.id == expected_id) {
        errors.push(format!("graph missing expected promise id {}", expected_id));
    }
    let (radius, _budget) = compute_story_impact(&observation, &pack, &kernel.memory, Some(500));
    if !radius.impacted_nodes.iter().any(|n| n.id == expected_id) {
        errors.push("radius should include memory-backed promise seed".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_memory_seed_ids_align",
        format!(
            "seed={} graph={} impacted={}",
            seeds.iter().any(|n| n.id == expected_id),
            nodes.iter().any(|n| n.id == expected_id),
            radius.impacted_nodes.iter().any(|n| n.id == expected_id)
        ),
        errors,
    )
}

struct StoryImpactEvalToolHandler;

#[async_trait::async_trait]
impl agent_harness_core::ToolHandler for StoryImpactEvalToolHandler {
    async fn execute(
        &self,
        tool_name: &str,
        _args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"reachedHandler": true, "tool": tool_name}))
    }
}

pub fn run_story_impact_radius_run_event_links_observation_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "寒玉戒指线推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "",
        )
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指",
            "寒玉戒指被黑衣人夺走，必须回收。",
            "Chapter-2",
            "Chapter-5",
            8,
        )
        .unwrap();

    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation_in_chapter("林墨摸了摸空荡荡的手指。", "Chapter-3");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    let observation_id = obs.id.clone();
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "这段接下来怎么推进？".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨摸了摸空荡荡的手指。".to_string(),
            paragraph: "林墨摸了摸空荡荡的手指。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o-mini",
        ),
    );
    if let Err(error) =
        kernel.prepare_task_run(request, provider, StoryImpactEvalToolHandler, "gpt-4o-mini")
    {
        errors.push(format!("prepare_task_run failed: {}", error));
    }
    let trace = kernel.trace_snapshot(20);
    let event = trace
        .run_events
        .iter()
        .find(|event| event.event_type == "writer.story_impact_radius_built");
    match event {
        Some(event) => {
            if event.task_id.as_deref() != Some(observation_id.as_str()) {
                errors.push(format!(
                    "story impact event task_id should be observation id, got {:?}",
                    event.task_id
                ));
            }
            if !event
                .source_refs
                .iter()
                .any(|source| source == &observation_id)
            {
                errors.push(format!(
                    "story impact event source_refs missing observation id: {:?}",
                    event.source_refs
                ));
            }
            if event
                .data
                .get("observationId")
                .and_then(|value| value.as_str())
                != Some(observation_id.as_str())
            {
                errors.push(format!(
                    "story impact event payload missing observationId: {:?}",
                    event.data
                ));
            }
        }
        None => errors.push("missing writer.story_impact_radius_built run event".to_string()),
    }

    eval_result(
        "writer_agent:story_impact_radius_run_event_links_observation",
        format!("event={} observation={}", event.is_some(), observation_id),
        errors,
    )
}

pub fn run_story_impact_radius_small_change_stays_minimal_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "task:1",
        StoryNodeKind::SeedTask,
        "",
        1.0,
        "fix typo",
    )];
    let nodes = vec![
        seeds[0].clone(),
        make_si_node("canon:1", StoryNodeKind::CanonEntity, "Lin", 0.9, "char"),
    ];
    let edges = vec![];
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);
    if radius.impacted_nodes.len() != 1 {
        errors.push(format!(
            "expected 1 node, got {}",
            radius.impacted_nodes.len()
        ));
    }
    if radius.truncated {
        errors.push("should not be truncated".to_string());
    }
    if !matches!(radius.risk, StoryImpactRisk::Low) {
        errors.push("should be low risk".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_small_change_stays_minimal",
        format!(
            "impacted={} risk={:?}",
            radius.impacted_nodes.len(),
            radius.risk
        ),
        errors,
    )
}
