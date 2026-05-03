use super::*;

use agent_writer_lib::writer_agent::provider_budget::{
    apply_provider_budget_approval, estimate_provider_cost_micros, evaluate_provider_budget,
    WriterProviderBudgetApproval, WriterProviderBudgetDecision, WriterProviderBudgetRequest,
    WriterProviderBudgetTask,
};

pub fn run_provider_budget_requires_approval_eval() -> EvalResult {
    let mut request = WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        "gpt-4o",
        70_000,
        18_000,
    );
    request.max_total_tokens_without_approval = 60_000;
    request.max_estimated_cost_micros_without_approval = 120_000;
    let report = evaluate_provider_budget(request);

    let mut approved_request = WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ExternalResearch,
        "deepseek/deepseek-v4-flash",
        42_000,
        8_000,
    );
    approved_request.max_total_tokens_without_approval = 45_000;
    approved_request.max_estimated_cost_micros_without_approval = 20_000;
    approved_request.already_approved = true;
    let approved_report = evaluate_provider_budget(approved_request);

    let mut errors = Vec::new();
    if report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "over-budget chapter generation should require approval, got {:?}",
            report.decision
        ));
    }
    if !report.approval_required {
        errors.push("over-budget report did not set approval_required".to_string());
    }
    if !report
        .reasons
        .iter()
        .any(|reason| reason.contains("tokens"))
        || !report.reasons.iter().any(|reason| reason.contains("cost"))
    {
        errors.push(format!(
            "provider budget reasons should include token and cost evidence: {:?}",
            report.reasons
        ));
    }
    if report.remediation.is_empty() {
        errors.push("provider budget approval report lacks remediation".to_string());
    }
    if approved_report.decision != WriterProviderBudgetDecision::Warn {
        errors.push(format!(
            "approved over-budget request should continue as warning, got {:?}",
            approved_report.decision
        ));
    }
    if approved_report.approval_required {
        errors.push("approved over-budget request still requires approval".to_string());
    }
    if estimate_provider_cost_micros("gpt-4o", 1_000_000, 1_000_000) <= 0 {
        errors.push("provider cost estimator returned zero for known model".to_string());
    }

    eval_result(
        "writer_agent:provider_budget_requires_approval",
        format!(
            "decision={:?} cost={} approvedDecision={:?}",
            report.decision, report.estimated_cost_micros, approved_report.decision
        ),
        errors,
    )
}

pub fn run_chapter_generation_provider_budget_preflight_eval() -> EvalResult {
    let target = ChapterTarget {
        title: "Chapter-9".to_string(),
        filename: "chapter-9.md".to_string(),
        number: Some(9),
        summary: "林墨追查寒玉戒指下落。".to_string(),
        status: "empty".to_string(),
    };
    let receipt = agent_writer_lib::chapter_generation::build_chapter_generation_receipt(
        "budget-preflight-1",
        &target,
        "rev-9",
        "写第九章。",
        &[ChapterContextSource {
            source_type: "instruction".to_string(),
            id: "user-instruction".to_string(),
            label: "User instruction".to_string(),
            original_chars: 5,
            included_chars: 5,
            truncated: false,
            score: None,
        }],
        now_ms(),
    );
    let over_budget_report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        "gpt-4o",
        90_000,
        24_000,
    ));
    let error = agent_writer_lib::chapter_generation::provider_budget_error(
        "budget-preflight-1",
        &receipt,
        over_budget_report.clone(),
    );
    let bundle = error.evidence.clone();

    let mut errors = Vec::new();
    if over_budget_report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "over-budget chapter preflight should require approval, got {:?}",
            over_budget_report.decision
        ));
    }
    if error.code != "PROVIDER_BUDGET_APPROVAL_REQUIRED" {
        errors.push(format!(
            "unexpected provider budget error code {}",
            error.code
        ));
    }
    let Some(bundle) = bundle else {
        errors.push("provider budget error lacks failure evidence bundle".to_string());
        return eval_result(
            "writer_agent:chapter_generation_provider_budget_preflight",
            "missing bundle".to_string(),
            errors,
        );
    };
    if bundle.category
        != agent_writer_lib::writer_agent::task_receipt::WriterFailureCategory::ProviderFailed
    {
        errors.push("provider budget failure does not map to provider_failed".to_string());
    }
    if bundle.remediation.is_empty() {
        errors.push("provider budget failure lacks remediation".to_string());
    }
    if bundle
        .details
        .get("providerBudget")
        .and_then(|value| value.get("approvalRequired"))
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        errors.push("failure bundle does not preserve approval-required budget report".to_string());
    }

    eval_result(
        "writer_agent:chapter_generation_provider_budget_preflight",
        format!(
            "decision={:?} evidenceRefs={}",
            over_budget_report.decision,
            bundle.evidence_refs.len()
        ),
        errors,
    )
}

