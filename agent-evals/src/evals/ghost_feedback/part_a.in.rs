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
    if ghost.is_none_or(|proposal| proposal.alternatives.len() != 3) {
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

pub fn run_product_metrics_manual_ask_conversion_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    kernel.observe(obs.clone()).unwrap();
    let proposal = kernel
        .create_inline_operation_proposal(
            obs,
            "把下一句改成更有行动压力的版本",
            "他抬手按住门环，终于决定当面追问张三。".to_string(),
            "eval-manual-request",
        )
        .unwrap();

    let trace = kernel.trace_snapshot(20);
    let metrics = trace.product_metrics;
    let mut errors = Vec::new();
    if proposal.operations.is_empty() {
        errors.push("manual ask proposal did not carry an operation".to_string());
    }
    if metrics.proposal_count != 1 {
        errors.push(format!(
            "manual ask metrics saw {} proposals",
            metrics.proposal_count
        ));
    }
    if (metrics.manual_ask_converted_to_operation_rate - 1.0).abs() > f64::EPSILON {
        errors.push(format!(
            "manual ask conversion rate was {:.2}",
            metrics.manual_ask_converted_to_operation_rate
        ));
    }
    if !kernel
        .export_trajectory(20)
        .jsonl
        .contains("\"manualAskConvertedToOperationRate\":1.0")
    {
        errors.push("trajectory product metrics lacks manual ask conversion rate".to_string());
    }

    eval_result(
        "writer_agent:product_metrics_manual_ask_conversion",
        format!(
            "manualAskOps={:.2} proposals={} operations={}",
            metrics.manual_ask_converted_to_operation_rate,
            metrics.proposal_count,
            proposal.operations.len()
        ),
        errors,
    )
}

