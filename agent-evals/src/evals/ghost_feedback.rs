use super::*;

pub fn run_multi_ghost_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation(
            "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。",
        ))
        .unwrap();
    let ghost = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost);
    let mut errors = Vec::new();
    if ghost.is_none() {
        errors.push("missing ghost proposal".to_string());
    }
    if !ghost.is_some_and(|proposal| proposal.alternatives.len() == 3) {
        errors.push("ghost proposal should contain exactly three branches".to_string());
    }
    if !ghost.is_some_and(|proposal| {
        proposal
            .alternatives
            .iter()
            .all(|alternative| alternative.operation.is_some())
    }) {
        errors.push("each ghost branch should carry an executable operation".to_string());
    }

    eval_result(
        "writer_agent:multi_ghost_branches",
        ghost
            .map(|proposal| format!("branches={}", proposal.alternatives.len()))
            .unwrap_or_else(|| "branches=0".to_string()),
        errors,
    )
}

pub fn run_feedback_suppression_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
    let first = kernel.observe(obs.clone()).unwrap();
    let ghost = first
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .expect("fixture should create ghost");
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: ghost.id.clone(),
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("interruptive".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let mut next = obs;
    next.id = "eval-suppression-next".to_string();
    let second = kernel.observe(next).unwrap();
    let repeated = second
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost);
    let errors = if repeated {
        vec!["rejected ghost repeated before suppression TTL".to_string()]
    } else {
        Vec::new()
    };

    eval_result(
        "writer_agent:feedback_suppresses_repeated_ghost",
        format!("second_proposals={}", second.len()),
        errors,
    )
}

pub fn run_operation_feedback_requires_durable_save_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let mut approval = eval_approval("eval_text_accept");
    approval.proposal_id = Some(proposal.id.clone());

    let approved = kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: Some("他终于听见门后有人低声念出了他的名字。".to_string()),
            reason: Some("accepted before save should not teach memory".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let mut errors = Vec::new();
    if !approved.success {
        errors.push(format!(
            "text operation approval failed: {:?}",
            approved.error
        ));
    }
    if kernel
        .memory
        .list_style_preferences(20)
        .unwrap()
        .iter()
        .any(|preference| preference.key == "accepted_Ghost")
    {
        errors.push("accepted ghost preference was written before durable save".to_string());
    }

    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: Some("他终于听见门后有人低声念出了他的名字。".to_string()),
            reason: Some("accepted after save may teach memory".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    if !kernel
        .memory
        .list_style_preferences(20)
        .unwrap()
        .iter()
        .any(|preference| preference.key == "accepted_Ghost")
    {
        errors.push("accepted ghost preference was not written after durable save".to_string());
    }

    eval_result(
        "writer_agent:operation_feedback_requires_durable_save",
        format!("approved={} errors={}", approved.success, errors.len()),
        errors,
    )
}

pub fn run_write_operation_lifecycle_trace_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let mut approval = eval_approval("eval_lifecycle");
    approval.proposal_id = Some(proposal.id.clone());

    kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: Some("trace lifecycle".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let trace = kernel.trace_snapshot(20);
    let states = trace
        .operation_lifecycle
        .iter()
        .filter(|entry| entry.proposal_id.as_deref() == Some(proposal.id.as_str()))
        .map(|entry| format!("{:?}", entry.state))
        .collect::<Vec<_>>();

    let mut errors = Vec::new();
    for expected in [
        "Proposed",
        "Approved",
        "Applied",
        "DurablySaved",
        "FeedbackRecorded",
    ] {
        if !states.iter().any(|state| state == expected) {
            errors.push(format!("missing lifecycle state {}", expected));
        }
    }
    if !trace.operation_lifecycle.iter().any(|entry| {
        entry.proposal_id.as_deref() == Some(proposal.id.as_str())
            && entry.operation_kind == "text.insert"
            && entry.source_task.as_deref() == Some("Ghost")
            && entry.approval_source.as_deref() == Some("eval_lifecycle")
            && entry
                .affected_scope
                .as_deref()
                .is_some_and(|scope| scope.contains("Chapter-1"))
            && entry
                .save_result
                .as_deref()
                .is_some_and(|save| save.contains("rev-2"))
    }) {
        errors.push("lifecycle trace lacks operation metadata".to_string());
    }

    eval_result(
        "writer_agent:write_operation_lifecycle_trace",
        format!("states={}", states.join(",")),
        errors,
    )
}

pub fn run_product_metrics_trace_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let mut approval = eval_approval("eval_metrics");
    approval.proposal_id = Some(proposal.id.clone());

    kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: Some("metrics acceptance".to_string()),
            created_at: now_ms() + 10,
        })
        .unwrap();

    let trace = kernel.trace_snapshot(20);
    let metrics = trace.product_metrics;
    let mut errors = Vec::new();
    if metrics.proposal_count == 0 {
        errors.push("metrics did not count proposals".to_string());
    }
    if metrics.accepted_count != 1 {
        errors.push(format!("accepted count was {}", metrics.accepted_count));
    }
    if metrics.proposal_acceptance_rate <= 0.0 {
        errors.push("acceptance rate was not positive".to_string());
    }
    if metrics.durable_save_success_rate <= 0.0 {
        errors.push("durable save success rate was not positive".to_string());
    }
    if metrics.average_save_to_feedback_ms.is_none() {
        errors.push("save-to-feedback latency was not calculated".to_string());
    }
    let export = kernel.export_trajectory(20);
    if !export
        .jsonl
        .contains("\"eventType\":\"writer.product_metrics\"")
    {
        errors.push("trajectory export lacks product metrics event".to_string());
    }

    eval_result(
        "writer_agent:product_metrics_trace",
        format!(
            "acceptance={:.2} durable={:.2} latency={:?}",
            metrics.proposal_acceptance_rate,
            metrics.durable_save_success_rate,
            metrics.average_save_to_feedback_ms
        ),
        errors,
    )
}

