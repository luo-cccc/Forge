//! Evaluation harness for the real Writer Agent Kernel.
//! These are product-behavior checks, not mirror implementations.

use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::intent::{AgentBehavior, IntentEngine, WritingIntent};
use agent_writer_lib::writer_agent::kernel::StoryReviewQueueStatus;
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
    results.push(run_timeline_contradiction_eval());
    results.push(run_promise_opportunity_eval());
    results.push(run_promise_opportunity_apply_eval());
    results.push(run_promise_stale_eval());
    results.push(run_promise_defer_operation_eval());
    results.push(run_promise_abandon_operation_eval());
    results.push(run_promise_resolve_operation_eval());
    results.push(run_story_review_queue_promise_eval());
    results.push(run_story_debt_snapshot_eval());

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
