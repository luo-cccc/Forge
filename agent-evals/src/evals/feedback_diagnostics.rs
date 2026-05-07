use crate::fixtures::*;
use std::path::Path;
use agent_writer_lib::writer_agent::diagnostics::author_ignore_rate;
use agent_writer_lib::writer_agent::memory::{MemoryAuditSummary, WriterMemory};

pub fn run_feedback_diagnostics_ignore_rate_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    let mut errors = Vec::new();

    // Scenario A: No feedback data — rate should be 0.0
    let rate_no_data = author_ignore_rate("canon_conflict", &memory);
    if rate_no_data > 0.0 {
        errors.push(format!(
            "expected rate 0.0 without feedback, got {}",
            rate_no_data
        ));
    }

    // Scenario B: 30 entries, 20 ignored ContinuityWarning equivalent
    // We use "story_contract_violation" which maps to continuity checks
    for i in 1..=30 {
        let action = if i <= 20 { "ignored" } else { "accepted" };
        memory
            .record_memory_audit(&MemoryAuditSummary {
                proposal_id: format!("diag-prop-{}", i),
                kind: "story_contract_violation".to_string(),
                action: action.to_string(),
                title: format!("continuity check {}", i),
                evidence: String::new(),
                rationale: String::new(),
                reason: if i <= 20 {
                    Some("author prefers this style".to_string())
                } else {
                    None
                },
                created_at: (2000 + i) as u64,
            })
            .unwrap();
    }

    let rate_high = author_ignore_rate("story_contract_violation", &memory);
    if rate_high <= 0.6 {
        errors.push(format!(
            "expected rate > 0.6 with 20/30 ignored, got {}",
            rate_high
        ));
    }

    // Scenario C: Few entries — rate should be 0.0 (below threshold of 5)
    let memory2 = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory2
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    for i in 1..=3 {
        memory2
            .record_memory_audit(&MemoryAuditSummary {
                proposal_id: format!("few-prop-{}", i),
                kind: "canon_conflict".to_string(),
                action: if i == 1 {
                    "accepted".to_string()
                } else {
                    "ignored".to_string()
                },
                title: format!("few entry {}", i),
                evidence: String::new(),
                rationale: String::new(),
                reason: None,
                created_at: (3000 + i) as u64,
            })
            .unwrap();
    }

    let rate_few = author_ignore_rate("canon_conflict", &memory2);
    if rate_few > 0.0 {
        errors.push(format!(
            "expected rate 0.0 with < 5 entries, got {}",
            rate_few
        ));
    }

    eval_result(
        "writer_agent:feedback_diagnostics_ignore_rate",
        format!("noData={} high={} few={}", rate_no_data, rate_high, rate_few),
        errors,
    )
}
