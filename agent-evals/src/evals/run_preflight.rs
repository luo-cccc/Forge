#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::kernel::{
    WriterAgentApprovalMode, WriterAgentFrontendState, WriterAgentRunRequest,
    WriterAgentStreamMode, WriterAgentTask,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;

pub fn run_preflight_ready_for_safe_planning_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "Test",
            "fantasy",
            "reader promise",
            "hero journey",
            "",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let observation = observation_in_chapter("planning review", "Chapter-1");
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::PlanningReview,
        observation,
        user_instruction: String::new(),
        frontend_state: WriterAgentFrontendState::default(),
        approval_mode: WriterAgentApprovalMode::ReadOnly,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: vec![],
    };
    let report = kernel.preflight(&request);
    if report.readiness == "blocked" {
        errors.push(format!(
            "PlanningReview should not be blocked: {:?}",
            report.blocks
        ));
    }
    eval_result(
        "writer_agent:run_preflight_ready_for_safe_planning",
        format!(
            "readiness={} warnings={} blocks={}",
            report.readiness,
            report.warnings.len(),
            report.blocks.len()
        ),
        errors,
    )
}

pub fn run_preflight_warns_provider_budget_approval_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "Test",
            "fantasy",
            "reader promise",
            "hero journey",
            "",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    // Large paragraph to push estimated tokens up
    let large_text = "a".repeat(12_000);
    let observation = observation_in_chapter(&large_text, "Chapter-1");
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation,
        user_instruction: String::new(),
        frontend_state: WriterAgentFrontendState::default(),
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: vec![],
    };
    let report = kernel.preflight(&request);
    if report.estimated_input_tokens < 100 {
        errors.push("estimated_input_tokens too low".to_string());
    }
    eval_result(
        "writer_agent:run_preflight_warns_provider_budget_approval",
        format!(
            "readiness={} estimatedTokens={}",
            report.readiness, report.estimated_input_tokens
        ),
        errors,
    )
}

pub fn run_preflight_reports_story_impact_truncation_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "Test",
            "fantasy",
            "reader promise",
            "hero journey",
            "",
        )
        .unwrap();
    for i in 0..30 {
        memory
            .upsert_canon_entity(
                "character",
                &format!("Entity{}", i),
                &[],
                &format!("Entity {} summary", i),
                &serde_json::Value::Object(serde_json::Map::new()),
                0.8,
            )
            .ok();
    }
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let observation = observation_in_chapter("Entity1 went to the tower", "Chapter-1");
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::GhostWriting,
        observation,
        user_instruction: String::new(),
        frontend_state: WriterAgentFrontendState::default(),
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: vec![],
    };
    let report = kernel.preflight(&request);
    if report.story_impact_risk.is_empty() {
        errors.push("story_impact_risk should not be empty".to_string());
    }
    eval_result(
        "writer_agent:run_preflight_reports_story_impact_truncation",
        format!(
            "risk={} truncated={} sources={}",
            report.story_impact_risk, report.story_impact_truncated, report.context_source_count
        ),
        errors,
    )
}

pub fn run_preflight_blocks_metacognitive_write_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "Test",
            "fantasy",
            "reader promise",
            "hero journey",
            "",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let session_id = kernel.session_id.clone();
    kernel
        .memory
        .record_run_event(&agent_writer_lib::writer_agent::memory::RunEventSummary {
            seq: 1,
            project_id: "eval".to_string(),
            session_id,
            task_id: Some("task-1".to_string()),
            event_type: "writer.error".to_string(),
            source_refs: vec!["source:ChapterMission".to_string()],
            data: serde_json::json!({
                "category": "ContextMissing",
                "code": "context_pack_missing_required_source"
            }),
            ts_ms: 1,
        })
        .ok();
    let observation = observation_in_chapter("write chapter 3", "Chapter-3");
    // GhostWriting is write-sensitive → should trigger metacognitive gate
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::GhostWriting,
        observation,
        user_instruction: String::new(),
        frontend_state: WriterAgentFrontendState::default(),
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: vec![],
    };
    let report = kernel.preflight(&request);
    let snapshot_after = kernel.trace_snapshot(20);
    if snapshot_after
        .recent_observations
        .iter()
        .any(|observation| observation.id == report.observation_id)
    {
        errors.push("preflight should not record observation traces".to_string());
    }
    if snapshot_after
        .recent_proposals
        .iter()
        .any(|proposal| proposal.observation_id == report.observation_id)
    {
        errors.push("preflight should not register proposals".to_string());
    }
    if report.readiness != "blocked" {
        errors.push(format!(
            "expected readiness=blocked for metacognitive write risk, got {}",
            report.readiness
        ));
    }
    if !report
        .blocks
        .iter()
        .any(|block| block.code == "metacognitive_blocked")
    {
        errors.push(format!(
            "missing metacognitive_blocked block: {:?}",
            report.blocks
        ));
    }
    eval_result(
        "writer_agent:run_preflight_blocks_metacognitive_write",
        format!(
            "readiness={} warnings={} blocks={}",
            report.readiness,
            report.warnings.len(),
            report.blocks.len()
        ),
        errors,
    )
}
