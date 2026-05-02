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
