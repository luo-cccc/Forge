#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_promise_subject_binding_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    let char_id = memory
        .upsert_character("林墨", &[], "protagonist", "lead")
        .unwrap();
    let promise_id = memory
        .add_promise(
            "plot_promise",
            "test ring",
            "find ring",
            "Chapter-1",
            "Chapter-5",
            4,
        )
        .unwrap();

    memory
        .bind_promise_subject(promise_id, &[char_id], "character")
        .unwrap();
    let promises = memory
        .get_promises_by_subject(char_id, "character")
        .unwrap();
    let bound = promises.iter().any(|p| p.id == promise_id);

    if !bound {
        errors.push("promise should be bound to character subject".to_string());
    }

    eval_result(
        "writer_agent:promise_subject_binding",
        format!("subjectBound={} promiseCount={}", bound, promises.len()),
        errors,
    )
}
