use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::{MemoryAuditSummary, WriterMemory};
use agent_writer_lib::writer_agent::promise_planner::promise_kind_rejection_penalty;
use std::path::Path;

pub fn run_feedback_planner_rejection_penalty_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    // Add 3 plot_promise promises
    for i in 1..=3 {
        memory
            .add_promise(
                "plot_promise",
                &format!("test promise {}", i),
                &format!("description {}", i),
                "Chapter-1",
                "Chapter-5",
                4,
            )
            .unwrap();
    }

    // Scenario A: No feedback data — penalty should be 1.0
    let penalty_no_data = promise_kind_rejection_penalty("plot_promise", &memory);
    let mut errors = Vec::new();
    if (penalty_no_data - 1.0).abs() > 1e-9 {
        errors.push(format!(
            "expected penalty 1.0 without feedback, got {}",
            penalty_no_data
        ));
    }

    // Scenario B: Simulate 5 audit entries, 3 rejected — rejection rate 60% > 50%
    for i in 1..=5 {
        let action = if i <= 3 { "rejected" } else { "accepted" };
        memory
            .record_memory_audit(&MemoryAuditSummary {
                proposal_id: format!("prop-{}", i),
                kind: "plot_promise".to_string(),
                action: action.to_string(),
                title: format!("promise {}", i),
                evidence: String::new(),
                rationale: String::new(),
                reason: if i <= 3 {
                    Some("too early".to_string())
                } else {
                    None
                },
                created_at: (1000 + i) as u64,
            })
            .unwrap();
    }

    let penalty_rejected = promise_kind_rejection_penalty("plot_promise", &memory);
    if penalty_rejected >= 1.0 {
        errors.push(format!(
            "expected penalty < 1.0 when rejection rate > 50%, got {}",
            penalty_rejected
        ));
    }
    // Should be 0.7 when >50% rejection
    if (penalty_rejected - 0.7).abs() > 1e-9 {
        errors.push(format!("expected penalty 0.7, got {}", penalty_rejected));
    }

    // Scenario C: Different kind not affected
    let penalty_other = promise_kind_rejection_penalty("emotional_debt", &memory);
    if (penalty_other - 1.0).abs() > 1e-9 {
        errors.push(format!(
            "expected penalty 1.0 for unrelated kind, got {}",
            penalty_other
        ));
    }

    eval_result(
        "writer_agent:feedback_planner_rejection_penalty",
        format!(
            "noData={} rejected={} other={}",
            penalty_no_data, penalty_rejected, penalty_other
        ),
        errors,
    )
}
