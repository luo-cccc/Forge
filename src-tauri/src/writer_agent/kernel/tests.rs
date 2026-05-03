use super::*;
use crate::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use crate::writer_agent::memory::WriterMemory;
use crate::writer_agent::observation::{
    ObservationReason, ObservationSource, TextRange, WriterObservation,
};
use agent_harness_core::TaskScope;

fn observation(paragraph: &str) -> WriterObservation {
    WriterObservation {
        id: "obs-1".to_string(),
        created_at: now_ms(),
        source: ObservationSource::Editor,
        reason: ObservationReason::Idle,
        project_id: "default".to_string(),
        chapter_title: Some("Chapter-1".to_string()),
        chapter_revision: Some("rev".to_string()),
        cursor: Some(TextRange { from: 10, to: 10 }),
        selection: None,
        prefix: paragraph.to_string(),
        suffix: String::new(),
        paragraph: paragraph.to_string(),
        full_text_digest: None,
        editor_dirty: true,
    }
}

fn test_approval(source: &str) -> crate::writer_agent::operation::OperationApproval {
    test_approval_for_proposal(source, "proposal-test")
}

fn test_approval_for_proposal(
    source: &str,
    proposal_id: &str,
) -> crate::writer_agent::operation::OperationApproval {
    crate::writer_agent::operation::OperationApproval {
        source: source.to_string(),
        actor: "author".to_string(),
        reason: "test approval".to_string(),
        proposal_id: Some(proposal_id.to_string()),
        surfaced_to_user: true,
        created_at: now_ms(),
    }
}

#[test]
fn observe_emits_intent_proposal_and_feedback_records_decision() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposals = kernel
            .observe(observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上，听见里面有人压低声音。"))
            .unwrap();

    assert!(proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost));
    assert!(proposals.iter().any(|proposal| matches!(
        proposal.operations.first(),
        Some(WriterOperation::TextInsert { .. })
    )));
    assert!(proposals
        .iter()
        .any(|proposal| proposal.rationale.contains("ContextPack")));
    let proposal = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap();
    let operation = proposal.operations[0].clone();
    kernel
        .approve_editor_operation_with_approval(
            operation.clone(),
            "rev",
            Some(&test_approval_for_proposal("ghost_feedback", &proposal.id)),
        )
        .unwrap();
    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
        )
        .unwrap();
    let proposal_id = proposal.id.clone();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id,
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: None,
            created_at: 2_000,
        })
        .unwrap();

    let status = kernel.status();
    assert_eq!(status.total_feedback_events, 1);
    assert_eq!(status.pending_proposals, 0);
    assert!(kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "accepted"));
}

#[test]
fn approve_editor_operation_checks_revision_without_mutating_text() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let ok = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::TextInsert {
                chapter: "Chapter-1".to_string(),
                at: 3,
                text: "续写".to_string(),
                revision: "rev-1".to_string(),
            },
            "rev-1",
            Some(&test_approval("text_revision")),
        )
        .unwrap();
    assert!(ok.success);

    let conflict = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::TextInsert {
                chapter: "Chapter-1".to_string(),
                at: 3,
                text: "续写".to_string(),
                revision: "rev-1".to_string(),
            },
            "rev-2",
            Some(&test_approval("text_revision")),
        )
        .unwrap();
    assert!(!conflict.success);
    assert_eq!(conflict.error.unwrap().code, "conflict");
}

#[test]
fn approve_editor_operation_requires_context_for_memory_writes() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let result = kernel
        .approve_editor_operation(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue".to_string(),
                value: "prefers_subtext".to_string(),
            },
            "",
        )
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.error.unwrap().code, "approval_required");
}

#[test]
fn execute_operation_records_annotation_without_text_revision() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let result = kernel
        .execute_operation(
            WriterOperation::TextAnnotate {
                chapter: "Chapter-1".to_string(),
                from: 1,
                to: 4,
                message: "这里与设定冲突".to_string(),
                severity: crate::writer_agent::operation::AnnotationSeverity::Warning,
            },
            "",
            "",
        )
        .unwrap();

    assert!(result.success);
    assert!(result.revision_after.is_none());
    assert_eq!(kernel.ledger_snapshot().recent_decisions.len(), 1);
}

#[test]
fn execute_operation_rejects_write_without_approval_context() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let result = kernel
        .execute_operation(
            WriterOperation::TextInsert {
                chapter: "Chapter-1".to_string(),
                at: 0,
                text: "续写".to_string(),
                revision: "rev-1".to_string(),
            },
            "",
            "rev-1",
        )
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.error.unwrap().code, "approval_required");
}

