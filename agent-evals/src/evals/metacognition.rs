use super::*;
use agent_writer_lib::writer_agent::metacognition::{
    WriterMetacognitiveAction, WriterMetacognitiveRiskLevel,
};
use agent_writer_lib::writer_agent::task_receipt::{
    WriterFailureCategory, WriterFailureEvidenceBundle,
};

pub fn run_metacognitive_snapshot_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let _ = kernel.observe(observation("林墨停在门前，反复确认旧案线索。"));

    let bundle = WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ContextMissing,
        "context_pack_missing_required_source",
        "Required context was missing before the next write.",
        true,
        Some("eval-meta-task".to_string()),
        vec!["context:chapter_mission".to_string()],
        serde_json::json!({
            "missingSource": "chapter_mission",
            "reason": "eval exercises metacognitive failure handling"
        }),
        vec!["rebuild_context_pack: Run Planning Review before drafting.".to_string()],
        now_ms(),
    );
    kernel.record_failure_evidence_bundle(&bundle);

    let trace = kernel.trace_snapshot(40);
    let meta = trace.metacognitive_snapshot;
    let inspector = kernel.inspector_timeline(40);
    let export = kernel.export_trajectory(40);

    let mut errors = Vec::new();
    if meta.risk_level < WriterMetacognitiveRiskLevel::High {
        errors.push(format!(
            "risk level should escalate, got {:?}",
            meta.risk_level
        ));
    }
    if !matches!(
        meta.recommended_action,
        WriterMetacognitiveAction::SwitchToPlanningReview
            | WriterMetacognitiveAction::AskClarifyingQuestion
            | WriterMetacognitiveAction::RunContinuityDiagnostic
            | WriterMetacognitiveAction::BlockWriteUntilAuthorConfirms
    ) {
        errors.push(format!(
            "expected a non-write recovery action, got {:?}",
            meta.recommended_action
        ));
    }
    if meta.recent_failure_count == 0 {
        errors.push("recent failure count was not surfaced".to_string());
    }
    if meta.reasons.is_empty() || meta.remediation.is_empty() {
        errors.push("metacognitive snapshot lacks reason/remediation".to_string());
    }
    if !inspector.events.iter().any(|event| {
        event.kind
            == agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::Metacognition
    }) {
        errors.push("inspector timeline lacks metacognition event".to_string());
    }
    if !export
        .jsonl
        .lines()
        .any(|line| line.contains("\"eventType\":\"writer.metacognition\""))
    {
        errors.push("trajectory export lacks metacognition event".to_string());
    }

    eval_result(
        "writer_agent:metacognitive_snapshot",
        format!(
            "risk={:?} action={:?} failures={} exportEvents={}",
            meta.risk_level, meta.recommended_action, meta.recent_failure_count, export.event_count
        ),
        errors,
    )
}

