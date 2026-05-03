use super::*;

pub fn run_task_packet_foundation_eval() -> EvalResult {
    let mut packet = agent_harness_core::TaskPacket::new(
        "eval-task-1",
        "继续审讯场景，保持章节任务、角色设定和伏笔账本一致。",
        agent_harness_core::TaskScope::Chapter,
        now_ms(),
    );
    packet.scope_ref = Some("Chapter-7".to_string());
    packet.intent = Some(agent_harness_core::Intent::GenerateContent);
    packet.constraints = vec![
        "不得提前泄露玉佩来源。".to_string(),
        "林墨说话保持克制，不改成外放型角色。".to_string(),
    ];
    packet.success_criteria = vec![
        "输出推进审讯冲突。".to_string(),
        "不制造与寒影刀设定冲突的武器描写。".to_string(),
    ];
    packet.beliefs = vec![
        agent_harness_core::TaskBelief::new("林墨", "惯用武器是寒影刀。", 0.95)
            .with_source("canon"),
        agent_harness_core::TaskBelief::new("玉佩", "来源仍属于禁区信息。", 0.90)
            .with_source("chapter_mission"),
    ];
    packet.required_context = vec![
        agent_harness_core::RequiredContext::new(
            "chapter_mission",
            "约束本章推进内容与禁止泄露事项。",
            700,
            true,
        ),
        agent_harness_core::RequiredContext::new(
            "promise_ledger",
            "追踪玉佩和角色承诺是否需要兑现。",
            600,
            true,
        ),
        agent_harness_core::RequiredContext::new(
            "canon_slice",
            "检查林墨设定和武器设定。",
            500,
            true,
        ),
    ];
    packet.tool_policy = agent_harness_core::ToolPolicyContract {
        max_side_effect_level: agent_harness_core::ToolSideEffectLevel::ProviderCall,
        allow_approval_required: false,
        required_tool_tags: vec!["project".to_string()],
    };
    packet.feedback = agent_harness_core::FeedbackContract {
        expected_signals: vec![
            "ghost accepted/rejected".to_string(),
            "continuity warning emitted".to_string(),
        ],
        checkpoints: vec![
            "record context sources in trace".to_string(),
            "write chapter result feedback after save".to_string(),
        ],
        memory_writes: vec!["chapter_result_summary".to_string()],
    };

    let mut errors = Vec::new();
    if let Err(error) = packet.validate() {
        errors.extend(error.errors().iter().cloned());
    }
    let coverage = packet.foundation_coverage();
    if !coverage.is_complete() {
        errors.push(format!(
            "foundation coverage incomplete: {:?}",
            coverage.missing
        ));
    }

    let filter = packet.to_tool_filter(None);
    if filter.intent != Some(agent_harness_core::Intent::GenerateContent) {
        errors.push(format!("tool filter intent mismatch: {:?}", filter.intent));
    }
    if filter.include_requires_approval {
        errors.push("tool filter should not expose approval-required tools".to_string());
    }
    if filter.max_side_effect_level != Some(agent_harness_core::ToolSideEffectLevel::ProviderCall) {
        errors.push(format!(
            "tool side-effect ceiling mismatch: {:?}",
            filter.max_side_effect_level
        ));
    }

    let plan = agent_harness_core::ExecutionPlan::from_task_packet(packet.clone());
    match plan {
        Ok(plan) => {
            if plan.task_packet.as_ref() != Some(&packet) {
                errors.push("execution plan did not retain task packet".to_string());
            }
            if !plan
                .steps
                .iter()
                .any(|step| step.action == "load_required_context")
            {
                errors.push("execution plan lacks required context loading step".to_string());
            }
            if !plan
                .steps
                .iter()
                .any(|step| step.action == "capture_feedback")
            {
                errors.push("execution plan lacks feedback capture step".to_string());
            }
        }
        Err(error) => errors.push(error),
    }

    eval_result(
        "agent_harness:task_packet_covers_five_foundation_axes",
        format!(
            "coverageComplete={} requiredContext={} beliefs={}",
            coverage.is_complete(),
            packet.required_context.len(),
            packet.beliefs.len()
        ),
        errors,
    )
}

