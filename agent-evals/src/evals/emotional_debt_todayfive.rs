#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_emotional_debt_todayfive_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "test",
            "fantasy",
            "A hero must choose between duty and love.",
            "The hero faces a moral dilemma.",
            "",
        )
        .unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();

    // Add a promise with emotional debt keywords to trigger the emotional tracking signal
    memory
        .add_promise(
            "emotional_debt",
            "林墨的愤怒与自责",
            "主角因过去的背叛而陷入深深的愤怒和自责之中",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();

    let kernel = WriterAgentKernel::new("eval", memory);
    let summary = kernel.today_five_summary();

    // Verify the guard slot exists
    let guard = summary.items.iter().find(|i| i.slot == "guard");
    if guard.is_none() {
        errors.push("missing guard slot in TodayFive".to_string());
        return eval_result(
            "writer_agent:emotional_debt_todayfive",
            "no guard slot".to_string(),
            errors,
        );
    }
    let guard_item = guard.unwrap();

    // The guard detail should contain emotional tracking signal when emotional cues exist
    let has_emotional_signal = guard_item.detail.contains("情绪跟踪: 已激活");
    if !has_emotional_signal {
        errors.push(format!(
            "guard detail should contain '情绪跟踪: 已激活' when emotional promises exist, got: {}",
            guard_item.detail
        ));
    }

    // Verify all 5 slots are present
    let slots: Vec<&str> = summary.items.iter().map(|i| i.slot.as_str()).collect();
    let all_slots_present = ["guard", "contract", "mission", "promise", "next"]
        .iter()
        .all(|expected| slots.contains(expected));
    if !all_slots_present {
        errors.push(format!("incomplete slots: {:?}", slots));
    }

    eval_result(
        "writer_agent:emotional_debt_todayfive",
        format!(
            "slots={} guard_detail_len={} has_emotional_signal={}",
            slots.len(),
            guard_item.detail.len(),
            has_emotional_signal
        ),
        errors,
    )
}
