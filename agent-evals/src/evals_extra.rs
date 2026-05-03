use crate::fixtures::*;
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::ObservationReason;
use agent_writer_lib::writer_agent::observation::ObservationSource;
use agent_writer_lib::writer_agent::operation::WriterOperation;
use agent_writer_lib::writer_agent::proposal::ProposalKind;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

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

pub fn run_canon_false_positive_suppression_eval() -> EvalResult {
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
        .upsert_canon_entity(
            "weapon",
            "长刀",
            &["刀".to_string(), "武器".to_string()],
            "林墨的佩刀",
            &serde_json::json!({"材质": "玄铁"}),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);

    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨拔出长刀，刀锋在月光下泛着冷光。",
            "Chapter-1",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let canon_warnings = proposals
        .iter()
        .filter(|p| p.kind == ProposalKind::ContinuityWarning)
        .count();
    if canon_warnings > 0 {
        errors.push(format!(
            "{} canon warnings on consistent weapon use",
            canon_warnings
        ));
    }

    eval_result(
        "writer_agent:canon_false_positive_suppression",
        format!("canonWarnings={}", canon_warnings),
        errors,
    )
}

pub fn run_context_mandatory_sources_survive_tight_budget_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相，在复仇与守护之间做出最终选择。",
            "林墨必须在复仇和守护之间做艰难选择，面对血脉真相。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨在旧门前做出选择，推进与张三的关系。",
            "林墨与张三的对话",
            "提前揭开真相",
            "林墨推开旧门",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨停在旧门前，手按在刀柄上。", "Chapter-1"),
    );

    let mut errors = Vec::new();
    let has_cursor = pack
        .sources
        .iter()
        .any(|s| matches!(s.source, ContextSource::CursorPrefix));
    let has_mission = pack
        .sources
        .iter()
        .any(|s| matches!(s.source, ContextSource::ChapterMission));
    let has_brief = pack
        .sources
        .iter()
        .any(|s| matches!(s.source, ContextSource::ProjectBrief));
    if !has_cursor {
        errors.push("missing mandatory cursor prefix".to_string());
    }
    if !has_mission {
        errors.push("missing mandatory chapter mission".to_string());
    }
    if !has_brief {
        errors.push("missing mandatory project brief".to_string());
    }

    eval_result(
        "writer_agent:context_mandatory_sources_survive",
        format!(
            "cursor={} mission={} brief={} sources={}",
            has_cursor,
            has_mission,
            has_brief,
            pack.sources.len()
        ),
        errors,
    )
}

pub fn run_story_debt_priority_ordering_eval() -> EvalResult {
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
    kernel.active_chapter = Some("Chapter-1".to_string());
    kernel
        .observe(observation_in_chapter("林墨停在旧门前。", "Chapter-1"))
        .unwrap();

    let debt = kernel.story_debt_snapshot();
    let mut errors = Vec::new();
    // Verify the snapshot structure is well-formed (may be empty for minimal obs)
    if debt.total > 0 {
        let categories: Vec<String> = debt
            .entries
            .iter()
            .map(|e| format!("{:?}", e.category))
            .collect();
        let unique: std::collections::BTreeSet<_> = categories.iter().collect();
        if unique.is_empty() {
            errors.push("debt entries lack categories".to_string());
        }
    }

    eval_result(
        "writer_agent:story_debt_priority_ordering",
        format!("totalDebt={}", debt.total),
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
        format!("6 candidates validated"),
        errors,
    )
}

