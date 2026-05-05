use super::*;

pub fn run_memory_candidate_quality_validation_eval() -> EvalResult {
    use agent_writer_lib::writer_agent::kernel::{
        validate_canon_candidate, validate_promise_candidate, validate_style_preference,
        MemoryCandidateQuality,
    };
    use agent_writer_lib::writer_agent::operation::{CanonEntityOp, PlotPromiseOp};

    let mut errors = Vec::new();

    // Vague canon: name too short
    let vague_canon = CanonEntityOp {
        kind: "character".to_string(),
        name: "A".to_string(),
        aliases: vec![],
        summary: "short".to_string(),
        attributes: serde_json::json!({}),
        confidence: 0.5,
    };
    match validate_canon_candidate(&vague_canon) {
        MemoryCandidateQuality::Vague { .. } => {}
        other => errors.push(format!("expected Vague for short canon, got {:?}", other)),
    }

    // Valid canon
    let valid_canon = CanonEntityOp {
        kind: "character".to_string(),
        name: "林墨".to_string(),
        aliases: vec!["刀客".to_string()],
        summary: "主角，追查玉佩下落的刀客".to_string(),
        attributes: serde_json::json!({}),
        confidence: 0.8,
    };
    match validate_canon_candidate(&valid_canon) {
        MemoryCandidateQuality::Acceptable => {}
        other => errors.push(format!(
            "expected Acceptable for valid canon, got {:?}",
            other
        )),
    }

    // Vague promise: description too short
    let vague_promise = PlotPromiseOp {
        kind: "mystery_clue".to_string(),
        title: "谜".to_string(),
        description: "短".to_string(),
        introduced_chapter: "Ch1".to_string(),
        expected_payoff: "Ch5".to_string(),
        priority: 3,
        related_entities: vec![],
    };
    match validate_promise_candidate(&vague_promise) {
        MemoryCandidateQuality::Vague { .. } => {}
        other => errors.push(format!("expected Vague for short promise, got {:?}", other)),
    }

    // Valid promise
    let valid_promise = PlotPromiseOp {
        kind: "mystery_clue".to_string(),
        title: "玉佩下落".to_string(),
        description: "张三带走了刻有龙纹的玉佩，下落不明。".to_string(),
        introduced_chapter: "Chapter-1".to_string(),
        expected_payoff: "Chapter-5".to_string(),
        priority: 5,
        related_entities: vec!["张三".to_string(), "玉佩".to_string()],
    };
    match validate_promise_candidate(&valid_promise) {
        MemoryCandidateQuality::Acceptable => {}
        other => errors.push(format!(
            "expected Acceptable for valid promise, got {:?}",
            other
        )),
    }

    match validate_style_preference("tone", "好") {
        MemoryCandidateQuality::Vague { .. } => {}
        other => errors.push(format!("expected Vague for vague style, got {:?}", other)),
    }

    match validate_style_preference("dialogue_subtext", "对话偏短句留白，避免直接解释情绪")
    {
        MemoryCandidateQuality::Acceptable => {}
        other => errors.push(format!(
            "expected Acceptable for valid style, got {:?}",
            other
        )),
    }

    eval_result(
        "writer_agent:memory_candidate_quality_validation",
        "6 candidates validated".to_string(),
        errors,
    )
}

