#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    ChapterSettlementDelta, CharacterStateDeltaEntry, RelationshipDeltaEntry,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::settlement_apply::apply_chapter_settlement_delta;
use std::path::Path;

pub fn run_entity_repair_state_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval-repair", "test", "fantasy", "p", "j", "")
        .unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "hero")
        .unwrap();
    memory
        .upsert_character("张三", &[], "antagonist", "rival")
        .unwrap();

    // Build settlement delta with entity deltas
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-2".to_string(),
        chapter_revision: "aaaa0001".to_string(),
        character_state_deltas: vec![CharacterStateDeltaEntry {
            character_name: "林墨".to_string(),
            chapter_title: "Chapter-2".to_string(),
            action: "upserted".to_string(),
            core_commitments: vec!["sworn to protect".to_string()],
            goal_state: serde_json::json!({"goal": "revenge"}),
            source_ref: "test-entity-repair".to_string(),
        }],
        relationship_deltas: vec![RelationshipDeltaEntry {
            character_a_name: "林墨".to_string(),
            character_b_name: "张三".to_string(),
            action: "upserted".to_string(),
            relation_type: "rivalry".to_string(),
            visibility: "public".to_string(),
            chapter_title: "Chapter-2".to_string(),
            source_ref: "test-entity-repair".to_string(),
        }],
        ..Default::default()
    };

    let result = apply_chapter_settlement_delta(&memory, "eval-repair", &delta).unwrap();
    if !result.applied {
        errors.push("settlement was not applied".to_string());
    }

    // Verify character state exists
    let state = memory
        .get_active_state(
            memory.get_character_by_name("林墨").unwrap().unwrap().id,
            "Chapter-2",
        )
        .unwrap();
    if state.is_none() {
        errors.push("character state should exist after settlement apply".to_string());
    }

    // Verify relationship exists
    let relationships = memory
        .get_active_relationships(
            memory.get_character_by_name("林墨").unwrap().unwrap().id,
            "Chapter-2",
        )
        .unwrap_or_default();
    if relationships.is_empty() {
        errors.push("relationship should exist after settlement apply".to_string());
    }

    eval_result(
        "writer_agent:entity_repair_state",
        format!(
            "applied={} stateExists={} relationshipExists={}",
            result.applied,
            state.is_some(),
            !relationships.is_empty()
        ),
        errors,
    )
}