pub fn run_metacognitive_gate_blocks_write_run_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    seed_metacognitive_failure(&mut kernel);

    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o-mini",
        ),
    );
    let ghost_request = WriterAgentRunRequest {
        task: WriterAgentTask::GhostWriting,
        observation: observation("林墨准备继续写下去。"),
        user_instruction: "续写一句。".to_string(),
        frontend_state: WriterAgentFrontendState::default(),
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let blocked = kernel.prepare_task_run(
        ghost_request,
        provider.clone(),
        EvalToolHandler,
        "gpt-4o-mini",
    );

    let planning_request = WriterAgentRunRequest {
        task: WriterAgentTask::PlanningReview,
        observation: observation("林墨准备继续写下去。"),
        user_instruction: "先只读审查风险，不要写正文。".to_string(),
        frontend_state: WriterAgentFrontendState::default(),
        approval_mode: WriterAgentApprovalMode::ReadOnly,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let allowed_recovery =
        kernel.prepare_task_run(planning_request, provider, EvalToolHandler, "gpt-4o-mini");
    let events = kernel.run_events(20);

    let mut errors = Vec::new();
    if blocked.is_ok() {
        errors.push("ghost writing run was not blocked by metacognitive gate".to_string());
    }
    if !blocked
        .as_ref()
        .err()
        .is_some_and(|error| error.contains("Metacognitive gate blocked"))
    {
        errors.push("blocked write run did not return metacognitive gate reason".to_string());
    }
    if let Err(error) = &allowed_recovery {
        errors.push(format!(
            "planning review should remain available as recovery path: {}",
            error
        ));
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.metacognitive_gate_blocked")
    {
        errors.push("metacognitive gate block event was not recorded".to_string());
    }

    eval_result(
        "writer_agent:metacognitive_gate_blocks_write_run",
        format!(
            "blocked={} recovery={} events={}",
            blocked.is_err(),
            allowed_recovery.is_ok(),
            events.len()
        ),
        errors,
    )
}

pub fn run_metacognitive_gate_blocks_approved_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    seed_metacognitive_failure(&mut kernel);

    let operation = WriterOperation::TextInsert {
        chapter: "Chapter-1".to_string(),
        at: 0,
        text: "林墨没有继续冒进。".to_string(),
        revision: "rev-1".to_string(),
    };
    let approval = eval_approval("metacognitive_gate_eval");
    let result = kernel
        .approve_editor_operation_with_approval(operation, "rev-1", Some(&approval))
        .unwrap();
    let trace = kernel.trace_snapshot(30);

    let mut errors = Vec::new();
    if result.success {
        errors.push("approved write operation bypassed metacognitive gate".to_string());
    }
    if !result
        .error
        .as_ref()
        .is_some_and(|error| error.message.contains("Metacognitive gate blocked"))
    {
        errors.push("operation error did not expose metacognitive gate reason".to_string());
    }
    if !trace
        .run_events
        .iter()
        .any(|event| event.event_type == "writer.metacognitive_gate_blocked")
    {
        errors.push("operation gate block event was not recorded".to_string());
    }
    if !trace.operation_lifecycle.iter().any(|lifecycle| {
        lifecycle.proposal_id == approval.proposal_id
            && lifecycle.state
                == agent_writer_lib::writer_agent::kernel::WriterOperationLifecycleState::Rejected
    }) {
        errors.push("blocked operation did not record rejected lifecycle".to_string());
    }

    eval_result(
        "writer_agent:metacognitive_gate_blocks_approved_operation",
        format!(
            "success={} runEvents={} lifecycle={}",
            result.success,
            trace.run_events.len(),
            trace.operation_lifecycle.len()
        ),
        errors,
    )
}

pub fn run_metacognitive_gate_allows_recovery_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩线索推进",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    seed_metacognitive_failure(&mut kernel);

    let operation = WriterOperation::ChapterMissionUpsert {
        mission: agent_writer_lib::writer_agent::operation::ChapterMissionOp {
            project_id: "eval".to_string(),
            chapter_title: "Chapter-1".to_string(),
            mission: "林墨追查玉佩下落。".to_string(),
            must_include: "玉佩线索推进".to_string(),
            must_not: "提前揭开真相".to_string(),
            expected_ending: "以误导线索收束。".to_string(),
            status: "drifted".to_string(),
            source_ref: "result_feedback:eval".to_string(),
            blocked_reason: String::new(),
            retired_history: String::new(),
        },
    };
    let approval = eval_approval("metacognitive_recovery_calibration");
    let result = kernel
        .approve_editor_operation_with_approval(operation, "", Some(&approval))
        .unwrap();
    let trace = kernel.trace_snapshot(40);
    let mission = kernel.ledger_snapshot().active_chapter_mission;

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "recovery calibration operation was blocked: {:?}",
            result.error.as_ref().map(|error| error.message.as_str())
        ));
    }
    if !mission.is_some_and(|mission| mission.status == "drifted") {
        errors.push("recovery calibration did not update chapter mission".to_string());
    }
    if trace
        .run_events
        .iter()
        .any(|event| event.event_type == "writer.metacognitive_gate_blocked")
    {
        errors.push("recovery operation recorded a metacognitive block event".to_string());
    }

    eval_result(
        "writer_agent:metacognitive_gate_allows_recovery_operation",
        format!(
            "success={} runEvents={} lifecycle={}",
            result.success,
            trace.run_events.len(),
            trace.operation_lifecycle.len()
        ),
        errors,
    )
}

fn seed_metacognitive_failure(kernel: &mut WriterAgentKernel) {
    let bundle = WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ContextMissing,
        "context_pack_missing_required_source",
        "Required context was missing before the next write.",
        true,
        Some("eval-meta-task".to_string()),
        vec!["context:chapter_mission".to_string()],
        serde_json::json!({
            "missingSource": "chapter_mission",
            "reason": "eval exercises metacognitive write gate"
        }),
        vec!["rebuild_context_pack: Run Planning Review before drafting.".to_string()],
        now_ms(),
    );
    kernel.record_failure_evidence_bundle(&bundle);
}
