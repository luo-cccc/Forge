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

pub fn run_trajectory_trace_viewer_export_eval() -> EvalResult {
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
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    if let Some(proposal) = proposals.first() {
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal.id.clone(),
                action: FeedbackAction::Rejected,
                final_text: None,
                reason: Some("trace viewer eval".to_string()),
                created_at: now_ms(),
            })
            .unwrap();
    }
    let export = kernel.export_trajectory(50);
    let lines = export.trace_viewer_jsonl.lines().collect::<Vec<_>>();
    let values = lines
        .iter()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect::<Vec<_>>();

    let mut errors = Vec::new();
    if export.trace_viewer_schema != "claude-code-jsonl-for-hf-agent-trace-viewer" {
        errors.push(format!(
            "unexpected trace viewer schema {}",
            export.trace_viewer_schema
        ));
    }
    if export.trace_viewer_event_count == 0 || values.len() != export.trace_viewer_event_count {
        errors.push(format!(
            "trace viewer count mismatch count={} parsed={}",
            export.trace_viewer_event_count,
            values.len()
        ));
    }
    if export.trace_viewer_event_count < export.event_count {
        errors.push(format!(
            "trace viewer export lost events traceViewer={} forge={}",
            export.trace_viewer_event_count, export.event_count
        ));
    }
    if !values.iter().all(|value| {
        value.get("type").and_then(|v| v.as_str()).is_some()
            && value.get("message").and_then(|v| v.as_object()).is_some()
            && value.get("uuid").and_then(|v| v.as_str()).is_some()
            && value.get("sessionId").and_then(|v| v.as_str()).is_some()
            && value.get("timestamp").and_then(|v| v.as_str()).is_some()
    }) {
        errors.push("trace viewer export lacks required Claude-style fields".to_string());
    }
    if !values
        .iter()
        .skip(1)
        .all(|value| value.get("parentUuid").and_then(|v| v.as_str()).is_some())
    {
        errors.push("trace viewer export lacks parentUuid chain after metadata".to_string());
    }
    if !values.iter().any(|value| {
        value.get("forgeEventType").and_then(|v| v.as_str()) == Some("writer.observation")
            && value.get("type").and_then(|v| v.as_str()) == Some("user")
    }) {
        errors.push("trace viewer export lacks user observation event".to_string());
    }
    if !values.iter().any(|value| {
        value.get("forgeEventType").and_then(|v| v.as_str()) == Some("writer.run_event")
            && value.get("forgeEvent").is_some()
    }) {
        errors.push("trace viewer export lacks bridged Forge run event".to_string());
    }
    if !values.iter().any(|value| {
        value
            .get("redactionWarning")
            .and_then(|v| v.as_str())
            .is_some_and(|warning| warning.contains("manuscript"))
    }) {
        errors.push("trace viewer export lacks redaction warning metadata".to_string());
    }

    eval_result(
        "writer_agent:trajectory_trace_viewer_export",
        format!(
            "forgeEvents={} traceViewerEvents={}",
            export.event_count, export.trace_viewer_event_count
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

pub fn run_post_write_diagnostics_after_accepted_operation_eval() -> EvalResult {
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
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation_in_chapter("林墨停在门前，低声确认这次不会惊动任何人。", "Chapter-4"),
            "林墨拔出长剑，逼问玉佩的下落。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();

    let mut approval = eval_approval("accepted_operation_post_write");
    approval.proposal_id = Some(proposal.id.clone());
    let approved = kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    let saved_text = "林墨停在门前，低声确认这次不会惊动任何人。林墨拔出长剑，逼问玉佩的下落。";
    kernel
        .record_operation_durable_save_with_post_write(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
            Some(saved_text.to_string()),
            Some("Chapter-4".to_string()),
            Some("rev-2".to_string()),
        )
        .unwrap();

    let snapshot = kernel.trace_snapshot(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let report = snapshot.post_write_diagnostics.first();

    let mut errors = Vec::new();
    if !approved.success {
        errors.push("accepted operation approval failed".to_string());
    }
    if report.is_none() {
        errors.push("accepted operation durable save did not record diagnostics".to_string());
    }
    if let Some(report) = report {
        if report.error_count == 0 {
            errors.push("accepted operation report missed canon conflict error".to_string());
        }
        if !report
            .source_refs
            .iter()
            .any(|source| source == &format!("proposal:{}", proposal.id))
        {
            errors.push(format!(
                "accepted operation report lacks proposal source ref: {:?}",
                report.source_refs
            ));
        }
        if !report
            .source_refs
            .iter()
            .any(|source| source == "operation:text.insert")
        {
            errors.push("accepted operation report lacks operation source ref".to_string());
        }
        if report.chapter_revision.as_deref() != Some("rev-2") {
            errors.push(format!(
                "accepted operation report has wrong revision: {:?}",
                report.chapter_revision
            ));
        }
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.post_write_diagnostics\"")
            && line.contains("operation:text.insert")
    }) {
        errors.push("trajectory export lacks accepted-operation post-write diagnostic".to_string());
    }

    eval_result(
        "writer_agent:post_write_diagnostics_after_accepted_operation",
        format!(
            "reports={} trajectoryLines={}",
            snapshot.post_write_diagnostics.len(),
            lines.len()
        ),
        errors,
    )
}

pub fn run_save_completed_links_post_write_diagnostics_eval() -> EvalResult {
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
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation_in_chapter("林墨停在门前，低声确认这次不会惊动任何人。", "Chapter-5"),
            "林墨拔出长剑，逼问玉佩的下落。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let mut approval = eval_approval("save_completed_trace");
    approval.proposal_id = Some(proposal.id.clone());
    kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    kernel
        .record_operation_durable_save_with_post_write(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-5".to_string(),
            Some("林墨停在门前。林墨拔出长剑，逼问玉佩的下落。".to_string()),
            Some("Chapter-5".to_string()),
            Some("rev-5".to_string()),
        )
        .unwrap();

    let events = kernel.run_events(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let save_event = events
        .iter()
        .find(|event| event.event_type == "writer.save_completed");

    let mut errors = Vec::new();
    if save_event.is_none() {
        errors.push("run event store lacks writer.save_completed".to_string());
    }
    if let Some(event) = save_event {
        if event
            .data
            .get("postWriteReportId")
            .and_then(|value| value.as_str())
            .is_none()
        {
            errors.push("save_completed lacks post-write report id".to_string());
        }
        if event
            .data
            .get("diagnosticErrorCount")
            .and_then(|value| value.as_u64())
            == Some(0)
        {
            errors.push("save_completed did not carry diagnostic error count".to_string());
        }
        if !event
            .source_refs
            .iter()
            .any(|source| source == &format!("proposal:{}", proposal.id))
        {
            errors.push(format!(
                "save_completed lacks proposal source ref: {:?}",
                event.source_refs
            ));
        }
        if !event
            .source_refs
            .iter()
            .any(|source| source == "operation:text.insert")
        {
            errors.push("save_completed lacks operation source ref".to_string());
        }
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.save_completed\"")
            && line.contains("\"postWriteReportId\"")
    }) {
        errors.push("trajectory export lacks linked save_completed run event".to_string());
    }

    eval_result(
        "writer_agent:save_completed_links_post_write_diagnostics",
        format!("events={} trajectoryLines={}", events.len(), lines.len()),
        errors,
    )
}

