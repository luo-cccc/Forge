use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_story_snapshot_contract_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_character("主角", &[], "protagonist", "主角描述")
        .unwrap();
    memory
        .upsert_character("配角", &[], "supporting", "配角描述")
        .unwrap();
    for i in 1..=3 {
        let result = ChapterResultSummary {
            id: 0,
            project_id: "eval".to_string(),
            chapter_title: format!("Chapter-{}", i),
            chapter_revision: format!("rev-{}", i),
            summary: format!("Chapter {} summary", i),
            state_changes: vec![],
            character_progress: vec![],
            new_conflicts: vec![],
            new_clues: vec![],
            promise_updates: vec![],
            canon_updates: vec![],
            source_ref: format!("test:ch{}", i),
            created_at: 1000,
        };
        memory.record_chapter_result(&result).unwrap();
    }
    let kernel = WriterAgentKernel::new("eval", memory);
    let snapshot = kernel.story_snapshot();
    let char_ok = snapshot.character_count >= 2;
    let has_protagonist = snapshot.protagonist_name.is_some();
    let chapters_ok = snapshot.total_chapters >= 3;
    let ok = char_ok && has_protagonist && chapters_ok;
    EvalResult::pass_if(
        "writer_agent:story_snapshot_contract",
        ok,
        format!(
            "characterCount={} hasProtagonist={} totalChapters={}",
            snapshot.character_count, has_protagonist, snapshot.total_chapters
        ),
    )
}
