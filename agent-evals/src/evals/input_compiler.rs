#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::input_governance::compiler::compile_input;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_input_compiler_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    memory
        .upsert_character("\u{6797}\u{58a8}", &[], "protagonist", "\u{4e3b}\u{89d2}")
        .unwrap();
    memory
        .upsert_knowledge_item("setting_valley", "objective", "test")
        .unwrap();
    let compiled = compile_input(&memory, "Chapter-1", "write action scene");
    let ok = !compiled.intent_text.is_empty() && !compiled.selected_evidence.is_empty();
    EvalResult::pass_if(
        "input_compiler",
        ok,
        format!(
            "compiledOk={} evidenceCount={}",
            ok,
            compiled.selected_evidence.len()
        ),
    )
}
