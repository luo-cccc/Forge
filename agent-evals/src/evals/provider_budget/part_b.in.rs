pub fn run_project_brain_provider_budget_eval() -> EvalResult {
    let long_context = "寒玉戒指仍未归还，旧门钥匙和潮汐祭账互相指向同一条线索。".repeat(2600);
    let messages = vec![
        serde_json::json!({"role": "system", "content": format!(
            "You are an expert on this novel. Answer using only these excerpts:\n{}",
            long_context
        )}),
        serde_json::json!({"role": "user", "content": "旧门钥匙和潮汐祭账之间是什么关系？"}),
    ];
    let report = agent_writer_lib::brain_service::project_brain_query_provider_budget_for_model(
        "gpt-4o", &messages,
    );
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.record_provider_budget_report(
        "project-brain-budget-1",
        &report,
        vec![
            "project_brain_query:test".to_string(),
            "project_brain:chunk-1".to_string(),
        ],
        now_ms(),
    );
    let snapshot = kernel.trace_snapshot(20);

    let mut errors = Vec::new();
    if report.task != WriterProviderBudgetTask::ProjectBrainQuery {
        errors.push(format!("unexpected budget task {:?}", report.task));
    }
    if report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "long Project Brain query should require approval, got {:?}",
            report.decision
        ));
    }
    if !report.approval_required {
        errors.push("Project Brain budget report did not require approval".to_string());
    }
    if report.remediation.is_empty() {
        errors.push("Project Brain budget report lacks remediation".to_string());
    }
    if !snapshot.run_events.iter().any(|event| {
        event.event_type == "writer.provider_budget"
            && event.task_id.as_deref() == Some("project-brain-budget-1")
            && event
                .data
                .get("providerBudget")
                .and_then(|budget| budget.get("task"))
                .and_then(|value| value.as_str())
                == Some("project_brain_query")
    }) {
        errors.push("Project Brain provider budget was not recorded as a run event".to_string());
    }

    eval_result(
        "writer_agent:project_brain_provider_budget_preflight",
        format!(
            "decision={:?} tokens={} runEvents={}",
            report.decision,
            report.estimated_total_tokens,
            snapshot.run_events.len()
        ),
        errors,
    )
}

pub fn run_project_brain_provider_budget_approval_eval() -> EvalResult {
    let long_context = "寒玉戒指仍未归还，旧门钥匙和潮汐祭账互相指向同一条线索。".repeat(2600);
    let messages = vec![
        serde_json::json!({"role": "system", "content": format!(
            "You are an expert on this novel. Answer using only these excerpts:\n{}",
            long_context
        )}),
        serde_json::json!({"role": "user", "content": "旧门钥匙和潮汐祭账之间是什么关系？"}),
    ];
    let report = agent_writer_lib::brain_service::project_brain_query_provider_budget_for_model(
        "gpt-4o", &messages,
    );
    let approval = WriterProviderBudgetApproval {
        task: report.task,
        model: report.model.clone(),
        approved_total_tokens: report.estimated_total_tokens,
        approved_cost_micros: report.estimated_cost_micros,
        approved_at_ms: now_ms(),
        source: "explore_project_brain".to_string(),
    };
    let approved = apply_provider_budget_approval(report.clone(), Some(&approval));

    let mut errors = Vec::new();
    if report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "fixture should require approval before coverage check, got {:?}",
            report.decision
        ));
    }
    if approved.decision != WriterProviderBudgetDecision::Warn {
        errors.push(format!(
            "covered Project Brain budget should downgrade to warning, got {:?}",
            approved.decision
        ));
    }
    if approved.approval_required {
        errors.push("covered Project Brain budget still requires approval".to_string());
    }
    if !approved
        .reasons
        .iter()
        .any(|reason| reason.contains("explore_project_brain"))
    {
        errors.push("approved Project Brain budget lacks approval source evidence".to_string());
    }

    eval_result(
        "writer_agent:project_brain_provider_budget_approval",
        format!(
            "before={:?} after={:?} tokens={}",
            report.decision, approved.decision, approved.estimated_total_tokens
        ),
        errors,
    )
}

pub fn run_manual_request_provider_budget_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "请完整分析接下来十章每一章的情节推进、伏笔回收和人物关系变化。"
            .repeat(2400),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            paragraph: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o",
        ),
    );
    let prepared = kernel.prepare_task_run(request, provider, EvalToolHandler, "gpt-4o");

    let mut errors = Vec::new();
    let Ok(prepared) = prepared else {
        return eval_result(
            "writer_agent:manual_request_provider_budget_preflight",
            "prepare failed".to_string(),
            vec!["manual request prepare_task_run failed".to_string()],
        );
    };
    let report =
        prepared.first_round_provider_budget(WriterProviderBudgetTask::ManualRequest, "gpt-4o");
    kernel.record_provider_budget_report(
        "manual-budget-1",
        &report,
        vec!["manual_request:eval".to_string()],
        now_ms(),
    );
    let snapshot = kernel.trace_snapshot(20);

    if report.task != WriterProviderBudgetTask::ManualRequest {
        errors.push(format!("unexpected manual budget task {:?}", report.task));
    }
    if report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "long manual request should require approval, got {:?}",
            report.decision
        ));
    }
    if !report.approval_required {
        errors.push("manual request budget report did not require approval".to_string());
    }
    if report.remediation.is_empty() {
        errors.push("manual request budget report lacks remediation".to_string());
    }
    if !snapshot.run_events.iter().any(|event| {
        event.event_type == "writer.provider_budget"
            && event.task_id.as_deref() == Some("manual-budget-1")
            && event
                .data
                .get("providerBudget")
                .and_then(|budget| budget.get("task"))
                .and_then(|value| value.as_str())
                == Some("manual_request")
    }) {
        errors.push("manual request provider budget was not recorded as run event".to_string());
    }

    eval_result(
        "writer_agent:manual_request_provider_budget_preflight",
        format!(
            "decision={:?} tokens={} runEvents={}",
            report.decision,
            report.estimated_total_tokens,
            snapshot.run_events.len()
        ),
        errors,
    )
}

