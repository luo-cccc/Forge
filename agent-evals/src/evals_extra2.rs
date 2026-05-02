use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_trajectory_product_metrics_present_eval() -> EvalResult {
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
    kernel.observe(observation("林墨停在旧门前。")).unwrap();

    let export = kernel.export_trajectory(100);
    let mut errors = Vec::new();
    if export.jsonl.is_empty() {
        errors.push("trajectory export is empty".to_string());
    }
    let has_metrics = export.jsonl.contains("writer.product_metrics");
    if !has_metrics {
        errors.push("trajectory missing product_metrics event".to_string());
    }

    eval_result(
        "writer_agent:trajectory_product_metrics_present",
        format!(
            "jsonlBytes={} hasMetrics={}",
            export.jsonl.len(),
            has_metrics
        ),
        errors,
    )
}

pub fn run_ghost_task_packet_foundation_coverage_eval() -> EvalResult {
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
    kernel
        .observe(observation_in_chapter(
            "林墨停在旧门前，风从门缝里钻出来，带着一股腐朽的气味。他想起张三的话，长刀在鞘中微微震动。",
            "Chapter-1",
        ))
        .unwrap();

    let trace = kernel.trace_snapshot(20);
    let mut errors = Vec::new();
    if trace.recent_proposals.is_empty() {
        errors.push("no proposal traces recorded for ghost observation".to_string());
    }

    eval_result(
        "writer_agent:ghost_task_packet_foundation",
        format!(
            "taskPackets={} proposalTraces={}",
            trace.task_packets.len(),
            trace.recent_proposals.len()
        ),
        errors,
    )
}
