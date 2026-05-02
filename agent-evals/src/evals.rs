use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::{
    build_chapter_generation_task_packet, BuiltChapterContext, ChapterContextBudgetReport,
    ChapterContextSource, ChapterTarget,
};
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::intent::{AgentBehavior, IntentEngine, WritingIntent};
use agent_writer_lib::writer_agent::kernel::{
    StoryDebtCategory, StoryReviewQueueStatus, WriterAgentApprovalMode, WriterAgentFrontendState,
    WriterAgentRunRequest, WriterAgentStreamMode, WriterAgentTask,
};
use agent_writer_lib::writer_agent::memory::{
    PromiseKind, StoryContractQuality, StoryContractSummary, WriterMemory,
};
use agent_writer_lib::writer_agent::observation::{
    ObservationReason, ObservationSource, TextRange, WriterObservation,
};
use agent_writer_lib::writer_agent::operation::{OperationApproval, WriterOperation};
use agent_writer_lib::writer_agent::proposal::{EvidenceSource, ProposalKind, ProposalPriority};
use agent_writer_lib::writer_agent::WriterAgentKernel;

pub fn run_intent_eval() -> Vec<EvalResult> {
    let engine = IntentEngine::new();
    let fixtures = [
        (
            "intent:dialogue",
            "林墨深吸一口气，说道：“我不能再替你隐瞒了。”",
            false,
            false,
            WritingIntent::Dialogue,
            AgentBehavior::SuggestContinuation,
            0.15,
        ),
        (
            "intent:emotional_silence",
            "她沉默着，眼泪无声滑落，手指微微颤抖。",
            false,
            false,
            WritingIntent::EmotionalBeat,
            AgentBehavior::StaySilent,
            0.10,
        ),
        (
            "intent:revision_selection",
            "一些被选中的文本",
            true,
            false,
            WritingIntent::Revision,
            AgentBehavior::OfferRevision,
            0.70,
        ),
        (
            "intent:chapter_switch",
            "",
            false,
            true,
            WritingIntent::StructuralPlanning,
            AgentBehavior::ProposeStructure,
            0.60,
        ),
    ];

    fixtures
        .into_iter()
        .map(
            |(
                name,
                text,
                has_selection,
                chapter_switch,
                expected_intent,
                expected_behavior,
                min_conf,
            )| {
                let estimate = engine.classify(text, has_selection, chapter_switch);
                let mut errors = Vec::new();
                if estimate.primary != expected_intent {
                    errors.push(format!(
                        "intent mismatch: got {:?}, expected {:?}",
                        estimate.primary, expected_intent
                    ));
                }
                if estimate.desired_behavior != expected_behavior {
                    errors.push(format!(
                        "behavior mismatch: got {:?}, expected {:?}",
                        estimate.desired_behavior, expected_behavior
                    ));
                }
                if estimate.confidence < min_conf {
                    errors.push(format!(
                        "confidence too low: got {:.2}, min {:.2}",
                        estimate.confidence, min_conf
                    ));
                }
                eval_result(
                    name,
                    format!(
                        "{:?} {:?} conf={:.2}",
                        estimate.primary, estimate.desired_behavior, estimate.confidence
                    ),
                    errors,
                )
            },
        )
        .collect()
}

pub fn run_canon_conflict_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();

    let mut errors = Vec::new();
    let conflict = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning);
    if conflict.is_none() {
        errors.push("missing continuity warning".to_string());
    }
    if !conflict.is_some_and(|proposal| {
        proposal.evidence.iter().any(|evidence| {
            evidence.source == EvidenceSource::Canon && evidence.snippet.contains("寒影刀")
        })
    }) {
        errors.push("continuity warning lacks canon evidence".to_string());
    }
    if !conflict.is_some_and(|proposal| {
        proposal.operations.iter().any(|operation| {
            matches!(
                operation,
                WriterOperation::TextReplace {
                    from: 4,
                    to: 6,
                    text,
                    revision,
                    ..
                    } if text == "寒影刀" && revision == "rev-1"
            )
        })
    }) {
        errors.push("continuity warning lacks executable canon text replacement".to_string());
    }
    if !conflict.is_some_and(|proposal| {
        proposal
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::CanonUpdateAttribute { .. }))
    }) {
        errors.push("continuity warning lacks executable canon update alternative".to_string());
    }

    eval_result(
        "writer_agent:canon_conflict_weapon",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_canon_conflict_update_canon_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let paragraph = "林墨拔出长剑，指向门外的人。";
    let proposals = kernel.observe(observation(paragraph)).unwrap();
    let operation = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .and_then(|proposal| {
            proposal
                .operations
                .iter()
                .find(|operation| matches!(operation, WriterOperation::CanonUpdateAttribute { .. }))
                .cloned()
        });

    let mut errors = Vec::new();
    let Some(operation) = operation else {
        return eval_result(
            "writer_agent:canon_conflict_update_canon_resolves_future_warning",
            format!("proposals={}", proposals.len()),
            vec!["missing canon.update_attribute operation".to_string()],
        );
    };
    let result = kernel
        .approve_editor_operation_with_approval(
            operation,
            "",
            Some(&eval_approval("canon_conflict_update")),
        )
        .unwrap();
    if !result.success {
        errors.push(format!(
            "canon update failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let mut next = observation(paragraph);
    next.id = "eval-canon-updated".to_string();
    let next_proposals = kernel.observe(next).unwrap();
    if next_proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
    {
        errors.push("canon warning repeated after updating canon".to_string());
    }
    if !kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| {
            decision.decision == "updated_canon" && decision.rationale.contains("weapon")
        })
    {
        errors.push("canon update did not record a creative decision".to_string());
    }

    eval_result(
        "writer_agent:canon_conflict_update_canon_resolves_future_warning",
        format!("success={} next={}", result.success, next_proposals.len()),
        errors,
    )
}

pub fn run_canon_conflict_apply_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let operation = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .and_then(|proposal| {
            proposal
                .operations
                .iter()
                .find(|operation| matches!(operation, WriterOperation::TextReplace { .. }))
                .cloned()
        });

    let mut errors = Vec::new();
    let Some(operation) = operation else {
        return eval_result(
            "writer_agent:canon_conflict_apply_replaces_text",
            format!("proposals={}", proposals.len()),
            vec!["missing text.replace operation on canon warning".to_string()],
        );
    };

    let mut approval = eval_approval("canon_conflict_apply");
    approval.proposal_id = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .map(|proposal| proposal.id.clone());
    let result = kernel
        .approve_editor_operation_with_approval(operation, "rev-1", Some(&approval))
        .unwrap();
    if !result.success {
        errors.push(format!(
            "text replacement failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }

    eval_result(
        "writer_agent:canon_conflict_apply_replaces_text",
        format!("success={}", result.success),
        errors,
    )
}

pub fn run_story_review_queue_canon_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();

    let queue = kernel.story_review_queue();
    let conflict = queue
        .iter()
        .find(|entry| entry.category == ProposalKind::ContinuityWarning);
    let mut errors = Vec::new();
    if conflict.is_none() {
        errors.push("missing canon conflict review entry".to_string());
    }
    if !conflict.is_some_and(|entry| entry.status == StoryReviewQueueStatus::Pending) {
        errors.push("canon conflict review entry is not pending".to_string());
    }
    if !conflict.is_some_and(|entry| {
        entry
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::TextReplace { .. }))
    }) {
        errors.push("canon conflict review entry lacks text.replace".to_string());
    }
    if !conflict.is_some_and(|entry| {
        entry
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::Canon)
    }) {
        errors.push("canon conflict review entry lacks canon evidence".to_string());
    }

    eval_result(
        "writer_agent:review_queue_canon_conflict_executable",
        format!("queue={}", queue.len()),
        errors,
    )
}

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

