#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::{PromiseKind, WriterMemory};
use agent_writer_lib::writer_agent::project_intake::build_project_intake_report;

pub fn run_project_intake_reports_sources_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({"weapon":"sword"}),
            0.9,
        )
        .ok();
    memory
        .add_promise(
            "plot_promise",
            "寒玉戒指",
            "母亲遗物被夺走",
            "Chapter-2",
            "Chapter-5",
            4,
        )
        .unwrap();

    let observations = vec![
        crate::fixtures::observation_in_chapter("林墨握紧剑柄", "Chapter-1"),
        crate::fixtures::observation_in_chapter("黑衣人夺走戒指", "Chapter-2"),
    ];

    let report = build_project_intake_report("eval", &observations, &memory);
    if report.chapter_count == 0 {
        errors.push("should have chapters".to_string());
    }
    if report.identified_characters.is_empty() {
        errors.push("should identify characters".to_string());
    }
    if report.open_promises.is_empty() {
        errors.push("should find open promises".to_string());
    }
    eval_result(
        "writer_agent:project_intake_reports_sources",
        format!(
            "chapters={} chars={} promises={}",
            report.chapter_count,
            report.identified_characters.len(),
            report.open_promises.len()
        ),
        errors,
    )
}

pub fn run_project_intake_does_not_auto_write_memory_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    let observations = vec![crate::fixtures::observation_in_chapter("test", "Chapter-1")];
    // Before: no canon entities written
    let canon_before = memory.list_canon_entities().unwrap().len();
    let _report = build_project_intake_report("eval", &observations, &memory);
    // After: still no new canon entities (report is read-only)
    let canon_after = memory.list_canon_entities().unwrap().len();
    if canon_before != canon_after {
        errors.push("intake report should not write canon".to_string());
    }
    eval_result(
        "writer_agent:project_intake_does_not_auto_write_memory",
        format!("canonBefore={} canonAfter={}", canon_before, canon_after),
        errors,
    )
}

pub fn run_project_intake_flags_open_promises_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "霜铃塔钥",
            "钥匙下落不明",
            "Chapter-1",
            "Chapter-10",
            5,
        )
        .unwrap();
    let observations = vec![crate::fixtures::observation_in_chapter(
        "tests",
        "Chapter-1",
    )];
    let report = build_project_intake_report("eval", &observations, &memory);
    if !report.recommendations.iter().any(|r| r.contains("伏笔")) {
        errors.push("should recommend reviewing open promises".to_string());
    }
    eval_result(
        "writer_agent:project_intake_flags_open_promises",
        format!(
            "promises={} recommendations={}",
            report.open_promises.len(),
            report.recommendations.len()
        ),
        errors,
    )
}