#[test]
fn accepted_text_feedback_requires_durable_save() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "test-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();

    kernel
        .approve_editor_operation_with_approval(
            operation.clone(),
            "rev",
            Some(&test_approval_for_proposal("ghost", &proposal.id)),
        )
        .unwrap();
    kernel
        .apply_feedback(proposal_feedback(
            proposal.id.clone(),
            FeedbackAction::Accepted,
            None,
        ))
        .unwrap();
    assert!(!kernel
        .memory
        .list_style_preferences(20)
        .unwrap()
        .iter()
        .any(|preference| preference.key == "accepted_Ghost"));

    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
        )
        .unwrap();
    kernel
        .apply_feedback(proposal_feedback(
            proposal.id.clone(),
            FeedbackAction::Accepted,
            None,
        ))
        .unwrap();

    assert!(kernel
        .memory
        .list_style_preferences(20)
        .unwrap()
        .iter()
        .any(|preference| preference.key == "accepted_Ghost"));
}

#[test]
fn execute_operation_upserts_canon_rule_and_style_preference() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);

    let rule = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::CanonUpsertRule {
                rule: crate::writer_agent::operation::CanonRuleOp {
                    rule: "林墨不会主动弃刀。".to_string(),
                    category: "character_rule".to_string(),
                    priority: 8,
                },
            },
            "",
            Some(&test_approval("canon_test")),
        )
        .unwrap();
    assert!(rule.success);

    let style = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue".to_string(),
                value: "prefers_subtext".to_string(),
            },
            "",
            Some(&test_approval("style_test")),
        )
        .unwrap();
    assert!(style.success);

    let ledger = kernel.ledger_snapshot();
    assert_eq!(ledger.canon_rules.len(), 1);
    assert_eq!(ledger.canon_rules[0].priority, 8);
    let preferences = kernel.memory.list_style_preferences(10).unwrap();
    assert!(preferences
        .iter()
        .any(|pref| pref.key == "dialogue" && pref.value == "prefers_subtext"));
}

#[test]
fn style_preference_operation_enforces_quality_gates() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);

    let vague = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "tone".to_string(),
                value: "好".to_string(),
            },
            "",
            Some(&test_approval("style_quality")),
        )
        .unwrap();
    assert!(!vague.success);
    assert!(vague
        .error
        .as_ref()
        .is_some_and(|error| error.message.contains("too vague")));

    let first = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_subtext".to_string(),
                value: "对话偏短句留白，避免直接解释情绪".to_string(),
            },
            "",
            Some(&test_approval("style_quality")),
        )
        .unwrap();
    assert!(first.success);

    let duplicate = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_subtext".to_string(),
                value: "对话偏短句留白，避免直接解释情绪".to_string(),
            },
            "",
            Some(&test_approval("style_quality")),
        )
        .unwrap();
    assert!(!duplicate.success);
    assert!(duplicate
        .error
        .as_ref()
        .is_some_and(|error| error.message.contains("already exists")));

    let conflict = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_subtext".to_string(),
                value: "对话要完整解释每个角色的真实情绪".to_string(),
            },
            "",
            Some(&test_approval("style_quality")),
        )
        .unwrap();
    assert!(!conflict.success);
    assert!(conflict
        .error
        .as_ref()
        .is_some_and(|error| error.message.contains("conflicts")));

    let preferences = kernel.memory.list_style_preferences(10).unwrap();
    assert_eq!(
        preferences
            .iter()
            .filter(|preference| preference.key == "dialogue_subtext")
            .count(),
        1
    );
}

#[test]
fn style_preference_taxonomy_detects_same_slot_conflicts() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);

    assert_eq!(
        style_preference_taxonomy_slot("dialogue_subtext", "对话偏短句留白，避免直接解释情绪")
            .as_deref(),
        Some("dialogue.subtext")
    );

    let first = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_subtext".to_string(),
                value: "对话偏短句留白，避免直接解释情绪".to_string(),
            },
            "",
            Some(&test_approval("style_taxonomy")),
        )
        .unwrap();
    assert!(first.success);

    let same_slot_conflict = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_emotion_explanation".to_string(),
                value: "对话要完整解释每个角色的真实情绪".to_string(),
            },
            "",
            Some(&test_approval("style_taxonomy")),
        )
        .unwrap();
    assert!(!same_slot_conflict.success);
    assert!(same_slot_conflict
        .error
        .as_ref()
        .is_some_and(|error| error.message.contains("dialogue.subtext")));

    let different_slot = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "description_sensory_detail".to_string(),
                value: "描写优先保留气味、触感和画面细节".to_string(),
            },
            "",
            Some(&test_approval("style_taxonomy")),
        )
        .unwrap();
    assert!(different_slot.success);
}