pub fn run_context_budget_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
            4,
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨看向张三，想起那枚玉佩，却没有把手从寒影刀上移开。");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_200);

    let mut errors = Vec::new();
    if pack.total_chars > pack.budget_limit {
        errors.push(format!(
            "context exceeded budget: used {} > {}",
            pack.total_chars, pack.budget_limit
        ));
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::CanonSlice)
    {
        errors.push("missing relevant canon slice".to_string());
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::PromiseSlice)
    {
        errors.push("missing active promise slice".to_string());
    }

    eval_result(
        "writer_agent:context_budget_required_sources",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_context_budget_trace_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation(
        "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。",
    );
    let proposals = kernel.observe(obs).unwrap();
    let ghost = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost);
    let trace = kernel.trace_snapshot(10);
    let trace_budget = ghost.and_then(|proposal| {
        trace
            .recent_proposals
            .iter()
            .find(|entry| entry.id == proposal.id)
            .and_then(|entry| entry.context_budget.as_ref())
    });

    let mut errors = Vec::new();
    let actual = if let Some(budget) = trace_budget {
        if budget.task != "GhostWriting" {
            errors.push(format!("unexpected task {}", budget.task));
        }
        if budget.used > budget.total_budget {
            errors.push(format!(
                "trace budget exceeded: used {} > {}",
                budget.used, budget.total_budget
            ));
        }
        if budget.source_reports.is_empty() {
            errors.push("trace missing source budget reports".to_string());
        }
        format!(
            "task={} used={} total={} sources={}",
            budget.task,
            budget.used,
            budget.total_budget,
            budget.source_reports.len()
        )
    } else {
        errors.push("missing context budget trace for ghost proposal".to_string());
        "missing".to_string()
    };

    eval_result("writer_agent:context_budget_trace", actual, errors)
}

