#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::{ChapterSettlementDelta, SceneResultProjection};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::settlement_apply::apply_chapter_settlement_delta;

pub fn run_scene_result_projection_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    let sid = memory
        .upsert_scene("Chapter-2", 0, "scene", "test")
        .unwrap();
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-2".to_string(),
        chapter_revision: "aaaa0001".to_string(),
        scene_deltas: vec![SceneResultProjection {
            scene_id: sid,
            outcome: "resolved".into(),
            consequence: "peace".into(),
            source_ref: "test".into(),
        }],
        ..Default::default()
    };
    let result = apply_chapter_settlement_delta(&memory, "eval", &delta).unwrap();
    let stored = memory.get_scene_results(sid).unwrap();
    if !result.applied {
        errors.push("settlement delta was not applied".to_string());
    }
    if stored.len() != 1 {
        errors.push(format!(
            "expected 1 scene result stored, got {}",
            stored.len()
        ));
    }
    eval_result(
        "writer_agent:scene_result_projection",
        format!(
            "sceneApplied={} resultStored={}",
            result.applied,
            stored.len() == 1
        ),
        errors,
    )
}
