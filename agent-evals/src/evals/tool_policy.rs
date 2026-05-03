use super::*;

pub fn run_tool_permission_guard_eval() -> EvalResult {
    let registry = agent_harness_core::default_writing_tool_registry();
    let mut executor = agent_harness_core::ToolExecutor::new(registry, EvalToolHandler);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let execution = runtime.block_on(async {
        executor
            .execute(
                "generate_chapter_draft",
                serde_json::json!({ "chapter": "Chapter-1" }),
            )
            .await
    });

    let mut errors = Vec::new();
    if !execution.output.is_null() {
        errors.push("approval-required write tool reached handler output".to_string());
    }
    if !execution
        .error
        .as_deref()
        .is_some_and(|error| error.contains("requires explicit approval"))
    {
        errors.push(format!(
            "write tool was not blocked by approval guard: {:?}",
            execution.error
        ));
    }

    eval_result(
        "agent_harness:tool_permission_blocks_approval_write",
        format!(
            "tool={} error={}",
            execution.tool_name,
            execution.error.clone().unwrap_or_default()
        ),
        errors,
    )
}

pub fn run_effective_tool_inventory_eval() -> EvalResult {
    let registry = agent_harness_core::default_writing_tool_registry();
    let policy = agent_harness_core::PermissionPolicy::new(
        agent_harness_core::PermissionMode::WorkspaceWrite,
    );
    let filter = agent_harness_core::ToolFilter {
        intent: Some(agent_harness_core::Intent::GenerateContent),
        include_requires_approval: true,
        include_disabled: false,
        max_side_effect_level: Some(agent_harness_core::ToolSideEffectLevel::Write),
        required_tags: Vec::new(),
    };
    let inventory = registry.effective_inventory(&filter, &policy);
    let model_tool_names: Vec<String> = inventory
        .to_openai_tools()
        .iter()
        .filter_map(|tool| {
            tool["function"]["name"]
                .as_str()
                .map(|name| name.to_string())
        })
        .collect();

    let mut errors = Vec::new();
    for expected in [
        "load_current_chapter",
        "search_lorebook",
        "query_project_brain",
        "generate_bounded_continuation",
    ] {
        if !inventory.allowed.iter().any(|tool| tool.name == expected) {
            errors.push(format!("{} is missing from allowed inventory", expected));
        }
        if !model_tool_names.iter().any(|name| name == expected) {
            errors.push(format!("{} is missing from model tools", expected));
        }
    }
    if inventory
        .allowed
        .iter()
        .any(|tool| tool.name == "generate_chapter_draft")
    {
        errors.push("approval-required write tool is present in allowed inventory".to_string());
    }
    if model_tool_names
        .iter()
        .any(|name| name == "generate_chapter_draft")
    {
        errors.push("approval-required write tool is exposed to model tools".to_string());
    }
    if !inventory.blocked.iter().any(|entry| {
        entry.descriptor.name == "generate_chapter_draft"
            && entry.status == agent_harness_core::EffectiveToolStatus::ApprovalRequired
            && entry
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("requires explicit approval"))
    }) {
        errors.push("blocked inventory lacks approval reason for chapter draft tool".to_string());
    }

    eval_result(
        "agent_harness:effective_tool_inventory_hides_approval_write",
        format!(
            "allowed={} blocked={} model_tools={}",
            inventory.allowed.len(),
            inventory.blocked.len(),
            model_tool_names.join(",")
        ),
        errors,
    )
}

pub fn run_manual_request_tool_boundary_eval() -> EvalResult {
    let registry = agent_harness_core::default_writing_tool_registry();
    let policy = agent_harness_core::PermissionPolicy::new(
        agent_harness_core::PermissionMode::WorkspaceWrite,
    );
    let filter =
        agent_writer_lib::writer_agent::kernel::tool_filter_for_task(AgentTask::ManualRequest);
    let inventory = registry.effective_inventory(&filter, &policy);
    let model_tool_names: Vec<String> = inventory
        .to_openai_tools()
        .iter()
        .filter_map(|tool| {
            tool["function"]["name"]
                .as_str()
                .map(|name| name.to_string())
        })
        .collect();

    let mut errors = Vec::new();
    for expected in ["search_lorebook", "query_project_brain"] {
        if !model_tool_names.iter().any(|name| name == expected) {
            errors.push(format!(
                "manual request model tools missing project context tool {}",
                expected
            ));
        }
    }
    for forbidden in [
        "generate_bounded_continuation",
        "generate_chapter_draft",
        "read_user_drift_profile",
        "record_run_trace",
    ] {
        if model_tool_names.iter().any(|name| name == forbidden) {
            errors.push(format!(
                "manual request exposed non-project or write/generation tool {}",
                forbidden
            ));
        }
    }
    if inventory.allowed.iter().any(|tool| {
        tool.requires_approval
            || tool.side_effect_level > agent_harness_core::ToolSideEffectLevel::ProviderCall
            || !tool.tags.iter().any(|tag| tag == "project")
    }) {
        errors.push(
            "manual request allowed inventory exceeds WriterAgent ManualRequest tool policy"
                .to_string(),
        );
    }

    eval_result(
        "agent_harness:manual_request_tool_boundary",
        format!(
            "allowed={} model_tools={}",
            inventory.allowed.len(),
            model_tool_names.join(",")
        ),
        errors,
    )
}
