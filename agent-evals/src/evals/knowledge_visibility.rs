#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::WriterMemory;

/// Verify that a character starts in misbelief mode and can transition to aware
/// (representing a knowledge reveal across chapters).
pub fn run_knowledge_visibility_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .ok();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    let kid = memory
        .upsert_knowledge_item("寒玉戒指的下落", "objective", "seed")
        .unwrap();
    memory
        .upsert_knowledge_ownership(kid, "character", 1, "misbelief", "Chapter-1", "seed")
        .unwrap();

    let ownerships = memory
        .get_knowledge_by_holder("character", 1, "Chapter-1")
        .unwrap();
    let in_misbelief = ownerships.iter().any(|o| o.knowledge_mode == "misbelief");

    if !in_misbelief {
        errors.push("character should start in misbelief mode".to_string());
    }

    // Change mode to aware (simulating reveal)
    memory
        .upsert_knowledge_ownership(kid, "character", 1, "aware", "Chapter-3", "reveal")
        .unwrap();
    let ownerships_after = memory
        .get_knowledge_by_holder("character", 1, "Chapter-3")
        .unwrap();
    let now_aware = ownerships_after
        .iter()
        .any(|o| o.knowledge_mode == "aware");

    if !now_aware {
        errors.push("character should transition to aware mode after reveal".to_string());
    }

    eval_result(
        "writer_agent:knowledge_visibility",
        format!(
            "misbeliefVisible={} revealTransition={}",
            in_misbelief, now_aware
        ),
        errors,
    )
}