#[test]
fn pure_kernel_rejects_outline_update_without_project_runtime() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::OutlineUpdate {
                node_id: "Chapter-1".to_string(),
                patch: serde_json::json!({"summary": "new"}),
            },
            "",
            Some(&test_approval("outline_test")),
        )
        .unwrap();

    assert!(!result.success);
    assert!(result
        .error
        .unwrap()
        .message
        .contains("project storage runtime"));
}

#[test]
fn create_llm_ghost_proposal_registers_typed_operation() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposal = kernel
            .create_llm_ghost_proposal(
                observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。"),
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "test-model",
            )
            .unwrap();

    assert!(proposal.rationale.contains("LLM增强续写"));
    assert!(matches!(
        proposal.operations.first(),
        Some(WriterOperation::TextInsert { .. })
    ));
    assert_eq!(kernel.status().pending_proposals, 1);
}

#[test]
fn create_inline_operation_proposal_uses_selection_replace() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨握住刀柄，沉默片刻。");
    obs.reason = ObservationReason::Explicit;
    obs.selection = Some(super::super::observation::TextSelection {
        from: 2,
        to: 6,
        text: "握住刀柄".to_string(),
    });

    let proposal = kernel
        .create_inline_operation_proposal(
            obs,
            "改得更紧张",
            "指节一点点扣紧刀柄".to_string(),
            "test-model",
        )
        .unwrap();

    assert_eq!(proposal.kind, ProposalKind::ParallelDraft);
    assert!(proposal.rationale.contains("Inline typed operation"));
    assert!(matches!(
        proposal.operations.first(),
        Some(WriterOperation::TextReplace { from: 2, to: 6, .. })
    ));
}

#[test]
fn create_inline_operation_proposal_without_selection_inserts_at_cursor() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨停在门前。");
    obs.reason = ObservationReason::Explicit;
    obs.cursor = Some(TextRange { from: 7, to: 7 });

    let proposal = kernel
        .create_inline_operation_proposal(
            obs,
            "补一句动作",
            "他把呼吸压得更低。".to_string(),
            "test-model",
        )
        .unwrap();

    assert!(matches!(
        proposal.operations.first(),
        Some(WriterOperation::TextInsert { at: 7, .. })
    ));
}

#[test]
fn duplicate_ghost_proposal_is_suppressed_for_same_observation_slot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");

    let first = kernel.observe(obs.clone()).unwrap();
    let second = kernel.observe(obs).unwrap();

    assert!(first
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost));
    assert!(!second
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost));
    assert_eq!(kernel.status().pending_proposals, 1);
}

#[test]
fn implicit_ghost_rejections_snooze_repeated_ignored_slot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let paragraph = "林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。";

    let mut first_obs = observation(paragraph);
    first_obs.id = "obs-ignored-1".to_string();
    let first = kernel.observe(first_obs).unwrap();
    let first_ghost = first
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap();
    let first_id = first_ghost.id.clone();
    assert!(!kernel
        .record_implicit_ghost_rejection(&first_id, now_ms())
        .unwrap());
    assert_eq!(kernel.status().pending_proposals, 0);

    let mut second_obs = observation(paragraph);
    second_obs.id = "obs-ignored-2".to_string();
    second_obs.cursor = Some(TextRange { from: 11, to: 11 });
    let second = kernel.observe(second_obs).unwrap();
    let second_id = second
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap()
        .id
        .clone();
    assert!(!kernel
        .record_implicit_ghost_rejection(&second_id, now_ms())
        .unwrap());

    let mut third_obs = observation(paragraph);
    third_obs.id = "obs-ignored-3".to_string();
    third_obs.cursor = Some(TextRange { from: 12, to: 12 });
    let third = kernel.observe(third_obs).unwrap();
    let third_id = third
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap()
        .id
        .clone();
    assert!(kernel
        .record_implicit_ghost_rejection(&third_id, now_ms())
        .unwrap());

    let mut fourth_obs = observation(paragraph);
    fourth_obs.id = "obs-ignored-4".to_string();
    fourth_obs.cursor = Some(TextRange { from: 13, to: 13 });
    let fourth = kernel.observe(fourth_obs).unwrap();
    assert!(!fourth
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost));
}

#[test]
fn llm_ghost_supersedes_local_ghost_for_same_observation_slot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");

    let local = kernel.observe(obs.clone()).unwrap();
    let local_ghost = local
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap()
        .id
        .clone();
    let llm = kernel
        .create_llm_ghost_proposal(
            obs,
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "test-model",
        )
        .unwrap();

    assert!(llm.rationale.contains("LLM增强续写"));
    assert!(kernel.superseded_proposals.contains(&local_ghost));
    assert_eq!(kernel.status().pending_proposals, 1);
}