pub fn run_context_window_guard_eval() -> EvalResult {
    let messages = vec![agent_harness_core::provider::LlmMessage {
        role: "user".to_string(),
        content: Some("风".repeat(12_000)),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];
    let guard = agent_harness_core::evaluate_context_window(
        agent_harness_core::ContextWindowInfo {
            tokens: 4_096,
            reference_tokens: None,
            source: agent_harness_core::ContextWindowSource::Env,
        },
        agent_harness_core::context_window_guard::estimate_request_tokens(&messages, None),
        512,
    );

    let mut errors = Vec::new();
    if !guard.should_block {
        errors.push("oversized prompt did not block against small context window".to_string());
    }
    if !guard
        .message
        .as_deref()
        .is_some_and(|message| message.contains("too small"))
    {
        errors.push("guard message does not explain context window failure".to_string());
    }

    eval_result(
        "writer_agent:context_window_guard_blocks_small_model",
        format!(
            "ctx={} estimated={} output={} block={}",
            guard.tokens,
            guard.estimated_input_tokens,
            guard.requested_output_tokens,
            guard.should_block
        ),
        errors,
    )
}

pub fn run_compaction_latest_user_anchor_eval() -> EvalResult {
    let messages = vec![
        eval_llm_message("user", "旧请求：分析第一章"),
        eval_llm_message("assistant", "旧回答：第一章节奏偏慢"),
        eval_llm_message("user", "ACTIVE TASK: 继续写第七章的审讯场景"),
        eval_llm_message("assistant", "我正在查看上下文"),
        eval_llm_message("assistant", "准备续写"),
    ];
    let anchored = agent_harness_core::anchor_latest_user_message(&messages, 4);
    let safe = agent_harness_core::find_safe_boundary(&messages, anchored);
    let preserved = &messages[safe..];

    let mut errors = Vec::new();
    if anchored != 2 {
        errors.push(format!(
            "latest user anchor should move cut to 2, got {}",
            anchored
        ));
    }
    if !preserved.iter().any(|message| {
        message.role == "user"
            && message
                .content
                .as_deref()
                .is_some_and(|content| content.contains("ACTIVE TASK"))
    }) {
        errors.push("latest user task was not preserved in compaction tail".to_string());
    }

    eval_result(
        "agent_harness:compaction_preserves_latest_user_task",
        format!(
            "anchored={} safe={} preserved={}",
            anchored,
            safe,
            preserved.len()
        ),
        errors,
    )
}

fn eval_llm_message(role: &str, content: &str) -> agent_harness_core::provider::LlmMessage {
    agent_harness_core::provider::LlmMessage {
        role: role.to_string(),
        content: Some(content.to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }
}

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

pub fn run_manual_request_kernel_owns_run_loop_eval() -> EvalResult {
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "这段接下来应该怎么推进？".to_string(),
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
            "gpt-4o-mini",
        ),
    );
    let prepared = kernel.prepare_task_run(request, provider, EvalToolHandler, "gpt-4o-mini");
    let trace = kernel.trace_snapshot(10);
    let packet = trace
        .task_packets
        .iter()
        .find(|packet| packet.task == "ManualRequest");

    let mut errors = Vec::new();
    if let Err(error) = &prepared {
        errors.push(format!("kernel failed to prepare manual run: {}", error));
    }
    if packet.is_none() {
        errors.push("manual request did not create kernel task packet before run loop".to_string());
    }
    if !packet.is_some_and(|packet| {
        packet.packet.intent == Some(agent_harness_core::Intent::Chat)
            && packet.max_side_effect_level == "ProviderCall"
            && !packet.packet.tool_policy.allow_approval_required
            && packet
                .packet
                .feedback
                .memory_writes
                .iter()
                .any(|write| write == "manual_agent_turn")
    }) {
        errors.push(
            "manual request packet does not own chat intent/tool/feedback policy".to_string(),
        );
    }
    if let Ok(prepared) = &prepared {
        let names = prepared
            .proposals()
            .iter()
            .map(|proposal| proposal.id.clone())
            .collect::<Vec<_>>();
        if !names.is_empty() && trace.recent_proposals.is_empty() {
            errors.push("prepared run proposals were not registered in kernel trace".to_string());
        }
    }

    eval_result(
        "writer_agent:manual_request_kernel_owns_run_loop",
        format!(
            "prepared={} taskPackets={}",
            prepared.is_ok(),
            trace.task_packets.len()
        ),
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
    let context = BuiltChapterContext {
        request_id: "chapter-eval-1".to_string(),
        target: ChapterTarget {
            title: "Chapter-7".to_string(),
            filename: "chapter-7.md".to_string(),
            number: Some(7),
            summary: "林墨逼问玉佩来源，但不能提前揭露幕后主使。".to_string(),
            status: "draft".to_string(),
        },
        base_revision: "rev-7".to_string(),
        prompt_context: "User instruction\nOutline / beat sheet\nRelevant lorebook entries"
            .to_string(),
        sources: vec![
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
        ],
        budget: ChapterContextBudgetReport {
            max_chars: 24_000,
            included_chars: 1_070,
            source_count: 4,
            truncated_source_count: 1,
            warnings: vec![],
        },
        warnings: vec![],
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

struct EvalToolHandler;

#[async_trait::async_trait]
impl agent_harness_core::ToolHandler for EvalToolHandler {
    async fn execute(
        &self,
        tool_name: &str,
        _args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"reachedHandler": true, "tool": tool_name}))
    }
}

pub fn run_result_feedback_tight_budget_eval() -> EvalResult {
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
            "Chapter-3",
            "承接上一章玉佩线索。",
            "玉佩",
            "提前揭开真相",
            "以新的选择收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter(
        "林墨发现玉佩仍在张三手里，新的冲突让两人信任受损。",
        "Chapter-2",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let obs = observation_in_chapter("林墨站在门外，想起上一章的争执。", "Chapter-3");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_050);
    let result_source = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::ResultFeedback);

    let mut errors = Vec::new();
    if result_source.is_none() {
        errors.push("tight budget dropped ResultFeedback source".to_string());
    }
    if !result_source.is_some_and(|source| source.content.contains("章节结果")) {
        errors.push("ResultFeedback source lacks rendered chapter result".to_string());
    }
    if pack.total_chars > pack.budget_limit {
        errors.push(format!(
            "context exceeded tight budget: used {} > {}",
            pack.total_chars, pack.budget_limit
        ));
    }

    eval_result(
        "writer_agent:result_feedback_survives_tight_budget",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_context_decision_slice_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .record_decision(
            "Chapter-1",
            "林墨不主动解释",
            "accepted",
            &[],
            "保持克制，不用大段自白。",
            &[],
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨看向张三，把快到嘴边的话又咽了回去。");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_200);

    let mut errors = Vec::new();
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::DecisionSlice)
    {
        errors.push("missing recent decision slice".to_string());
    }
    if !pack.sources.iter().any(|source| {
        source.source == ContextSource::DecisionSlice && source.content.contains("不用大段自白")
    }) {
        errors.push("decision slice lacks recorded rationale".to_string());
    }

    eval_result(
        "writer_agent:context_includes_recent_decisions",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_story_contract_context_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨握着寒影刀，想起那枚玉佩。");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_500);

    let mut errors = Vec::new();
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::ProjectBrief)
    {
        errors.push("missing story contract project brief".to_string());
    }
    if !pack.sources.iter().any(|source| {
        source.source == ContextSource::ProjectBrief && source.content.contains("读者承诺")
    }) {
        errors.push("project brief lacks story contract content".to_string());
    }

    eval_result(
        "writer_agent:story_contract_context_source",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_foundation_write_validation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let contract_result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StoryContractUpsert {
                contract: agent_writer_lib::writer_agent::operation::StoryContractOp {
                    project_id: "eval".to_string(),
                    title: "寒影录".to_string(),
                    genre: "玄幻".to_string(),
                    target_reader: "".to_string(),
                    reader_promise: "爽文".to_string(),
                    first_30_chapter_promise: "".to_string(),
                    main_conflict: "复仇".to_string(),
                    structural_boundary: "".to_string(),
                    tone_contract: "".to_string(),
                },
            },
            "",
            Some(&eval_approval("foundation_validation")),
        )
        .unwrap();
    let mission_result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::ChapterMissionUpsert {
                mission: agent_writer_lib::writer_agent::operation::ChapterMissionOp {
                    project_id: "eval".to_string(),
                    chapter_title: "Chapter-1".to_string(),
                    mission: "打架".to_string(),
                    must_include: "".to_string(),
                    must_not: "剧透".to_string(),
                    expected_ending: "".to_string(),
                    status: "in_progress".to_string(),
                    source_ref: "eval".to_string(),
                },
            },
            "",
            Some(&eval_approval("foundation_validation")),
        )
        .unwrap();
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    if contract_result.success {
        errors.push("incomplete story contract was accepted".to_string());
    }
    if !contract_result
        .error
        .as_ref()
        .is_some_and(|error| error.code == "invalid" && error.message.contains("Story Contract"))
    {
        errors.push(format!(
            "story contract validation error was not explicit: {:?}",
            contract_result.error
        ));
    }
    if mission_result.success {
        errors.push("vague chapter mission was accepted".to_string());
    }
    if !mission_result
        .error
        .as_ref()
        .is_some_and(|error| error.code == "invalid" && error.message.contains("Chapter Mission"))
    {
        errors.push(format!(
            "chapter mission validation error was not explicit: {:?}",
            mission_result.error
        ));
    }
    if ledger.story_contract.is_some() || ledger.active_chapter_mission.is_some() {
        errors.push("invalid foundation writes polluted the ledger snapshot".to_string());
    }

    eval_result(
        "writer_agent:foundation_write_validation",
        format!(
            "contract={} mission={}",
            contract_result.success, mission_result.success
        ),
        errors,
    )
}

