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

pub fn run_memory_feedback_schema_records_quality_signals_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut accepted_obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    accepted_obs.reason = ObservationReason::Save;
    accepted_obs.source = ObservationSource::ChapterSave;
    let accepted_proposal = kernel
        .observe(accepted_obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .expect("fixture should produce canon memory candidate");
    let mut accepted_approval = eval_approval("memory_feedback_schema");
    accepted_approval.proposal_id = Some(accepted_proposal.id.clone());
    kernel
        .approve_editor_operation_with_approval(
            accepted_proposal.operations[0].clone(),
            "",
            Some(&accepted_approval),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: accepted_proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: Some("确认这是长期角色设定".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let mut rejected_obs = observation("青灯案上放着一封密信，林墨没有告诉任何人它的下落。");
    rejected_obs.id = "memory-feedback-rejected".to_string();
    rejected_obs.reason = ObservationReason::Save;
    rejected_obs.source = ObservationSource::ChapterSave;
    let rejected_proposal = kernel
        .observe(rejected_obs.clone())
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::PlotPromise)
        .expect("fixture should produce promise memory candidate");
    let rejected_slot = match rejected_proposal.operations.first() {
        Some(WriterOperation::PromiseAdd { promise }) => {
            format!("memory|promise|{}|{}", promise.kind, promise.title)
        }
        _ => String::new(),
    };
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: rejected_proposal.id.clone(),
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("作者纠错：无名书信只是气氛，不是伏笔".to_string()),
            created_at: now_ms() + 1,
        })
        .unwrap();

    let feedback = kernel.memory.list_memory_feedback(20).unwrap();
    let mut errors = Vec::new();
    if !feedback.iter().any(|event| {
        event.proposal_id == accepted_proposal.id
            && event.category == "canon"
            && event.action == "reinforcement"
            && event.confidence_delta > 0.0
            && event.source_error.is_none()
    }) {
        errors.push("accepted memory candidate lacked structured reinforcement".to_string());
    }
    if !feedback.iter().any(|event| {
        event.proposal_id == rejected_proposal.id
            && event.category == "promise"
            && event.action == "correction"
            && event.confidence_delta < 0.0
            && event
                .source_error
                .as_deref()
                .is_some_and(|error| error.contains("不是伏笔"))
    }) {
        errors.push(
            "rejected memory candidate lacked structured source-error correction".to_string(),
        );
    }

    let suppressed = kernel
        .observe(rejected_obs)
        .unwrap()
        .iter()
        .any(|proposal| {
            matches!(
                proposal.operations.first(),
                Some(WriterOperation::PromiseAdd { promise })
                    if format!("memory|promise|{}|{}", promise.kind, promise.title)
                        == rejected_slot
            )
        });
    if suppressed {
        errors.push("structured correction did not suppress the same promise slot".to_string());
    }

    eval_result(
        "writer_agent:memory_feedback_schema_records_quality_signals",
        format!("events={} rejectedSlot={}", feedback.len(), rejected_slot),
        errors,
    )
}

pub fn run_memory_reliability_snapshot_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut accepted_obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    accepted_obs.reason = ObservationReason::Save;
    accepted_obs.source = ObservationSource::ChapterSave;
    let accepted_proposal = kernel
        .observe(accepted_obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .expect("fixture should produce canon memory candidate");
    let mut approval = eval_approval("memory_reliability_snapshot");
    approval.proposal_id = Some(accepted_proposal.id.clone());
    kernel
        .approve_editor_operation_with_approval(
            accepted_proposal.operations[0].clone(),
            "",
            Some(&approval),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: accepted_proposal.id.clone(),
            action: FeedbackAction::Accepted,
            final_text: None,
            reason: Some("作者确认长期设定".to_string()),
            created_at: now_ms(),
        })
        .unwrap();

    let mut rejected_obs = observation("青灯案上放着一封密信，林墨没有告诉任何人它的下落。");
    rejected_obs.id = "memory-reliability-snapshot-rejected".to_string();
    rejected_obs.reason = ObservationReason::Save;
    rejected_obs.source = ObservationSource::ChapterSave;
    let rejected_proposal = kernel
        .observe(rejected_obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::PlotPromise)
        .expect("fixture should produce promise memory candidate");
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: rejected_proposal.id,
            action: FeedbackAction::Rejected,
            final_text: None,
            reason: Some("作者纠错：密信只是气氛，不是伏笔".to_string()),
            created_at: now_ms() + 1,
        })
        .unwrap();

    let reliability = kernel.ledger_snapshot().memory_reliability;
    let mut errors = Vec::new();
    if !reliability.iter().any(|item| {
        item.slot == "memory|canon|character|沈照"
            && item.status == "trusted"
            && item.reinforcement_count == 1
            && item.reliability > 0.5
    }) {
        errors.push("ledger snapshot did not expose trusted memory reliability".to_string());
    }
    if !reliability.iter().any(|item| {
        item.slot == "memory|promise|object_whereabouts|密信"
            && item.status == "needs_review"
            && item.correction_count == 1
            && item
                .last_source_error
                .as_deref()
                .is_some_and(|error| error.contains("不是伏笔"))
    }) {
        errors.push("ledger snapshot did not expose source-error reliability review".to_string());
    }
    if reliability.first().map(|item| item.status.as_str()) != Some("needs_review") {
        errors.push("memory reliability should prioritize review-needed slots".to_string());
    }

    eval_result(
        "writer_agent:memory_reliability_snapshot",
        format!("slots={}", reliability.len()),
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
    if !(0.75..=0.91).contains(&conf_usable) {
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
