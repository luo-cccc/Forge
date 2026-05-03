use super::*;

use agent_writer_lib::writer_agent::operation::WriterOperation;
use agent_writer_lib::writer_agent::proposal::{EvidenceRef, EvidenceSource};
use agent_writer_lib::writer_agent::research_subtask::{
    build_evidence_only_subtask_result, create_subtask_workspace, safe_subtask_artifact_path,
    tool_filter_for_subtask, validate_evidence_only_subtask_result, write_subtask_artifact,
    WriterSubtaskKind,
};

pub fn run_research_subtask_uses_isolated_workspace_eval() -> EvalResult {
    let root = std::env::temp_dir().join(format!(
        "forge-research-subtask-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let workspace = create_subtask_workspace(&root, WriterSubtaskKind::Research, "research-1");
    let artifact = write_subtask_artifact(
        &root,
        "research-1",
        "evidence/project-brain-notes.json",
        r#"{"summary":"寒玉戒指线索来自 Project Brain。"}"#,
    );

    let mut errors = Vec::new();
    match &workspace {
        Ok(workspace) => {
            if !workspace.workspace_dir.contains("agent_subtasks") {
                errors.push("workspace dir is not under agent_subtasks".to_string());
            }
            if !workspace.artifact_dir.ends_with("artifacts") {
                errors.push("artifact dir is not isolated under workspace artifacts".to_string());
            }
        }
        Err(error) => errors.push(format!("failed to create isolated workspace: {}", error)),
    }
    if let Err(error) = &artifact {
        errors.push(format!("failed to write subtask artifact: {}", error));
    }
    for unsafe_path in ["../secret.md", "evidence/../../secret.md"] {
        if safe_subtask_artifact_path(&root, "research-1", unsafe_path).is_ok() {
            errors.push(format!("unsafe artifact path accepted: {}", unsafe_path));
        }
    }
    if safe_subtask_artifact_path(&root, "../bad", "evidence.json").is_ok() {
        errors.push("unsafe subtask id accepted for workspace".to_string());
    }
    let _ = std::fs::remove_dir_all(&root);

    eval_result(
        "writer_agent:research_subtask_uses_isolated_workspace",
        format!(
            "workspace={} artifact={}",
            workspace.is_ok(),
            artifact.unwrap_or_else(|error| error)
        ),
        errors,
    )
}

pub fn run_research_subtask_outputs_evidence_only_eval() -> EvalResult {
    let result = build_evidence_only_subtask_result(
        WriterSubtaskKind::Research,
        "research-2",
        "Find whether Chapter-5 should mention the ring crack.",
        "Project Brain and lore both point to the ring crack as payoff evidence.",
        vec![
            EvidenceRef {
                source: EvidenceSource::Lorebook,
                reference: "lorebook:ring".to_string(),
                snippet: "寒玉戒指的裂纹会在霜铃塔附近显现。".to_string(),
            },
            EvidenceRef {
                source: EvidenceSource::ChapterText,
                reference: "project_brain:chunk-ring-payoff".to_string(),
                snippet: "林墨在霜铃塔发现寒玉戒指的裂纹。".to_string(),
            },
        ],
        vec!["subtask:research-2:artifact:evidence/project-brain-notes.json".to_string()],
        &[WriterOperation::PromiseResolve {
            promise_id: "ring-crack".to_string(),
            chapter: "Chapter-5".to_string(),
        }],
        now_ms(),
    )
    .unwrap();
    let validation = validate_evidence_only_subtask_result(&result);

    let mut errors = validation;
    if result.evidence_refs.len() < 2 {
        errors.push("research subtask did not preserve evidence refs".to_string());
    }
    if result.artifact_refs.is_empty() {
        errors.push("research subtask did not preserve artifact refs".to_string());
    }
    if !result
        .blocked_operation_kinds
        .iter()
        .any(|kind| kind == "promise.resolve")
    {
        errors.push("research subtask did not block attempted memory/write operation".to_string());
    }

    eval_result(
        "writer_agent:research_subtask_outputs_evidence_only",
        format!(
            "evidence={} artifacts={} blocked={}",
            result.evidence_refs.len(),
            result.artifact_refs.len(),
            result.blocked_operation_kinds.join(",")
        ),
        errors,
    )
}

pub fn run_diagnostic_subtask_denies_writes_eval() -> EvalResult {
    let registry = agent_harness_core::default_writing_tool_registry();
    let policy = agent_harness_core::PermissionPolicy::new(
        agent_harness_core::PermissionMode::WorkspaceWrite,
    );
    let filter = tool_filter_for_subtask(WriterSubtaskKind::Diagnostic);
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
    let result = build_evidence_only_subtask_result(
        WriterSubtaskKind::Diagnostic,
        "diag-1",
        "Check whether the candidate paragraph violates mission must_not.",
        "The paragraph references the forbidden reveal but does not write a fix.",
        vec![EvidenceRef {
            source: EvidenceSource::ChapterMission,
            reference: "Chapter-5".to_string(),
            snippet: "不要提前揭开玉佩来源。".to_string(),
        }],
        vec!["subtask:diag-1:artifact:evidence/mission-check.json".to_string()],
        &[WriterOperation::TextReplace {
            chapter: "Chapter-5".to_string(),
            from: 0,
            to: 8,
            text: "替换正文".to_string(),
            revision: "rev-5".to_string(),
        }],
        now_ms(),
    )
    .unwrap();

    let mut errors = Vec::new();
    for expected in [
        "load_current_chapter",
        "load_outline_node",
        "search_lorebook",
    ] {
        if !model_tool_names.iter().any(|name| name == expected) {
            errors.push(format!(
                "diagnostic subtask missing read-only tool {}",
                expected
            ));
        }
    }
    for forbidden in [
        "query_project_brain",
        "generate_bounded_continuation",
        "generate_chapter_draft",
        "record_run_trace",
    ] {
        if model_tool_names.iter().any(|name| name == forbidden) {
            errors.push(format!(
                "diagnostic subtask exposed write/provider tool {}",
                forbidden
            ));
        }
    }
    if inventory.allowed.iter().any(|tool| {
        tool.requires_approval
            || tool.side_effect_level > agent_harness_core::ToolSideEffectLevel::Read
            || !tool.tags.iter().any(|tag| tag == "project")
    }) {
        errors.push("diagnostic subtask inventory exceeds read-only project policy".to_string());
    }
    if !inventory.blocked.iter().any(|entry| {
        entry.descriptor.name == "query_project_brain"
            && entry.status == agent_harness_core::EffectiveToolStatus::SideEffectTooHigh
    }) {
        errors.push("diagnostic subtask does not block provider-call project brain".to_string());
    }
    if !result
        .blocked_operation_kinds
        .iter()
        .any(|kind| kind == "text.replace")
    {
        errors.push("diagnostic subtask did not block attempted text write".to_string());
    }

    eval_result(
        "writer_agent:diagnostic_subtask_denies_writes",
        format!(
            "allowed={} modelTools={} blockedOps={}",
            inventory.allowed.len(),
            model_tool_names.join(","),
            result.blocked_operation_kinds.join(",")
        ),
        errors,
    )
}