pub fn run_style_memory_validation_eval() -> EvalResult {
    use agent_writer_lib::writer_agent::kernel::{
        style_preference_memory_key, style_preference_taxonomy_slot,
        validate_style_preference_with_memory, MemoryCandidateQuality,
    };

    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_style_preference("dialogue_subtext", "对话偏短句留白，避免直接解释情绪", true)
        .unwrap();

    let mut errors = Vec::new();
    match validate_style_preference_with_memory("tone", "好", &memory) {
        MemoryCandidateQuality::Vague { .. } => {}
        other => errors.push(format!("expected Vague for vague style, got {:?}", other)),
    }
    match validate_style_preference_with_memory(
        "dialogue_subtext",
        "对话偏短句留白，避免直接解释情绪",
        &memory,
    ) {
        MemoryCandidateQuality::Duplicate { .. } => {}
        other => errors.push(format!(
            "expected Duplicate for repeated style, got {:?}",
            other
        )),
    }
    match validate_style_preference_with_memory(
        "dialogue_subtext",
        "对话要完整解释每个角色的真实情绪",
        &memory,
    ) {
        MemoryCandidateQuality::Conflict { .. } => {}
        other => errors.push(format!(
            "expected Conflict for same-key style change, got {:?}",
            other
        )),
    }
    match style_preference_taxonomy_slot("dialogue_subtext", "对话偏短句留白，避免直接解释情绪")
        .as_deref()
    {
        Some("dialogue.subtext") => {}
        other => errors.push(format!(
            "expected dialogue.subtext taxonomy slot, got {:?}",
            other
        )),
    }
    match validate_style_preference_with_memory(
        "dialogue_emotion_explanation",
        "对话要完整解释每个角色的真实情绪",
        &memory,
    ) {
        MemoryCandidateQuality::Conflict { reason, .. } if reason.contains("dialogue.subtext") => {}
        other => errors.push(format!(
            "expected Conflict for same taxonomy slot, got {:?}",
            other
        )),
    }
    match validate_style_preference_with_memory(
        "dialogue_subtext_followup",
        "对话继续偏潜台词和短句留白，不要把情绪说满",
        &memory,
    ) {
        MemoryCandidateQuality::Acceptable => {}
        other => errors.push(format!(
            "expected Acceptable for same-polarity style merge, got {:?}",
            other
        )),
    }
    match style_preference_memory_key(
        "dialogue_subtext_followup",
        "对话继续偏潜台词和短句留白，不要把情绪说满",
    )
    .as_str()
    {
        "style:dialogue.subtext" => {}
        other => errors.push(format!(
            "expected normalized style memory key, got {}",
            other
        )),
    }
    match validate_style_preference_with_memory(
        "description_sensory_detail",
        "描写优先保留气味、触感和画面细节",
        &memory,
    ) {
        MemoryCandidateQuality::Acceptable => {}
        other => errors.push(format!(
            "expected Acceptable for different taxonomy slot, got {:?}",
            other
        )),
    }

    let mut kernel = WriterAgentKernel::new("eval", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_subtext".to_string(),
                value: "对话要完整解释每个角色的真实情绪".to_string(),
            },
            "",
            Some(&eval_approval("style_memory_validation")),
        )
        .unwrap();
    if result.success {
        errors.push("conflicting style operation should be rejected".to_string());
    }
    let merge_result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StyleUpdatePreference {
                key: "dialogue_subtext_followup".to_string(),
                value: "对话继续偏潜台词和短句留白，不要把情绪说满".to_string(),
            },
            "",
            Some(&eval_approval("style_memory_validation")),
        )
        .unwrap();
    if !merge_result.success {
        errors.push(format!(
            "same-polarity style operation should merge: {:?}",
            merge_result.error
        ));
    }
    let style_count = kernel
        .memory
        .list_style_preferences(20)
        .unwrap()
        .into_iter()
        .filter(|preference| {
            preference.key == "dialogue_subtext" || preference.key == "style:dialogue.subtext"
        })
        .count();
    if style_count != 2 {
        errors.push(format!(
            "style ledger should keep base row plus merged taxonomy row, got {}",
            style_count
        ));
    }

    eval_result(
        "writer_agent:style_memory_validation",
        format!(
            "operationRejected={} mergeAccepted={} styleRows={} taxonomy=dialogue.subtext",
            !result.success, merge_result.success, style_count
        ),
        errors,
    )
}

