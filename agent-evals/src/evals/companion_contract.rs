#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::kernel::WriterAgentApprovalMode;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_companion_contract_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "promise", "conflict", "")
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let summary = kernel.today_five_summary();
    let has_five = summary.items.len() == 5;
    let slots = ["guard", "contract", "mission", "promise", "next"];
    let all_slots = slots
        .iter()
        .all(|s| summary.items.iter().any(|i| i.slot == *s));
    let ok = has_five && all_slots;
    EvalResult::pass_if(
        "writer_agent:companion_write_mode_boundary",
        ok,
        format!("itemCount={} allSlots={}", summary.items.len(), all_slots),
    )
}
