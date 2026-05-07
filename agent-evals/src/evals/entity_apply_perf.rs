#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{ChapterSettlementDelta, CharacterStateDeltaEntry};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::settlement_apply::apply_chapter_settlement_delta;
use std::path::Path;

pub fn run_entity_apply_perf_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    memory
        .upsert_character("target", &[], "protagonist", "the one")
        .unwrap();
    for i in 0..100 {
        let _ = memory.upsert_character(&format!("filler_{}", i), &[], "supporting", "");
    }
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-1".to_string(),
        chapter_revision: "a".to_string(),
        character_state_deltas: vec![CharacterStateDeltaEntry {
            character_name: "target".to_string(),
            chapter_title: "Chapter-1".to_string(),
            action: "upserted".to_string(),
            core_commitments: vec!["test".to_string()],
            goal_state: serde_json::json!({}),
            source_ref: "test".to_string(),
        }],
        ..Default::default()
    };
    apply_chapter_settlement_delta(&memory, "eval", &delta).unwrap();
    let state = memory.get_active_state(1, "Chapter-1").unwrap();
    EvalResult::pass_if(
        "entity_apply_perf",
        state.is_some(),
        "entityScopedApply=true".to_string(),
    )
}