pub fn run_chapter_generation_task_packet_eval() -> EvalResult {
    let sources = vec![
        ChapterContextSource {
            source_type: "instruction".to_string(),
            id: "user-instruction".to_string(),
            label: "User instruction".to_string(),
            original_chars: 40,
            included_chars: 40,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "target_beat".to_string(),
            id: "Chapter-7".to_string(),
            label: "Current chapter beat".to_string(),
            original_chars: 80,
            included_chars: 80,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "lorebook".to_string(),
            id: "lorebook.json".to_string(),
            label: "Relevant lorebook entries".to_string(),
            original_chars: 800,
            included_chars: 500,
            truncated: true,
            score: Some(0.86),
        },
        ChapterContextSource {
            source_type: "project_brain".to_string(),
            id: "project_brain.json".to_string(),
            label: "Project Brain relevant chunks".to_string(),
            original_chars: 600,
            included_chars: 450,
            truncated: false,
            score: Some(0.74),
        },
    ];
    let target = ChapterTarget {
        title: "Chapter-7".to_string(),
        filename: "chapter-7.md".to_string(),
        number: Some(7),
        summary: "林墨逼问玉佩来源，但不能提前揭露幕后主使。".to_string(),
        status: "draft".to_string(),
    };
    let receipt = build_chapter_generation_receipt(
        "chapter-eval-1",
        &target,
        "rev-7",
        "帮我写这一章完整初稿，重点是审讯张力。",
        &sources,
        now_ms(),
    );
    let context = BuiltChapterContext {
        request_id: "chapter-eval-1".to_string(),
        target,
        base_revision: "rev-7".to_string(),
        prompt_context: "User instruction\nOutline / beat sheet\nRelevant lorebook entries"
            .to_string(),
        sources,
        budget: ChapterContextBudgetReport {
            max_chars: 24_000,
            included_chars: 1_070,
            source_count: 4,
            truncated_source_count: 1,
            warnings: vec![],
        },
        warnings: vec![],
        receipt,
    };
    let packet = build_chapter_generation_task_packet(
        "eval-project",
        "eval-session",
        &context,
        "帮我写这一章完整初稿，重点是审讯张力。",
        now_ms(),
    );

    let mut errors = Vec::new();
    if let Err(error) = packet.validate() {
        errors.extend(error.errors().iter().cloned());
    }
    let coverage = packet.foundation_coverage();
    if !coverage.is_complete() {
        errors.push(format!(
            "foundation coverage incomplete: {:?}",
            coverage.missing
        ));
    }
    if packet.scope != agent_harness_core::TaskScope::Chapter {
        errors.push(format!("scope mismatch: {:?}", packet.scope));
    }
    if packet.intent != Some(agent_harness_core::Intent::GenerateContent) {
        errors.push(format!("intent mismatch: {:?}", packet.intent));
    }
    if packet.tool_policy.max_side_effect_level != agent_harness_core::ToolSideEffectLevel::Write {
        errors.push(format!(
            "side effect ceiling mismatch: {:?}",
            packet.tool_policy.max_side_effect_level
        ));
    }
    if !packet.tool_policy.allow_approval_required {
        errors
            .push("chapter generation packet must allow approval-required save tools".to_string());
    }
    if !packet
        .required_context
        .iter()
        .any(|context| context.source_type == "target_beat" && context.required)
    {
        errors.push("target beat is not marked as required context".to_string());
    }
    if !packet
        .required_context
        .iter()
        .any(|context| context.source_type == "lorebook" && context.required)
    {
        errors.push("lorebook is not marked as required context".to_string());
    }
    if !packet
        .feedback
        .checkpoints
        .iter()
        .any(|checkpoint| checkpoint.contains("revision"))
    {
        errors.push("feedback checkpoints do not include save conflict/revision guard".to_string());
    }
    if !packet
        .feedback
        .memory_writes
        .iter()
        .any(|write| write == "chapter_result_summary")
    {
        errors.push("feedback contract does not write chapter result summary".to_string());
    }

    eval_result(
        "writer_agent:chapter_generation_task_packet_foundation",
        format!(
            "coverageComplete={} requiredContext={} beliefs={} sideEffect={:?}",
            coverage.is_complete(),
            packet.required_context.len(),
            packet.beliefs.len(),
            packet.tool_policy.max_side_effect_level
        ),
        errors,
    )
}