#[test]
fn rejected_ghost_suppresses_same_slot_temporarily() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
    let first = kernel.observe(obs.clone()).unwrap();
    let ghost = first
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap();

    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: ghost.id.clone(),
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("too soon".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let mut next_obs = obs;
    next_obs.id = "obs-2".to_string();
    let second = kernel.observe(next_obs).unwrap();

    assert!(!second
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::Ghost));
}

#[test]
fn pending_proposals_excludes_superseded_feedback_and_expired() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
    let local = kernel.observe(obs.clone()).unwrap();
    let local_id = local
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap()
        .id
        .clone();
    let llm = kernel
        .create_llm_ghost_proposal(
            obs,
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "test-model",
        )
        .unwrap();

    let pending = kernel.pending_proposals();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, llm.id);
    assert!(!pending.iter().any(|proposal| proposal.id == local_id));
}

#[test]
fn trace_snapshot_records_observation_proposal_and_state() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
    let local = kernel.observe(obs.clone()).unwrap();
    let local_id = local
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost)
        .unwrap()
        .id
        .clone();
    let llm = kernel
        .create_llm_ghost_proposal(
            obs,
            "他终于听见门后有人低声念出了他的名字。".to_string(),
            "test-model",
        )
        .unwrap();

    let trace = kernel.trace_snapshot(10);
    assert_eq!(trace.recent_observations.len(), 1);
    assert!(trace
        .recent_proposals
        .iter()
        .any(|proposal| proposal.id == local_id && proposal.state == "superseded"));
    let llm_trace = trace
        .recent_proposals
        .iter()
        .find(|proposal| proposal.id == llm.id && proposal.state == "pending")
        .expect("llm proposal trace should exist");
    let budget = llm_trace
        .context_budget
        .as_ref()
        .expect("context budget should be recorded for LLM proposal");
    assert_eq!(budget.task, "GhostWriting");
    assert!(budget.used <= budget.total_budget);
    assert!(!budget.source_reports.is_empty());
}

#[test]
fn trace_snapshot_survives_kernel_restart() {
    let db_path = std::env::temp_dir().join(format!(
        "forge-trace-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let ghost_id = {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
        let proposals = kernel.observe(obs).unwrap();
        let ghost = proposals
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: ghost.clone(),
                action: FeedbackAction::Rejected,
                final_text: None,
                reason: Some("too early".to_string()),
                created_at: 42,
            })
            .unwrap();
        ghost
    };

    let memory = WriterMemory::open(&db_path).unwrap();
    let kernel = WriterAgentKernel::new("default", memory);
    let trace = kernel.trace_snapshot(10);
    let _ = std::fs::remove_file(&db_path);

    assert_eq!(trace.recent_observations.len(), 1);
    let ghost_trace = trace
        .recent_proposals
        .iter()
        .find(|proposal| proposal.id == ghost_id && proposal.state == "feedback:Rejected")
        .expect("persisted ghost trace should exist");
    assert!(ghost_trace.context_budget.is_some());
    assert!(trace
        .recent_feedback
        .iter()
        .any(|feedback| feedback.proposal_id == ghost_id
            && feedback.action == "Rejected"
            && feedback.reason.as_deref() == Some("too early")));
}

#[test]
fn proposal_ids_do_not_collide_across_kernel_restarts() {
    let db_path = std::env::temp_dir().join(format!(
        "forge-proposal-id-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let first_id = {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposals = kernel
                .observe(observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。"))
                .unwrap();
        proposals
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone()
    };

    let second_id = {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
        obs.id = "obs-restart-second".to_string();
        let proposals = kernel.observe(obs).unwrap();
        proposals
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone()
    };

    let memory = WriterMemory::open(&db_path).unwrap();
    let kernel = WriterAgentKernel::new("default", memory);
    let trace = kernel.trace_snapshot(10);
    let _ = std::fs::remove_file(&db_path);

    assert_ne!(first_id, second_id);
    assert!(trace
        .recent_proposals
        .iter()
        .any(|proposal| proposal.id == first_id));
    assert!(trace
        .recent_proposals
        .iter()
        .any(|proposal| proposal.id == second_id));
}

#[test]
fn observe_emits_canon_conflict_from_memory_facts() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();

    assert!(proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::ContinuityWarning));
}

#[test]
fn observe_emits_and_dedupes_diagnostic_pacing_proposal() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let paragraph = "风".repeat(2001);
    let mut obs = observation(&paragraph);
    obs.cursor = Some(TextRange {
        from: paragraph.chars().count(),
        to: paragraph.chars().count(),
    });

    let first = kernel.observe(obs.clone()).unwrap();
    let second = kernel.observe(obs).unwrap();

    assert!(first
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::StyleNote
            && proposal.preview.contains("段落较长")));
    assert!(!second
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::StyleNote
            && proposal.preview.contains("段落较长")));
}

