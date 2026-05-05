#![allow(unused_imports)]
use crate::fixtures::*;

use agent_writer_lib::writer_agent::supervised_sprint::{
    advance_sprint, can_advance_to_next_chapter, create_sprint_plan, sprint_progress,
};

pub fn run_supervised_sprint_stops_before_unapproved_save_eval() -> EvalResult {
    let mut errors = Vec::new();
    let mut sprint = create_sprint_plan("s1", &["Ch1".to_string(), "Ch2".to_string()], true);
    sprint.chapters[0].receipt_id = Some("r1".to_string());
    sprint.chapters[0].preflight_readiness = Some("ready".to_string());
    sprint.chapters[0].status = "drafting".to_string(); // NOT saved — author hasn't approved
    if can_advance_to_next_chapter(&sprint) {
        errors.push("should stop before unapproved save when approval is required".to_string());
    }
    eval_result(
        "writer_agent:supervised_sprint_stops_before_unapproved_save",
        format!("canAdvance={}", can_advance_to_next_chapter(&sprint)),
        errors,
    )
}

pub fn run_supervised_sprint_carries_forward_settlement_feedback_eval() -> EvalResult {
    let mut errors = Vec::new();
    let mut sprint = create_sprint_plan("s2", &["Ch1".to_string(), "Ch2".to_string()], false);
    sprint.chapters[0].receipt_id = Some("r1".to_string());
    sprint.chapters[0].preflight_readiness = Some("ready".to_string());
    sprint.chapters[0].status = "settled".to_string();
    let next = advance_sprint(&mut sprint);
    if next.is_none() {
        errors.push("should advance to Ch2".to_string());
    }
    if sprint.chapters[1].status != "preflight" {
        errors.push("next chapter should be in preflight".to_string());
    }
    let progress = sprint_progress(&sprint);
    if progress.settlements_completed == 0 {
        errors.push("should record settlements completed".to_string());
    }
    eval_result(
        "writer_agent:supervised_sprint_carries_forward_settlement_feedback",
        format!(
            "next={:?} status={} settlements={}",
            next, sprint.status, progress.settlements_completed
        ),
        errors,
    )
}

pub fn run_supervised_sprint_records_receipts_per_chapter_eval() -> EvalResult {
    let mut errors = Vec::new();
    let mut sprint = create_sprint_plan(
        "s3",
        &["Ch1".to_string(), "Ch2".to_string(), "Ch3".to_string()],
        false,
    );
    sprint.chapters[0].receipt_id = Some("receipt-1".to_string());
    sprint.chapters[0].preflight_readiness = Some("ready".to_string());
    sprint.chapters[0].status = "saved".to_string();
    sprint.chapters[1].receipt_id = Some("receipt-2".to_string());
    sprint.chapters[1].preflight_readiness = Some("ready".to_string());
    sprint.chapters[1].status = "saved".to_string();
    let progress = sprint_progress(&sprint);
    if progress.receipts_recorded != 2 {
        errors.push(format!(
            "should record 2 receipts, got {}",
            progress.receipts_recorded
        ));
    }
    eval_result(
        "writer_agent:supervised_sprint_records_receipts_per_chapter",
        format!(
            "receipts={} completed={}",
            progress.receipts_recorded, progress.chapters_completed
        ),
        errors,
    )
}
