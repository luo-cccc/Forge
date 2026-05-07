#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{ChapterSettlementDelta, CharacterStateDeltaEntry};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::settlement_apply::apply_chapter_settlement_delta;
use std::path::Path;

pub fn run_fact_dedup_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval-fact-dedup", "test", "fantasy", "p", "j", "")
        .unwrap();
    // Create canon entity with a fact attribute
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "hero",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.9,
        )
        .unwrap();

    // Apply the same fact (same entity, same key) via settlement delta twice
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-2".to_string(),
        chapter_revision: "aaaa0001".to_string(),
        chapter_fact_delta: vec!["林墨拔出寒影刀".to_string()],
        ..Default::default()
    };

    let _ = apply_chapter_settlement_delta(&memory, "eval-fact-dedup", &delta).unwrap();
    let _ = apply_chapter_settlement_delta(&memory, "eval-fact-dedup", &delta).unwrap();

    // Verify canon_facts for the entity are deduplicated
    let facts = memory
        .get_canon_facts_for_entity("林墨")
        .unwrap_or_default();

    // Count facts with key "weapon" — should be exactly 1
    let weapon_count = facts.iter().filter(|(k, _)| k == "weapon").count();
    if weapon_count != 1 {
        errors.push(format!(
            "expected 1 weapon fact for 林墨, found {}",
            weapon_count
        ));
    }

    eval_result(
        "writer_agent:fact_dedup",
        format!(
            "weaponFactCount={} totalFacts={}",
            weapon_count,
            facts.len()
        ),
        errors,
    )
}
