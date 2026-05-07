use crate::fixtures::*;
use agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine;
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};
use std::path::Path;

pub fn run_pacing_monotony_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed 5 recent chapter results with action keywords in summaries
    for i in 1..=5 {
        memory
            .record_chapter_result(&ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: format!("第{}章", i),
                chapter_revision: "rev-1".to_string(),
                summary: if i <= 4 {
                    "本章以激烈的冲突开场，主角卷入一场大规模战斗。".to_string()
                } else {
                    "主角在花园中静静沉思，回忆过往。".to_string()
                },
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec![],
                promise_updates: vec![],
                canon_updates: vec![],
                source_ref: "test".to_string(),
                created_at: (1000 + i) as u64,
            })
            .unwrap();
    }

    let paragraph = "测试段落。";
    let engine = DiagnosticsEngine::new();
    let results = engine.diagnose(paragraph, 0, "ch1", "eval", &memory);

    let pacing_monotony = results.iter().find(|r| r.message.contains("节奏单调"));
    if pacing_monotony.is_none() {
        errors.push(format!(
            "expected pacing monotony diagnostic with 4+ action chapters, got {} results",
            results.len()
        ));
    } else {
        let r = pacing_monotony.unwrap();
        if !r.message.contains("4") || !r.message.contains("5") {
            errors.push(format!(
                "pacing message should reference chapter counts, got: {}",
                r.message
            ));
        }
    }

    eval_result(
        "writer_agent:pacing_monotony",
        format!(
            "results={} found={}",
            results.len(),
            pacing_monotony.is_some()
        ),
        errors,
    )
}
