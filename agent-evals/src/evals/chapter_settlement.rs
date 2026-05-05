#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::chapter_settlement::{
    build_chapter_settlement_queue, build_chapter_settlement_queue_with_evidence,
    ChapterSettlementEvidence,
};
use agent_writer_lib::writer_agent::diagnostics::{DiagnosticCategory, DiagnosticSeverity};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::TextRange;
use agent_writer_lib::writer_agent::post_write_diagnostics::{
    WriterPostWriteDiagnosticItem, WriterPostWriteDiagnosticReport,
};

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
    let report = WriterPostWriteDiagnosticReport {
        observation_id: "obs-save-1".to_string(),
        chapter_title: Some("Chapter-3".to_string()),
        chapter_revision: Some("rev-1".to_string()),
        total_count: 1,
        error_count: 1,
        warning_count: 0,
        info_count: 0,
        categories: vec![],
        diagnostics: vec![WriterPostWriteDiagnosticItem {
            diagnostic_id: "diag-canon-1".to_string(),
            severity: DiagnosticSeverity::Error,
            category: DiagnosticCategory::CanonConflict,
            message: "Canon conflict after save".to_string(),
            target: TextRange { from: 0, to: 5 },
            evidence_refs: vec!["canon:林墨".to_string(), "revision:rev-1".to_string()],
            fix_suggestion: Some("Review the saved revision against Canon.".to_string()),
            operation_count: 1,
        }],
        source_refs: vec![
            "observation:obs-save-1".to_string(),
            "revision:rev-1".to_string(),
        ],
        remediation: vec!["Review post-write diagnostics.".to_string()],
        created_at_ms: 100,
    };
    let impact_sources = vec!["story_impact:canon:林墨".to_string()];
    let queue = build_chapter_settlement_queue_with_evidence(
        "Chapter-3",
        "rev-1",
        &memory,
        "eval",
        ChapterSettlementEvidence {
            saved_chapter_text: Some("林墨拔剑，和旧 canon 冲突。"),
            post_write_diagnostics: Some(&report),
            story_impact_risk: Some("High"),
            story_impact_sources: &impact_sources,
        },
    );
    if queue.canon_updates.is_empty() && queue.promise_updates.is_empty() {
        errors.push("should have at least some reviewable updates".to_string());
    }
    if !queue
        .evidence_refs
        .iter()
        .any(|source| source.contains("obs-save-1"))
    {
        errors.push("settlement should retain post-write diagnostic evidence refs".to_string());
    }
    if queue.continuity_risks.is_empty() {
        errors.push(
            "high story impact or blocking diagnostics should create continuity risk".to_string(),
        );
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
