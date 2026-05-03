use super::*;

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