pub fn run_memory_candidate_created_run_event_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("新来的护卫名叫沈照，他随身带着玉佩。", "Chapter-8");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    save.editor_dirty = false;
    let proposals = kernel.observe(save).unwrap();
    let events = kernel.run_events(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let memory_candidate_events = events
        .iter()
        .filter(|event| event.event_type == "writer.memory_candidate_created")
        .collect::<Vec<_>>();

    let mut errors = Vec::new();
    if !proposals.iter().any(|proposal| {
        matches!(
            proposal.kind,
            agent_writer_lib::writer_agent::proposal::ProposalKind::CanonUpdate
                | agent_writer_lib::writer_agent::proposal::ProposalKind::PlotPromise
        )
    }) {
        errors.push("save observation did not surface memory candidate proposal".to_string());
    }
    if memory_candidate_events.is_empty() {
        errors.push("run event store lacks writer.memory_candidate_created".to_string());
    }
    for event in &memory_candidate_events {
        if event
            .data
            .get("slot")
            .and_then(|value| value.as_str())
            .is_none()
        {
            errors.push("memory candidate event lacks slot".to_string());
        }
        if event
            .data
            .get("requiresAuthorReview")
            .and_then(|value| value.as_bool())
            != Some(true)
        {
            errors.push("memory candidate event does not mark author review".to_string());
        }
        if event
            .data
            .get("writesLedgerImmediately")
            .and_then(|value| value.as_bool())
            != Some(false)
        {
            errors.push("memory candidate event implies direct ledger write".to_string());
        }
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.memory_candidate_created\"")
            && line.contains("\"requiresAuthorReview\":true")
    }) {
        errors.push(
            "trajectory export lacks reviewable memory_candidate_created run event".to_string(),
        );
    }
    let ledger = kernel.ledger_snapshot();
    if ledger
        .canon_entities
        .iter()
        .any(|entity| entity.name == "沈照")
    {
        errors.push("memory candidate was written to canon before author approval".to_string());
    }

    eval_result(
        "writer_agent:memory_candidate_created_run_event",
        format!(
            "proposals={} memoryEvents={} trajectoryLines={}",
            proposals.len(),
            memory_candidate_events.len(),
            lines.len()
        ),
        errors,
    )
}

