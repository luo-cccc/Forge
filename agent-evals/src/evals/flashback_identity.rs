#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_flashback_identity_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();
    memory
        .upsert_identity_layer(1, "流浪剑客", "北境少主", &[], "Chapter-1")
        .unwrap();
    let ts = memory.upsert_time_slice("五年前", 0, "", "").unwrap();
    memory
        .upsert_chapter_time_mapping("Chapter-2", None, ts, "flashback")
        .unwrap();
    let mappings = memory.get_time_mapping_for_chapter("Chapter-2").unwrap();
    let has_ts = mappings.iter().any(|m| m.time_slice_id == ts);
    let identity = memory.get_active_identity(1, "Chapter-1").unwrap();
    if !has_ts {
        errors.push("time slice not mapped for Chapter-2".to_string());
    }
    if identity.is_none() {
        errors.push("identity not accessible for Chapter-1".to_string());
    }
    eval_result(
        "writer_agent:flashback_identity",
        format!(
            "timeMapped={} identityPresent={}",
            has_ts,
            identity.is_some()
        ),
        errors,
    )
}
