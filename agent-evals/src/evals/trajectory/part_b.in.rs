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

pub fn run_trajectory_product_metrics_present_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    kernel.observe(observation("林墨停在旧门前。")).unwrap();

    let export = kernel.export_trajectory(100);
    let mut errors = Vec::new();
    if export.jsonl.is_empty() {
        errors.push("trajectory export is empty".to_string());
    }
    let has_metrics = export.jsonl.contains("writer.product_metrics");
    if !has_metrics {
        errors.push("trajectory missing product_metrics event".to_string());
    }

    eval_result(
        "writer_agent:trajectory_product_metrics_present",
        format!(
            "jsonlBytes={} hasMetrics={}",
            export.jsonl.len(),
            has_metrics
        ),
        errors,
    )
}