pub fn run_task_packet_trace_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨在审讯里逼近玉佩线索。",
            "玉佩线索",
            "提前揭开玉佩来源",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation(
            "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。",
        ))
        .unwrap();
    let trace = kernel.trace_snapshot(10);
    let packet = trace
        .task_packets
        .iter()
        .find(|packet| packet.task == "GhostWriting");
    let export = kernel.export_trajectory(20);
    let lines = export.jsonl.lines().collect::<Vec<_>>();

    let mut errors = Vec::new();
    if !proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost)
    {
        errors.push("fixture did not create ghost proposal".to_string());
    }
    if packet.is_none() {
        errors.push("missing GhostWriting task packet trace".to_string());
    }
    if !packet.is_some_and(|packet| packet.foundation_complete) {
        errors.push("task packet foundation coverage is incomplete".to_string());
    }
    if !packet.is_some_and(|packet| {
        packet.required_context_count >= 3
            && packet.belief_count >= 1
            && packet.success_criteria_count >= 2
            && packet.max_side_effect_level == "ProviderCall"
    }) {
        errors.push("task packet lacks context, beliefs, criteria, or tool boundary".to_string());
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.task_packet\""))
    {
        errors.push("trajectory export lacks writer.task_packet event".to_string());
    }
    if !lines
        .iter()
        .filter(|line| line.contains("\"eventType\":\"writer.task_packet\""))
        .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    {
        errors.push("task packet trajectory event is not valid json".to_string());
    }

    eval_result(
        "writer_agent:task_packet_trace_export",
        format!(
            "taskPackets={} events={}",
            trace.task_packets.len(),
            export.event_count
        ),
        errors,
    )
}

pub fn run_chapter_generation_task_packet_trace_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let sources = vec![
        ChapterContextSource {
            source_type: "instruction".to_string(),
            id: "user-instruction".to_string(),
            label: "User instruction".to_string(),
            original_chars: 38,
            included_chars: 38,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "outline".to_string(),
            id: "outline.json".to_string(),
            label: "Outline / beat sheet".to_string(),
            original_chars: 900,
            included_chars: 700,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "target_beat".to_string(),
            id: "Chapter-8".to_string(),
            label: "Current chapter beat".to_string(),
            original_chars: 120,
            included_chars: 120,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "project_brain".to_string(),
            id: "project_brain.json".to_string(),
            label: "Project Brain relevant chunks".to_string(),
            original_chars: 640,
            included_chars: 480,
            truncated: false,
            score: Some(0.72),
        },
    ];
    let target = ChapterTarget {
        title: "Chapter-8".to_string(),
        filename: "chapter-8.md".to_string(),
        number: Some(8),
        summary: "林墨追查玉佩下落，并把张三逼到选择边缘。".to_string(),
        status: "draft".to_string(),
    };
    let receipt = build_chapter_generation_receipt(
        "chapter-trace-eval",
        &target,
        "rev-8",
        "继续写这一章完整初稿，重点保持玉佩线的选择压力。",
        &sources,
        now_ms(),
    );
    let context = BuiltChapterContext {
        request_id: "chapter-trace-eval".to_string(),
        target,
        base_revision: "rev-8".to_string(),
        prompt_context: "User instruction\nCurrent chapter beat\nRelevant lorebook entries"
            .to_string(),
        sources,
        budget: ChapterContextBudgetReport {
            max_chars: 24_000,
            included_chars: 1_338,
            source_count: 4,
            truncated_source_count: 0,
            warnings: vec![],
        },
        warnings: vec![],
        receipt,
    };
    let packet = build_chapter_generation_task_packet(
        &kernel.project_id,
        &kernel.session_id,
        &context,
        "继续写这一章完整初稿，重点保持玉佩线的选择压力。",
        now_ms(),
    );
    let record_result = kernel.record_task_packet(&context.request_id, "ChapterGeneration", packet);
    let trace = kernel.trace_snapshot(10);
    let recorded = trace
        .task_packets
        .iter()
        .find(|packet| packet.task == "ChapterGeneration");

    let mut errors = Vec::new();
    if let Err(error) = record_result {
        errors.push(format!("record task packet failed: {}", error));
    }
    if recorded.is_none() {
        errors.push("missing ChapterGeneration task packet trace".to_string());
    }
    if !recorded.is_some_and(|packet| packet.foundation_complete) {
        errors.push("chapter generation task packet foundation is incomplete".to_string());
    }
    if !recorded.is_some_and(|packet| {
        packet.max_side_effect_level == "Write"
            && packet.required_context_count >= 4
            && packet.feedback_checkpoint_count >= 3
            && packet.packet.tool_policy.allow_approval_required
    }) {
        errors
            .push("chapter generation trace lacks write boundary or feedback contract".to_string());
    }
    if !recorded.is_some_and(|packet| {
        packet
            .packet
            .required_context
            .iter()
            .any(|context| context.source_type == "target_beat" && context.required)
            && packet
                .packet
                .feedback
                .memory_writes
                .iter()
                .any(|write| write == "chapter_result_summary")
    }) {
        errors.push(
            "chapter generation packet lacks target beat or result feedback write".to_string(),
        );
    }

    eval_result(
        "writer_agent:chapter_generation_task_packet_trace",
        format!(
            "taskPackets={} recorded={}",
            trace.task_packets.len(),
            recorded.is_some()
        ),
        errors,
    )
}