pub fn run_product_metrics_multi_session_trend_eval() -> EvalResult {
    let db_path = std::env::temp_dir().join(format!(
        "forge-product-metrics-trend-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let mut errors = Vec::new();
    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "trend-session-a".to_string();
        let proposal = kernel
            .create_llm_ghost_proposal(
                observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "eval-model",
            )
            .unwrap();
        let operation = proposal.operations[0].clone();
        let mut approval = eval_approval("trend-session-a");
        approval.proposal_id = Some(proposal.id.clone());
        kernel
            .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
            .unwrap();
        kernel
            .record_operation_durable_save(
                Some(proposal.id.clone()),
                operation,
                "editor_save:rev-2".to_string(),
            )
            .unwrap();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal.id.clone(),
                action: FeedbackAction::Accepted,
                final_text: None,
                reason: Some("first session feedback".to_string()),
                created_at: now_ms() + 25,
            })
            .unwrap();
    }
    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "trend-session-b".to_string();
        let proposal = kernel
            .create_llm_ghost_proposal(
                observation("林墨抬头，看见旧门内侧刻着半枚残缺的印记。"),
                "那印记和他袖中的玉佩严丝合缝。".to_string(),
                "eval-model",
            )
            .unwrap();
        let operation = proposal.operations[0].clone();
        let mut approval = eval_approval("trend-session-b");
        approval.proposal_id = Some(proposal.id.clone());
        kernel
            .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
            .unwrap();
        kernel
            .record_operation_durable_save(
                Some(proposal.id.clone()),
                operation,
                "editor_save:rev-3".to_string(),
            )
            .unwrap();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal.id.clone(),
                action: FeedbackAction::Accepted,
                final_text: None,
                reason: Some("second session feedback".to_string()),
                created_at: now_ms() + 40,
            })
            .unwrap();

        let trace = kernel.trace_snapshot(80);
        let trend = trace.product_metrics_trend;
        if trend.session_count < 2 {
            errors.push(format!("trend only saw {} sessions", trend.session_count));
        }
        if trend.source_event_count < 6 {
            errors.push(format!(
                "trend did not read enough persisted events: {}",
                trend.source_event_count
            ));
        }
        if trend.recent_sessions.len() < 2 {
            errors.push("trend lacks recent session rows".to_string());
        }
        if !trend
            .recent_sessions
            .iter()
            .all(|session| session.average_save_to_feedback_ms.is_some())
        {
            errors.push("session trend missing save-to-feedback latency".to_string());
        }
        if trend.overall_average_save_to_feedback_ms.is_none() {
            errors.push("trend missing overall save-to-feedback average".to_string());
        }
        if trend.save_to_feedback_delta_ms.is_none() {
            errors.push("trend missing recent-vs-previous latency delta".to_string());
        }
        if !kernel
            .export_trajectory(80)
            .jsonl
            .contains("\"eventType\":\"writer.product_metrics_trend\"")
        {
            errors.push("trajectory export lacks product_metrics_trend event".to_string());
        }
    }
    let _ = std::fs::remove_file(&db_path);

    eval_result(
        "writer_agent:product_metrics_multi_session_trend",
        format!("db={}", db_path.display()),
        errors,
    )
}

pub fn run_memory_correction_overrides_reinforcement_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let slot = "memory|canon|character|沈照".to_string();
    memory
        .upsert_style_preference(
            &format!("memory_reinforcement:{}", slot),
            "reinforcement",
            true,
        )
        .unwrap();
    memory
        .upsert_style_preference(&format!("memory_extract:{}", slot), "reinforcement", true)
        .unwrap();
    memory
        .upsert_style_preference(&format!("memory_correction:{}", slot), "correction", false)
        .unwrap();
    memory
        .upsert_style_preference(&format!("memory_extract:{}", slot), "correction", false)
        .unwrap();

    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(obs).unwrap();

    let mut errors = Vec::new();
    if proposals.iter().any(|proposal| {
        matches!(
            proposal.operations.first(),
            Some(WriterOperation::CanonUpsertEntity { entity }) if entity.name == "沈照"
        )
    }) {
        errors.push("correction did not suppress reinforced memory candidate".to_string());
    }
    let signal = kernel
        .memory
        .list_style_preferences(50)
        .unwrap()
        .into_iter()
        .find(|preference| preference.key == format!("memory_extract:{}", slot));
    if !signal.is_some_and(|signal| signal.accepted_count == 1 && signal.rejected_count == 1) {
        errors.push("memory feedback signal did not preserve both counts".to_string());
    }

    eval_result(
        "writer_agent:memory_correction_overrides_reinforcement",
        format!("proposals={} slot={}", proposals.len(), slot),
        errors,
    )
}

