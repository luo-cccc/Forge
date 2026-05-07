#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::WriterMemory;

/// Verify that one character can remain in misbelief while another is aware
/// of the same knowledge item — preserving dramatic irony across chapters.
pub fn run_false_belief_preservation_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .ok();
    memory
        .upsert_character("张三", &[], "supporting", "配角")
        .ok();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    // Character A in misbelief, Character B aware of same topic
    let kid = memory
        .upsert_knowledge_item("北境宗主的真实身份", "objective", "seed")
        .unwrap();
    memory
        .upsert_knowledge_ownership(kid, "character", 1, "misbelief", "Chapter-1", "seed")
        .unwrap();
    memory
        .upsert_knowledge_ownership(kid, "character", 2, "aware", "Chapter-1", "seed")
        .unwrap();

    // Verify A still in misbelief at Chapter-2 (no reveal yet)
    let a_knowledge = memory
        .get_knowledge_by_holder("character", 1, "Chapter-2")
        .unwrap();
    let a_still_misbelieves = a_knowledge.iter().any(|o| o.knowledge_mode == "misbelief");

    if !a_still_misbelieves {
        errors.push("character A should still be in misbelief at Chapter-2".to_string());
    }

    // Verify B is aware
    let b_knowledge = memory
        .get_knowledge_by_holder("character", 2, "Chapter-2")
        .unwrap();
    let b_is_aware = b_knowledge.iter().any(|o| o.knowledge_mode == "aware");

    if !b_is_aware {
        errors.push("character B should be aware at Chapter-2".to_string());
    }

    eval_result(
        "writer_agent:false_belief_preservation",
        format!(
            "misbeliefPreserved={} otherAware={}",
            a_still_misbelieves, b_is_aware
        ),
        errors,
    )
}