pub fn run_product_metrics_manual_ask_conversion_trend_eval() -> EvalResult {
    let db_path = std::env::temp_dir().join(format!(
        "forge-manual-ask-conversion-trend-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let mut errors = Vec::new();
    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "manual-ask-trend-session".to_string();
        let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
        obs.source = ObservationSource::ManualRequest;
        obs.reason = ObservationReason::Explicit;
        kernel.observe(obs.clone()).unwrap();
        let proposal = kernel
            .create_inline_operation_proposal(
                obs,
                "把下一句改成更有行动压力的版本",
                "他抬手按住门环，终于决定当面追问张三。".to_string(),
                "eval-manual-request",
            )
            .unwrap();

        if !kernel.run_events(20).iter().any(|event| {
            event.event_type == "writer.proposal_created"
                && event
                    .data
                    .get("observationSource")
                    .and_then(|value| value.as_str())
                    == Some("manual_request")
                && event
                    .data
                    .get("operationCount")
                    .and_then(|value| value.as_u64())
                    == Some(1)
        }) {
            errors.push(
                "manual ask proposal_created event lacks source or operation count".to_string(),
            );
        }
        if proposal.operations.is_empty() {
            errors.push("manual ask proposal did not carry an operation".to_string());
        }
    }
    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let kernel = WriterAgentKernel::new("eval", memory);
        let trend = kernel.trace_snapshot(40).product_metrics_trend;
        let session = trend
            .recent_sessions
            .iter()
            .find(|session| session.session_id == "manual-ask-trend-session");

        if session.is_none() {
            errors.push("manual ask trend session was not replayed".to_string());
        }
        if let Some(session) = session {
            if session.manual_ask_proposal_count != 1 {
                errors.push(format!(
                    "manual ask trend proposal count was {}",
                    session.manual_ask_proposal_count
                ));
            }
            if session.manual_ask_operation_count != 1 {
                errors.push(format!(
                    "manual ask trend operation count was {}",
                    session.manual_ask_operation_count
                ));
            }
            if (session.manual_ask_converted_to_operation_rate - 1.0).abs() > f64::EPSILON {
                errors.push(format!(
                    "manual ask trend conversion was {:.2}",
                    session.manual_ask_converted_to_operation_rate
                ));
            }
        }
        if !kernel
            .export_trajectory(40)
            .jsonl
            .contains("\"manualAskConvertedToOperationRate\":1.0")
        {
            errors.push("trajectory trend lacks manual ask conversion rate".to_string());
        }
    }
    let _ = std::fs::remove_file(&db_path);

    eval_result(
        "writer_agent:product_metrics_manual_ask_conversion_trend",
        format!("db={}", db_path.display()),
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

pub fn run_product_metrics_context_pressure_trend_eval() -> EvalResult {
    let db_path = std::env::temp_dir().join(format!(
        "forge-context-pressure-trend-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let mut errors = Vec::new();
    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "context-pressure-session-a".to_string();
        kernel
            .create_llm_ghost_proposal(
                observation("林墨停在旧门前，想起张三带走的玉佩和未解释的密令。"),
                "他把声音压低，先确认门后的呼吸声。".to_string(),
                "eval-model",
            )
            .unwrap();
    }
    {
        let memory = WriterMemory::open(&db_path).unwrap();
        memory
            .ensure_story_contract_seed(
                "eval",
                "寒影录",
                "玄幻",
                &"玉佩线推动林墨做选择，所有线索都要延迟揭示。".repeat(10),
                &"林墨必须在复仇和守护之间做选择。".repeat(8),
                &"不得提前泄露玉佩来源。".repeat(8),
            )
            .unwrap();
        memory
            .ensure_chapter_mission_seed(
                "eval",
                "Chapter-9",
                &"林墨审问张三，同时必须保留玉佩真正来源的悬念。".repeat(10),
                "逼近玉佩线索",
                "直接揭开玉佩真实来源",
                &"以伪造令牌收束。".repeat(8),
                "eval",
            )
            .unwrap();
        memory
            .upsert_style_preference(
                "dialogue_subtext",
                &"对白保持克制，用动作和停顿暗示压力。".repeat(12),
                true,
            )
            .unwrap();
        for idx in 0..4 {
            memory
                .add_promise(
                    "clue",
                    &format!("玉佩压力{}", idx),
                    &format!(
                        "第{}条玉佩线索必须后续回收。{}",
                        idx,
                        "延迟揭示。".repeat(18)
                    ),
                    "Chapter-2",
                    "Chapter-12",
                    5,
                )
                .unwrap();
        }

        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "context-pressure-session-b".to_string();
        kernel.active_chapter = Some("Chapter-9".to_string());
        kernel
            .create_llm_ghost_proposal(
                observation_in_chapter(
                    &format!(
                        "{}林墨看着张三，问他那枚玉佩到底从谁手里来。",
                        "审讯室里只剩烛火和铁链轻响。".repeat(70)
                    ),
                    "Chapter-9",
                ),
                "张三没有回答，只把袖口往阴影里藏得更深。".to_string(),
                "eval-model",
            )
            .unwrap();

        let trend = kernel.trace_snapshot(80).product_metrics_trend;
        let recent = trend.recent_sessions.first();
        if trend.session_count < 2 {
            errors.push(format!(
                "context pressure trend saw {} sessions",
                trend.session_count
            ));
        }
        if trend.overall_context_coverage_rate <= 0.0 {
            errors.push("overall context coverage was not aggregated".to_string());
        }
        if trend.context_coverage_delta.is_none() {
            errors.push("context coverage trend lacks recent-vs-previous delta".to_string());
        }
        if !recent.is_some_and(|session| {
            session.session_id == "context-pressure-session-b"
                && session.context_pack_count >= 1
                && session.context_requested_chars > session.context_provided_chars
                && session.context_truncated_source_count > 0
                && session.context_coverage_rate > 0.0
        }) {
            errors.push(format!(
                "recent context pressure session missing pressure: {:?}",
                recent
            ));
        }
        if !kernel
            .export_trajectory(80)
            .jsonl
            .contains("\"contextCoverageRate\"")
        {
            errors.push("trajectory trend lacks context coverage fields".to_string());
        }
    }
    let _ = std::fs::remove_file(&db_path);

    eval_result(
        "writer_agent:product_metrics_context_pressure_trend",
        format!("db={}", db_path.display()),
        errors,
    )
}

