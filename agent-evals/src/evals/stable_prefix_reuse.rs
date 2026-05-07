#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    build_chapter_generation_spine, ChapterContextSpine, ChapterTarget,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_stable_prefix_reuse_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();

    let target = ChapterTarget {
        title: "Chapter-1".to_string(),
        filename: "chapter-1.md".to_string(),
        number: Some(1),
        summary: "Opening chapter".to_string(),
        status: "draft".to_string(),
    };

    let spine_a = build_chapter_generation_spine(&target, None, None, None, None, &memory);
    let spine_b = build_chapter_generation_spine(&target, None, None, None, None, &memory);

    let frozen_match = spine_a.frozen_prefix == spine_b.frozen_prefix;
    let stable_match = spine_a.project_stable == spine_b.project_stable;

    let ok = frozen_match && stable_match;
    EvalResult::pass_if(
        "stable_prefix_reuse",
        ok,
        format!(
            "frozenMatch={} stableMatch={} frozenChars={} stableChars={}",
            frozen_match,
            stable_match,
            spine_a.frozen_prefix.chars().count(),
            spine_a.project_stable.chars().count(),
        ),
    )
}