pub fn run_provider_budget_records_run_event_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        "gpt-4o",
        44_000,
        12_000,
    ));
    kernel.record_provider_budget_report(
        "budget-run-event-1",
        &report,
        vec![
            "receipt:budget-run-event-1".to_string(),
            "chapter:Chapter-9".to_string(),
        ],
        now_ms(),
    );
    let snapshot = kernel.trace_snapshot(20);
    let export = kernel.export_trajectory(20);
    let lines = export.jsonl.lines().collect::<Vec<_>>();

    let mut errors = Vec::new();
    if !snapshot.run_events.iter().any(|event| {
        event.event_type == "writer.provider_budget"
            && event.task_id.as_deref() == Some("budget-run-event-1")
            && event
                .data
                .get("providerBudget")
                .and_then(|budget| budget.get("estimatedTotalTokens"))
                .and_then(|value| value.as_u64())
                == Some(report.estimated_total_tokens)
            && event
                .data
                .get("decision")
                .and_then(|value| value.as_str())
                .is_some()
    }) {
        errors.push("trace snapshot lacks provider budget run event detail".to_string());
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.provider_budget\"")
            && line.contains("\"providerBudget\"")
    }) {
        errors.push("trajectory export lacks provider budget run event".to_string());
    }

    eval_result(
        "writer_agent:provider_budget_records_run_event",
        format!(
            "decision={:?} runEvents={} trajectoryLines={}",
            report.decision,
            snapshot.run_events.len(),
            lines.len()
        ),
        errors,
    )
}

pub fn run_model_started_run_event_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let blocked_report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        "gpt-4o",
        90_000,
        24_000,
    ));
    if !blocked_report.approval_required {
        return eval_result(
            "writer_agent:model_started_run_event",
            "blocked fixture did not require approval".to_string(),
            vec!["blocked fixture should require approval before model_started check".to_string()],
        );
    }
    let allowed_report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ManualRequest,
        "gpt-4o-mini",
        1_200,
        512,
    ));
    kernel.record_provider_budget_report(
        "model-start-1",
        &allowed_report,
        vec!["manual_request:model-start".to_string()],
        now_ms(),
    );
    kernel.record_model_started_run_event(
        "model-start-1",
        allowed_report.task,
        allowed_report.model.clone(),
        "openai-compatible",
        true,
        vec!["manual_request:model-start".to_string()],
        Some(&allowed_report),
        now_ms(),
    );
    let snapshot = kernel.trace_snapshot(20);
    let export = kernel.export_trajectory(20);
    let lines = export.jsonl.lines().collect::<Vec<_>>();

    let mut errors = Vec::new();
    if snapshot
        .run_events
        .iter()
        .any(|event| event.event_type == "writer.model_started" && event.task_id.is_none())
    {
        errors.push("model_started event lacks task id".to_string());
    }
    if !snapshot.run_events.iter().any(|event| {
        event.event_type == "writer.model_started"
            && event.task_id.as_deref() == Some("model-start-1")
            && event.data.get("task").and_then(|value| value.as_str()) == Some("manual_request")
            && event.data.get("model").and_then(|value| value.as_str()) == Some("gpt-4o-mini")
            && event.data.get("provider").and_then(|value| value.as_str())
                == Some("openai-compatible")
            && event.data.get("stream").and_then(|value| value.as_bool()) == Some(true)
            && event
                .data
                .get("approvalRequired")
                .and_then(|value| value.as_bool())
                == Some(false)
            && event
                .data
                .get("estimatedTotalTokens")
                .and_then(|value| value.as_u64())
                == Some(allowed_report.estimated_total_tokens)
    }) {
        errors.push("model_started event lacks model/provider/budget facts".to_string());
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.model_started\"")
            && line.contains("\"estimatedTotalTokens\"")
    }) {
        errors.push("trajectory export lacks model_started run event".to_string());
    }
    if snapshot.run_events.iter().any(|event| {
        event.event_type == "writer.model_started"
            && event
                .data
                .get("approvalRequired")
                .and_then(|value| value.as_bool())
                == Some(true)
    }) {
        errors.push("model_started recorded an approval-required call".to_string());
    }

    eval_result(
        "writer_agent:model_started_run_event",
        format!(
            "runEvents={} trajectoryLines={}",
            snapshot.run_events.len(),
            lines.len()
        ),
        errors,
    )
}