pub fn run_story_contract_guard_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三终于说出真相：玉佩其实来自禁地。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let guard = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::StoryContract);
    if guard.is_none() {
        errors.push("missing story contract guard proposal".to_string());
    }
    if !guard.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::StoryContract)
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::TextAnnotate { .. }))
    }) {
        errors.push(
            "story contract guard lacks contract evidence or annotation operation".to_string(),
        );
    }
    if !debt.entries.iter().any(|entry| {
        entry.category == StoryDebtCategory::StoryContract && entry.title.contains("contract")
    }) {
        errors.push("story contract guard did not enter story debt".to_string());
    }

    eval_result(
        "writer_agent:story_contract_guard_story_debt",
        format!("proposals={} debt={}", proposals.len(), debt.total),
        errors,
    )
}

pub fn run_story_contract_negated_guard_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三没有说出真相，也拒绝解释玉佩来源。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    if proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::StoryContract)
    {
        errors.push("negated reveal still created story contract proposal".to_string());
    }
    if debt.contract_count != 0 {
        errors.push(format!(
            "negated reveal created {} contract debts",
            debt.contract_count
        ));
    }

    eval_result(
        "writer_agent:story_contract_negated_reveal_no_debt",
        format!(
            "proposals={} contractDebt={}",
            proposals.len(),
            debt.contract_count
        ),
        errors,
    )
}

pub fn run_story_contract_quality_nominal_eval() -> EvalResult {
    let empty = StoryContractSummary::default();
    let mut vague = StoryContractSummary::default();
    vague.project_id = "eval".to_string();
    vague.title = "测试".to_string();
    vague.genre = "玄幻".to_string();
    vague.reader_promise = "一个故事".to_string();
    vague.main_conflict = "冲突".to_string();
    let mut usable = vague.clone();
    usable.reader_promise = "刀客追查玉佩真相，在复仇与守护之间做出最终选择。".to_string();
    usable.main_conflict = "林墨必须在复仇和守护之间做艰难选择。".to_string();
    usable.tone_contract = "冷峻克制的武侠叙述".to_string();
    let mut strong = usable.clone();
    strong.reader_promise =
        "刀客追查玉佩真相，在复仇与守护之间做出最终选择，揭示隐藏身份。".to_string();
    strong.main_conflict = "林墨必须在复仇和守护之间做艰难选择，同时面对血脉真相。".to_string();
    strong.first_30_chapter_promise = "前30章完成玉佩线第一次大转折".to_string();
    strong.structural_boundary = "不得提前泄露玉佩来源".to_string();
    strong.tone_contract = "冷峻克制的武侠叙述，对话精准，心理描写内敛".to_string();

    let mut errors = Vec::new();
    if empty.quality() != StoryContractQuality::Missing {
        errors.push("empty contract should be Missing".to_string());
    }
    if vague.quality() != StoryContractQuality::Vague {
        errors.push(format!("vague contract was {:?}", vague.quality()));
    }
    if usable.quality() != StoryContractQuality::Usable {
        errors.push(format!("usable contract was {:?}", usable.quality()));
    }
    if strong.quality() != StoryContractQuality::Strong {
        errors.push(format!("strong contract was {:?}", strong.quality()));
    }
    if empty.quality_gaps().len() < 4 {
        errors.push("empty contract should report several gaps".to_string());
    }
    if vague.quality_gaps().len() < 3 {
        errors.push("vague contract should report specific gaps".to_string());
    }
    if strong.quality_gaps().len() != 0 {
        errors.push("strong contract should have zero gaps".to_string());
    }

    eval_result(
        "writer_agent:story_contract_quality_nominal",
        format!(
            "qualities={:?}/{:?}/{:?}/{:?} strongGaps={}",
            empty.quality(),
            vague.quality(),
            usable.quality(),
            strong.quality(),
            strong.quality_gaps().len()
        ),
        errors,
    )
}

pub fn run_story_contract_vague_excluded_from_context_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "寒影录", "玄幻", "一个故事", "选择", "")
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨停在旧门前。");
    let pack = kernel.context_pack_for_default(
        agent_writer_lib::writer_agent::context::AgentTask::GhostWriting,
        &obs,
    );

    let mut errors = Vec::new();
    let contract_source = pack.sources.iter().find(|source| {
        matches!(
            source.source,
            agent_writer_lib::writer_agent::context::ContextSource::ProjectBrief
        )
    });
    if contract_source.is_some() {
        errors.push("vague StoryContract leaked into context pack".to_string());
    }
    if pack.sources.is_empty() {
        errors.push("context pack has zero sources after vague contract exclusion".to_string());
    }

    eval_result(
        "writer_agent:story_contract_vague_excluded_from_context",
        format!(
            "sources={} contractIncluded={}",
            pack.sources.len(),
            contract_source.is_some()
        ),
        errors,
    )
}

