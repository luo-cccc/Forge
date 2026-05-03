use super::*;
use agent_harness_core::{
    FeedbackContract, Intent, RequiredContext, TaskBelief, TaskPacket, TaskScope,
    ToolPolicyContract, ToolSideEffectLevel,
};

pub fn run_trajectory_export_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let _ = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let export = kernel.export_trajectory(20);
    let lines = export.jsonl.lines().collect::<Vec<_>>();

    let mut errors = Vec::new();
    if export.schema != "forge-writer-agent-trajectory" {
        errors.push(format!("unexpected trajectory schema {}", export.schema));
    }
    if export.event_count == 0 || lines.len() != export.event_count {
        errors.push(format!(
            "event count mismatch count={} lines={}",
            export.event_count,
            lines.len()
        ));
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.observation\""))
    {
        errors.push("missing observation trajectory event".to_string());
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.proposal\""))
    {
        errors.push("missing proposal trajectory event".to_string());
    }
    if !lines
        .iter()
        .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    {
        errors.push("trajectory contains invalid jsonl line".to_string());
    }

    eval_result(
        "writer_agent:trajectory_export_jsonl",
        format!("events={} bytes={}", export.event_count, export.jsonl.len()),
        errors,
    )
}

pub fn run_append_only_run_event_store_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角惯用武器是寒影刀",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation(
            "林墨拔出长剑，指向门外的人，准备在审讯中逼问玉佩的下落。",
        ))
        .unwrap();
    let mut packet = TaskPacket::new(
        "eval-run-event-task",
        "Record an append-only timeline for this writer agent run.",
        TaskScope::Chapter,
        now_ms(),
    );
    packet.scope_ref = Some("Chapter-1".to_string());
    packet.intent = Some(Intent::AnalyzeText);
    packet.constraints = vec!["Do not write manuscript text from this timeline check.".to_string()];
    packet.success_criteria = vec!["Run events can be replayed by sequence.".to_string()];
    packet.beliefs = vec![TaskBelief::new(
        "timeline",
        "Append-only events preserve the writer run order.",
        0.9,
    )];
    packet.required_context = vec![RequiredContext::new(
        "observation",
        "Anchor the timeline to the triggering observation.",
        200,
        true,
    )];
    packet.tool_policy = ToolPolicyContract {
        max_side_effect_level: ToolSideEffectLevel::Read,
        allow_approval_required: false,
        required_tool_tags: vec!["project".to_string()],
    };
    packet.feedback = FeedbackContract {
        expected_signals: vec!["timeline replayed".to_string()],
        checkpoints: vec!["run event store exported".to_string()],
        memory_writes: Vec::new(),
    };
    kernel
        .record_task_packet("eval-observation", "RunEventStore", packet)
        .unwrap();
    if let Some(proposal) = proposals.first() {
        let feedback = ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("eval rejects proposal to exercise feedback timeline".to_string()),
            created_at: now_ms(),
        };
        kernel.apply_feedback(feedback).unwrap();
    }

    let events = kernel.run_events(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();

    let mut errors = Vec::new();
    if events.is_empty() {
        errors.push("run event store is empty".to_string());
    }
    let seqs = events.iter().map(|event| event.seq).collect::<Vec<_>>();
    let expected = (1..=events.len() as u64).collect::<Vec<_>>();
    if seqs != expected {
        errors.push(format!("run event seqs are not append-only: {:?}", seqs));
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.observation")
    {
        errors.push("missing observation run event".to_string());
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.proposal_created")
    {
        errors.push("missing proposal_created run event".to_string());
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.task_packet_created")
    {
        errors.push("missing task_packet_created run event".to_string());
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.feedback_recorded")
    {
        errors.push("missing feedback_recorded run event".to_string());
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.operation_lifecycle")
    {
        errors.push("missing operation_lifecycle run event".to_string());
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.run_event\""))
    {
        errors.push("trajectory export lacks writer.run_event entries".to_string());
    }

    eval_result(
        "writer_agent:append_only_run_event_store",
        format!("events={} trajectory_lines={}", events.len(), lines.len()),
        errors,
    )
}

pub fn run_inspector_timeline_hides_from_default_companion_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "玉佩线推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let _ = kernel
        .observe(observation("林墨停在旧门前，想起张三带走的玉佩。"))
        .unwrap();
    let mut packet = TaskPacket::new(
        "eval-inspector-task",
        "Build an inspector timeline for this writer run.",
        TaskScope::Chapter,
        now_ms(),
    );
    packet.scope_ref = Some("Chapter-1".to_string());
    packet.intent = Some(Intent::AnalyzeText);
    packet.constraints = vec!["Inspect only.".to_string()];
    packet.success_criteria = vec!["Timeline separates companion and inspector.".to_string()];
    packet.beliefs = vec![TaskBelief::new(
        "trace",
        "Inspector owns internal run details.",
        0.9,
    )];
    packet.required_context = vec![RequiredContext::new(
        "observation",
        "Anchor timeline.",
        120,
        true,
    )];
    packet.tool_policy = ToolPolicyContract {
        max_side_effect_level: ToolSideEffectLevel::Read,
        allow_approval_required: false,
        required_tool_tags: vec!["project".to_string()],
    };
    packet.feedback = FeedbackContract {
        expected_signals: vec!["timeline inspected".to_string()],
        checkpoints: vec!["companion remains quiet".to_string()],
        memory_writes: Vec::new(),
    };
    kernel
        .record_task_packet("eval-observation", "InspectorTimeline", packet)
        .unwrap();

    let inspector = kernel.inspector_timeline(50);
    let companion = kernel.companion_timeline_summary();

    let mut errors = Vec::new();
    if !inspector.includes_internal_trace
        || inspector.audience
            != agent_writer_lib::writer_agent::inspector::WriterTimelineAudience::Inspector
    {
        errors.push("inspector timeline is not marked as internal inspector view".to_string());
    }
    if !inspector.events.iter().any(|event| {
        event.kind == agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::TaskPacket
    }) {
        errors.push("inspector timeline lacks task packet event".to_string());
    }
    if !inspector.events.iter().any(|event| {
        event.kind == agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::RunEvent
    }) {
        errors.push("inspector timeline lacks run event replay entries".to_string());
    }
    if companion.includes_internal_trace {
        errors.push("companion summary is marked as containing internal trace".to_string());
    }
    if companion.events.iter().any(|event| {
        matches!(
            event.kind,
            agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::TaskPacket
                | agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::RunEvent
                | agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::OperationLifecycle
        )
    }) {
        errors.push("default companion summary exposes internal timeline events".to_string());
    }

    eval_result(
        "writer_agent:inspector_timeline_hides_from_default_companion",
        format!(
            "inspectorEvents={} companionEvents={}",
            inspector.events.len(),
            companion.events.len()
        ),
        errors,
    )
}

pub fn run_trajectory_export_has_redaction_warning_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let _ = kernel
        .observe(observation("林墨停在旧门前，想起张三带走的玉佩。"))
        .unwrap();
    let export = kernel.export_trajectory(20);

    let mut errors = Vec::new();
    if export.redaction_warning.trim().is_empty() {
        errors.push("trajectory export lacks redaction warning".to_string());
    }
    for expected in ["manuscript", "project memory", "author feedback"] {
        if !export.redaction_warning.contains(expected) {
            errors.push(format!(
                "redaction warning missing sensitive content class: {}",
                expected
            ));
        }
    }
    if !export.local_only {
        errors.push("trajectory export is not local-only by default".to_string());
    }
    if export.jsonl.trim().is_empty() {
        errors.push("trajectory export jsonl is empty".to_string());
    }

    eval_result(
        "writer_agent:trajectory_export_has_redaction_warning",
        format!(
            "warningLen={} localOnly={} events={}",
            export.redaction_warning.len(),
            export.local_only,
            export.event_count
        ),
        errors,
    )
}

pub fn run_post_write_diagnostics_recorded_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角惯用武器是寒影刀",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨拔出长剑，准备逼问玉佩的下落。", "Chapter-3");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    save.editor_dirty = false;
    save.chapter_revision = Some("rev-save-1".to_string());
    let proposals = kernel.observe(save).unwrap();

    let snapshot = kernel.trace_snapshot(50);
    let events = kernel.run_events(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let report = snapshot.post_write_diagnostics.first();

    let mut errors = Vec::new();
    if report.is_none() {
        errors.push("trace snapshot lacks post-write diagnostic report".to_string());
    }
    if let Some(report) = report {
        if report.total_count == 0 {
            errors.push("post-write diagnostic report recorded zero diagnostics".to_string());
        }
        if report.error_count == 0 {
            errors
                .push("post-write diagnostic report lacks canon-conflict error count".to_string());
        }
        if !report
            .source_refs
            .iter()
            .any(|source| source == "canon:林墨")
        {
            errors.push(format!(
                "post-write diagnostic report lacks canon evidence refs: {:?}",
                report.source_refs
            ));
        }
        if report.remediation.is_empty() {
            errors.push("post-write diagnostic report lacks remediation".to_string());
        }
    }
    if !events
        .iter()
        .any(|event| event.event_type == "writer.post_write_diagnostics")
    {
        errors.push("run event store lacks writer.post_write_diagnostics".to_string());
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.post_write_diagnostics\""))
    {
        errors.push("trajectory export lacks writer.post_write_diagnostics event".to_string());
    }
    if !proposals.iter().any(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.reference == "林墨")
    }) {
        errors.push("save observation did not keep diagnostic proposals surfaced".to_string());
    }

    eval_result(
        "writer_agent:post_write_diagnostics_recorded",
        format!(
            "reports={} events={} trajectoryLines={}",
            snapshot.post_write_diagnostics.len(),
            events.len(),
            lines.len()
        ),
        errors,
    )
}
