#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_emotional_debt_diagnostics_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "test",
            "fantasy",
            "A hero must choose between duty and love.",
            "The hero faces a moral dilemma.",
            "",
        )
        .unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();

    // Create a paragraph with emotional pressure keyword but no resolution
    let paragraph = "林墨看着眼前的一切，心中充满了愤怒与不甘。他握紧拳头，指甲深深嵌入掌心。";

    let engine = DiagnosticsEngine::new();
    let results = engine.diagnose(paragraph, 0, "Chapter-3", "eval", &memory);

    // Verify at least one result with source="emotional_debt" exists
    let emotional_results: Vec<_> = results
        .iter()
        .filter(|r| r.evidence.iter().any(|e| e.source == "emotional_debt"))
        .collect();

    if emotional_results.is_empty() {
        errors.push(format!(
            "expected at least one emotional_debt diagnostic result, got {} total results: {:?}",
            results.len(),
            results.iter().map(|r| &r.message).collect::<Vec<_>>()
        ));
    } else {
        // Verify the message contains the expected content
        let msg = &emotional_results[0].message;
        if !msg.contains("情绪压力") && !msg.contains("释放") && !msg.contains("解决") {
            errors.push(format!(
                "emotional_debt diagnostic message should mention emotional pressure or resolution, got: {}",
                msg
            ));
        }
    }

    // Verify that a paragraph with resolution does NOT produce the diagnostic
    let paragraph_with_resolution =
        "林墨深吸一口气，决定放下过去的愤怒。他原谅了那个曾背叛他的人。";
    let results_resolved =
        engine.diagnose(paragraph_with_resolution, 0, "Chapter-3", "eval", &memory);
    let emotional_resolved: Vec<_> = results_resolved
        .iter()
        .filter(|r| r.evidence.iter().any(|e| e.source == "emotional_debt"))
        .collect();
    if !emotional_resolved.is_empty() {
        errors.push(format!(
            "paragraph with resolution should not trigger emotional_debt diagnostic, got: {:?}",
            emotional_resolved
                .iter()
                .map(|r| &r.message)
                .collect::<Vec<_>>()
        ));
    }

    eval_result(
        "writer_agent:emotional_debt_diagnostics",
        format!(
            "pressure_detected={} resolution_skipped={} total_results={}",
            emotional_results.len(),
            results_resolved.len(),
            results.len()
        ),
        errors,
    )
}