#[test]
fn observe_ghost_uses_context_pack_evidence() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
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
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩",
            "Chapter-1",
            "Chapter-4",
            4,
        )
        .unwrap();

    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposals = kernel
        .observe(observation(
            "林墨停在旧门前，风声压低。他想起张三离开时攥紧的玉佩，却没有立刻追问。",
        ))
        .unwrap();
    let ghost = proposals
        .iter()
        .find(|p| p.kind == ProposalKind::Ghost)
        .unwrap();

    assert!(ghost
        .evidence
        .iter()
        .any(|e| e.source == EvidenceSource::Canon));
    assert!(ghost
        .evidence
        .iter()
        .any(|e| e.source == EvidenceSource::PromiseLedger));
    assert!(ghost.preview.contains("旧事") || ghost.preview.contains("兵器"));
}

#[test]
fn observe_records_context_recalls_from_surfaced_evidence() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let warning = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .expect("continuity warning should exist");

    let trace = kernel.trace_snapshot(10);
    let ledger = kernel.ledger_snapshot();

    assert!(trace.context_recalls.iter().any(|recall| {
        recall.source == "Canon"
            && recall.last_proposal_id == warning.id
            && recall.snippet.contains("寒影刀")
    }));
    assert!(ledger
        .context_recalls
        .iter()
        .any(|recall| recall.source == "Canon"));
}

#[test]
fn observe_ghost_contains_three_parallel_branches() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let text = "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。";
    let mut obs = observation(text);
    let cursor = text.chars().count();
    obs.cursor = Some(TextRange {
        from: cursor,
        to: cursor,
    });
    let proposals = kernel.observe(obs).unwrap();
    let ghost = proposals
        .iter()
        .find(|p| p.kind == ProposalKind::Ghost)
        .unwrap();

    assert_eq!(ghost.alternatives.len(), 3);
    assert_eq!(ghost.alternatives[0].label, "A 直接表态");
    assert!(ghost.alternatives.iter().all(|alternative| matches!(
        alternative.operation,
        Some(WriterOperation::TextInsert { .. })
    )));
}

#[test]
fn save_observation_suggests_memory_candidates_without_writing_ledgers() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩，却始终没有告诉任何人它的下落。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.observe(obs).unwrap();

    assert!(proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::CanonUpdate
            && matches!(
                proposal.operations.first(),
                Some(WriterOperation::CanonUpsertEntity { .. })
            )
    }));
    assert!(proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::PlotPromise
            && matches!(
                proposal.operations.first(),
                Some(WriterOperation::PromiseAdd { .. })
            )
    }));
    let ledger = kernel.ledger_snapshot();
    assert!(ledger.canon_entities.is_empty());
    assert!(ledger.open_promises.is_empty());
    assert_eq!(ledger.recent_chapter_results.len(), 1);
    assert!(ledger.recent_chapter_results[0].summary.contains("沈照"));
    assert!(ledger.recent_chapter_results[0]
        .new_clues
        .contains(&"玉佩".to_string()));
}

#[test]
fn save_observation_records_chapter_result_feedback() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs =
        observation("林墨发现玉佩的下落，却开始怀疑张三。张三选择隐瞒真相，新的冲突就此埋下。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs.chapter_title = Some("第一章".to_string());
    obs.chapter_revision = Some("rev-1".to_string());
    obs.prefix = obs.paragraph.clone();

    kernel.observe(obs).unwrap();

    let ledger = kernel.ledger_snapshot();
    let result = ledger.recent_chapter_results.first().unwrap();
    assert_eq!(result.chapter_title, "第一章");
    assert_eq!(result.chapter_revision, "rev-1");
    assert!(result.summary.contains("玉佩"));
    assert!(result
        .new_conflicts
        .iter()
        .any(|line| line.contains("冲突")));
    assert!(result.new_clues.contains(&"玉佩".to_string()));
    assert!(result.source_ref.contains("chapter_save:第一章:rev-1"));
}

#[test]
fn invalid_task_packet_is_rejected_before_trace() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let packet = TaskPacket::new(
        "bad-packet",
        "missing foundation fields",
        TaskScope::Chapter,
        1,
    );

    let error = kernel
        .record_task_packet("obs-1", "ChapterGeneration", packet)
        .unwrap_err();

    assert!(error.contains("scopeRef"));
    assert!(kernel.trace_snapshot(10).task_packets.is_empty());
}