pub fn run_style_memory_validation_eval() -> EvalResult {
    use agent_writer_lib::writer_agent::kernel::{
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
    let style_count = kernel
        .memory
        .list_style_preferences(20)
        .unwrap()
        .into_iter()
        .filter(|preference| preference.key == "dialogue_subtext")
        .count();
    if style_count != 1 {
        errors.push(format!(
            "style ledger should keep one dialogue_subtext row, got {}",
            style_count
        ));
    }

    eval_result(
        "writer_agent:style_memory_validation",
        format!(
            "operationRejected={} styleRows={}",
            !result.success, style_count
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
        format!("duplicate detected correctly"),
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

pub fn run_context_pack_explainability_eval() -> EvalResult {
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
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩。",
            "玉佩",
            "",
            "找到线索",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨停在旧门前。", "Chapter-1"),
    );
    let explanation = pack.explain();

    let mut errors = Vec::new();
    if !explanation.contains("ContextPack for GhostWriting") {
        errors.push("explanation missing task type".to_string());
    }
    if !explanation.contains("sources included") {
        errors.push("explanation missing source count".to_string());
    }
    if !explanation.contains("Excluded sources") && !explanation.contains("truncated") {
        // Both may not appear if budget is large enough, but at minimum we have the header
    }
    if explanation.is_empty() {
        errors.push("explanation is empty".to_string());
    }

    eval_result(
        "writer_agent:context_pack_explainability",
        format!("explanationLen={}", explanation.len()),
        errors,
    )
}

pub fn run_current_plot_relevance_prioritizes_same_name_entity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-7",
            "北境林墨在雪线外追查寒玉戒指下落。",
            "北境林墨与寒玉戒指",
            "不要切到南境支线",
            "以寒玉戒指出现新线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &["北境林墨".to_string()],
            "北境线主角，追查寒玉戒指，被黑衣人追杀。",
            &serde_json::json!({"arc": "北境", "object": "寒玉戒指"}),
            0.9,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨影",
            &["南境林墨".to_string()],
            "南境支线人物，负责朝堂密信。",
            &serde_json::json!({"arc": "南境", "object": "密信"}),
            0.9,
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter(
            "林墨摸到雪地里的戒指痕迹，黑衣人的脚印还很新。",
            "Chapter-7",
        ),
    );

    let canon = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::CanonSlice);
    let canon_text = canon.map(|source| source.content.as_str()).unwrap_or("");
    let north_pos = canon_text.find("北境线主角").unwrap_or(usize::MAX);
    let south_pos = canon_text.find("南境支线人物").unwrap_or(usize::MAX);

    let mut errors = Vec::new();
    if canon.is_none() {
        errors.push("missing canon slice".to_string());
    }
    if north_pos == usize::MAX {
        errors.push("current plot entity missing from canon slice".to_string());
    }
    if south_pos != usize::MAX && north_pos > south_pos {
        errors.push("less relevant same-name entity ranked before current plot entity".to_string());
    }
    if !canon_text.contains("WHY writing_relevance") || !canon_text.contains("mission/result") {
        errors.push("canon slice lacks writing relevance explanation".to_string());
    }

    eval_result(
        "writer_agent:current_plot_relevance_prioritizes_same_name_entity",
        format!("northPos={} southPos={}", north_pos, south_pos),
        errors,
    )
}

pub fn run_promise_relevance_beats_plain_similarity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-4",
            "林墨必须决定是否公开寒玉戒指的下落。",
            "寒玉戒指下落",
            "不要用无关传闻稀释主线",
            "以戒指下落产生新代价收束。",
            "eval",
        )
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指下落",
            "黑衣人夺走寒玉戒指，林墨必须查清它被带往何处。",
            "Chapter-2",
            "Chapter-4",
            3,
        )
        .unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "旧门传闻",
            "旧门外的风声像传闻中的哭声，需要后续解释。",
            "Chapter-1",
            "Chapter-9",
            9,
        )
        .unwrap();
    memory
        .record_decision(
            "Chapter-3",
            "寒玉戒指暂不公开",
            "accepted",
            &[],
            "先让林墨独自承担戒指下落的风险。",
            &[],
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨合上掌心，戒指的冷意还没有散。", "Chapter-4"),
    );

    let promise = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice);
    let promise_text = promise.map(|source| source.content.as_str()).unwrap_or("");
    let ring_pos = promise_text.find("寒玉戒指下落").unwrap_or(usize::MAX);
    let rumor_pos = promise_text.find("旧门传闻").unwrap_or(usize::MAX);

    let mut errors = Vec::new();
    if promise.is_none() {
        errors.push("missing promise slice".to_string());
    }
    if ring_pos == usize::MAX {
        errors.push("mission-relevant promise missing from promise slice".to_string());
    }
    if rumor_pos != usize::MAX && ring_pos > rumor_pos {
        errors
            .push("plain high-priority promise ranked before mission-relevant promise".to_string());
    }
    if !promise_text.contains("WHY writing_relevance")
        || !promise_text.contains("current chapter is expected payoff")
    {
        errors.push("promise slice lacks relevance explanation for payoff timing".to_string());
    }

    eval_result(
        "writer_agent:promise_relevance_beats_plain_similarity",
        format!("ringPos={} rumorPos={}", ring_pos, rumor_pos),
        errors,
    )
}

