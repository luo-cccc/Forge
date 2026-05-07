#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::operation::WriterOperation;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_reader_todayfive_eval() -> EvalResult {
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
    // Add a promise with expected payoff to trigger reader expectation
    memory
        .add_promise(
            "plot_promise",
            "失落权杖",
            "权杖在远古遗迹深处等待被发现",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let summary = kernel.today_five_summary();

    // Verify the next slot exists
    let next = summary.items.iter().find(|i| i.slot == "next");
    if next.is_none() {
        errors.push("missing next slot in TodayFive".to_string());
        return eval_result(
            "writer_agent:reader_todayfive",
            "no next slot".to_string(),
            errors,
        );
    }
    let next_item = next.unwrap();
    let detail = &next_item.detail;

    // The next slot detail should contain reader-related text:
    // either "读者期待" (reader expectation) or the promise payoff chapter
    let has_reader_hint = detail.contains("读者期待") || detail.contains("期待");
    if !has_reader_hint && detail.contains("权杖") {
        // Pass: detail contains promise-related text even without explicit label
    } else if has_reader_hint {
        // Pass: detail contains reader expectation hint
    } else {
        errors.push(format!(
            "next detail should contain reader-related text, got: {}",
            detail
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

    // Verify the promise slot contains the promise we added
    let promise_slot = summary.items.iter().find(|i| i.slot == "promise");
    if let Some(ps) = promise_slot {
        if !ps.value.contains("权杖") && !ps.detail.contains("权杖") {
            errors.push(format!(
                "promise slot should mention the added promise, got value={} detail={}",
                ps.value, ps.detail
            ));
        }
    }

    eval_result(
        "writer_agent:reader_todayfive",
        format!(
            "slots={} next_detail_len={} has_reader_hint={}",
            slots.len(),
            detail.len(),
            has_reader_hint
        ),
        errors,
    )
}