pub fn run_story_contract_quality_chapter_gen_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-2".to_string());
    let obs = observation_in_chapter("林墨拔出长刀，迎着风雪走向旧门。", "Chapter-2");
    let pack = kernel.context_pack_for_default(
        agent_writer_lib::writer_agent::context::AgentTask::ChapterGeneration,
        &obs,
    );

    let mut errors = Vec::new();
    let contract_source = pack.sources.iter().find(|source| {
        matches!(
            source.source,
            agent_writer_lib::writer_agent::context::ContextSource::ProjectBrief
        )
    });
    if contract_source.is_none() {
        errors.push("chapter generation pack must include StoryContract source".to_string());
    }
    if let Some(source) = contract_source {
        if !source.content.contains("合同质量") {
            errors.push("chapter generation contract source lacks quality annotation".to_string());
        }
        if !source.content.contains("可用") && !source.content.contains("完整") {
            errors.push("chapter generation contract source quality not visible".to_string());
        }
    }

    eval_result(
        "writer_agent:story_contract_quality_chapter_gen",
        format!(
            "sources={} contractIncluded={}",
            pack.sources.len(),
            contract_source.is_some()
        ),
        errors,
    )
}

pub fn run_chapter_mission_result_feedback_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩",
            "提前揭开真相",
            "下落",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation("林墨发现玉佩的下落，但张三仍没有说出真相。");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let ledger = kernel.ledger_snapshot();
    let mut errors = Vec::new();
    let mission = ledger.active_chapter_mission.as_ref();
    if !mission.is_some_and(|mission| mission.status == "completed") {
        errors.push(format!(
            "mission was not completed: {:?}",
            mission.map(|mission| mission.status.as_str())
        ));
    }
    if ledger.recent_chapter_results.is_empty() {
        errors.push("save did not record chapter result".to_string());
    }
    if !ledger
        .recent_chapter_results
        .iter()
        .any(|result| result.new_clues.iter().any(|clue| clue == "玉佩"))
    {
        errors.push("chapter result lacks carried clue".to_string());
    }

    eval_result(
        "writer_agent:chapter_mission_result_feedback",
        format!(
            "mission={} results={}",
            mission
                .map(|mission| mission.status.as_str())
                .unwrap_or("missing"),
            ledger.recent_chapter_results.len()
        ),
        errors,
    )
}

pub fn run_chapter_mission_partial_progress_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落。",
            "玉佩下落",
            "提前揭开真相",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter(
        "林墨找到玉佩的下落，却发现线索指向另一个疑问。",
        "Chapter-2",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    let mission = ledger.active_chapter_mission.as_ref();
    if !mission.is_some_and(|mission| mission.status == "completed") {
        errors.push(format!(
            "expected mission completed from must_include + ending, got {:?}",
            mission.map(|mission| mission.status.as_str())
        ));
    }
    if proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::ChapterMission && proposal.preview.contains("必保事项")
    }) {
        errors.push("completed mission still emitted save-gap proposal".to_string());
    }

    eval_result(
        "writer_agent:chapter_mission_completed_no_save_gap",
        format!(
            "mission={} proposals={}",
            mission
                .map(|mission| mission.status.as_str())
                .unwrap_or("missing"),
            proposals.len()
        ),
        errors,
    )
}

pub fn run_chapter_mission_guard_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨直接揭开了真相，玉佩来自禁地。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let guard = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ChapterMission);
    if guard.is_none() {
        errors.push("missing chapter mission guard proposal".to_string());
    }
    if !guard.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::TextAnnotate { .. }))
    }) {
        errors.push(
            "chapter mission guard lacks mission evidence or annotation operation".to_string(),
        );
    }
    if !debt.entries.iter().any(|entry| {
        entry.category == StoryDebtCategory::ChapterMission && entry.title.contains("mission")
    }) {
        errors.push("chapter mission guard did not enter story debt".to_string());
    }

    eval_result(
        "writer_agent:chapter_mission_guard_story_debt",
        format!("proposals={} debt={}", proposals.len(), debt.total),
        errors,
    )
}

