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
    if promise.is_none_or(|promise| promise.last_seen_chapter != "Chapter-2") {
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

pub fn run_promise_object_cross_chapter_tracking_eval() -> EvalResult {
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
    memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指",
            "林墨母亲的遗物被黑衣人夺走",
            "Chapter-2",
            "Chapter-5",
            4,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-3".to_string());

    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨摸了摸空荡荡的手指。", "Chapter-3"),
    );
    let promise_in_ctx = pack.sources.iter().any(|s| s.content.contains("寒玉戒指"));

    let mut errors = Vec::new();
    let promises = kernel.memory.get_open_promise_summaries().unwrap();
    let ring = promises.iter().find(|p| p.title.contains("寒玉"));
    if ring.is_none() {
        errors.push("object promise not found in ledger".to_string());
    }
    if !promise_in_ctx && ring.is_some() {
        errors.push("open object promise missing from context pack".to_string());
    }

    eval_result(
        "writer_agent:promise_object_cross_chapter_tracking",
        format!(
            "promiseInContext={} lastSeen={}",
            promise_in_ctx,
            ring.map(|p| p.last_seen_chapter.as_str()).unwrap_or("none")
        ),
        errors,
    )
}

pub fn run_promise_kind_extraction_from_text_eval() -> EvalResult {
    let mut errors = Vec::new();

    // Object whereabouts
    let mut obs = observation("张三带走了那枚玉佩，从此下落不明。");
    obs.chapter_title = Some("Chapter-3".to_string());
    let promises = agent_writer_lib::writer_agent::kernel::extract_plot_promises(
        "张三带走了那枚玉佩，从此下落不明。",
        &obs,
    );
    let has_object = promises
        .iter()
        .any(|p| p.kind.contains("object_whereabouts"));
    if !has_object {
        errors.push(format!(
            "object whereabouts not detected, got kinds: {:?}",
            promises.iter().map(|p| p.kind.as_str()).collect::<Vec<_>>()
        ));
    }

    // Mystery clue
    let mut obs2 = observation("这个秘密已经埋藏了二十年。");
    obs2.chapter_title = Some("Chapter-1".to_string());
    let promises2 = agent_writer_lib::writer_agent::kernel::extract_plot_promises(
        "这个秘密已经埋藏了二十年。",
        &obs2,
    );
    let has_mystery = promises2.iter().any(|p| p.kind.contains("mystery_clue"));
    if !has_mystery {
        errors.push("mystery clue not detected for secret".to_string());
    }

    // Priority: object/mystery should get higher priority than generic
    if !promises.is_empty() && promises[0].priority < 4 {
        errors.push(format!(
            "object promise should have priority >= 4, got {}",
            promises[0].priority
        ));
    }

    eval_result(
        "writer_agent:promise_kind_extraction_from_text",
        format!(
            "objectPromises={} mysteryPromises={} objPriority={}",
            promises.len(),
            promises2.len(),
            promises.first().map(|p| p.priority).unwrap_or(0)
        ),
        errors,
    )
}

pub fn run_promise_related_entities_extraction_eval() -> EvalResult {
    let mut obs = observation("张三带走了林墨的玉佩，从此下落不明。");
    obs.chapter_title = Some("Chapter-3".to_string());
    let promises = agent_writer_lib::writer_agent::kernel::extract_plot_promises(
        "张三带走了林墨的玉佩，从此下落不明。",
        &obs,
    );

    let mut errors = Vec::new();
    if promises.is_empty() {
        errors.push("no promises extracted".to_string());
    } else {
        let p = &promises[0];
        if p.related_entities.is_empty() || p.related_entities[0] == "unknown" {
            errors.push(format!(
                "related_entities should contain named entities, got: {:?}",
                p.related_entities
            ));
        }
    }

    eval_result(
        "writer_agent:promise_related_entities_extraction",
        format!(
            "promises={} entities={:?}",
            promises.len(),
            promises.first().map(|p| &p.related_entities)
        ),
        errors,
    )
}

pub fn run_promise_dedup_against_existing_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "玉佩下落",
            "张三带走了玉佩",
            "Ch1",
            "Ch5",
            4,
        )
        .unwrap();

    use agent_writer_lib::writer_agent::kernel::{
        validate_promise_candidate_with_dedup, MemoryCandidateQuality,
    };
    use agent_writer_lib::writer_agent::operation::PlotPromiseOp;

    let duplicate = PlotPromiseOp {
        kind: "mystery_clue".to_string(),
        title: "玉佩下落".to_string(),
        description: "张三带走了玉佩，去向不明。".to_string(),
        introduced_chapter: "Ch2".to_string(),
        expected_payoff: "Ch5".to_string(),
        priority: 4,
        related_entities: vec!["张三".to_string(), "玉佩".to_string()],
    };
    let new_promise = PlotPromiseOp {
        kind: "object_whereabouts".to_string(),
        title: "寒玉戒指".to_string(),
        description: "林墨母亲的遗物在旧门后被发现。".to_string(),
        introduced_chapter: "Ch3".to_string(),
        expected_payoff: "Ch6".to_string(),
        priority: 4,
        related_entities: vec!["林墨".to_string(), "戒指".to_string()],
    };

    let mut errors = Vec::new();
    match validate_promise_candidate_with_dedup(&duplicate, &memory) {
        MemoryCandidateQuality::Duplicate { .. } => {}
        other => errors.push(format!(
            "expected Duplicate for same-title promise, got {:?}",
            other
        )),
    }
    match validate_promise_candidate_with_dedup(&new_promise, &memory) {
        MemoryCandidateQuality::Acceptable => {}
        other => errors.push(format!(
            "expected Acceptable for unique promise, got {:?}",
            other
        )),
    }

    eval_result(
        "writer_agent:promise_dedup_against_existing",
        "duplicate detected correctly".to_string(),
        errors,
    )
}
