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

pub fn run_continuity_diagnostic_task_receipt_eval() -> EvalResult {
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
            "Chapter-8",
            "林墨审查玉佩线索是否偏离本章目标。",
            "指出玉佩线索风险",
            "不要提前揭开玉佩来源",
            "以诊断后的下一步问题收束。",
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
            "mystery_clue",
            "玉佩来源",
            "玉佩来源必须延后揭示。",
            "Chapter-1",
            "Chapter-12",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ContinuityDiagnostic,
        observation: observation_in_chapter(
            "林墨发现玉佩来自禁地，但他还没确认张三是否说谎。",
            "Chapter-8",
        ),
        user_instruction: "做一次长诊断，找出连续性、任务和伏笔风险。".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨发现玉佩来自禁地，但他还没确认张三是否说谎。".to_string(),
            paragraph: "林墨发现玉佩来自禁地，但他还没确认张三是否说谎。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::ReadOnly,
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
    let prepared = kernel.prepare_task_run(request, provider, EvalToolHandler, "gpt-4o-mini");
    let trace = kernel.trace_snapshot(20);
    let receipt_event = trace.run_events.iter().find(|event| {
        event.event_type == "writer.task_receipt"
            && event.data.get("taskKind").and_then(|value| value.as_str())
                == Some("ContinuityDiagnostic")
    });
    let receipt = receipt_event.and_then(|event| {
        serde_json::from_value::<agent_writer_lib::writer_agent::task_receipt::WriterTaskReceipt>(
            event.data.clone(),
        )
        .ok()
    });
    let inspector = kernel.inspector_timeline(20);
    let export = kernel.export_trajectory(40);

    let mut errors = Vec::new();
    if let Err(error) = &prepared {
        errors.push(format!("failed to prepare diagnostic run: {}", error));
    }
    if receipt.is_none() {
        errors.push("diagnostic task receipt was not recorded".to_string());
    }
    if !receipt.as_ref().is_some_and(|receipt| {
        receipt.task_kind == "ContinuityDiagnostic"
            && receipt.chapter.as_deref() == Some("Chapter-8")
            && receipt
                .required_evidence
                .iter()
                .any(|evidence| evidence == "ChapterMission")
            && receipt
                .required_evidence
                .iter()
                .any(|evidence| evidence == "CanonSlice")
            && receipt
                .expected_artifacts
                .iter()
                .any(|artifact| artifact == "diagnostic_report")
            && receipt.must_not.iter().any(|rule| rule == "saved_chapter")
            && receipt
                .validate_artifact_attempt(&receipt.task_id, "diagnostic_report")
                .is_empty()
    }) {
        errors.push(
            "diagnostic receipt lacks required evidence, artifact, or read-only guard".to_string(),
        );
    }
    if !receipt.as_ref().is_some_and(|receipt| {
        receipt
            .validate_artifact_attempt(&receipt.task_id, "saved_chapter")
            .iter()
            .any(|mismatch| mismatch.field == "expected_artifacts" || mismatch.field == "must_not")
    }) {
        errors.push("diagnostic receipt did not block saved_chapter artifact".to_string());
    }
    if !export.jsonl.lines().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.task_receipt\"")
            && line.contains("ContinuityDiagnostic")
    }) {
        errors.push("trajectory export lacks diagnostic task receipt run event".to_string());
    }
    if !inspector.events.iter().any(|event| {
        event.kind
            == agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::TaskReceipt
            && event.label.contains("ContinuityDiagnostic")
            && event.summary.contains("guards=")
            && event
                .detail
                .as_ref()
                .and_then(|detail| detail.get("mustNot"))
                .and_then(|value| value.as_array())
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.as_str() == Some("saved_chapter"))
                })
    }) {
        errors.push("inspector does not expose diagnostic receipt as a receipt event".to_string());
    }

    eval_result(
        "writer_agent:continuity_diagnostic_task_receipt",
        format!(
            "prepared={} receipt={} runEvents={} inspectorEvents={}",
            prepared.is_ok(),
            receipt.is_some(),
            trace.run_events.len(),
            inspector.events.len()
        ),
        errors,
    )
}

pub fn run_continuity_diagnostic_artifact_recorded_eval() -> EvalResult {
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
            "Chapter-9",
            "林墨审查玉佩线索是否偏离本章目标。",
            "指出玉佩线索风险",
            "不要提前揭开玉佩来源",
            "以诊断后的下一步问题收束。",
            "eval",
        )
        .unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "玉佩来源",
            "玉佩来源必须延后揭示。",
            "Chapter-1",
            "Chapter-12",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ContinuityDiagnostic,
        observation: observation_in_chapter(
            "林墨差点说出玉佩来自禁地，但张三仍没有给出证据。",
            "Chapter-9",
        ),
        user_instruction: "只做诊断报告，列出证据和建议，不写正文。".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨差点说出玉佩来自禁地，但张三仍没有给出证据。".to_string(),
            paragraph: "林墨差点说出玉佩来自禁地，但张三仍没有给出证据。".to_string(),
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
        "诊断报告：Chapter-9 存在提前揭示玉佩来源风险。证据：ChapterMission 要求不要提前揭开玉佩来源；PromiseLedger 要求延后揭示。建议：保留张三沉默，把问题转为林墨是否继续追问。",
    ));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let run_result = runtime.block_on(async {
        kernel
            .run_task(request, provider, EvalToolHandler, "gpt-4o-mini", None)
            .await
    });
    let events = kernel.run_events(60);
    let artifact_event = events
        .iter()
        .find(|event| event.event_type == "writer.task_artifact");
    let inspector = kernel.inspector_timeline(60);
    let export = kernel.export_trajectory(80);

    let mut errors = Vec::new();
    if let Err(error) = &run_result {
        errors.push(format!("diagnostic run failed: {}", error));
    }
    if !run_result.as_ref().is_ok_and(|result| {
        result.task_receipt.as_ref().is_some_and(|receipt| {
            receipt
                .validate_artifact_attempt(&receipt.task_id, "diagnostic_report")
                .is_empty()
        })
    }) {
        errors.push("diagnostic run result lacks a valid diagnostic_report receipt".to_string());
    }
    if !artifact_event.is_some_and(|event| {
        event
            .data
            .get("artifactKind")
            .and_then(|value| value.as_str())
            == Some("diagnostic_report")
            && event.data.get("taskKind").and_then(|value| value.as_str())
                == Some("ContinuityDiagnostic")
            && event
                .data
                .get("content")
                .and_then(|value| value.as_str())
                .is_some_and(|text| text.contains("诊断报告") && text.contains("玉佩来源风险"))
            && event
                .source_refs
                .iter()
                .any(|reference| reference.starts_with("artifact:"))
    }) {
        errors.push("diagnostic artifact run event lacks report content or refs".to_string());
    }
    if !inspector.events.iter().any(|event| {
        event.kind
            == agent_writer_lib::writer_agent::inspector::WriterTimelineEventKind::TaskArtifact
            && event.label.contains("diagnostic_report")
            && event.summary.contains("chars=")
    }) {
        errors.push("inspector does not expose diagnostic artifact event".to_string());
    }
    if !export.jsonl.lines().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.task_artifact\"")
            && line.contains("diagnostic_report")
    }) {
        errors.push("trajectory export lacks diagnostic artifact run event".to_string());
    }

    eval_result(
        "writer_agent:continuity_diagnostic_artifact_recorded",
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