#[test]
fn save_observation_result_feedback_feeds_next_task_packet() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs =
        observation("林墨发现玉佩的下落，却开始怀疑张三。张三选择隐瞒真相，新的冲突就此埋下。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs.chapter_title = Some("第一章".to_string());
    obs.chapter_revision = Some("rev-1".to_string());
    obs.prefix = obs.paragraph.clone();

    kernel.observe(obs).unwrap();
    let next = observation("林墨深吸一口气，说道：“");
    kernel
        .create_llm_ghost_proposal(next, "我已经知道玉佩在哪了。".to_string(), "eval-model")
        .unwrap();

    let trace = kernel.trace_snapshot(10);
    let packet_trace = trace
        .task_packets
        .iter()
        .find(|packet| packet.task == "GhostWriting")
        .expect("next ghost task should record a task packet");
    assert!(packet_trace.foundation_complete);
    assert_eq!(packet_trace.packet.scope, TaskScope::CursorWindow);
    assert!(packet_trace
        .packet
        .required_context
        .iter()
        .any(|context| context.source_type == "ResultFeedback" && context.required));
    assert!(packet_trace
        .packet
        .beliefs
        .iter()
        .any(|belief| belief.subject == "ResultFeedback" && belief.statement.contains("玉佩")));
}

#[test]
fn save_observation_calibrates_completed_chapter_mission() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "default",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩",
            "提前揭开真相",
            "下落",
            "test",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨发现玉佩的下落，但张三仍没有说出真相。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    kernel.observe(obs).unwrap();

    let mission = kernel
        .ledger_snapshot()
        .active_chapter_mission
        .expect("mission should stay active");
    assert_eq!(mission.status, "completed");
    assert!(mission.source_ref.contains("result_feedback:chapter_save"));
}

#[test]
fn save_observation_marks_chapter_mission_drifted_on_must_not_hit() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "default",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩",
            "真相",
            "下落",
            "test",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨发现玉佩的下落，并当场揭开真相。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    kernel.observe(obs).unwrap();

    let mission = kernel
        .ledger_snapshot()
        .active_chapter_mission
        .expect("mission should stay active");
    assert_eq!(mission.status, "drifted");
    assert!(kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "mission_status:drifted"));
}

#[test]
fn ledger_snapshot_derives_next_beat_from_latest_result_and_promises() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩",
            "Chapter-1",
            "Chapter-3",
            4,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨发现玉佩的下落，却开始怀疑张三。新的冲突就此埋下。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs.chapter_title = Some("Chapter-2".to_string());

    kernel.observe(obs).unwrap();

    let next_beat = kernel
        .ledger_snapshot()
        .next_beat
        .expect("next beat should be derived from saved result");
    assert!(next_beat.goal.contains("冲突"));
    assert!(next_beat
        .carryovers
        .iter()
        .any(|line| line.contains("玉佩")));
    assert!(next_beat
        .source_refs
        .iter()
        .any(|source| source.contains("chapter_save:Chapter-2")));
}

#[test]
fn accepted_memory_candidate_writes_ledger() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposal = kernel
        .observe(obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .unwrap();
    let result = kernel
        .approve_editor_operation_with_approval(
            proposal.operations[0].clone(),
            "",
            Some(&test_approval("memory_candidate")),
        )
        .unwrap();

    assert!(result.success);
    assert!(kernel
        .ledger_snapshot()
        .canon_entities
        .iter()
        .any(|entity| entity.name == "沈照"));
}

#[test]
fn promise_resolve_operation_closes_open_promise() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩",
            "Chapter-1",
            "Chapter-4",
            4,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseResolve {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-4".to_string(),
            },
            "",
            Some(&test_approval("promise_test")),
        )
        .unwrap();

    assert!(result.success);
    assert!(kernel.ledger_snapshot().open_promises.is_empty());
}

#[test]
fn story_contract_operation_updates_ledger_snapshot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StoryContractUpsert {
                contract: crate::writer_agent::operation::StoryContractOp {
                    project_id: "novel-a".to_string(),
                    title: "寒影录".to_string(),
                    genre: "玄幻".to_string(),
                    target_reader: "长篇玄幻读者".to_string(),
                    reader_promise: "刀客追查玉佩真相。".to_string(),
                    first_30_chapter_promise: "建立宗门危机与玉佩谜团。".to_string(),
                    main_conflict: "复仇与守护的冲突。".to_string(),
                    structural_boundary: "不得提前泄露玉佩来源。".to_string(),
                    tone_contract: "克制、冷峻、少解释。".to_string(),
                },
            },
            "",
            Some(&test_approval("contract_test")),
        )
        .unwrap();

    assert!(result.success);
    let ledger = kernel.ledger_snapshot();
    let contract = ledger.story_contract.unwrap();
    assert_eq!(contract.title, "寒影录");
    assert!(contract.render_for_context().contains("前30章承诺"));
}

