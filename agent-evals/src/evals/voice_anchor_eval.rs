use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::author_voice_sample;
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};

pub fn run_voice_anchor_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed a chapter result with a summary
    memory
        .record_chapter_result(&ChapterResultSummary {
            id: 0,
            project_id: "eval".to_string(),
            chapter_title: "第一章".to_string(),
            chapter_revision: "rev-1".to_string(),
            summary: "云逸踏入破庙，发现墙上刻有关于密道的古老文字。他听到铜铃声从地下传来，决定探索密道入口。".to_string(),
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

    let voice = author_voice_sample(&memory, "eval");
    if voice.is_empty() {
        errors.push("voice sample should not be empty with a chapter result".to_string());
    }
    if !voice.contains("参考你的写作风格") {
        errors.push("voice sample should contain '参考你的写作风格' header".to_string());
    }

    eval_result(
        "writer_agent:voice_anchor",
        format!("len={}", voice.len()),
        errors,
    )
}