pub fn run_chapter_mission_negated_guard_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨没有揭开真相，只确认玉佩仍在张三袖中。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    if proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::ChapterMission)
    {
        errors.push("negated mission reveal still created guard proposal".to_string());
    }
    if debt.mission_count != 0 {
        errors.push(format!(
            "negated mission reveal created {} mission debts",
            debt.mission_count
        ));
    }

    eval_result(
        "writer_agent:chapter_mission_negated_reveal_no_debt",
        format!(
            "proposals={} missionDebt={}",
            proposals.len(),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_chapter_mission_save_gap_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落。",
            "玉佩线索",
            "提前揭开真相",
            "以线索推进收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨站在雨里，沉默地看着远处灯火。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let guard = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::ChapterMission && proposal.preview.contains("必保事项")
    });
    if guard.is_none() {
        errors.push("missing chapter mission save-gap guard".to_string());
    }
    if !guard.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && proposal
                .evidence
                .iter()
                .any(|evidence| evidence.source == EvidenceSource::ChapterText)
    }) {
        errors.push("save-gap guard lacks mission and chapter-result evidence".to_string());
    }
    if debt.mission_count != 1 {
        errors.push(format!(
            "expected 1 mission debt after save gap, got {}",
            debt.mission_count
        ));
    }

    eval_result(
        "writer_agent:chapter_mission_save_gap_story_debt",
        format!(
            "proposals={} missionDebt={}",
            proposals.len(),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_chapter_mission_drifted_no_duplicate_save_gap_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨直接揭开真相，玉佩来自禁地。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let mission = kernel.ledger_snapshot().active_chapter_mission;
    if !mission.is_some_and(|mission| mission.status == "drifted") {
        errors.push("mission did not calibrate to drifted".to_string());
    }
    let mission_proposals = proposals
        .iter()
        .filter(|proposal| proposal.kind == ProposalKind::ChapterMission)
        .count();
    if mission_proposals != 1 {
        errors.push(format!(
            "expected one mission violation proposal, got {}",
            mission_proposals
        ));
    }
    if debt.mission_count != 1 {
        errors.push(format!(
            "expected one mission debt after drift, got {}",
            debt.mission_count
        ));
    }

    eval_result(
        "writer_agent:chapter_mission_drift_no_duplicate_gap",
        format!(
            "proposals={} missionDebt={}",
            proposals.len(),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_next_beat_context_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut save = observation_in_chapter(
        "林墨发现玉佩的下落，却开始怀疑张三。新的冲突就此埋下。",
        "Chapter-2",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let obs = observation_in_chapter("林墨站在门外，没有立刻进去。", "Chapter-3");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 2_000);
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    if ledger.next_beat.is_none() {
        errors.push("ledger missing next beat handoff".to_string());
    }
    if !ledger.next_beat.as_ref().is_some_and(|beat| {
        beat.goal.contains("冲突")
            && beat
                .carryovers
                .iter()
                .any(|carryover| carryover.contains("玉佩"))
    }) {
        errors.push("next beat does not carry conflict and promise context".to_string());
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::NextBeat)
    {
        errors.push("ContextPack missing NextBeat source".to_string());
    }
    if !pack.sources.iter().any(|source| {
        source.source == ContextSource::NextBeat && source.content.contains("下一拍目标")
    }) {
        errors.push("NextBeat source lacks rendered handoff content".to_string());
    }

    eval_result(
        "writer_agent:next_beat_context_handoff",
        format!(
            "nextBeat={} sources={}",
            ledger.next_beat.is_some(),
            pack.sources.len()
        ),
        errors,
    )
}

pub fn run_timeline_contradiction_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "张三",
            &["三哥".to_string()],
            "第三章已死亡。",
            &serde_json::json!({ "status": "已死亡" }),
            0.92,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "三哥推门而入，说道：“我回来了。”",
            "Chapter-5",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let warning = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::ContinuityWarning
            && proposal.preview.contains("时间线疑点")
            && proposal.preview.contains("张三")
    });
    if warning.is_none() {
        errors.push("missing timeline contradiction warning".to_string());
    }
    if !warning.is_some_and(|proposal| {
        proposal.evidence.iter().any(|evidence| {
            evidence.source == EvidenceSource::Canon && evidence.snippet.contains("已死亡")
        })
    }) {
        errors.push("timeline warning lacks canon status evidence".to_string());
    }

    eval_result(
        "writer_agent:timeline_contradiction_dead_character",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_promise_opportunity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
        .observe(observation_in_chapter(
            "张三沉默片刻，终于把那枚玉佩放回桌上。",
            "Chapter-3",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let reminder = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("伏笔回收机会")
    });
    if reminder.is_none() {
        errors.push("missing promise payoff opportunity".to_string());
    }
    if !reminder.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::PromiseLedger)
    }) {
        errors.push("promise opportunity lacks promise ledger evidence".to_string());
    }
    if !reminder.is_some_and(|proposal| {
        proposal
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
    }) {
        errors.push("promise opportunity lacks executable resolve operation".to_string());
    }

    eval_result(
        "writer_agent:promise_payoff_opportunity",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_promise_opportunity_apply_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
        .observe(observation_in_chapter(
            "张三沉默片刻，终于把那枚玉佩放回桌上。",
            "Chapter-3",
        ))
        .unwrap();
    let operation = proposals
        .iter()
        .find(|proposal| {
            proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("伏笔回收机会")
        })
        .and_then(|proposal| {
            proposal
                .operations
                .iter()
                .find(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
                .cloned()
        });

    let mut errors = Vec::new();
    let Some(operation) = operation else {
        return eval_result(
            "writer_agent:promise_opportunity_apply_closes_ledger",
            format!("proposals={}", proposals.len()),
            vec!["missing resolve operation on opportunity proposal".to_string()],
        );
    };

    let result = kernel
        .approve_editor_operation_with_approval(
            operation,
            "",
            Some(&eval_approval("promise_opportunity_apply")),
        )
        .unwrap();
    if !result.success {
        errors.push(format!(
            "resolve operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open_count = kernel.ledger_snapshot().open_promises.len();
    if open_count != 0 {
        errors.push(format!(
            "promise ledger still has {} open entries",
            open_count
        ));
    }

    eval_result(
        "writer_agent:promise_opportunity_apply_closes_ledger",
        format!("success={} open={}", result.success, open_count),
        errors,
    )
}

pub fn run_promise_stale_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "mystery",
            "破庙密道",
            "破庙里有密道，需要说明用途。",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨把窗关上，屋外的雨声立刻远了。",
            "Chapter-3",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let stale = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("仍未回收")
    });
    if stale.is_none() {
        errors.push("missing stale promise warning at expected payoff chapter".to_string());
    }
    if !stale.is_some_and(|proposal| {
        matches!(
            proposal.priority,
            ProposalPriority::Normal | ProposalPriority::Urgent
        )
    }) {
        errors.push("stale promise warning priority too low".to_string());
    }
    if !stale.is_some_and(|proposal| {
        proposal
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::PromiseDefer { .. }))
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::PromiseAbandon { .. }))
    }) {
        errors.push("stale promise warning lacks resolve/defer/abandon choices".to_string());
    }

    eval_result(
        "writer_agent:stale_promise_at_payoff_chapter",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_promise_defer_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "mystery",
            "破庙密道",
            "破庙里有密道，需要说明用途。",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseDefer {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-3".to_string(),
                expected_payoff: "Chapter-5".to_string(),
            },
            "",
            Some(&eval_approval("promise_defer")),
        )
        .unwrap();

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "defer operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open = kernel.ledger_snapshot().open_promises;
    if open.len() != 1 || open[0].expected_payoff != "Chapter-5" {
        errors.push("promise payoff chapter was not updated while staying open".to_string());
    }
    if !kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "deferred_promise")
    {
        errors.push("promise defer did not record a creative decision".to_string());
    }
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨把窗关上，屋外的雨声立刻远了。",
            "Chapter-3",
        ))
        .unwrap();
    if proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("仍未回收")
    }) {
        errors.push("deferred promise still warns at the old payoff chapter".to_string());
    }

    eval_result(
        "writer_agent:promise_defer_updates_expected_payoff",
        format!("success={} open={}", result.success, open.len()),
        errors,
    )
}