pub fn run_manual_request_provider_budget_approval_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "请完整分析接下来十章每一章的情节推进、伏笔回收和人物关系变化。"
            .repeat(2400),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            paragraph: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o",
        ),
    );
    let prepared = kernel.prepare_task_run(request, provider, EvalToolHandler, "gpt-4o");

    let mut errors = Vec::new();
    let Ok(prepared) = prepared else {
        return eval_result(
            "writer_agent:manual_request_provider_budget_approval",
            "prepare failed".to_string(),
            vec!["manual request prepare_task_run failed".to_string()],
        );
    };
    let report =
        prepared.first_round_provider_budget(WriterProviderBudgetTask::ManualRequest, "gpt-4o");
    let approval = WriterProviderBudgetApproval {
        task: report.task,
        model: report.model.clone(),
        approved_total_tokens: report.estimated_total_tokens,
        approved_cost_micros: report.estimated_cost_micros,
        approved_at_ms: now_ms(),
        source: "explore_manual_request".to_string(),
    };
    let approved = apply_provider_budget_approval(report.clone(), Some(&approval));

    if report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "fixture should require approval before coverage check, got {:?}",
            report.decision
        ));
    }
    if approved.decision != WriterProviderBudgetDecision::Warn {
        errors.push(format!(
            "covered manual request budget should downgrade to warning, got {:?}",
            approved.decision
        ));
    }
    if approved.approval_required {
        errors.push("covered manual request budget still requires approval".to_string());
    }
    if !approved
        .reasons
        .iter()
        .any(|reason| reason.contains("explore_manual_request"))
    {
        errors.push("approved manual request budget lacks approval source evidence".to_string());
    }

    eval_result(
        "writer_agent:manual_request_provider_budget_approval",
        format!(
            "before={:?} after={:?} tokens={}",
            report.decision, approved.decision, approved.estimated_total_tokens
        ),
        errors,
    )
}

pub fn run_manual_request_multi_round_provider_budget_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "先查当前章节，再给我一个简短判断。".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            paragraph: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = Arc::new(TwoRoundBudgetProvider::new("gpt-4o"));
    let prepared = kernel.prepare_task_run(request, provider, EvalToolHandler, "gpt-4o");
    let Ok(mut prepared) = prepared else {
        return eval_result(
            "writer_agent:manual_request_multi_round_provider_budget",
            "prepare failed".to_string(),
            vec!["manual request prepare_task_run failed".to_string()],
        );
    };

    let budget_events: Arc<std::sync::Mutex<Vec<(u32, WriterProviderBudgetDecision)>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let budget_events_for_guard = budget_events.clone();
    prepared.set_provider_call_guard(Arc::new(move |context| {
        let report = agent_writer_lib::writer_agent::kernel::WriterAgentPreparedRun::<
            TwoRoundBudgetProvider,
            EvalToolHandler,
        >::provider_budget_from_call_context(
            WriterProviderBudgetTask::ManualRequest, &context
        );
        budget_events_for_guard
            .lock()
            .unwrap()
            .push((context.round, report.decision));
        if context.round == 2 {
            return Err("eval second provider round blocked by budget guard".to_string());
        }
        Ok(())
    }));

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let run_result = runtime.block_on(async { prepared.run().await });
    let events = budget_events.lock().unwrap().clone();

    let mut errors = Vec::new();
    if run_result.is_ok() {
        errors.push("multi-round guard did not stop the second provider call".to_string());
    }
    if !run_result
        .as_ref()
        .err()
        .is_some_and(|error| error.contains("second provider round blocked"))
    {
        errors.push(format!(
            "run failed with unexpected error {:?}",
            run_result.err()
        ));
    }
    if events.len() != 2 {
        errors.push(format!(
            "expected provider budget guard for two rounds, got {:?}",
            events
        ));
    }
    if !events.iter().any(|(round, _)| *round == 1) || !events.iter().any(|(round, _)| *round == 2)
    {
        errors.push(format!(
            "provider budget guard did not record both rounds: {:?}",
            events
        ));
    }

    eval_result(
        "writer_agent:manual_request_multi_round_provider_budget",
        format!("rounds={:?}", events),
        errors,
    )
}
