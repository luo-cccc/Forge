#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::chapter_settlement::build_chapter_settlement_queue;
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_chapter_settlement_creates_reviewable_updates_eval() -> EvalResult {
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
            0.4,
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
    let queue = build_chapter_settlement_queue("Chapter-3", "rev-1", &memory, "eval");
    if queue.canon_updates.is_empty() && queue.promise_updates.is_empty() {
        errors.push("should have at least some reviewable updates".to_string());
    }
    eval_result(
        "writer_agent:chapter_settlement_creates_reviewable_updates",
        format!(
            "canon={} promise={} mission={} highPri={}",
            queue.canon_updates.len(),
            queue.promise_updates.len(),
            queue.mission_suggestions.len(),
            queue.high_priority_count
        ),
        errors,
    )
}

pub fn run_chapter_settlement_requires_approval_for_ledger_writes_eval() -> EvalResult {
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
            0.4,
        )
        .ok();
    let queue = build_chapter_settlement_queue("Chapter-3", "rev-1", &memory, "eval");
    for item in queue
        .canon_updates
        .iter()
        .chain(queue.promise_updates.iter())
    {
        if !item.requires_approval {
            errors.push(format!("{} should require approval", item.id));
        }
    }
    eval_result(
        "writer_agent:chapter_settlement_requires_approval_for_ledger_writes",
        format!(
            "approvalItems={}",
            queue.canon_updates.len() + queue.promise_updates.len()
        ),
        errors,
    )
}

pub fn run_chapter_settlement_prioritizes_high_risk_promises_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "霜铃塔钥",
            "钥匙下落不明",
            "Chapter-1",
            "Chapter-10",
            5,
        )
        .unwrap();
    let queue = build_chapter_settlement_queue("Chapter-3", "rev-1", &memory, "eval");
    if queue.high_priority_count == 0 && !queue.promise_updates.is_empty() {
        errors.push("high-risk promises should be prioritized".to_string());
    }
    eval_result(
        "writer_agent:chapter_settlement_prioritizes_high_risk_promises",
        format!(
            "highPri={} promiseUpdates={}",
            queue.high_priority_count,
            queue.promise_updates.len()
        ),
        errors,
    )
}