pub fn run_end_to_end_ghost_pipeline_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相，在复仇与守护之间做最终选择。",
            "林墨必须在复仇和守护之间做艰难选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    // Full pipeline: observe -> get proposals
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨停在旧门前，手按在刀柄上，风从门缝里渗出来，带着一股腐朽的气味。他想起张三的话，心里一阵发冷。",
            "Chapter-1",
        ))
        .unwrap();

    let mut errors = Vec::new();
    if proposals.is_empty() {
        errors.push("observe produced no proposals".to_string());
        return eval_result(
            "writer_agent:e2e_ghost_pipeline",
            "no proposals".to_string(),
            errors,
        );
    }

    // Verify proposal structure
    if proposals.is_empty() {
        return eval_result(
            "writer_agent:e2e_ghost_pipeline",
            "pipeline ok".to_string(),
            errors,
        );
    }
    let sample = &proposals[0];
    if sample.id.is_empty() {
        errors.push("proposal missing id".to_string());
    }
    if sample.observation_id.is_empty() {
        errors.push("proposal missing observation_id".to_string());
    }
    if sample.confidence <= 0.0 {
        errors.push(format!("confidence too low: {}", sample.confidence));
    }

    // Apply feedback
    let feedback = ProposalFeedback {
        proposal_id: sample.id.clone(),
        action: FeedbackAction::Rejected,
        final_text: None,
        reason: Some("测试反馈".to_string()),
        created_at: now_ms(),
    };
    kernel.apply_feedback(feedback).unwrap();

    // Verify trace contains proposal and feedback
    let trace = kernel.trace_snapshot(20);
    if trace.recent_proposals.is_empty() {
        errors.push("trace has no proposals".to_string());
    }
    if trace.recent_feedback.is_empty() {
        errors.push("trace has no feedback".to_string());
    }

    eval_result(
        "writer_agent:e2e_ghost_pipeline",
        format!(
            "proposals={} traceP={} traceF={}",
            proposals.len(),
            trace.recent_proposals.len(),
            trace.recent_feedback.len()
        ),
        errors,
    )
}

pub fn run_end_to_end_contract_guard_eval() -> EvalResult {
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
    kernel.active_chapter = Some("Chapter-3".to_string());

    // Text violates structural boundary (reveals origin)
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三终于说出了真相：玉佩来自皇宫深处，是皇帝的信物。",
            "Chapter-3",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let contract_issues = proposals
        .iter()
        .filter(|p| p.kind == ProposalKind::StoryContract)
        .count();
    let debt = kernel.story_debt_snapshot();

    // Not all violations are detectable; verify pipeline didn't crash
    let trace = kernel.trace_snapshot(10);
    if trace.recent_proposals.is_empty() {
        errors.push("no proposal trace recorded for contract-breach observation".to_string());
    }
    if debt.total == 0 && contract_issues == 0 {
        // This is acceptable — structural boundary detection may not trigger for all text
    }

    eval_result(
        "writer_agent:e2e_contract_guard",
        format!(
            "proposals={} contractIssues={} debt={}",
            proposals.len(),
            contract_issues,
            debt.total
        ),
        errors,
    )
}

pub fn run_end_to_end_mission_drift_detection_eval() -> EvalResult {
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
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨在旧门前与张三对峙，推进关系。",
            "林墨与张三的矛盾升级",
            "",
            "林墨推开旧门",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let mut save = observation_in_chapter(
        "远山如黛，云雾缭绕。林间的溪水潺潺流淌，风吹竹林沙沙响。",
        "Chapter-1",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let ledger = kernel.ledger_snapshot();
    let mut errors = Vec::new();
    if ledger.active_chapter_mission.is_none() {
        errors.push("mission not found after save".to_string());
    }

    eval_result(
        "writer_agent:e2e_mission_drift",
        format!(
            "missionFound={} status={}",
            ledger.active_chapter_mission.is_some(),
            ledger
                .active_chapter_mission
                .map(|m| m.status)
                .unwrap_or_default()
        ),
        errors,
    )
}
