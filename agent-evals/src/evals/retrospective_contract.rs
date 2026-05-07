use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_retrospective_contract_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    for i in 1..=3 {
        let result = ChapterResultSummary {
            id: 0,
            project_id: "eval".to_string(),
            chapter_title: format!("Chapter-{}", i),
            chapter_revision: format!("rev-{}", i),
            summary: format!("Chapter {} summary text", i),
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
    let summary = kernel.recent_session_summary(5);
    let chapters_ok = summary.chapters_written >= 3;
    let words_ok = summary.total_words > 0;
    let ok = chapters_ok && words_ok;
    EvalResult::pass_if(
        "writer_agent:retrospective_contract",
        ok,
        format!(
            "chaptersWritten={} totalWords={}",
            summary.chapters_written, summary.total_words
        ),
    )
}
