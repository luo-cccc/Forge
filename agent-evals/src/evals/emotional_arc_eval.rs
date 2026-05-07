use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::emotional_arc_guidance;
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};

pub fn run_emotional_arc_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed a chapter result whose summary describes an emotional beat
    memory
        .record_chapter_result(&ChapterResultSummary {
            id: 0,
            project_id: "eval".to_string(),
            chapter_title: "第一章".to_string(),
            chapter_revision: "rev-1".to_string(),
            summary: "读者此刻应感到紧张不安，期待主角冲破困境。但尚未揭示幕后黑手的身份。".to_string(),
            state_changes: vec![],
            character_progress: vec![],
            new_conflicts: vec![],
            new_clues: vec![],
            promise_updates: vec![],
            canon_updates: vec![],
            source_ref: "test".to_string(),
            created_at: 100,
        })
        .ok();

    let guidance = emotional_arc_guidance(&memory, "eval");
    if guidance.is_empty() {
        errors.push("emotional arc guidance should not be empty with a chapter result".to_string());
    }
    if !guidance.contains("情感指引") {
        errors.push("emotional arc guidance should contain '情感指引' header".to_string());
    }
    if !guidance.contains("紧张") {
        errors.push("emotional arc guidance should reference the summary content".to_string());
    }

    eval_result(
        "writer_agent:emotional_arc",
        format!("len={}", guidance.len()),
        errors,
    )
}