pub fn run_memory_auto_write_cannot_bypass_review_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter(
        "那个少年名叫沈照，袖中藏着一枚玉佩，却始终没有告诉任何人它的下落。",
        "Chapter-8",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    save.editor_dirty = false;

    let proposals = kernel.observe(save).unwrap();
    let ledger = kernel.ledger_snapshot();
    let events = kernel.run_events(50);
    let memory_events = events
        .iter()
        .filter(|event| event.event_type == "writer.memory_candidate_created")
        .collect::<Vec<_>>();

    let has_canon_candidate =
        proposals.iter().any(|proposal| {
            proposal.kind == agent_writer_lib::writer_agent::proposal::ProposalKind::CanonUpdate
                && matches!(
                proposal.operations.first(),
                Some(agent_writer_lib::writer_agent::operation::WriterOperation::CanonUpsertEntity {
                    ..
                })
            )
        });
    let has_promise_candidate = proposals.iter().any(|proposal| {
        proposal.kind == agent_writer_lib::writer_agent::proposal::ProposalKind::PlotPromise
            && matches!(
                proposal.operations.first(),
                Some(agent_writer_lib::writer_agent::operation::WriterOperation::PromiseAdd { .. })
            )
    });

    let mut errors = Vec::new();
    if !has_canon_candidate {
        errors.push("save observation did not create reviewable Canon candidate".to_string());
    }
    if !has_promise_candidate {
        errors.push("save observation did not create reviewable Promise candidate".to_string());
    }
    if ledger
        .canon_entities
        .iter()
        .any(|entity| entity.name == "沈照")
    {
        errors.push("Canon candidate bypassed review and wrote ledger".to_string());
    }
    if ledger
        .open_promises
        .iter()
        .any(|promise| promise.title.contains("玉佩") || promise.description.contains("玉佩"))
    {
        errors.push("Promise candidate bypassed review and wrote ledger".to_string());
    }
    if memory_events.len() < 2 {
        errors.push(format!(
            "expected memory_candidate_created events for canon and promise, got {}",
            memory_events.len()
        ));
    }
    for event in memory_events {
        if event
            .data
            .get("requiresAuthorReview")
            .and_then(|value| value.as_bool())
            != Some(true)
            || event
                .data
                .get("writesLedgerImmediately")
                .and_then(|value| value.as_bool())
                != Some(false)
        {
            errors.push(format!(
                "memory candidate event lacks review/no-write flags: {}",
                event.data
            ));
        }
    }

    eval_result(
        "writer_agent:memory_auto_write_cannot_bypass_review",
        format!(
            "proposals={} canonCandidate={} promiseCandidate={} canonLedger={} promiseLedger={}",
            proposals.len(),
            has_canon_candidate,
            has_promise_candidate,
            ledger.canon_entities.len(),
            ledger.open_promises.len()
        ),
        errors,
    )
}

