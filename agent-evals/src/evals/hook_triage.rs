#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::{PlotPromiseSummary, WriterMemory};
use agent_writer_lib::writer_agent::promise_planner::{
    hook_debt_triage_factor, promise_subject_pressure,
};
use std::path::Path;

pub fn run_hook_triage_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval-hook-triage", "test", "fantasy", "p", "j", "")
        .unwrap();

    // Create a stale promise: last_seen = Chapter-1, current = Chapter-15 (>10 gap)
    let stale_promise = PlotPromiseSummary {
        id: 1,
        kind: "mystery_clue".to_string(),
        title: "远古王座的秘密".to_string(),
        description: "需要揭示远古王座的真相。".to_string(),
        introduced_chapter: "Chapter-1".to_string(),
        last_seen_chapter: "Chapter-1".to_string(),
        last_seen_ref: "test-ref".to_string(),
        expected_payoff: "Chapter-20".to_string(),
        priority: 5,
        risk: "high".to_string(),
        blocked_reason: String::new(),
        status: String::new(),
        promoted: false,
        core: false,
        related_entities: vec![],
    };

    // Stale factor should be > 1.0 (gap of 14 > 10 triggers 1.5x)
    let factor = hook_debt_triage_factor(&stale_promise, "Chapter-15");
    if factor <= 1.0 {
        errors.push(format!(
            "expected stale factor > 1.0 for 14-chapter gap, got {}",
            factor
        ));
    }

    // A fresh promise (last_seen = current) should have factor ~1.0
    let fresh_promise = PlotPromiseSummary {
        id: 2,
        kind: "character_commitment".to_string(),
        title: "守护城池".to_string(),
        description: "守护城池的誓言。".to_string(),
        introduced_chapter: "Chapter-14".to_string(),
        last_seen_chapter: "Chapter-14".to_string(),
        last_seen_ref: "test-ref-2".to_string(),
        expected_payoff: "Chapter-16".to_string(),
        priority: 3,
        risk: "medium".to_string(),
        blocked_reason: String::new(),
        status: String::new(),
        promoted: false,
        core: false,
        related_entities: vec![],
    };

    let fresh_factor = hook_debt_triage_factor(&fresh_promise, "Chapter-14");
    if fresh_factor < 0.9 || fresh_factor > 1.1 {
        errors.push(format!("expected fresh factor ~1.0, got {}", fresh_factor));
    }

    // Verify promise_subject_pressure integrates the factor (just checks it runs)
    // Need a character for the protagonist check
    memory
        .upsert_character("主角", &[], "protagonist", "hero")
        .unwrap();

    let pressure_promise = PlotPromiseSummary {
        id: 3,
        kind: "mystery_clue".to_string(),
        title: "异常冷清".to_string(),
        description: "旧城异常冷清需要解释。".to_string(),
        introduced_chapter: "Chapter-1".to_string(),
        last_seen_chapter: "Chapter-1".to_string(),
        last_seen_ref: "test-ref-3".to_string(),
        expected_payoff: String::new(),
        priority: 5,
        risk: "high".to_string(),
        blocked_reason: String::new(),
        status: String::new(),
        promoted: false,
        core: true,
        related_entities: vec!["character:主角".to_string()],
    };

    let pressure = promise_subject_pressure(&pressure_promise, &memory, "Chapter-15");
    // With core=true, protagonist linked, and stale gap >10, pressure should be significant
    if pressure <= 0.0 {
        errors.push(format!(
            "promise_subject_pressure should be positive, got {}",
            pressure
        ));
    }

    eval_result(
        "writer_agent:hook_triage",
        format!(
            "staleFactor={} freshFactor={} pressure={}",
            factor, fresh_factor, pressure
        ),
        errors,
    )
}
