use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::build_writing_checklist;
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_writing_checklist_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed open promises with high priority
    memory
        .add_promise(
            "clue",
            "密道秘密",
            "破庙地下有密道",
            "ch1",
            "ch5揭示密道真相",
            8,
        )
        .ok();
    memory
        .add_promise(
            "conflict",
            "背叛伏笔",
            "同伴行为可疑",
            "ch2",
            "ch6背叛揭示",
            6,
        )
        .ok();
    memory
        .add_promise(
            "goal",
            "寻剑任务",
            "主角需找到古剑",
            "ch1",
            "ch4找到古剑",
            4,
        )
        .ok();

    // Seed a protagonist character
    memory
        .upsert_character("云逸", &[], "protagonist", "踏上旅途寻找真相")
        .ok();
    memory
        .upsert_character("墨尘", &[], "protagonist", "暗中观察同伴行动")
        .ok();

    let checklist = build_writing_checklist(&memory, "测试章节");
    if checklist.is_empty() {
        errors.push("checklist should not be empty when promises exist".to_string());
    }
    // Should include at least one high-priority promise
    if !checklist.iter().any(|item| item.contains("密道秘密")) {
        errors.push("checklist should include high-priority promise '密道秘密'".to_string());
    }
    // Should include protagonist
    if !checklist
        .iter()
        .any(|item| item.contains("云逸") || item.contains("墨尘"))
    {
        errors.push("checklist should include protagonist character".to_string());
    }

    eval_result(
        "writer_agent:writing_checklist",
        format!("items={}", checklist.len()),
        errors,
    )
}
