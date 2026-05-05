pub fn run_planning_review_artifact_recorded_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻悬疑",
            "玉佩线索推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-6",
            "林墨在旧门前决定是否继续追问张三。",
            "推进林墨与张三的信任裂痕",
            "不要提前揭开玉佩来源",
            "以一个可执行的下一步选择收束。",
            "eval",
        )
        .unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "玉佩来源",
            "玉佩来源必须延后揭示，当前只能推进疑点。",
            "Chapter-1",
            "Chapter-12",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::PlanningReview,
        observation: observation_in_chapter(
            "林墨停在旧门前，张三避开他的目光，袖中玉佩只露出一道裂纹。",
            "Chapter-6",
        ),
        user_instruction: "只做规划审查，给出风险和下一步，不写正文。".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨停在旧门前，张三避开他的目光，袖中玉佩只露出一道裂纹。"
                .to_string(),
            paragraph: "林墨停在旧门前，张三避开他的目光，袖中玉佩只露出一道裂纹。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::ReadOnly,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(StaticDiagnosticProvider::new(
        "规划审查：Chapter-6 应推进林墨与张三的信任裂痕。证据：ChapterMission 要求推进信任裂痕；PromiseLedger 要求玉佩来源延后揭示。下一步：让张三承认玉佩裂纹来自旧门，而不解释来源。",
    ));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let run_result = runtime.block_on(async {
        kernel
            .run_task(request, provider, EvalToolHandler, "gpt-4o-mini", None)
            .await
    });
    let events = kernel.run_events(80);
    let artifact_event = events
        .iter()
        .find(|event| event.event_type == "writer.task_artifact");
    let inspector = kernel.inspector_timeline(80);
    let export = kernel.export_trajectory(100);

    let mut errors = Vec::new();
    if let Err(error) = &run_result {
        errors.push(format!("planning review run failed: {}", error));
    }
    if !run_result.as_ref().is_ok_and(|result| {
        result.task_receipt.as_ref().is_some_and(|receipt| {
            receipt
                .validate_artifact_attempt(&receipt.task_id, "planning_review_report")
                .is_empty()
                && receipt
                    .must_not
                    .iter()
                    .any(|artifact| artifact == "saved_chapter")
        })
    }) {
        errors.push(
            "planning review result lacks a valid planning_review_report receipt".to_string(),
        );
    }
    if !artifact_event.is_some_and(|event| {
        event
            .data
            .get("artifactKind")
            .and_then(|value| value.as_str())
            == Some("planning_review_report")
            && event.data.get("taskKind").and_then(|value| value.as_str()) == Some("PlanningReview")
            && event
                .data
                .get("content")
                .and_then(|value| value.as_str())
                .is_some_and(|text| text.contains("规划审查") && text.contains("信任裂痕"))
            && event
                .source_refs
                .iter()
                .any(|reference| reference.starts_with("artifact:"))
            && event
                .source_refs
                .iter()
                .any(|reference| reference == "StoryImpactRadius")
    }) {
        errors.push("planning review artifact run event lacks report content or refs".to_string());
    }
    if !inspector.events.iter().any(|event| {
        event.kind
            == agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::TaskArtifact
            && event.label.contains("planning_review_report")
            && event.summary.contains("chars=")
    }) {
        errors.push("inspector does not expose planning review artifact event".to_string());
    }
    if !export.jsonl.lines().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.task_artifact\"")
            && line.contains("planning_review_report")
    }) {
        errors.push("trajectory export lacks planning review artifact run event".to_string());
    }

    eval_result(
        "writer_agent:planning_review_artifact_recorded",
        format!(
            "run={} artifacts={} inspectorEvents={} trajectoryLines={}",
            run_result.is_ok(),
            events
                .iter()
                .filter(|event| event.event_type == "writer.task_artifact")
                .count(),
            inspector.events.len(),
            export.jsonl.lines().count()
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

pub fn run_ghost_task_packet_foundation_coverage_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相，在复仇与守护之间做最终选择。",
            "林墨必须在复仇和守护之间做艰难选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    kernel
        .observe(observation_in_chapter(
            "林墨停在旧门前，风从门缝里钻出来，带着一股腐朽的气味。他想起张三的话，长刀在鞘中微微震动。",
            "Chapter-1",
        ))
        .unwrap();

    let trace = kernel.trace_snapshot(20);
    let mut errors = Vec::new();
    if trace.recent_proposals.is_empty() {
        errors.push("no proposal traces recorded for ghost observation".to_string());
    }

    eval_result(
        "writer_agent:ghost_task_packet_foundation",
        format!(
            "taskPackets={} proposalTraces={}",
            trace.task_packets.len(),
            trace.recent_proposals.len()
        ),
        errors,
    )
}