#[test]
fn story_contract_operation_rejects_incomplete_foundation() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StoryContractUpsert {
                contract: crate::writer_agent::operation::StoryContractOp {
                    project_id: "novel-a".to_string(),
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
            Some(&test_approval("contract_test")),
        )
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.error.unwrap().code, "invalid");
    assert!(kernel.ledger_snapshot().story_contract.is_none());
}

#[test]
fn chapter_mission_operation_updates_ledger_snapshot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    kernel.active_chapter = Some("第一章".to_string());
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::ChapterMissionUpsert {
                mission: crate::writer_agent::operation::ChapterMissionOp {
                    project_id: "novel-a".to_string(),
                    chapter_title: "第一章".to_string(),
                    mission: "林墨发现玉佩线索。".to_string(),
                    must_include: "推进玉佩线索".to_string(),
                    must_not: "不要提前揭开真相".to_string(),
                    expected_ending: "以新的疑问收束。".to_string(),
                    status: "active".to_string(),
                    source_ref: "test".to_string(),
                },
            },
            "",
            Some(&test_approval("mission_test")),
        )
        .unwrap();

    assert!(result.success);
    let ledger = kernel.ledger_snapshot();
    assert_eq!(ledger.chapter_missions.len(), 1);
    assert_eq!(
        ledger.active_chapter_mission.unwrap().mission,
        "林墨发现玉佩线索。"
    );
    assert_eq!(
        kernel
            .ledger_snapshot()
            .active_chapter_mission
            .unwrap()
            .status,
        "in_progress"
    );
}

#[test]
fn chapter_mission_operation_rejects_vague_foundation() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    kernel.active_chapter = Some("第一章".to_string());
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::ChapterMissionUpsert {
                mission: crate::writer_agent::operation::ChapterMissionOp {
                    project_id: "novel-a".to_string(),
                    chapter_title: "第一章".to_string(),
                    mission: "打架".to_string(),
                    must_include: "".to_string(),
                    must_not: "剧透".to_string(),
                    expected_ending: "".to_string(),
                    status: "in_progress".to_string(),
                    source_ref: "test".to_string(),
                },
            },
            "",
            Some(&test_approval("mission_test")),
        )
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.error.unwrap().code, "invalid");
    assert!(kernel.ledger_snapshot().active_chapter_mission.is_none());
}

#[test]
fn llm_memory_candidates_parse_filter_and_dedupe() {
    let obs = observation("沈照把玉佩藏进袖中。");
    let value = serde_json::json!({
        "canon": [
            {
                "kind": "character",
                "name": "沈照",
                "aliases": ["少年"],
                "summary": "沈照把玉佩藏进袖中。",
                "attributes": { "object": "玉佩" },
                "confidence": 0.82
            },
            {
                "kind": "character",
                "name": "沈照",
                "summary": "重复条目",
                "confidence": 0.92
            },
            {
                "kind": "object",
                "name": "低",
                "summary": "置信太低",
                "confidence": 0.3
            }
        ],
        "promises": [
            {
                "kind": "object_in_motion",
                "title": "玉佩",
                "description": "玉佩的下落需要后续交代。",
                "introducedChapter": "Chapter-1",
                "expectedPayoff": "说明玉佩来源",
                "priority": 4,
                "confidence": 0.81
            }
        ]
    });

    let candidates = llm_memory_candidates_from_value(value, &obs, "test-model");

    assert_eq!(candidates.len(), 2);
    assert!(matches!(
        &candidates[0],
        MemoryCandidate::Canon(entity) if entity.name == "沈照"
    ));
    assert!(matches!(
        &candidates[1],
        MemoryCandidate::Promise(promise) if promise.title == "玉佩"
    ));
}

#[test]
fn llm_memory_proposal_replaces_local_candidate_for_same_slot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let local = kernel.observe(obs.clone()).unwrap();
    let local_canon_id = local
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .unwrap()
        .id
        .clone();
    let llm = kernel.create_llm_memory_proposals(
        obs,
        serde_json::json!({
            "canon": [{
                "kind": "character",
                "name": "沈照",
                "summary": "沈照是本章出现的少年，袖中藏着玉佩。",
                "attributes": { "object": "玉佩" },
                "confidence": 0.86
            }],
            "promises": []
        }),
        "test-model",
    );

    assert_eq!(llm.len(), 1);
    assert!(llm[0].rationale.contains("LLM增强记忆抽取"));
    assert!(kernel.superseded_proposals.contains(&local_canon_id));
    assert!(kernel
        .pending_proposals()
        .iter()
        .any(|proposal| proposal.id == llm[0].id));
}

