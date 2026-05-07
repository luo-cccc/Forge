#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::project_intake::seed_project_from_idea;
use std::path::Path;

pub fn run_idea_seed_adversarial_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();

    // Test 1: Oversized text rejected
    let huge = "林墨".repeat(60_000);
    let r1 = seed_project_from_idea(&memory, "eval-huge", &huge);
    let oversized_rejected = r1.is_err();

    // Test 2: Code injection rejected
    let r2 = seed_project_from_idea(
        &memory,
        "eval-inject",
        "SELECT * FROM users; DROP TABLE characters;",
    );
    let code_rejected = r2.is_err();

    // Test 3: Random spam rejected
    let r3 = seed_project_from_idea(&memory, "eval-spam", "!@#$%^&*()))))))))))))))))))))))))))");
    let spam_handled = r3.is_err() || r3.as_ref().map(|r| r.confidence < 0.3).unwrap_or(false);

    // Test 4: Normal text still works
    let r4 = seed_project_from_idea(&memory, "eval-ok", "林墨是剑客，寻找真相。");
    let normal_works = r4.is_ok() && r4.as_ref().unwrap().identified_characters.len() >= 1;

    let all_ok = oversized_rejected && code_rejected && spam_handled && normal_works;
    EvalResult::pass_if(
        "idea_seed_adversarial",
        all_ok,
        format!(
            "oversized={} code={} spam={} normal={}",
            oversized_rejected, code_rejected, spam_handled, normal_works
        ),
    )
}
