use crate::fixtures::*;
use agent_writer_lib::writer_agent::context::ContextSource;
use agent_writer_lib::writer_agent::context_relevance::story_impact_filter;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_impact_scoped_recall_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    // Create 5 context sources with a mix of story-level and cursor-level types.
    let sources = vec![
        ContextSource::CursorPrefix,
        ContextSource::CursorSuffix,
        ContextSource::CanonSlice,
        ContextSource::PromiseSlice,
        ContextSource::ChapterMission,
    ];

    let filtered = story_impact_filter(&sources, &memory, "Chapter-1");
    let filtered_count = filtered.len();
    let original_count = sources.len();

    // With no entities/promises in the fresh in-memory DB, the filter should
    // return all sources (fallback). When impact data is present, filtered
    // count should be <= original count.
    let ok = filtered_count <= original_count && !filtered.is_empty();
    EvalResult::pass_if(
        "impact_scoped_recall",
        ok,
        format!(
            "original={} filtered={} filtered_leq_original={} non_empty={}",
            original_count,
            filtered_count,
            filtered_count <= original_count,
            !filtered.is_empty(),
        ),
    )
}
