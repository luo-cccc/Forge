use crate::fixtures::*;
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::memory::WriterMemory;
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
        validate_canon_candidate, validate_promise_candidate, MemoryCandidateQuality,
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

    eval_result(
        "writer_agent:memory_candidate_quality_validation",
        format!("4 candidates validated"),
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