pub fn run_tool_called_run_event_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let args = serde_json::json!({
        "query": "玉佩秘密不应进入工具事件原文",
        "limit": 3,
    });
    kernel.record_tool_called_run_event(
        "tool-call-1",
        "query_project_brain",
        "start",
        Some(&args),
        None,
        vec!["tool:query_project_brain".to_string()],
        now_ms(),
    );
    let execution = agent_harness_core::ToolExecution {
        tool_name: "query_project_brain".to_string(),
        input: args.clone(),
        output: serde_json::json!({
            "answer": "模型返回的正文也不应进入 tool_called 摘要"
        }),
        error: None,
        remediation: Vec::new(),
        duration_ms: 17,
    };
    kernel.record_tool_called_run_event(
        "tool-call-1",
        "query_project_brain",
        "end",
        Some(&execution.input),
        Some(&execution),
        vec!["tool:query_project_brain".to_string()],
        now_ms(),
    );
    let snapshot = kernel.trace_snapshot(20);
    let export = kernel.export_trajectory(20);
    let lines = export.jsonl.lines().collect::<Vec<_>>();
    let tool_events = snapshot
        .run_events
        .iter()
        .filter(|event| event.event_type == "writer.tool_called")
        .collect::<Vec<_>>();

    let mut errors = Vec::new();
    if tool_events.len() != 2 {
        errors.push(format!(
            "expected start/end tool_called events, got {}",
            tool_events.len()
        ));
    }
    if !tool_events.iter().any(|event| {
        event.data.get("phase").and_then(|value| value.as_str()) == Some("start")
            && event
                .data
                .get("inputKeys")
                .and_then(|value| value.as_array())
                .is_some_and(|keys| {
                    keys.iter().any(|key| key.as_str() == Some("query"))
                        && keys.iter().any(|key| key.as_str() == Some("limit"))
                })
            && event
                .data
                .get("success")
                .is_some_and(|value| value.is_null())
    }) {
        errors.push("tool_called start event lacks phase/input key summary".to_string());
    }
    if !tool_events.iter().any(|event| {
        event.data.get("phase").and_then(|value| value.as_str()) == Some("end")
            && event.data.get("success").and_then(|value| value.as_bool()) == Some(true)
            && event
                .data
                .get("durationMs")
                .and_then(|value| value.as_u64())
                == Some(17)
            && event
                .data
                .get("outputBytes")
                .and_then(|value| value.as_u64())
                .is_some_and(|bytes| bytes > 0)
    }) {
        errors.push("tool_called end event lacks success/duration/output summary".to_string());
    }
    let serialized = tool_events
        .iter()
        .map(|event| event.data.to_string())
        .collect::<String>();
    for leaked in ["玉佩秘密", "模型返回的正文"] {
        if serialized.contains(leaked) {
            errors.push(format!("tool_called leaked raw value: {}", leaked));
        }
    }
    if !lines.iter().any(|line| {
        line.contains("\"eventType\":\"writer.run_event\"")
            && line.contains("\"writer.tool_called\"")
            && line.contains("\"inputKeys\"")
    }) {
        errors.push("trajectory export lacks tool_called run event".to_string());
    }

    eval_result(
        "writer_agent:tool_called_run_event",
        format!(
            "toolEvents={} trajectoryLines={}",
            tool_events.len(),
            lines.len()
        ),
        errors,
    )
}