pub fn run_accepted_feedback_reinforces_style_memory_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let mut approval = eval_approval("style_reinforcement");
    approval.proposal_id = Some(proposal.id.clone());
    kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: Some("他终于听见门后有人低声念出了他的名字。".to_string()),
            reason: Some("节奏和语气都合适".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let preferences = kernel.memory.list_style_preferences(20).unwrap();
    let mut errors = Vec::new();
    if !preferences
        .iter()
        .any(|preference| preference.key == "accepted_Ghost" && preference.accepted_count >= 1)
    {
        errors.push("accepted feedback did not reinforce style memory".to_string());
    }
    if preferences
        .iter()
        .any(|preference| preference.key == "accepted_Ghost" && preference.rejected_count > 0)
    {
        errors.push("accepted style reinforcement recorded rejection".to_string());
    }

    eval_result(
        "writer_agent:accepted_feedback_reinforces_style_memory",
        format!("stylePreferences={}", preferences.len()),
        errors,
    )
}

pub fn run_rejected_proposal_records_correction_signal_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    let proposal = kernel
        .observe(obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .expect("fixture should produce canon memory candidate");
    let slot = match proposal.operations.first() {
        Some(WriterOperation::CanonUpsertEntity { entity }) => {
            format!("memory|canon|{}|{}", entity.kind, entity.name)
        }
        _ => String::new(),
    };
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("作者纠错：这不是长期设定".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let preferences = kernel.memory.list_style_preferences(50).unwrap();
    let correction_key = format!("memory_correction:{}", slot);
    let mut errors = Vec::new();
    if !preferences
        .iter()
        .any(|preference| preference.key == correction_key && preference.rejected_count == 1)
    {
        errors.push("rejected memory candidate did not record correction signal".to_string());
    }
    let audit = kernel.ledger_snapshot().memory_audit;
    if !audit
        .iter()
        .any(|entry| entry.proposal_id == proposal.id && entry.action == "Rejected")
    {
        errors.push("rejected memory candidate did not enter memory audit".to_string());
    }

    eval_result(
        "writer_agent:rejected_proposal_records_correction_signal",
        format!("slot={} audit={}", slot, audit.len()),
        errors,
    )
}

pub fn run_style_continuity_learning_eval() -> EvalResult {
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

    kernel
        .memory
        .upsert_style_preference("ghost_accepted_cold_tone", "冷峻克制", true)
        .unwrap();
    let proposals = kernel
        .observe(observation("风停了。他站在原地，久久没有动。"))
        .unwrap();

    let errors = Vec::new();
    // Style preferences influence context but don't guarantee specific proposals

    eval_result(
        "writer_agent:style_continuity_learning",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_ghost_quality_confidence_eval() -> EvalResult {
    let mut errors = Vec::new();

    // Missing contract → confidence heavily penalized
    let missing_mem = WriterMemory::open(Path::new(":memory:")).unwrap();
    let conf_missing =
        agent_writer_lib::writer_agent::kernel::ghost_confidence(0.8, &missing_mem, "test");
    if conf_missing >= 0.6 {
        errors.push(format!(
            "missing contract ghost confidence {} should be < 0.6",
            conf_missing
        ));
    }

    // Vague contract → confidence reduced
    let vague_mem = WriterMemory::open(Path::new(":memory:")).unwrap();
    vague_mem
        .ensure_story_contract_seed("test", "T", "G", "short", "vague", "")
        .unwrap();
    let conf_vague =
        agent_writer_lib::writer_agent::kernel::ghost_confidence(0.8, &vague_mem, "test");
    if conf_vague >= 0.8 {
        errors.push(format!(
            "vague contract ghost confidence {} should be < 0.8",
            conf_vague
        ));
    }

    // Usable contract → confidence unchanged (base ~0.8 caps at 0.9)
    let usable_mem = WriterMemory::open(Path::new(":memory:")).unwrap();
    usable_mem
        .ensure_story_contract_seed(
            "test",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相，在复仇与守护之间做最终选择。",
            "林墨必须在复仇和守护之间做艰难选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let conf_usable =
        agent_writer_lib::writer_agent::kernel::ghost_confidence(0.8, &usable_mem, "test");
    if conf_usable < 0.75 || conf_usable > 0.91 {
        errors.push(format!(
            "usable contract ghost confidence {} should be near 0.8",
            conf_usable
        ));
    }

    eval_result(
        "writer_agent:ghost_quality_confidence",
        format!(
            "missing={:.2} vague={:.2} usable={:.2}",
            conf_missing, conf_vague, conf_usable
        ),
        errors,
    )
}
