#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    build_chapter_generation_spine, ChapterContextSpine, ChapterTarget,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_spine_telemetry_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();

    let target = ChapterTarget {
        title: "Chapter-1".to_string(),
        filename: "chapter-1.md".to_string(),
        number: Some(1),
        summary: "Opening chapter".to_string(),
        status: "draft".to_string(),
    };

    let spine = build_chapter_generation_spine(&target, None, None, None, None, &memory);

    let total = spine.total_chars();
    let prefix = spine.prefix_char_count();
    let tail = spine.tail_char_count();

    let has_frozen = !spine.frozen_prefix.is_empty();
    let has_hot = !spine.hot_buffer.is_empty();
    let total_nonzero = total > 0;
    let prefix_nonzero = prefix > 0;
    let math_consistent = prefix + tail == total || (prefix == 0 && tail == total);

    let ok = has_frozen && has_hot && total_nonzero && prefix_nonzero && math_consistent;
    EvalResult::pass_if(
        "spine_telemetry",
        ok,
        format!(
            "total={} prefix={} tail={} frozenNonEmpty={} hotNonEmpty={} mathConsistent={}",
            total, prefix, tail, has_frozen, has_hot, math_consistent,
        ),
    )
}