#[test]
fn rejected_memory_candidate_suppresses_future_same_slot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let first = kernel.observe(obs.clone()).unwrap();
    let canon = first
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: canon.id.clone(),
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("not a durable canon item".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let mut next = obs;
    next.id = "obs-save-2".to_string();
    let second = kernel.observe(next).unwrap();

    assert!(!second.iter().any(|proposal| {
        matches!(
            proposal.operations.first(),
            Some(WriterOperation::CanonUpsertEntity { entity }) if entity.name == "沈照"
        )
    }));
}

#[test]
fn same_entity_memory_candidate_uses_attribute_merge_operation() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "林墨惯用寒影刀的刀客。",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨的师门是北境寒山宗，他仍握着寒影刀。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.create_llm_memory_proposals(
        obs,
        serde_json::json!({
            "canon": [{
                "kind": "character",
                "name": "林墨",
                "summary": "林墨出身北境寒山宗，惯用寒影刀。",
                "attributes": { "origin": "北境寒山宗" },
                "confidence": 0.88
            }],
            "promises": []
        }),
        "test-model",
    );

    let merge = proposals
        .iter()
        .find_map(|proposal| proposal.operations.first())
        .expect("expected canon attribute merge operation");
    assert!(matches!(
        merge,
        WriterOperation::CanonUpdateAttribute {
            entity,
            attribute,
            value,
            ..
        } if entity == "林墨" && attribute == "origin" && value == "北境寒山宗"
    ));
    assert!(!proposals.iter().any(|proposal| {
        matches!(
            proposal.operations.first(),
            Some(WriterOperation::CanonUpsertEntity { .. })
        )
    }));
}

#[test]
fn accepted_memory_candidate_records_positive_extraction_preference() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposal = kernel
        .observe(obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .unwrap();
    let proposal_id = proposal.id.clone();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal_id.clone(),
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: None,
            created_at: now_ms(),
        })
        .unwrap();

    let preferences = kernel.memory.list_style_preferences(20).unwrap();
    assert!(!preferences.iter().any(|preference| {
        preference
            .key
            .contains("memory_extract:memory|canon|character|沈照")
    }));

    let approval = test_approval_for_proposal("memory_candidate", &proposal_id);
    kernel
        .approve_editor_operation_with_approval(proposal.operations[0].clone(), "", Some(&approval))
        .unwrap();
    kernel
        .apply_feedback(proposal_feedback(
            proposal_id,
            FeedbackAction::Accepted,
            None,
        ))
        .unwrap();

    let preferences = kernel.memory.list_style_preferences(20).unwrap();
    assert!(preferences.iter().any(|preference| {
        preference
            .key
            .contains("memory_extract:memory|canon|character|沈照")
            && preference.accepted_count == 1
    }));
}

#[test]
fn ledger_snapshot_includes_memory_audit_for_candidate_feedback() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposal = kernel
        .observe(obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .unwrap();
    kernel
        .approve_editor_operation_with_approval(
            proposal.operations[0].clone(),
            "",
            Some(&test_approval_for_proposal("memory_audit", &proposal.id)),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: Some("durable character".to_string()),
            created_at: 42,
        })
        .unwrap();

    let audit = kernel.ledger_snapshot().memory_audit;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].proposal_id, proposal.id);
    assert_eq!(audit[0].action, "Accepted");
    assert!(audit[0].title.contains("沈照"));
    assert!(audit[0].evidence.contains("沈照"));
    assert_eq!(audit[0].reason.as_deref(), Some("durable character"));
}

#[test]
fn memory_audit_survives_kernel_restart() {
    let db_path = std::env::temp_dir().join(format!(
        "forge-memory-audit-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let proposal = kernel
            .observe(obs)
            .unwrap()
            .into_iter()
            .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
            .unwrap();
        kernel
            .approve_editor_operation_with_approval(
                proposal.operations[0].clone(),
                "",
                Some(&test_approval_for_proposal("memory_audit", &proposal.id)),
            )
            .unwrap();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal.id,
                action: FeedbackAction::Accepted,
                final_text: None,
                reason: Some("durable character".to_string()),
                created_at: 42,
            })
            .unwrap();
    }

    let memory = WriterMemory::open(&db_path).unwrap();
    let kernel = WriterAgentKernel::new("default", memory);
    let audit = kernel.ledger_snapshot().memory_audit;
    let _ = std::fs::remove_file(&db_path);

    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action, "Accepted");
    assert!(audit[0].title.contains("沈照"));
    assert_eq!(audit[0].reason.as_deref(), Some("durable character"));
}
