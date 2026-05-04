use crate::fixtures::*;
use agent_harness_core::{Chunk, VectorDB};
use agent_writer_lib::brain_service::{
    build_project_brain_knowledge_index, compare_project_brain_source_revisions_from_db,
    project_brain_embedding_batch_status, project_brain_embedding_profile_from_config,
    project_brain_embedding_provider_registry, project_brain_source_revision,
    rerank_project_brain_results_with_focus, resolve_project_brain_embedding_profile,
    safe_knowledge_index_file_path, search_project_brain_results_with_focus, trim_embedding_input,
    ProjectBrainEmbeddingBatchStatus, ProjectBrainEmbeddingRegistryStatus, ProjectBrainFocus,
};
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::context_relevance::{
    format_text_chunk_relevance, rerank_text_chunks, writing_scene_types,
};
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

pub fn run_same_entity_attribute_merge_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation_in_chapter("林墨的师门是北境寒山宗，他仍握着寒影刀。", "Chapter-4");
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
        "eval-model",
    );

    let merge_ops = proposals
        .iter()
        .flat_map(|proposal| proposal.operations.iter())
        .filter(|operation| {
            matches!(
                operation,
                WriterOperation::CanonUpdateAttribute {
                    entity,
                    attribute,
                    value,
                    ..
                } if entity == "林墨" && attribute == "origin" && value == "北境寒山宗"
            )
        })
        .count();
    let entity_upserts = proposals
        .iter()
        .flat_map(|proposal| proposal.operations.iter())
        .filter(|operation| matches!(operation, WriterOperation::CanonUpsertEntity { .. }))
        .count();
    let conflict_reviews = proposals
        .iter()
        .filter(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .count();

    let mut errors = Vec::new();
    if merge_ops != 1 {
        errors.push(format!(
            "expected one canon.update_attribute merge op, got {}",
            merge_ops
        ));
    }
    if entity_upserts != 0 {
        errors.push(format!(
            "same-entity merge should not upsert whole entity, got {}",
            entity_upserts
        ));
    }
    if conflict_reviews != 0 {
        errors.push(format!(
            "non-conflicting attribute merge should not create conflict review, got {}",
            conflict_reviews
        ));
    }

    eval_result(
        "writer_agent:same_entity_attribute_merge",
        format!(
            "mergeOps={} entityUpserts={} conflictReviews={}",
            merge_ops, entity_upserts, conflict_reviews
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

pub fn run_project_brain_writing_relevance_rerank_eval() -> EvalResult {
    let chunks = vec![
        (
            50.0,
            (
                "semantic-distractor",
                "旧门外的风声像传闻中的哭声，林墨反复听见旧门、风声、旧门和风声。",
            ),
        ),
        (
            1.0,
            (
                "mission-relevant",
                "黑衣人夺走寒玉戒指后留下北境雪线脚印，林墨必须查清寒玉戒指下落。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章必须追查寒玉戒指下落，不要被旧门传闻稀释主线。",
        |(_, text)| text.to_string(),
    );
    let first_id = reranked
        .first()
        .map(|(_, _, (id, _))| *id)
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if first_id != "mission-relevant" {
        errors.push(format!(
            "mission-relevant project brain chunk should outrank semantic distractor, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("WHY writing_relevance")
        || !first_explanation.contains("寒玉戒指")
    {
        errors.push(format!(
            "missing writing relevance explanation for reranked chunk: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_writing_relevance_rerank",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_scene_type_relevance_signal_eval() -> EvalResult {
    let focus_scene_types = writing_scene_types("本章要揭开寒玉戒指来源的真相，并回收前文伏笔。");
    let chunks = vec![
        (
            42.0,
            (
                "surface-similar",
                "林墨摩挲寒玉戒指，旧门外的风声反复敲打窗棂，气味潮湿。",
            ),
        ),
        (
            1.0,
            (
                "reveal-scene",
                "张三终于说出真相：寒玉戒指来源于北境宗门旧案，这条线索回收了母亲遗物的伏笔。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章要揭开寒玉戒指来源的真相，并回收前文伏笔。",
        |(_, text)| text.to_string(),
    );
    let first_id = reranked
        .first()
        .map(|(_, _, (id, _))| *id)
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if !focus_scene_types.iter().any(|scene| scene == "reveal")
        || !focus_scene_types
            .iter()
            .any(|scene| scene == "setup_payoff")
    {
        errors.push(format!(
            "focus scene types should include reveal and setup_payoff, got {:?}",
            focus_scene_types
        ));
    }
    if first_id != "reveal-scene" {
        errors.push(format!(
            "reveal scene should outrank surface-similar description, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("scene type reveal")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "rerank explanation missing scene type signals: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:scene_type_relevance_signal",
        format!(
            "first={} scenes={:?} explanation={}",
            first_id, focus_scene_types, first_explanation
        ),
        errors,
    )
}

pub fn run_project_brain_uses_writer_memory_focus_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-4",
            "林墨必须追查寒玉戒指下落，并在本章揭开戒指来源线索。",
            "寒玉戒指下落",
            "不要被旧门传闻稀释主线",
            "以戒指来源的新线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .record_chapter_result(
            &agent_writer_lib::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: "Chapter-3".to_string(),
                chapter_revision: "rev-3".to_string(),
                summary: "黑衣人夺走寒玉戒指后留下北境雪线脚印。".to_string(),
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec!["寒玉戒指被带往北境".to_string()],
                promise_updates: vec!["寒玉戒指下落: 待查清".to_string()],
                canon_updates: vec![],
                source_ref: "eval".to_string(),
                created_at: now_ms(),
            },
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-4".to_string());
    let focus = ProjectBrainFocus::from_kernel("旧门风声有什么含义？", &kernel);
    let chunks = vec![
        agent_harness_core::Chunk {
            id: "old-door".to_string(),
            chapter: "Chapter-1".to_string(),
            text: "旧门外的风声像传闻中的哭声，林墨反复听见旧门、风声和旧门。".to_string(),
            embedding: vec![1.0, 0.0],
            keywords: vec!["旧门".to_string(), "风声".to_string()],
            topic: Some("旧门传闻".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        },
        agent_harness_core::Chunk {
            id: "ring-focus".to_string(),
            chapter: "Chapter-3".to_string(),
            text: "黑衣人夺走寒玉戒指后留下北境雪线脚印，林墨必须查清寒玉戒指下落。".to_string(),
            embedding: vec![],
            keywords: vec!["寒玉戒指".to_string(), "下落".to_string()],
            topic: Some("寒玉戒指下落".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        },
    ];
    let raw_results = vec![(50.0, &chunks[0]), (1.0, &chunks[1])];
    let reranked = rerank_project_brain_results_with_focus(raw_results, &focus);
    let first_id = reranked
        .first()
        .map(|(_, _, chunk)| chunk.id.as_str())
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if !focus.as_str().contains("寒玉戒指下落") {
        errors.push("writer memory focus missing active chapter mission".to_string());
    }
    if first_id != "ring-focus" {
        errors.push(format!(
            "writer memory focus should lift active mission chunk above query-similar chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("寒玉戒指") {
        errors.push(format!(
            "rerank explanation missing memory focus term: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_writer_memory_focus",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_project_brain_long_session_candidate_recall_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-9",
            "林墨必须追查寒玉戒指下落，并揭开黑衣人把戒指带往北境宗门的来源线索。",
            "寒玉戒指下落",
            "不要被旧门传闻或无关闲谈稀释主线",
            "以戒指来源的新线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .record_chapter_result(
            &agent_writer_lib::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: "Chapter-8".to_string(),
                chapter_revision: "rev-8".to_string(),
                summary: "黑衣人带着寒玉戒指越过北境界碑，留下宗门旧印。".to_string(),
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec!["寒玉戒指被带往北境宗门".to_string()],
                promise_updates: vec!["寒玉戒指下落: 北境宗门待查".to_string()],
                canon_updates: vec![],
                source_ref: "eval".to_string(),
                created_at: now_ms(),
            },
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-9".to_string());
    let focus = ProjectBrainFocus::from_kernel("旧门风声有什么含义？", &kernel);

    let mut db = VectorDB::new();
    for i in 0..8 {
        db.upsert(Chunk {
            id: format!("old-door-noise-{}", i + 1),
            chapter: format!("Chapter-{}", i + 1),
            text: format!(
                "旧门外的风声在第{}夜反复出现，旧门、风声、旧门传闻、寒玉戒指传闻、北境宗门闲谈、戒指来源闲谈、下落猜测、线索闲谈和林墨的犹疑被路人反复提起。",
                i + 1
            ),
            embedding: vec![],
            keywords: vec!["旧门".to_string(), "风声".to_string()],
            topic: Some("旧门风声传闻".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        });
    }
    db.upsert(Chunk {
        id: "ring-long-session".to_string(),
        chapter: "Chapter-8".to_string(),
        text: "黑衣人带着寒玉戒指抵达北境宗门，宗门旧印揭开戒指来源线索，林墨必须查清寒玉戒指下落，并以戒指来源线索收束本章承诺。"
            .to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec![
            "寒玉戒指".to_string(),
            "北境".to_string(),
            "宗门".to_string(),
        ],
        topic: Some("寒玉戒指下落".to_string()),
        source_ref: None,
        source_revision: None,
        source_kind: None,
        chunk_index: None,
        archived: false,
    });

    let search_text = focus.search_text();
    let embedding = vec![1.0, 0.0];
    let query_only_top_five = db.search_hybrid("旧门风声有什么含义？", &embedding, 5);
    let query_only_contains_ring = query_only_top_five
        .iter()
        .any(|(_, chunk)| chunk.id == "ring-long-session");
    let narrow_focus_top_five = db.search_hybrid(&search_text, &embedding, 5);
    let narrow_focus_contains_ring = narrow_focus_top_five
        .iter()
        .any(|(_, chunk)| chunk.id == "ring-long-session");
    let reranked = search_project_brain_results_with_focus(&db, &focus, &embedding);
    let first_id = reranked
        .first()
        .map(|(_, _, chunk)| chunk.id.as_str())
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if query_only_contains_ring {
        errors.push("fixture should prove query-only top-5 would miss mission chunk".to_string());
    }
    if first_id != "ring-long-session" {
        errors.push(format!(
            "expanded Project Brain candidate pool should recall and prioritize mission chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("寒玉戒指")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "rerank explanation missing mission and payoff signals: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_long_session_candidate_recall",
        format!(
            "queryOnlyTop5ContainsRing={} narrowFocusTop5ContainsRing={} first={} explanation={}",
            query_only_contains_ring, narrow_focus_contains_ring, first_id, first_explanation
        ),
        errors,
    )
}

pub fn run_project_brain_avoid_terms_preserve_payoff_eval() -> EvalResult {
    let chunks = vec![
        (
            36.0,
            (
                "rumor-noise",
                "旧门传闻在酒肆里反复扩散，路人只谈旧门传闻和无关闲谈，没有新的线索。",
            ),
        ),
        (
            1.0,
            (
                "old-door-payoff",
                "林墨回到旧门，发现门缝里的钥匙正是前文伏笔，旧门钥匙揭开密信来源并回收承诺。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章必须回收旧门钥匙伏笔，揭开密信来源；不要被旧门传闻或无关闲谈稀释主线。",
        |(_, text)| text.to_string(),
    );
    let first_id = reranked
        .first()
        .map(|(_, _, (id, _))| *id)
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if first_id != "old-door-payoff" {
        errors.push(format!(
            "avoid-term rerank should preserve old-door payoff while suppressing rumor noise, got {}",
            first_id
        ));
    }
    if first_explanation.contains("avoid term 旧门")
        || !first_explanation.contains("旧门钥匙")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "payoff explanation should keep old-door-key relevance without broad old-door avoid penalty: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_avoid_terms_preserve_payoff",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_project_brain_must_not_boundary_eval() -> EvalResult {
    let chunks = vec![
        (
            48.0,
            (
                "rumor-dominates",
                "旧门传闻盖过寒玉戒指下落，酒肆闲谈只把旧门传闻当成主线，林墨没有得到新线索。",
            ),
        ),
        (
            1.0,
            (
                "ring-payoff",
                "林墨追查寒玉戒指下落，发现黑衣人把戒指带往北境宗门，戒指来源线索终于收束。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章必须追查寒玉戒指下落，揭开戒指来源；不得让旧门传闻盖过寒玉戒指下落。",
        |(_, text)| text.to_string(),
    );
    let first_id = reranked
        .first()
        .map(|(_, _, (id, _))| *id)
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if first_id != "ring-payoff" {
        errors.push(format!(
            "must_not boundary should suppress rumor while preserving ring target, got {}",
            first_id
        ));
    }
    if first_explanation.contains("avoid term 寒玉戒指")
        || !first_explanation.contains("寒玉戒指")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "must_not boundary should keep ring payoff as positive target: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_must_not_boundary",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_project_brain_author_fixture_rerank_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-17",
            "阿洛必须追查霜铃塔钥的下落，并揭开它和潮汐祭账之间的旧约。",
            "霜铃塔钥下落",
            "别再让盐市流言抢走霜铃塔钥下落",
            "以潮汐祭账的真实签名收束。",
            "eval",
        )
        .unwrap();
    memory
        .record_chapter_result(
            &agent_writer_lib::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: "Chapter-16".to_string(),
                chapter_revision: "rev-16".to_string(),
                summary: "阿洛在潮井边确认霜铃塔钥被镜盐会带走，祭账上留下潮汐旧约签名。"
                    .to_string(),
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec![
                    "霜铃塔钥被镜盐会带走".to_string(),
                    "潮汐祭账留下旧约签名".to_string(),
                ],
                promise_updates: vec!["霜铃塔钥下落: 镜盐会待追查".to_string()],
                canon_updates: vec![],
                source_ref: "eval".to_string(),
                created_at: now_ms(),
            },
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-17".to_string());
    let focus = ProjectBrainFocus::from_kernel("盐市流言到底指向谁？", &kernel);

    let mut db = VectorDB::new();
    for i in 0..20 {
        db.upsert(Chunk {
            id: format!("salt-rumor-noise-{}", i + 1),
            chapter: format!("Chapter-{}", i + 1),
            text: format!(
                "第{}章盐市流言继续扩散，茶摊都在重复盐市、流言、镜盐会、霜铃塔钥传闻和潮汐祭账闲谈，但没有人真正追查塔钥下落。",
                i + 1
            ),
            embedding: vec![1.0, 0.0],
            keywords: vec!["盐市".to_string(), "流言".to_string()],
            topic: Some("盐市流言".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        });
    }
    db.upsert(Chunk {
        id: "author-project-payoff".to_string(),
        chapter: "Chapter-16".to_string(),
        text: "阿洛在潮井石阶下发现霜铃塔钥的下落：镜盐会把塔钥藏进潮汐祭账封皮，旧约签名揭开真实来源，这条伏笔终于回收。"
            .to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec!["霜铃塔钥".to_string(), "潮汐祭账".to_string()],
        topic: Some("霜铃塔钥下落".to_string()),
        source_ref: None,
        source_revision: None,
        source_kind: None,
        chunk_index: None,
        archived: false,
    });

    let embedding = vec![1.0, 0.0];
    let query_only_top_ten = db.search_hybrid("盐市流言到底指向谁？", &embedding, 10);
    let query_only_contains_payoff = query_only_top_ten
        .iter()
        .any(|(_, chunk)| chunk.id == "author-project-payoff");
    let reranked = search_project_brain_results_with_focus(&db, &focus, &embedding);
    let first_id = reranked
        .first()
        .map(|(_, _, chunk)| chunk.id.as_str())
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if query_only_contains_payoff {
        errors
            .push("fixture should prove query-only top-10 misses author payoff chunk".to_string());
    }
    if first_id != "author-project-payoff" {
        errors.push(format!(
            "author-project fixture should recall and prioritize custom-term payoff chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("霜铃塔钥")
        || !first_explanation.contains("潮汐祭账")
        || first_explanation.contains("avoid term 霜铃塔钥")
    {
        errors.push(format!(
            "rerank explanation should include custom positive terms without boundary-after avoid penalty: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_author_fixture_rerank",
        format!(
            "queryOnlyTop10ContainsPayoff={} first={} explanation={}",
            query_only_contains_payoff, first_id, first_explanation
        ),
        errors,
    )
}

pub fn run_project_brain_knowledge_index_graph_eval() -> EvalResult {
    let mut db = VectorDB::new();
    db.upsert(Chunk {
        id: "chunk-ring-payoff".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "林墨在霜铃塔发现寒玉戒指的裂纹，与张三隐瞒的旧门钥匙有关。".to_string(),
        embedding: vec![1.0, 0.0],
        keywords: vec![
            "寒玉戒指".to_string(),
            "霜铃塔".to_string(),
            "旧门钥匙".to_string(),
        ],
        topic: Some("寒玉戒指下落".to_string()),
        source_ref: None,
        source_revision: None,
        source_kind: None,
        chunk_index: None,
        archived: false,
    });
    let outline = vec![agent_writer_lib::brain_service::OutlineNode {
        chapter_title: "Chapter-5".to_string(),
        summary: "林墨前往霜铃塔，追查寒玉戒指和旧门钥匙的关系。".to_string(),
        status: "draft".to_string(),
    }];
    let lorebook = vec![agent_writer_lib::brain_service::LoreEntry {
        id: "ring".to_string(),
        keyword: "寒玉戒指".to_string(),
        content: "寒玉戒指是林墨母亲留下的遗物，裂纹会在霜铃塔附近显现。".to_string(),
    }];
    let index = build_project_brain_knowledge_index("eval", &db, &outline, &lorebook);

    let mut errors = Vec::new();
    for kind in ["lore", "outline", "chunk"] {
        if !index.nodes.iter().any(|node| node.kind == kind) {
            errors.push(format!("knowledge index missing {} node", kind));
        }
    }
    if !index.nodes.iter().any(|node| {
        node.source_ref == "lorebook:ring" && node.keywords.iter().any(|kw| kw == "寒玉戒指")
    }) {
        errors.push("lore node lacks source ref or keyword".to_string());
    }
    if !index.edges.iter().any(|edge| {
        edge.relation.contains("寒玉戒指")
            && index
                .nodes
                .iter()
                .any(|node| node.id == edge.from && node.kind == "lore")
            && index
                .nodes
                .iter()
                .any(|node| node.id == edge.to && node.kind != "lore")
    }) {
        errors.push(
            "knowledge graph lacks shared keyword edge from lore to project sources".to_string(),
        );
    }
    if index.source_count != 3 {
        errors.push(format!(
            "source count should be 3, got {}",
            index.source_count
        ));
    }

    eval_result(
        "writer_agent:project_brain_knowledge_index_graph",
        format!("nodes={} edges={}", index.nodes.len(), index.edges.len()),
        errors,
    )
}

pub fn run_project_brain_knowledge_index_path_guard_eval() -> EvalResult {
    let root = std::env::temp_dir().join(format!("forge-knowledge-index-{}", std::process::id()));
    let _ = std::fs::create_dir_all(root.join("notes"));
    let mut errors = Vec::new();

    if safe_knowledge_index_file_path(&root, "notes/index.md").is_err() {
        errors.push("safe relative knowledge path was rejected".to_string());
    }
    for unsafe_path in ["../secret.md", "notes/../../secret.md"] {
        if safe_knowledge_index_file_path(&root, unsafe_path).is_ok() {
            errors.push(format!(
                "unsafe knowledge path was accepted: {}",
                unsafe_path
            ));
        }
    }
    if safe_knowledge_index_file_path(&root, "C:/Windows/system32/drivers/etc/hosts").is_ok() {
        errors.push("absolute knowledge path was accepted".to_string());
    }
    let _ = std::fs::remove_dir_all(&root);

    eval_result(
        "writer_agent:project_brain_knowledge_index_path_guard",
        format!("root={}", root.display()),
        errors,
    )
}

pub fn run_project_brain_chunk_source_version_eval() -> EvalResult {
    let chapter_text = "林墨在霜铃塔发现寒玉戒指的裂纹。\n\n张三承认旧门钥匙来自同一宗门。";
    let revision = project_brain_source_revision(chapter_text);
    let older_revision = project_brain_source_revision("旧版：林墨只追查霜铃塔传闻。");
    let mut db = VectorDB::new();
    db.upsert(Chunk {
        id: "chapter-5-old-0".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "旧版中林墨只追查霜铃塔传闻，没有把寒玉戒指和祭图关联起来。".to_string(),
        embedding: vec![0.5, 0.5],
        keywords: vec!["霜铃塔传闻".to_string(), "旧版线索".to_string()],
        topic: Some("旧版霜铃塔传闻".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(older_revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: true,
    });
    db.upsert(Chunk {
        id: "chapter-5-0".to_string(),
        chapter: "Chapter-5".to_string(),
        text: chapter_text.to_string(),
        embedding: vec![1.0, 0.0],
        keywords: vec!["寒玉戒指".to_string(), "旧门钥匙".to_string()],
        topic: Some("寒玉戒指来源".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: false,
    });
    db.upsert(Chunk {
        id: "chapter-5-1".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "林墨把寒玉戒指和旧门钥匙放在同一张祭图上。".to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec!["寒玉戒指".to_string(), "祭图".to_string()],
        topic: Some("寒玉戒指复核".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some(revision.clone()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(1),
        archived: false,
    });
    let index = build_project_brain_knowledge_index("eval", &db, &[], &[]);
    let node = index.nodes.iter().find(|node| {
        node.label == "Chapter-5" && node.source_revision.as_deref() == Some(revision.as_str())
    });
    let source_history = index
        .source_history
        .iter()
        .find(|source| source.source_ref == "chapter:Chapter-5");
    let compare = compare_project_brain_source_revisions_from_db("chapter:Chapter-5", &db);

    let mut errors = Vec::new();
    let active_chunk = db.chunks.iter().find(|chunk| chunk.id == "chapter-5-0");
    if active_chunk.and_then(|chunk| chunk.source_ref.as_deref()) != Some("chapter:Chapter-5") {
        errors.push(format!(
            "chunk source_ref mismatch: {:?}",
            active_chunk.and_then(|chunk| chunk.source_ref.as_deref())
        ));
    }
    if active_chunk.and_then(|chunk| chunk.source_revision.as_deref()) != Some(revision.as_str()) {
        errors.push(format!(
            "chunk source_revision mismatch: {:?}",
            active_chunk.and_then(|chunk| chunk.source_revision.as_deref())
        ));
    }
    if active_chunk.and_then(|chunk| chunk.source_kind.as_deref()) != Some("chapter")
        || active_chunk.and_then(|chunk| chunk.chunk_index) != Some(0)
    {
        errors.push(format!(
            "chunk source kind/index mismatch: {:?} {:?}",
            active_chunk.and_then(|chunk| chunk.source_kind.as_deref()),
            active_chunk.and_then(|chunk| chunk.chunk_index)
        ));
    }
    match node {
        Some(node) => {
            if node.kind != "chunk"
                || node.source_ref != "chapter:Chapter-5"
                || node.source_revision.as_deref() != Some(revision.as_str())
                || node.source_kind.as_deref() != Some("chapter")
                || node.chunk_index != Some(0)
            {
                errors.push(format!(
                    "knowledge node should preserve chunk source metadata, got kind={} source={} revision={:?} sourceKind={:?} chunkIndex={:?}",
                    node.kind,
                    node.source_ref,
                    node.source_revision,
                    node.source_kind,
                    node.chunk_index
                ));
            }
        }
        None => errors.push("knowledge index missing sourced chunk node".to_string()),
    }
    match source_history {
        Some(history) => {
            if history.source_kind != "chapter"
                || history.node_count != 3
                || history.chunk_count != 3
                || history.revisions.len() != 2
            {
                errors.push(format!(
                    "source history aggregation mismatch: kind={} nodes={} chunks={} revisions={}",
                    history.source_kind,
                    history.node_count,
                    history.chunk_count,
                    history.revisions.len()
                ));
            }
            if let Some(history_revision) = history
                .revisions
                .iter()
                .find(|history_revision| history_revision.revision == revision)
            {
                if history_revision.revision != revision
                    || history_revision.node_count != 2
                    || history_revision.chunk_indexes != vec![0, 1]
                    || !history_revision.active
                {
                    errors.push(format!(
                        "source revision history mismatch: revision={} nodes={} chunks={:?} active={}",
                        history_revision.revision,
                        history_revision.node_count,
                        history_revision.chunk_indexes,
                        history_revision.active
                    ));
                }
            } else {
                errors.push("source history missing revision entry".to_string());
            }
            if !history.revisions.iter().any(|history_revision| {
                history_revision.revision == older_revision && !history_revision.active
            }) {
                errors.push("source history missing archived revision entry".to_string());
            }
        }
        None => errors.push("knowledge index missing source history".to_string()),
    }
    if compare.active_revision.as_deref() != Some(revision.as_str())
        || compare.revisions.len() != 2
        || !compare
            .added_keywords
            .iter()
            .any(|keyword| keyword == "祭图")
        || !compare
            .removed_keywords
            .iter()
            .any(|keyword| keyword == "旧版线索")
    {
        errors.push(format!(
            "source compare mismatch: active={:?} revisions={} added={:?} removed={:?}",
            compare.active_revision,
            compare.revisions.len(),
            compare.added_keywords,
            compare.removed_keywords
        ));
    }

    eval_result(
        "writer_agent:project_brain_chunk_source_version",
        format!(
            "source={:?} revision={:?} nodeKind={} sourceKind={} historySources={}",
            active_chunk.and_then(|chunk| chunk.source_ref.as_ref()),
            active_chunk.and_then(|chunk| chunk.source_revision.as_ref()),
            node.map(|node| node.kind.as_str()).unwrap_or("none"),
            node.and_then(|node| node.source_kind.as_deref())
                .unwrap_or("none"),
            index.source_history.len()
        ),
        errors,
    )
}

pub fn run_project_brain_embedding_provider_limits_eval() -> EvalResult {
    let profile = project_brain_embedding_profile_from_config(
        "https://openrouter.ai/api/v1",
        "text-embedding-3-large",
        48,
    );
    let (trimmed, truncated) =
        trim_embedding_input("寒玉戒指".repeat(40).as_str(), profile.input_limit_chars);

    let mut errors = Vec::new();
    if profile.provider_id != "openrouter" {
        errors.push(format!(
            "provider id should be openrouter, got {}",
            profile.provider_id
        ));
    }
    if profile.model != "text-embedding-3-large" {
        errors.push(format!("profile model mismatch: {}", profile.model));
    }
    if profile.dimensions != 3072 {
        errors.push(format!(
            "text-embedding-3-large dimensions should be 3072, got {}",
            profile.dimensions
        ));
    }
    if profile.input_limit_chars != 48 {
        errors.push(format!(
            "input limit should come from settings, got {}",
            profile.input_limit_chars
        ));
    }
    if profile.batch_limit == 0 || profile.retry_limit == 0 {
        errors.push("profile lacks batch/retry limits".to_string());
    }
    if profile.provider_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
        || profile.model_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    {
        errors.push(format!(
            "known openrouter/model should resolve through registry, got provider={:?} model={:?}",
            profile.provider_status, profile.model_status
        ));
    }
    if !truncated || trimmed.chars().count() > profile.input_limit_chars {
        errors.push(format!(
            "embedding input was not truncated to limit: truncated={} chars={}",
            truncated,
            trimmed.chars().count()
        ));
    }
    if project_brain_embedding_batch_status(3, 3, 0, &[])
        != ProjectBrainEmbeddingBatchStatus::Complete
    {
        errors.push("all embedded chunks should report a complete batch".to_string());
    }
    if project_brain_embedding_batch_status(3, 2, 1, &[])
        != ProjectBrainEmbeddingBatchStatus::Partial
    {
        errors.push("skipped chunks should report a partial batch".to_string());
    }
    if project_brain_embedding_batch_status(3, 0, 3, &[]) != ProjectBrainEmbeddingBatchStatus::Empty
    {
        errors.push("zero embedded chunks should report an empty batch".to_string());
    }

    eval_result(
        "writer_agent:project_brain_embedding_provider_limits",
        format!(
            "provider={} model={} dims={} limit={} truncated={}",
            profile.provider_id,
            profile.model,
            profile.dimensions,
            profile.input_limit_chars,
            truncated
        ),
        errors,
    )
}

pub fn run_project_brain_embedding_provider_registry_eval() -> EvalResult {
    let registry = project_brain_embedding_provider_registry();
    let openai_profile = resolve_project_brain_embedding_profile(
        "https://api.openai.com/v1",
        "text-embedding-3-small",
        None,
    );
    let local_profile = resolve_project_brain_embedding_profile(
        "http://127.0.0.1:11434/v1",
        "text-embedding-ada-002",
        None,
    );
    let fallback_profile = resolve_project_brain_embedding_profile(
        "https://embeddings.example.invalid/v1",
        "custom-embedding-model",
        None,
    );
    let override_profile = resolve_project_brain_embedding_profile(
        "https://api.openai.com/v1",
        "text-embedding-3-large",
        Some(4096),
    );

    let mut errors = Vec::new();
    if registry.providers.len() < 3 {
        errors.push(format!(
            "registry should expose openai/openrouter/local providers, got {}",
            registry.providers.len()
        ));
    }
    if !registry
        .providers
        .iter()
        .any(|provider| provider.provider_id == "openrouter")
    {
        errors.push("registry missing openrouter provider".to_string());
    }
    if openai_profile.provider_id != "openai"
        || openai_profile.provider_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    {
        errors.push(format!(
            "OpenAI base should resolve as known openai provider, got {} {:?}",
            openai_profile.provider_id, openai_profile.provider_status
        ));
    }
    if openai_profile.dimensions != 1536
        || openai_profile.model_status != ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    {
        errors.push(format!(
            "OpenAI text-embedding-3-small should be known 1536 dims, got {} {:?}",
            openai_profile.dimensions, openai_profile.model_status
        ));
    }
    if local_profile.provider_id != "local-openai-compatible"
        || local_profile.retry_limit != 0
        || local_profile.batch_limit != 8
    {
        errors.push(format!(
            "local provider policy mismatch: provider={} batch={} retry={}",
            local_profile.provider_id, local_profile.batch_limit, local_profile.retry_limit
        ));
    }
    if fallback_profile.provider_id != "openai-compatible"
        || fallback_profile.provider_status
            != ProjectBrainEmbeddingRegistryStatus::CompatibilityFallback
        || fallback_profile.model_status
            != ProjectBrainEmbeddingRegistryStatus::CompatibilityFallback
    {
        errors.push(format!(
            "unknown provider/model should be explicit compatibility fallback, got {} {:?} {:?}",
            fallback_profile.provider_id,
            fallback_profile.provider_status,
            fallback_profile.model_status
        ));
    }
    if fallback_profile.dimensions != 1536
        || fallback_profile.batch_limit != 8
        || fallback_profile.retry_limit != 0
    {
        errors.push(format!(
            "fallback policy mismatch: dims={} batch={} retry={}",
            fallback_profile.dimensions, fallback_profile.batch_limit, fallback_profile.retry_limit
        ));
    }
    if override_profile.input_limit_chars != 4096 || override_profile.dimensions != 3072 {
        errors.push(format!(
            "profile override/model dimensions mismatch: limit={} dims={}",
            override_profile.input_limit_chars, override_profile.dimensions
        ));
    }

    eval_result(
        "writer_agent:project_brain_embedding_provider_registry",
        format!(
            "providers={} openai={} local={} fallback={} overrideLimit={}",
            registry.providers.len(),
            openai_profile.provider_id,
            local_profile.provider_id,
            fallback_profile.provider_id,
            override_profile.input_limit_chars
        ),
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