pub fn run_tool_executor_audit_records_tool_called_eval() -> EvalResult {
    struct AuditHandler;

    #[async_trait::async_trait]
    impl agent_harness_core::ToolHandler for AuditHandler {
        async fn execute(
            &self,
            _tool_name: &str,
            args: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({
                "answer": format!(
                    "公开资料摘要：{}",
                    args.get("query").and_then(|value| value.as_str()).unwrap_or("")
                )
            }))
        }
    }

    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let kernel = std::sync::Arc::new(std::sync::Mutex::new(WriterAgentKernel::new(
        "eval", memory,
    )));
    let audit_kernel = kernel.clone();
    let audit_sink: agent_harness_core::ToolExecutionAuditSink =
        std::sync::Arc::new(move |event| {
            let Ok(mut kernel) = audit_kernel.lock() else {
                return;
            };
            match event {
                agent_harness_core::ToolExecutionAuditEvent::Start { tool_name, input } => {
                    kernel.record_tool_called_run_event(
                        "direct-tool-eval",
                        tool_name.clone(),
                        "start",
                        Some(&input),
                        None,
                        vec![
                            "direct_tool_executor".to_string(),
                            format!("tool:{}", tool_name),
                        ],
                        now_ms(),
                    );
                }
                agent_harness_core::ToolExecutionAuditEvent::End { execution } => {
                    kernel.record_tool_called_run_event(
                        "direct-tool-eval",
                        execution.tool_name.clone(),
                        "end",
                        Some(&execution.input),
                        Some(&execution),
                        vec![
                            "direct_tool_executor".to_string(),
                            format!("tool:{}", execution.tool_name),
                        ],
                        now_ms(),
                    );
                }
            }
        });

    let registry = agent_harness_core::default_writing_tool_registry();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut executor =
        agent_harness_core::ToolExecutor::new(registry, AuditHandler).with_audit_sink(audit_sink);
    let execution = runtime.block_on(async {
        executor
            .execute(
                "query_project_brain",
                serde_json::json!({
                    "query": "玉佩秘密",
                    "limit": 3
                }),
            )
            .await
    });

    let snapshot = kernel.lock().unwrap().trace_snapshot(20);
    let tool_events = snapshot
        .run_events
        .iter()
        .filter(|event| event.event_type == "writer.tool_called")
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    if execution.error.is_some() {
        errors.push(format!(
            "direct tool execution failed: {:?}",
            execution.error
        ));
    }
    if tool_events.len() != 2 {
        errors.push(format!(
            "expected direct executor start/end tool events, got {}",
            tool_events.len()
        ));
    }
    if !tool_events.iter().any(|event| {
        event.data.get("phase").and_then(|value| value.as_str()) == Some("start")
            && event
                .data
                .get("inputKeys")
                .and_then(|value| value.as_array())
                .is_some_and(|keys| {
                    keys.iter().any(|key| key.as_str() == Some("query"))
                        && keys.iter().any(|key| key.as_str() == Some("limit"))
                })
            && event
                .source_refs
                .iter()
                .any(|reference| reference == "direct_tool_executor")
    }) {
        errors.push("direct tool start event lacks input key/source summary".to_string());
    }
    if !tool_events.iter().any(|event| {
        event.data.get("phase").and_then(|value| value.as_str()) == Some("end")
            && event.data.get("success").and_then(|value| value.as_bool()) == Some(true)
            && event
                .data
                .get("outputBytes")
                .and_then(|value| value.as_u64())
                .is_some_and(|bytes| bytes > 0)
    }) {
        errors.push("direct tool end event lacks success/output summary".to_string());
    }
    let trajectory = kernel.lock().unwrap().export_trajectory(20).jsonl;
    for leaked in ["玉佩秘密", "公开资料摘要"] {
        if trajectory.contains(leaked) {
            errors.push(format!("direct tool audit leaked raw text: {}", leaked));
        }
    }

    eval_result(
        "writer_agent:tool_executor_audit_records_tool_called",
        format!(
            "toolEvents={} outputBytes={}",
            tool_events.len(),
            execution.output.to_string().len()
        ),
        errors,
    )
}

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
    let report = prepared.first_round_provider_budget("gpt-4o");
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
    let report = prepared.first_round_provider_budget("gpt-4o");
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
