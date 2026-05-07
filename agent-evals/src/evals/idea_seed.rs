#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::project_intake::seed_project_from_idea;

pub fn run_idea_seed_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let text = "林墨是北境剑客，寻找寒玉戒指的真相。对手是影子宗。";
    let report = seed_project_from_idea(&memory, "eval-idea", text).unwrap();

    let has_chars = !report.identified_characters.is_empty();
    let chars_in_memory = memory.list_characters(None).unwrap_or_default().len() > 0;

    EvalResult::pass_if(
        "idea_seed",
        has_chars && chars_in_memory,
        format!("extracted_chars={} memory_chars={}", report.identified_characters.len(), chars_in_memory)
    )
}