pub fn run_promise_abandon_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "mystery",
            "破庙密道",
            "破庙里有密道，需要说明用途。",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseAbandon {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-3".to_string(),
                reason: "Author cut this thread during restructuring.".to_string(),
            },
            "",
            Some(&eval_approval("promise_abandon")),
        )
        .unwrap();

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "abandon operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open = kernel.ledger_snapshot().open_promises;
    if !open.is_empty() {
        errors.push(format!("abandoned promise still open: {}", open.len()));
    }
    if !kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "abandoned_promise")
    {
        errors.push("promise abandon did not record a creative decision".to_string());
    }

    eval_result(
        "writer_agent:promise_abandon_closes_with_decision",
        format!("success={} open={}", result.success, open.len()),
        errors,
    )
}

pub fn run_promise_resolve_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
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
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseResolve {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-4".to_string(),
            },
            "",
            Some(&eval_approval("promise_resolve")),
        )
        .unwrap();

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "resolve operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open = kernel.ledger_snapshot().open_promises;
    if !open.is_empty() {
        errors.push(format!("promise still open after resolve: {}", open.len()));
    }

    eval_result(
        "writer_agent:promise_resolve_operation_closes_ledger",
        format!("success={} open={}", result.success, open.len()),
        errors,
    )
}

pub fn run_promise_last_seen_context_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut save = observation_in_chapter("张三把玉佩藏进袖中，没有交代下落。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let ledger = kernel.ledger_snapshot();
    let promise = ledger
        .open_promises
        .iter()
        .find(|promise| promise.title == "玉佩");
    let obs = observation_in_chapter("林墨看着张三空空的袖口。", "Chapter-3");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_200);
    let promise_slice = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice);
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    if !promise.is_some_and(|promise| promise.last_seen_chapter == "Chapter-2") {
        errors.push(format!(
            "promise last seen not updated: {:?}",
            promise.map(|promise| promise.last_seen_chapter.as_str())
        ));
    }
    if !promise_slice.is_some_and(|source| source.content.contains("last seen: Chapter-2")) {
        errors.push("promise context lacks last-seen trail".to_string());
    }
    if !debt.entries.iter().any(|entry| {
        entry.category == StoryDebtCategory::Promise
            && entry
                .evidence
                .iter()
                .any(|evidence| evidence.snippet.contains("last seen: Chapter-2"))
    }) {
        errors.push("story debt promise evidence lacks last-seen trail".to_string());
    }

    eval_result(
        "writer_agent:promise_last_seen_trail",
        format!(
            "lastSeen={} sources={}",
            promise
                .map(|promise| promise.last_seen_chapter.as_str())
                .unwrap_or("missing"),
            pack.sources.len()
        ),
        errors,
    )
}

pub fn run_promise_kind_classification_eval() -> EvalResult {
    let mut errors = Vec::new();

    let kinds = vec![
        ("plot_promise", PromiseKind::PlotPromise, "medium"),
        ("emotional_debt", PromiseKind::EmotionalDebt, "medium"),
        ("object_whereabouts", PromiseKind::ObjectWhereabouts, "high"),
        (
            "character_commitment",
            PromiseKind::CharacterCommitment,
            "medium",
        ),
        ("mystery_clue", PromiseKind::MysteryClue, "high"),
        (
            "relationship_tension",
            PromiseKind::RelationshipTension,
            "medium",
        ),
        ("unknown_type", PromiseKind::Other, "low"),
    ];

    for (input, expected_kind, expected_risk) in &kinds {
        let kind = PromiseKind::from_kind_str(input);
        if kind != *expected_kind {
            errors.push(format!(
                "kind {} classified as {:?}, expected {:?}",
                input, kind, expected_kind
            ));
        }
        let risk = kind.default_risk();
        if risk != *expected_risk {
            errors.push(format!(
                "kind {:?} default_risk={}, expected {}",
                kind, risk, expected_risk
            ));
        }
        if kind.as_kind_str() != *input && *expected_kind != PromiseKind::Other {
            errors.push(format!(
                "kind {:?} roundtrip as_kind_str={}",
                kind,
                kind.as_kind_str()
            ));
        }
    }

    if PromiseKind::default() != PromiseKind::PlotPromise {
        errors.push("default PromiseKind should be PlotPromise".to_string());
    }

    eval_result(
        "writer_agent:promise_kind_classification",
        format!("{} kinds verified", kinds.len()),
        errors,
    )
}

pub fn run_story_review_queue_promise_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    kernel
        .observe(observation_in_chapter(
            "张三沉默片刻，终于把那枚玉佩放回桌上。",
            "Chapter-3",
        ))
        .unwrap();

    let queue = kernel.story_review_queue();
    let payoff = queue.iter().find(|entry| {
        entry.category == ProposalKind::PlotPromise && entry.title.contains("payoff")
    });
    let mut errors = Vec::new();
    if payoff.is_none() {
        errors.push("missing promise payoff review entry".to_string());
    }
    if !payoff.is_some_and(|entry| entry.status == StoryReviewQueueStatus::Pending) {
        errors.push("promise payoff review entry is not pending".to_string());
    }
    if !payoff.is_some_and(|entry| {
        entry
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
    }) {
        errors.push("promise payoff review entry lacks promise.resolve".to_string());
    }

    if let Some(entry) = payoff {
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: entry.proposal_id.clone(),
                action: FeedbackAction::Snoozed,
                final_text: None,
                reason: Some("review later".to_string()),
                created_at: now_ms(),
            })
            .unwrap();
        let updated = kernel
            .story_review_queue()
            .into_iter()
            .find(|updated| updated.id == entry.id);
        if !updated.is_some_and(|updated| updated.status == StoryReviewQueueStatus::Snoozed) {
            errors.push("promise payoff review entry did not move to snoozed".to_string());
        }
    }

    eval_result(
        "writer_agent:review_queue_promise_payoff_status",
        format!("queue={}", queue.len()),
        errors,
    )
}

