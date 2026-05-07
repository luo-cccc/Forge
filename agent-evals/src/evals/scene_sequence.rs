#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_scene_sequence_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let s1 = memory
        .upsert_scene("Chapter-1", 0, "scene", "opening")
        .unwrap();
    let s2 = memory
        .upsert_scene("Chapter-1", 1, "scene", "confrontation")
        .unwrap();
    let s3 = memory
        .upsert_scene("Chapter-1", 2, "scene", "resolution")
        .unwrap();
    // Reorder: move s3 before s1
    memory.reorder_scenes("Chapter-1", &[s3, s1, s2]).unwrap();
    let scenes = memory.list_scenes_by_chapter("Chapter-1").unwrap();
    let reordered = scenes.first().map(|s| s.id) == Some(s3);
    if !reordered {
        errors.push("scene reorder failed: expected s3 first".to_string());
    }
    eval_result(
        "writer_agent:scene_sequence",
        format!("reordered={} sceneCount={}", reordered, scenes.len()),
        errors,
    )
}
