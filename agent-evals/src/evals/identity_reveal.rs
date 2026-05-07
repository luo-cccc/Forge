#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::WriterMemory;

/// Verify that a dual identity layer is established and a reveal event is recorded
/// when the identity transitions from private to public.
pub fn run_identity_reveal_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .ok();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    let layer_id = memory
        .upsert_identity_layer(1, "流浪剑客", "北境少主", &[], "Chapter-1")
        .unwrap();

    let identity = memory.get_active_identity(1, "Chapter-1").unwrap();
    let has_dual = identity.is_some()
        && identity.as_ref().unwrap().public_identity == "流浪剑客"
        && identity.as_ref().unwrap().private_identity == "北境少主";

    if !has_dual {
        errors.push("dual identity layer not established".to_string());
    }

    // Record reveal event
    memory
        .record_reveal_event(layer_id, "identity", "public", "Chapter-5", "reveal")
        .unwrap();
    let reveals = memory.list_reveals_by_chapter("Chapter-5").unwrap();
    let reveal_recorded = reveals.iter().any(|r| r.reveal_type == "identity");

    if !reveal_recorded {
        errors.push("identity reveal event not recorded".to_string());
    }

    eval_result(
        "writer_agent:identity_reveal",
        format!(
            "dualIdentity={} revealRecorded={}",
            has_dual, reveal_recorded
        ),
        errors,
    )
}