pub fn run_story_debt_snapshot_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    kernel
        .observe(observation_in_chapter(
            "林墨拔出长剑，指向门外的人。",
            "Chapter-3",
        ))
        .unwrap();

    let debt = kernel.story_debt_snapshot();
    let mut errors = Vec::new();
    if debt.canon_risk_count != 1 {
        errors.push(format!(
            "expected 1 canon risk, got {}",
            debt.canon_risk_count
        ));
    }
    if debt.promise_count != 1 {
        errors.push(format!(
            "expected 1 promise debt, got {}",
            debt.promise_count
        ));
    }
    if debt.open_count < 2 {
        errors.push(format!(
            "expected at least 2 open debts, got {}",
            debt.open_count
        ));
    }
    if !debt
        .entries
        .iter()
        .any(|entry| entry.title.contains("Story truth"))
    {
        errors.push("missing story truth debt entry".to_string());
    }
    if !debt
        .entries
        .iter()
        .any(|entry| entry.title.contains("Open promise"))
    {
        errors.push("missing open promise debt entry".to_string());
    }
    if !debt.entries.iter().any(|entry| {
        entry.title.contains("Open promise")
            && entry
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
    }) {
        errors.push("open promise debt is not executable".to_string());
    }

    eval_result(
        "writer_agent:story_debt_snapshot_counts_foundation",
        format!(
            "total={} open={} canon={} promise={}",
            debt.total, debt.open_count, debt.canon_risk_count, debt.promise_count
        ),
        errors,
    )
}

pub fn run_story_debt_priority_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
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
            "密信",
            "密信被张三拿走，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel
        .observe(observation_in_chapter(
            "林墨拔出长剑，张三直接揭开真相：玉佩来自禁地。",
            "Chapter-2",
        ))
        .unwrap();

    let debt = kernel.story_debt_snapshot();
    let categories = debt
        .entries
        .iter()
        .take(4)
        .map(|entry| entry.category.clone())
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    if categories.len() < 4 {
        errors.push(format!(
            "expected at least 4 debt entries, got {}",
            categories.len()
        ));
    }
    let expected = [
        StoryDebtCategory::StoryContract,
        StoryDebtCategory::ChapterMission,
        StoryDebtCategory::CanonRisk,
        StoryDebtCategory::Promise,
    ];
    for (index, expected_category) in expected.iter().enumerate() {
        if categories.get(index) != Some(expected_category) {
            errors.push(format!(
                "debt priority index {} got {:?}, expected {:?}",
                index,
                categories.get(index),
                expected_category
            ));
        }
    }

    eval_result(
        "writer_agent:story_debt_priority_foundation",
        format!("categories={:?}", categories),
        errors,
    )
}

pub fn run_guard_trace_evidence_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落。",
            "玉佩线索",
            "提前揭开真相",
            "以线索推进收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨站在雨里，沉默地看着远处灯火。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    let gap = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::ChapterMission && proposal.preview.contains("必保事项")
    });
    let trace = kernel.trace_snapshot(10);
    let trace_entry = gap.and_then(|proposal| {
        trace
            .recent_proposals
            .iter()
            .find(|entry| entry.id == proposal.id)
    });

    let mut errors = Vec::new();
    if trace_entry.is_none() {
        errors.push("missing trace entry for mission guard".to_string());
    }
    if !trace_entry.is_some_and(|entry| {
        entry
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && entry
                .evidence
                .iter()
                .any(|evidence| evidence.source == EvidenceSource::ChapterText)
    }) {
        errors.push("trace entry lacks mission and chapter text evidence".to_string());
    }

    eval_result(
        "writer_agent:guard_trace_evidence",
        format!(
            "traceEvidence={}",
            trace_entry.map(|entry| entry.evidence.len()).unwrap_or(0)
        ),
        errors,
    )
}

pub fn run_trajectory_export_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let _ = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let export = kernel.export_trajectory(20);
    let lines = export.jsonl.lines().collect::<Vec<_>>();

    let mut errors = Vec::new();
    if export.schema != "forge-writer-agent-trajectory" {
        errors.push(format!("unexpected trajectory schema {}", export.schema));
    }
    if export.event_count == 0 || lines.len() != export.event_count {
        errors.push(format!(
            "event count mismatch count={} lines={}",
            export.event_count,
            lines.len()
        ));
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.observation\""))
    {
        errors.push("missing observation trajectory event".to_string());
    }
    if !lines
        .iter()
        .any(|line| line.contains("\"eventType\":\"writer.proposal\""))
    {
        errors.push("missing proposal trajectory event".to_string());
    }
    if !lines
        .iter()
        .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
    {
        errors.push("trajectory contains invalid jsonl line".to_string());
    }

    eval_result(
        "writer_agent:trajectory_export_jsonl",
        format!("events={} bytes={}", export.event_count, export.jsonl.len()),
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
    let context = BuiltChapterContext {
        request_id: "chapter-trace-eval".to_string(),
        target: ChapterTarget {
            title: "Chapter-8".to_string(),
            filename: "chapter-8.md".to_string(),
            number: Some(8),
            summary: "林墨追查玉佩下落，并把张三逼到选择边缘。".to_string(),
            status: "draft".to_string(),
        },
        base_revision: "rev-8".to_string(),
        prompt_context: "User instruction\nCurrent chapter beat\nRelevant lorebook entries"
            .to_string(),
        sources: vec![
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
        ],
        budget: ChapterContextBudgetReport {
            max_chars: 24_000,
            included_chars: 1_338,
            source_count: 4,
            truncated_source_count: 0,
            warnings: vec![],
        },
        warnings: vec![],
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

pub fn run_context_recall_tracking_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let warning = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning);
    let trace = kernel.trace_snapshot(10);
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    if warning.is_none() {
        errors.push("missing continuity warning proposal".to_string());
    }
    if !trace.context_recalls.iter().any(|recall| {
        warning.is_some_and(|proposal| recall.last_proposal_id == proposal.id)
            && recall.source == "Canon"
            && recall.snippet.contains("寒影刀")
    }) {
        errors.push("trace context recall missing surfaced canon evidence".to_string());
    }
    if !ledger
        .context_recalls
        .iter()
        .any(|recall| recall.source == "Canon" && recall.recall_count >= 1)
    {
        errors.push("ledger context recalls did not expose canon recall".to_string());
    }

    eval_result(
        "writer_agent:context_recall_tracks_surfaced_evidence",
        format!(
            "proposals={} recalls={}",
            proposals.len(),
            trace.context_recalls.len()
        ),
        errors,
    )
}