pub fn run_vague_memory_candidate_rejected_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation_in_chapter("林墨收起玉佩，没有解释它的来历。", "Chapter-2");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.create_llm_memory_proposals(
        obs,
        serde_json::json!({
            "canon": [{
                "kind": "character",
                "name": "林墨",
                "summary": "刀客追查玉佩",
                "attributes": {},
                "confidence": 0.82
            }],
            "promises": [{
                "kind": "mystery_clue",
                "title": "玉佩",
                "description": "下落仍未说明",
                "introducedChapter": "Chapter-2",
                "expectedPayoff": "Chapter-5",
                "priority": 4,
                "confidence": 0.8
            }]
        }),
        "eval-model",
    );

    let mut errors = Vec::new();
    if !proposals.is_empty() {
        errors.push(format!(
            "vague LLM memory candidates should not create proposals, got {}",
            proposals.len()
        ));
    }

    eval_result(
        "writer_agent:vague_memory_candidate_rejected",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_duplicate_memory_candidate_deduped_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "林墨惯用寒影刀的刀客，正在追查玉佩。",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.9,
        )
        .unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "玉佩下落",
            "张三带走了玉佩，去向不明。",
            "Chapter-1",
            "Chapter-5",
            4,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation_in_chapter("林墨按住寒影刀，想起张三带走的玉佩。", "Chapter-2");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.create_llm_memory_proposals(
        obs,
        serde_json::json!({
            "canon": [{
                "kind": "character",
                "name": "林墨",
                "summary": "林墨惯用寒影刀的刀客，正在追查玉佩。",
                "attributes": { "weapon": "寒影刀" },
                "confidence": 0.88
            }],
            "promises": [{
                "kind": "mystery_clue",
                "title": "玉佩下落",
                "description": "张三带走了玉佩，去向不明。",
                "introducedChapter": "Chapter-1",
                "expectedPayoff": "Chapter-5",
                "priority": 4,
                "confidence": 0.86
            }]
        }),
        "eval-model",
    );

    let memory_write_count = proposals
        .iter()
        .flat_map(|proposal| proposal.operations.iter())
        .filter(|operation| {
            matches!(
                operation,
                WriterOperation::CanonUpsertEntity { .. } | WriterOperation::PromiseAdd { .. }
            )
        })
        .count();

    let mut errors = Vec::new();
    if memory_write_count > 0 {
        errors.push(format!(
            "duplicate candidates should not create memory writes, got {}",
            memory_write_count
        ));
    }

    eval_result(
        "writer_agent:duplicate_memory_candidate_deduped",
        format!(
            "proposals={} memoryWrites={}",
            proposals.len(),
            memory_write_count
        ),
        errors,
    )
}

pub fn run_conflicting_memory_candidate_requires_review_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "林墨惯用寒影刀，不轻易改用其他兵器。",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.92,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation_in_chapter("林墨拔出长剑，剑锋贴着雨水发亮。", "Chapter-3");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.create_llm_memory_proposals(
        obs,
        serde_json::json!({
            "canon": [{
                "kind": "character",
                "name": "林墨",
                "summary": "林墨在雨夜使用长剑压制敌人。",
                "attributes": { "weapon": "长剑" },
                "confidence": 0.91
            }],
            "promises": []
        }),
        "eval-model",
    );

    let canon_writes = proposals
        .iter()
        .flat_map(|proposal| proposal.operations.iter())
        .filter(|operation| matches!(operation, WriterOperation::CanonUpsertEntity { .. }))
        .count();
    let conflict_reviews = proposals
        .iter()
        .filter(|proposal| {
            proposal.kind == ProposalKind::ContinuityWarning
                && proposal.operations.is_empty()
                && proposal.preview.contains("设定冲突")
                && proposal.rationale.contains("明确确认")
        })
        .count();

    let mut errors = Vec::new();
    if canon_writes > 0 {
        errors.push(format!(
            "conflicting canon candidate should not create direct canon write, got {}",
            canon_writes
        ));
    }
    if conflict_reviews != 1 {
        errors.push(format!(
            "expected one explicit conflict review proposal, got {}",
            conflict_reviews
        ));
    }

    eval_result(
        "writer_agent:conflicting_memory_candidate_requires_review",
        format!(
            "proposals={} canonWrites={} conflictReviews={}",
            proposals.len(),
            canon_writes,
            conflict_reviews
        ),
        errors,
    )
}
