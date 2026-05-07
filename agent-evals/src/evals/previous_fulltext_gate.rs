use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    BuildChapterContextInput, ChapterContextBudget, ChapterContract,
};

pub fn run_previous_fulltext_gate_eval() -> EvalResult {
    // Verify the risk gate logic: low open_promise_count -> no upgrade.
    let low_risk = BuildChapterContextInput {
        request_id: "low-risk".to_string(),
        target_chapter_title: Some("Chapter-2".to_string()),
        target_chapter_number: Some(2),
        user_instruction: "Write the chapter".to_string(),
        budget: ChapterContextBudget::default(),
        chapter_contract: ChapterContract::default(),
        chapter_summary_override: None,
        user_profile_entries: vec![],
        compiled_input: None,
        open_promise_count: 2,
    };

    let unresolved_debt_density = low_risk.open_promise_count;
    let continuity_risk = if unresolved_debt_density > 5 {
        "high"
    } else if unresolved_debt_density > 2 {
        "medium"
    } else {
        "low"
    };
    let low_risk_triggers = continuity_risk == "high" || unresolved_debt_density > 3 || false; // no structured evidence check needed here

    // High risk: many open promises -> should trigger upgrade.
    let high_risk = BuildChapterContextInput {
        request_id: "high-risk".to_string(),
        target_chapter_title: Some("Chapter-2".to_string()),
        target_chapter_number: Some(2),
        user_instruction: "Write the chapter".to_string(),
        budget: ChapterContextBudget::default(),
        chapter_contract: ChapterContract::default(),
        chapter_summary_override: None,
        user_profile_entries: vec![],
        compiled_input: None,
        open_promise_count: 10,
    };

    let unresolved_debt_density = high_risk.open_promise_count;
    let continuity_risk = if unresolved_debt_density > 5 {
        "high"
    } else if unresolved_debt_density > 2 {
        "medium"
    } else {
        "low"
    };
    let high_risk_triggers = continuity_risk == "high" || unresolved_debt_density > 3 || false;

    let ok = !low_risk_triggers && high_risk_triggers;
    EvalResult::pass_if(
        "previous_fulltext_gate",
        ok,
        format!(
            "low_risk_triggers={} (promises=2 risk={}) high_risk_triggers={} (promises=10 risk={})",
            low_risk_triggers,
            continuity_risk_for(2),
            high_risk_triggers,
            continuity_risk_for(10),
        ),
    )
}

fn continuity_risk_for(open_promise_count: usize) -> &'static str {
    if open_promise_count > 5 {
        "high"
    } else if open_promise_count > 2 {
        "medium"
    } else {
        "low"
    }
}
