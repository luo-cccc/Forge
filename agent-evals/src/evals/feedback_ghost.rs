use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_feedback_ghost_style_preference_roundtrip_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    let mut errors = Vec::new();

    // Store a style preference (accepted)
    memory
        .upsert_style_preference("sentence_length", "short_punchy", true)
        .unwrap();

    // Store another preference (rejected)
    memory
        .upsert_style_preference("tense", "present_tense", false)
        .unwrap();

    // Read back preferences
    let prefs = memory.list_style_preferences(10).unwrap();
    if prefs.len() < 2 {
        errors.push(format!(
            "expected at least 2 style preferences, got {}",
            prefs.len()
        ));
    }

    // Verify accepted preference round-trip
    let accepted = prefs.iter().find(|p| p.key == "sentence_length");
    match accepted {
        Some(p) => {
            if p.accepted_count < 1 {
                errors.push("accepted_count should be >= 1".to_string());
            }
            if !p.value.contains("short_punchy") {
                errors.push(format!(
                    "expected value to contain 'short_punchy', got '{}'",
                    p.value
                ));
            }
        }
        None => errors.push("missing sentence_length preference".to_string()),
    }

    // Verify rejected preference exists
    let rejected = prefs.iter().find(|p| p.key == "tense");
    match rejected {
        Some(p) => {
            if p.rejected_count < 1 {
                errors.push("rejected_count should be >= 1".to_string());
            }
        }
        None => errors.push("missing tense preference".to_string()),
    }

    // Test upsert updates count on duplicated key
    memory
        .upsert_style_preference("sentence_length", "medium_varied", true)
        .unwrap();
    let prefs2 = memory.list_style_preferences(10).unwrap();
    let updated = prefs2.iter().find(|p| p.key == "sentence_length");
    match updated {
        Some(p) => {
            if p.accepted_count < 2 {
                errors.push(format!(
                    "accepted_count should be >= 2 after duplicate upsert, got {}",
                    p.accepted_count
                ));
            }
        }
        None => errors.push("missing sentence_length after update".to_string()),
    }

    eval_result(
        "writer_agent:feedback_ghost_style_preference_roundtrip",
        format!("prefs={}", prefs.len()),
        errors,
    )
}
