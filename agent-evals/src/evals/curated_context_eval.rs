use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::curated_context_summary;
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_curated_context_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed 5 promises
    for i in 0..5 {
        memory
            .add_promise(
                "clue",
                &format!("线索{}", i + 1),
                &format!("描述{}", i + 1),
                "ch1",
                "payoff",
                (5 + i) as i32,
            )
            .ok();
    }

    // Seed 5 knowledge items
    for i in 0..5 {
        memory
            .upsert_knowledge_item(
                &format!("背景知识{}", i + 1),
                "confirmed",
                &format!("source-{}", i + 1),
            )
            .ok();
    }

    let summary = curated_context_summary(&memory);
    if summary.is_empty() {
        errors.push("curated context should not be empty with seeded data".to_string());
    }

    // Count "线索:" lines (promises) — should be <= 3
    let promise_count = summary.lines().filter(|l| l.starts_with("线索:")).count();
    if promise_count > 3 {
        errors.push(format!(
            "curated context should have <=3 promises, got {}",
            promise_count
        ));
    }
    if promise_count == 0 {
        errors.push("curated context should have at least 1 promise".to_string());
    }

    // Count "背景:" lines (knowledge items) — should be <= 3
    let knowledge_count = summary.lines().filter(|l| l.starts_with("背景:")).count();
    if knowledge_count > 3 {
        errors.push(format!(
            "curated context should have <=3 knowledge items, got {}",
            knowledge_count
        ));
    }

    eval_result(
        "writer_agent:curated_context",
        format!(
            "promises={} knowledge={}",
            promise_count, knowledge_count
        ),
        errors,
    )
}
