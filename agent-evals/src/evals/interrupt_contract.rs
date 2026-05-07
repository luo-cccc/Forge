use crate::fixtures::*;
use agent_writer_lib::writer_agent::metacognition::{
    metacognitive_write_gate_reason, WriterMetacognitiveAction, WriterMetacognitiveRiskLevel,
    WriterMetacognitiveSnapshot,
};

pub fn run_interrupt_contract_eval() -> EvalResult {
    let has_high = matches!(
        WriterMetacognitiveRiskLevel::High,
        WriterMetacognitiveRiskLevel::High
    );
    let has_medium = matches!(
        WriterMetacognitiveRiskLevel::Medium,
        WriterMetacognitiveRiskLevel::Medium
    );
    let has_low = matches!(
        WriterMetacognitiveRiskLevel::Low,
        WriterMetacognitiveRiskLevel::Low
    );
    let variants_ok = has_high && has_medium && has_low;

    let high_risk_snapshot = WriterMetacognitiveSnapshot {
        risk_level: WriterMetacognitiveRiskLevel::High,
        recommended_action: WriterMetacognitiveAction::BlockWriteUntilAuthorConfirms,
        confidence: 0.3,
        summary: "Multiple recent failures indicate unsafe write conditions.".to_string(),
        reasons: vec!["3 post-write errors in last 5 saves".to_string()],
        remediation: vec!["Run continuity diagnostic before next write.".to_string()],
        context_coverage_rate: 0.4,
        context_truncated_source_count: 5,
        context_dropped_source_count: 2,
        recent_failure_count: 3,
        post_write_error_count: 2,
        low_confidence_proposal_count: 2,
        ignored_repeated_suggestion_rate: 0.1,
    };
    let blocked = metacognitive_write_gate_reason(&high_risk_snapshot).is_some();

    let low_risk_snapshot = WriterMetacognitiveSnapshot {
        risk_level: WriterMetacognitiveRiskLevel::Low,
        recommended_action: WriterMetacognitiveAction::Proceed,
        ..Default::default()
    };
    let not_blocked = metacognitive_write_gate_reason(&low_risk_snapshot).is_none();

    let ok = variants_ok && blocked && not_blocked;
    EvalResult::pass_if(
        "writer_agent:interrupt_vs_silent_contract",
        ok,
        format!(
            "variants={} blocked={} notBlocked={}",
            variants_ok, blocked, not_blocked
        ),
    )
}
