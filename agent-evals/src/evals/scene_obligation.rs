#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_scene_obligation_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let sid = memory
        .upsert_scene("Chapter-1", 0, "scene", "test")
        .unwrap();
    memory
        .upsert_scene_obligations(
            sid,
            &[1, 2],
            &["mission-1".to_string()],
            &["payoff-1".to_string()],
        )
        .unwrap();
    let obl = memory.get_scene_obligations(sid).unwrap();
    let bound = obl.is_some() && obl.as_ref().unwrap().promise_ids == vec![1, 2];
    if !bound {
        errors.push("scene obligations not bound correctly".to_string());
    }
    eval_result(
        "writer_agent:scene_obligation",
        format!("obligationBound={}", bound),
        errors,
    )
}