pub fn run_chapter_generation_task_receipt_required_eval() -> EvalResult {
    let sources = vec![
        ChapterContextSource {
            source_type: "instruction".to_string(),
            id: "user-instruction".to_string(),
            label: "User instruction".to_string(),
            original_chars: 40,
            included_chars: 40,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "target_beat".to_string(),
            id: "Chapter-9".to_string(),
            label: "Current chapter beat".to_string(),
            original_chars: 120,
            included_chars: 120,
            truncated: false,
            score: None,
        },
        ChapterContextSource {
            source_type: "project_brain".to_string(),
            id: "project_brain.json".to_string(),
            label: "Project Brain relevant chunks".to_string(),
            original_chars: 600,
            included_chars: 480,
            truncated: false,
            score: Some(0.76),
        },
    ];
    let target = ChapterTarget {
        title: "Chapter-9".to_string(),
        filename: "chapter-9.md".to_string(),
        number: Some(9),
        summary: "林墨逼近玉佩线索。".to_string(),
        status: "draft".to_string(),
    };
    let receipt = build_chapter_generation_receipt(
        "receipt-eval-1",
        &target,
        "rev-9",
        "写这一章，重点保持玉佩线压力。",
        &sources,
        now_ms(),
    );

    let mut errors = Vec::new();
    if receipt.task_id != "receipt-eval-1" {
        errors.push("receipt task id mismatch".to_string());
    }
    if receipt.task_kind != "ChapterGeneration" {
        errors.push("receipt task kind mismatch".to_string());
    }
    if receipt.chapter.as_deref() != Some("Chapter-9") {
        errors.push("receipt chapter mismatch".to_string());
    }
    for required in ["instruction", "target_beat", "project_brain"] {
        if !receipt
            .required_evidence
            .iter()
            .any(|evidence| evidence == required)
        {
            errors.push(format!("receipt missing required evidence {}", required));
        }
    }
    for artifact in ["chapter_draft", "saved_chapter"] {
        if !receipt
            .expected_artifacts
            .iter()
            .any(|expected| expected == artifact)
        {
            errors.push(format!("receipt missing expected artifact {}", artifact));
        }
    }
    if !receipt
        .must_not
        .iter()
        .any(|rule| rule == "overwrite_without_revision_match")
    {
        errors.push("receipt missing overwrite guard".to_string());
    }
    if !receipt
        .validate_write_attempt("receipt-eval-1", "Chapter-9", "rev-9", "saved_chapter")
        .is_empty()
    {
        errors.push("valid receipt write attempt produced mismatch".to_string());
    }

    eval_result(
        "writer_agent:chapter_generation_task_receipt_required",
        format!(
            "requiredEvidence={} artifacts={} mustNot={}",
            receipt.required_evidence.len(),
            receipt.expected_artifacts.len(),
            receipt.must_not.len()
        ),
        errors,
    )
}