pub fn run_operation_approval_decided_run_event_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation_in_chapter("林墨停在门前，低声确认这次不会惊动任何人。", "Chapter-9"),
            "林墨拔出寒影刀，挡在门前。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let rejected = kernel
        .approve_editor_operation(operation.clone(), "rev-1")
        .unwrap();
    let mut approval = eval_approval("approval_decided_eval");
    approval.proposal_id = Some(proposal.id.clone());
    let approved = kernel
        .approve_editor_operation_with_approval(operation, "rev-1", Some(&approval))
        .unwrap();
    let events = kernel.run_events(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let approval_events = events
        .iter()
        .filter(|event| event.event_type == "writer.approval_decided")
        .collect::<Vec<_>>();

    let mut errors = Vec::new();
    if rejected.success {
        errors.push("operation without approval should be rejected".to_string());
    }
    if !approved.success {
        errors.push("operation with approval should be approved/applied".to_string());
    }
    if approval_events.len() < 2 {
        errors.push(format!(
            "expected rejected and approved approval events, got {}",
            approval_events.len()
        ));
    }
    if !approval_events.iter().any(|event| {
        event.data.get("decision").and_then(|value| value.as_str()) == Some("rejected")
            && event
                .data
                .get("surfacedToUser")
                .and_then(|value| value.as_bool())
                == Some(false)
    }) {
        errors.push("approval event lacks rejected missing-context decision".to_string());
    }
    if !approval_events.iter().any(|event| {
        event.data.get("decision").and_then(|value| value.as_str()) == Some("approved")
            && event
                .data
                .get("approvalSource")
                .and_then(|value| value.as_str())
                == Some("approval_decided_eval")
            && event
                .data
                .get("surfacedToUser")
                .and_then(|value| value.as_bool())
                == Some(true)
    }) {
        errors.push("approval event lacks approved surfaced decision".to_string());
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.approval_decided\"")
            && line.contains("\"decision\":\"approved\"")
    }) {
        errors.push("trajectory export lacks approved approval_decided run event".to_string());
    }
    if !events.iter().any(|event| {
        event.event_type == "writer.operation_lifecycle"
            && event.data.get("state").and_then(|value| value.as_str()) == Some("approved")
    }) {
        errors.push("approval decision no longer records approved lifecycle".to_string());
    }

    eval_result(
        "writer_agent:operation_approval_decided_run_event",
        format!(
            "approvalEvents={} trajectoryLines={}",
            approval_events.len(),
            lines.len()
        ),
        errors,
    )
}

pub fn run_context_pack_built_run_event_eval() -> EvalResult {
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
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-10",
            "林墨在旧门前确认玉佩线索。",
            "让张三的隐瞒形成新压力",
            "不要提前揭开玉佩来源",
            "以林墨必须决定是否追问张三收束。",
            "eval",
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩下落",
            "张三带走玉佩，需要在后续章节交代去向。",
            "Chapter-1",
            "Chapter-12",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let secret_sentence = "林墨停在旧门前，想起张三带走的玉佩。";
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation_in_chapter(secret_sentence, "Chapter-10"),
            "他把声音压低，先确认门后的呼吸声。".to_string(),
            "eval-model",
        )
        .unwrap();
    let events = kernel.run_events(50);
    let export = kernel.export_trajectory(50);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let context_events = events
        .iter()
        .filter(|event| event.event_type == "writer.context_pack_built")
        .collect::<Vec<_>>();

    let mut errors = Vec::new();
    if proposal.evidence.is_empty() {
        errors.push("ghost proposal did not keep context evidence".to_string());
    }
    if context_events.is_empty() {
        errors.push("run event store lacks writer.context_pack_built".to_string());
    }
    if !context_events.iter().any(|event| {
        event.data.get("task").and_then(|value| value.as_str()) == Some("GhostWriting")
            && event
                .data
                .get("sourceCount")
                .and_then(|value| value.as_u64())
                .is_some_and(|count| count >= 3)
            && event
                .data
                .get("budgetLimit")
                .and_then(|value| value.as_u64())
                == Some(3000)
            && event
                .data
                .get("sourceReports")
                .and_then(|value| value.as_array())
                .is_some_and(|reports| {
                    reports.iter().any(|report| {
                        report.get("source").and_then(|value| value.as_str())
                            == Some("ChapterMission")
                            && report.get("required").and_then(|value| value.as_bool())
                                == Some(true)
                    }) && reports.iter().any(|report| {
                        report.get("source").and_then(|value| value.as_str())
                            == Some("PromiseSlice")
                            && report
                                .get("provided")
                                .and_then(|value| value.as_u64())
                                .is_some_and(|provided| provided > 0)
                    })
                })
    }) {
        errors.push(
            "context_pack_built event lacks task, budget, required source, or promise source facts"
                .to_string(),
        );
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.context_pack_built\"")
            && line.contains("\"sourceReports\"")
    }) {
        errors.push("trajectory export lacks context_pack_built run event".to_string());
    }
    if context_events
        .iter()
        .any(|event| event.data.to_string().contains(secret_sentence))
    {
        errors.push("context_pack_built event leaked manuscript text".to_string());
    }

    eval_result(
        "writer_agent:context_pack_built_run_event",
        format!(
            "contextEvents={} trajectoryLines={}",
            context_events.len(),
            lines.len()
        ),
        errors,
    )
}
