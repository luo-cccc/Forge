use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_onboarding_contract_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let summary = kernel.today_five_summary();
    let is_onboarding = summary.is_onboarding;
    let has_five = summary.items.len() == 5;
    let welcome_labels = [
        "欢迎使用 Forge",
        "我是你的写作伙伴",
        "先写一个开头",
        "然后点'生成下一章'",
        "我会帮你记住角色、线索和承诺",
    ];
    let all_labels_match = welcome_labels
        .iter()
        .enumerate()
        .all(|(i, label)| summary.items[i].label == *label);
    let ok = is_onboarding && has_five && all_labels_match;
    EvalResult::pass_if(
        "writer_agent:onboarding_contract",
        ok,
        format!(
            "isOnboarding={} itemCount={} labelsMatch={}",
            is_onboarding,
            summary.items.len(),
            all_labels_match
        ),
    )
}
