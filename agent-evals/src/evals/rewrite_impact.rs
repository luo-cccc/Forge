#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::rewrite_impact::compute_rewrite_impact_preview;

pub fn run_rewrite_impact_preview_is_read_only_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({"weapon":"sword"}),
            0.9,
        )
        .ok();
    let canon_before = memory.list_canon_entities().unwrap().len();
    let observation = observation_in_chapter("林墨举起剑", "Chapter-3");
    let _preview = compute_rewrite_impact_preview(&observation, &memory);
    let canon_after = memory.list_canon_entities().unwrap().len();
    if canon_before != canon_after {
        errors.push("rewrite impact preview should not modify memory".to_string());
    }
    eval_result(
        "writer_agent:rewrite_impact_preview_is_read_only",
        format!("canonBefore={} canonAfter={}", canon_before, canon_after),
        errors,
    )
}

pub fn run_rewrite_impact_preview_includes_bidirectional_story_edges_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({"weapon":"sword"}),
            0.9,
        )
        .ok();
    memory
        .add_promise(
            "plot_promise",
            "寒玉戒指",
            "遗物被夺",
            "Chapter-1",
            "Chapter-5",
            4,
        )
        .unwrap();
    let observation = observation_in_chapter("林墨寻找戒指", "Chapter-3");
    let preview = compute_rewrite_impact_preview(&observation, &memory);
    if preview.impacted_canon.is_empty() && preview.impacted_promises.is_empty() {
        errors.push("should have at least some impacted nodes".to_string());
    }
    eval_result(
        "writer_agent:rewrite_impact_preview_includes_bidirectional_story_edges",
        format!(
            "canon={} promises={} risk={}",
            preview.impacted_canon.len(),
            preview.impacted_promises.len(),
            preview.risk
        ),
        errors,
    )
}

pub fn run_rewrite_impact_preview_warns_on_truncated_high_risk_sources_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    for i in 0..30 {
        let long_summary = format!("Entity{} has a very long summary text that should consume budget characters and cause truncation in story impact radius computation", i);
        memory
            .upsert_canon_entity(
                "character",
                &format!("Entity{}", i),
                &[],
                &long_summary,
                &serde_json::Value::Object(serde_json::Map::new()),
                0.8,
            )
            .ok();
    }
    let observation = observation_in_chapter("test", "Chapter-1");
    let preview = compute_rewrite_impact_preview(&observation, &memory);
    if preview.risk.is_empty() {
        errors.push("risk should not be empty".to_string());
    }
    eval_result(
        "writer_agent:rewrite_impact_preview_warns_on_truncated_high_risk_sources",
        format!(
            "risk={} truncated={} recommendPlan={}",
            preview.risk,
            preview.truncated_high_risk_sources.len(),
            preview.recommend_planning_review
        ),
        errors,
    )
}
