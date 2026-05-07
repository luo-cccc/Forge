use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_todayfive_content_quality_eval() -> EvalResult {
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
        .upsert_character_with_attrs(
            "Hero",
            &[],
            "protagonist",
            "The main hero, struggles with choices.",
            &serde_json::json!({"weapon": "sword"}),
            0.95,
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "Ancient Sword",
            "The hero must retrieve the ancient sword before the enemy.",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let summary = kernel.today_five_summary();
    let guard = summary.items.iter().find(|i| i.slot == "guard");
    let contract = summary.items.iter().find(|i| i.slot == "contract");
    let mission = summary.items.iter().find(|i| i.slot == "mission");
    let promise = summary.items.iter().find(|i| i.slot == "promise");
    let next = summary.items.iter().find(|i| i.slot == "next");
    let guard_ok = guard.is_some_and(|i| !i.value.is_empty());
    let contract_ok = contract.is_some_and(|i| !i.value.is_empty());
    let mission_ok = mission.is_some_and(|i| !i.value.is_empty());
    let promise_ok = promise.is_some_and(|i| !i.value.is_empty());
    let next_ok = next.is_some_and(|i| !i.value.is_empty());
    let all_ok = guard_ok && contract_ok && mission_ok && promise_ok && next_ok;
    EvalResult::pass_if(
        "writer_agent:todayfive_content_quality",
        all_ok,
        format!(
            "guard={} contract={} mission={} promise={} next={}",
            guard_ok, contract_ok, mission_ok, promise_ok, next_ok
        ),
    )
}
