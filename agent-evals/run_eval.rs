//! Evaluation harness for the real Writer Agent Kernel.
//! These are product-behavior checks, not mirror implementations.

use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::intent::{AgentBehavior, IntentEngine, WritingIntent};
use agent_writer_lib::writer_agent::kernel::{StoryDebtCategory, StoryReviewQueueStatus};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::{
    ObservationReason, ObservationSource, TextRange, WriterObservation,
};
use agent_writer_lib::writer_agent::operation::WriterOperation;
use agent_writer_lib::writer_agent::proposal::{EvidenceSource, ProposalKind, ProposalPriority};
use agent_writer_lib::writer_agent::WriterAgentKernel;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct EvalResult {
    fixture: String,
    passed: bool,
    actual: String,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EvalReport {
    total: usize,
    passed: usize,
    failed: usize,
    results: Vec<EvalResult>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn observation(paragraph: &str) -> WriterObservation {
    observation_in_chapter(paragraph, "Chapter-1")
}

fn observation_in_chapter(paragraph: &str, chapter_title: &str) -> WriterObservation {
    let cursor = paragraph.chars().count();
    WriterObservation {
        id: format!("eval-{}", now_ms()),
        created_at: now_ms(),
        source: ObservationSource::Editor,
        reason: ObservationReason::Idle,
        project_id: "eval".to_string(),
        chapter_title: Some(chapter_title.to_string()),
        chapter_revision: Some("rev-1".to_string()),
        cursor: Some(TextRange {
            from: cursor,
            to: cursor,
        }),
        selection: None,
        prefix: paragraph.to_string(),
        suffix: String::new(),
        paragraph: paragraph.to_string(),
        full_text_digest: None,
        editor_dirty: true,
    }
}

fn eval_result(fixture: &str, actual: String, errors: Vec<String>) -> EvalResult {
    EvalResult {
        fixture: fixture.to_string(),
        passed: errors.is_empty(),
        actual,
        errors,
    }
}

fn run_intent_eval() -> Vec<EvalResult> {
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

fn run_canon_conflict_eval() -> EvalResult {
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

fn run_canon_conflict_update_canon_eval() -> EvalResult {
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
    let result = kernel.approve_editor_operation(operation, "").unwrap();
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

fn run_canon_conflict_apply_eval() -> EvalResult {
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

    let result = kernel
        .execute_operation(operation, "林墨拔出长剑，指向门外的人。", "rev-1")
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

fn run_story_review_queue_canon_eval() -> EvalResult {
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

fn run_multi_ghost_eval() -> EvalResult {
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

fn run_feedback_suppression_eval() -> EvalResult {
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

fn run_context_budget_eval() -> EvalResult {
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

fn run_context_budget_trace_eval() -> EvalResult {
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

fn run_context_window_guard_eval() -> EvalResult {
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

fn run_compaction_latest_user_anchor_eval() -> EvalResult {
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

fn run_tool_permission_guard_eval() -> EvalResult {
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

fn run_effective_tool_inventory_eval() -> EvalResult {
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

fn run_result_feedback_tight_budget_eval() -> EvalResult {
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

fn run_context_decision_slice_eval() -> EvalResult {
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

fn run_story_contract_context_eval() -> EvalResult {
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

fn run_story_contract_guard_eval() -> EvalResult {
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

fn run_story_contract_negated_guard_eval() -> EvalResult {
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

fn run_chapter_mission_result_feedback_eval() -> EvalResult {
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

fn run_chapter_mission_partial_progress_eval() -> EvalResult {
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

fn run_chapter_mission_guard_eval() -> EvalResult {
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

fn run_chapter_mission_negated_guard_eval() -> EvalResult {
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

fn run_chapter_mission_save_gap_eval() -> EvalResult {
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

fn run_chapter_mission_drifted_no_duplicate_save_gap_eval() -> EvalResult {
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

fn run_next_beat_context_eval() -> EvalResult {
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

fn run_timeline_contradiction_eval() -> EvalResult {
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

fn run_promise_opportunity_eval() -> EvalResult {
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

fn run_promise_opportunity_apply_eval() -> EvalResult {
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

    let result = kernel.approve_editor_operation(operation, "").unwrap();
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

fn run_promise_stale_eval() -> EvalResult {
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

fn run_promise_defer_operation_eval() -> EvalResult {
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
        .approve_editor_operation(
            WriterOperation::PromiseDefer {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-3".to_string(),
                expected_payoff: "Chapter-5".to_string(),
            },
            "",
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

fn run_promise_abandon_operation_eval() -> EvalResult {
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
        .approve_editor_operation(
            WriterOperation::PromiseAbandon {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-3".to_string(),
                reason: "Author cut this thread during restructuring.".to_string(),
            },
            "",
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

fn run_promise_resolve_operation_eval() -> EvalResult {
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
        .approve_editor_operation(
            WriterOperation::PromiseResolve {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-4".to_string(),
            },
            "",
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

fn run_promise_last_seen_context_eval() -> EvalResult {
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

fn run_story_review_queue_promise_eval() -> EvalResult {
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

fn run_story_debt_snapshot_eval() -> EvalResult {
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

fn run_story_debt_priority_eval() -> EvalResult {
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

fn run_guard_trace_evidence_eval() -> EvalResult {
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

fn run_trajectory_export_eval() -> EvalResult {
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

fn run_context_recall_tracking_eval() -> EvalResult {
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

fn main() {
    let mut results = Vec::new();
    results.extend(run_intent_eval());
    results.push(run_canon_conflict_eval());
    results.push(run_canon_conflict_update_canon_eval());
    results.push(run_canon_conflict_apply_eval());
    results.push(run_story_review_queue_canon_eval());
    results.push(run_multi_ghost_eval());
    results.push(run_feedback_suppression_eval());
    results.push(run_context_budget_eval());
    results.push(run_context_budget_trace_eval());
    results.push(run_context_window_guard_eval());
    results.push(run_compaction_latest_user_anchor_eval());
    results.push(run_tool_permission_guard_eval());
    results.push(run_effective_tool_inventory_eval());
    results.push(run_result_feedback_tight_budget_eval());
    results.push(run_context_decision_slice_eval());
    results.push(run_story_contract_context_eval());
    results.push(run_story_contract_guard_eval());
    results.push(run_story_contract_negated_guard_eval());
    results.push(run_chapter_mission_result_feedback_eval());
    results.push(run_chapter_mission_partial_progress_eval());
    results.push(run_chapter_mission_guard_eval());
    results.push(run_chapter_mission_negated_guard_eval());
    results.push(run_chapter_mission_save_gap_eval());
    results.push(run_chapter_mission_drifted_no_duplicate_save_gap_eval());
    results.push(run_next_beat_context_eval());
    results.push(run_timeline_contradiction_eval());
    results.push(run_promise_opportunity_eval());
    results.push(run_promise_opportunity_apply_eval());
    results.push(run_promise_stale_eval());
    results.push(run_promise_defer_operation_eval());
    results.push(run_promise_abandon_operation_eval());
    results.push(run_promise_resolve_operation_eval());
    results.push(run_promise_last_seen_context_eval());
    results.push(run_story_review_queue_promise_eval());
    results.push(run_story_debt_snapshot_eval());
    results.push(run_story_debt_priority_eval());
    results.push(run_guard_trace_evidence_eval());
    results.push(run_trajectory_export_eval());
    results.push(run_context_recall_tracking_eval());

    let passed = results.iter().filter(|result| result.passed).count();
    let report = EvalReport {
        total: results.len(),
        passed,
        failed: results.len() - passed,
        results,
    };

    println!("=== Writer Agent Eval Report ===");
    println!(
        "Total: {} | Passed: {} | Failed: {}",
        report.total, report.passed, report.failed
    );
    println!();

    for result in &report.results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        println!("[{}] {} ({})", status, result.fixture, result.actual);
        for error in &result.errors {
            println!("  -> {}", error);
        }
    }

    let report_dir = Path::new("reports");
    let _ = std::fs::create_dir_all(report_dir);
    let report_path = report_dir.join("eval_report.json");
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        std::fs::write(&report_path, json).ok();
        println!("\nReport saved to {}", report_path.display());
    }

    if report.failed > 0 {
        std::process::exit(1);
    }
}
