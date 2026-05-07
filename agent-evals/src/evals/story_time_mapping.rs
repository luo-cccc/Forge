#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_story_time_mapping_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let ts = memory.upsert_time_slice("三年前", 0, "", "").unwrap();
    memory
        .upsert_chapter_time_mapping("Chapter-1", None, ts, "flashback")
        .unwrap();
    memory
        .upsert_chapter_time_mapping("Chapter-5", None, ts, "present")
        .unwrap();
    let mappings = memory.get_time_mapping_for_chapter("Chapter-1").unwrap();
    let has_flashback = mappings
        .iter()
        .any(|m| m.narrative_mode == "flashback" && m.time_slice_id == ts);
    if !has_flashback {
        errors.push("flashback mapping not found for Chapter-1".to_string());
    }
    eval_result(
        "writer_agent:story_time_mapping",
        format!(
            "flashbackMapped={} mappingCount={}",
            has_flashback,
            mappings.len()
        ),
        errors,
    )
}