pub fn run_task_receipt_mismatch_blocks_write_eval() -> EvalResult {
    let target = ChapterTarget {
        title: "Chapter-10".to_string(),
        filename: "chapter-10.md".to_string(),
        number: Some(10),
        summary: "林墨确认玉佩线索。".to_string(),
        status: "draft".to_string(),
    };
    let receipt = build_chapter_generation_receipt(
        "receipt-eval-2",
        &target,
        "rev-10",
        "写这一章。",
        &[ChapterContextSource {
            source_type: "instruction".to_string(),
            id: "user-instruction".to_string(),
            label: "User instruction".to_string(),
            original_chars: 12,
            included_chars: 12,
            truncated: false,
            score: None,
        }],
        now_ms(),
    );
    let mismatches = receipt.validate_write_attempt(
        "receipt-eval-2",
        "Chapter-10",
        "rev-later",
        "saved_chapter",
    );
    let mut errors = Vec::new();
    if mismatches.is_empty() {
        errors.push("receipt mismatch did not block changed revision".to_string());
    }
    if !mismatches
        .iter()
        .any(|mismatch| mismatch.field == "base_revision")
    {
        errors.push("receipt mismatch lacks base_revision evidence".to_string());
    }
    let evidence = agent_writer_lib::writer_agent::task_receipt::WriterFailureEvidenceBundle::new(
        agent_writer_lib::writer_agent::task_receipt::WriterFailureCategory::ReceiptMismatch,
        "RECEIPT_MISMATCH",
        "receipt mismatch",
        true,
        Some(receipt.task_id.clone()),
        mismatches
            .iter()
            .map(|mismatch| format!("{}:{}", mismatch.field, mismatch.actual))
            .collect(),
        serde_json::json!({ "mismatches": mismatches }),
        vec!["rebuild receipt".to_string()],
        now_ms(),
    );
    if evidence.category
        != agent_writer_lib::writer_agent::task_receipt::WriterFailureCategory::ReceiptMismatch
    {
        errors.push("failure bundle category is not receipt_mismatch".to_string());
    }
    if evidence.evidence_refs.is_empty() {
        errors.push("failure bundle lacks mismatch evidence refs".to_string());
    }

    eval_result(
        "writer_agent:task_receipt_mismatch_blocks_write",
        format!("mismatches={}", evidence.evidence_refs.len()),
        errors,
    )
}

pub fn run_failure_evidence_bundle_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let error = ChapterGenerationError::with_details(
        "PROVIDER_TIMEOUT",
        "The model provider timed out.",
        true,
        "timeout after 120s",
    );
    let bundle = failure_bundle_from_chapter_error("receipt-eval-3", &error, now_ms());
    kernel.record_failure_evidence_bundle(&bundle);
    let trace = kernel.trace_snapshot(10);
    let export = kernel.export_trajectory(20);

    let mut errors = Vec::new();
    if bundle.category
        != agent_writer_lib::writer_agent::task_receipt::WriterFailureCategory::ProviderFailed
    {
        errors.push("provider timeout did not map to provider_failed".to_string());
    }
    if bundle.remediation.is_empty() {
        errors.push("failure bundle lacks remediation".to_string());
    }
    if !trace.run_events.iter().any(|event| {
        event.event_type == "writer.error"
            && event.data.get("category").and_then(|value| value.as_str())
                == Some("provider_failed")
    }) {
        errors.push("failure bundle was not recorded as writer.error run event".to_string());
    }
    if !export.jsonl.lines().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"") && line.contains("\"writer.error\"")
    }) {
        errors.push("trajectory export lacks writer.error run event".to_string());
    }

    eval_result(
        "writer_agent:run_failure_evidence_bundle",
        format!(
            "category={:?} runEvents={} trajectoryEvents={}",
            bundle.category,
            trace.run_events.len(),
            export.event_count
        ),
        errors,
    )
}
