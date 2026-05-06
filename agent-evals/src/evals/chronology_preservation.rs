#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};

pub fn run_chronology_preservation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let project_id = "eval-chronology";

    // Setup: record chapter results in known order
    let chapters = vec![
        ("第一章", "aaaa0001"),
        ("第二章", "aaaa0002"),
        ("第三章", "aaaa0003"),
    ];

    for (title, revision) in &chapters {
        let result = ChapterResultSummary {
            id: 0,
            project_id: project_id.to_string(),
            chapter_title: title.to_string(),
            chapter_revision: revision.to_string(),
            summary: format!("{} summary", title),
            state_changes: vec![],
            character_progress: vec![],
            new_conflicts: vec![],
            new_clues: vec![],
            promise_updates: vec![],
            canon_updates: vec![],
            source_ref: format!("test:{}", title),
            created_at: 1000,
        };
        memory.upsert_chapter_result(&result).unwrap();
    }

    // Capture ordering before simulated repair
    let results_before: Vec<String> = memory
        .list_recent_chapter_results(project_id, 10)
        .unwrap_or_default()
        .iter()
        .map(|r| r.chapter_title.clone())
        .collect();

    // Simulate repair: re-upsert the same result (idempotency test)
    let repair_result = ChapterResultSummary {
        id: 0,
        project_id: project_id.to_string(),
        chapter_title: "第二章".to_string(),
        chapter_revision: "aaaa0002".to_string(),
        summary: "第二章 summary".to_string(),
        state_changes: vec![],
        character_progress: vec![],
        new_conflicts: vec![],
        new_clues: vec![],
        promise_updates: vec![],
        canon_updates: vec![],
        source_ref: "test:repair".to_string(),
        created_at: 2000,
    };
    memory.upsert_chapter_result(&repair_result).unwrap();

    // Verify ordering preserved
    let results_after: Vec<String> = memory
        .list_recent_chapter_results(project_id, 10)
        .unwrap_or_default()
        .iter()
        .map(|r| r.chapter_title.clone())
        .collect();

    let chronology_preserved = results_before == results_after;
    let idempotent = results_before.len() == results_after.len();

    let mut errors = Vec::new();
    if !chronology_preserved {
        errors.push(format!(
            "chronology NOT preserved: before={:?} after={:?}",
            results_before, results_after
        ));
    }
    if !idempotent {
        errors.push(format!(
            "repair is NOT idempotent: before_len={} after_len={}",
            results_before.len(),
            results_after.len()
        ));
    }

    eval_result(
        "writer_agent:chronology_preservation",
        format!(
            "chronologyPreserved={} idempotent={} before={:?} after={:?}",
            chronology_preserved, idempotent, results_before, results_after
        ),
        errors,
    )
}
