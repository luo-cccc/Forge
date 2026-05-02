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
