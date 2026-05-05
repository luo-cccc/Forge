#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::writer_agent::author_voice::{
    build_author_voice_snapshot, compute_style_drift,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_author_voice_guard_uses_author_samples_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    // Add a style preference as an author sample
    memory
        .upsert_style_preference("sentence_length", "短句", false)
        .ok();
    memory
        .record_feedback("p1", "accepted", "matches author voice", "")
        .ok();
    let voice = build_author_voice_snapshot(&memory, &["Chapter-1".to_string()], 100);
    if voice.sample_refs.is_empty() {
        errors.push("should have sample refs from style prefs".to_string());
    }
    eval_result(
        "writer_agent:author_voice_guard_uses_author_samples",
        format!(
            "voiceId={} samples={} confidence={}",
            voice.voice_id,
            voice.sample_refs.len(),
            voice.confidence
        ),
        errors,
    )
}

pub fn run_author_voice_guard_records_feedback_corrections_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .record_memory_feedback(
            &agent_writer_lib::writer_agent::memory::MemoryFeedbackSummary {
                slot: "style|prose|voice".to_string(),
                category: "style".to_string(),
                action: "correction".to_string(),
                confidence_delta: -0.3,
                source_error: Some("too formal".to_string()),
                proposal_id: "p1".to_string(),
                reason: Some("author prefers colloquial tone".to_string()),
                created_at: 100,
            },
        )
        .ok();
    let voice = build_author_voice_snapshot(&memory, &[], 100);
    let has_correction = voice.taboo_phrases.iter().any(|p| p.contains("correction"));
    if !has_correction {
        errors.push("should record correction signals in taboo phrases".to_string());
    }
    eval_result(
        "writer_agent:author_voice_guard_records_feedback_corrections",
        format!(
            "taboos={} confidence={}",
            voice.taboo_phrases.len(),
            voice.confidence
        ),
        errors,
    )
}

pub fn run_style_drift_diagnostic_links_evidence_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();
    memory
        .upsert_style_preference("literary_prose", "文学表达", false)
        .ok();
    let voice = build_author_voice_snapshot(&memory, &["Chapter-1".to_string()], 100);
    let drift = compute_style_drift(
        &voice,
        "这是一个极长极长极长极长极长极长极长极长极长极长极长极长极长极长极长极长极长极长的句子。",
        "Chapter-2",
    );
    if drift.evidence_links.is_empty() {
        errors.push("drift diagnostic should link evidence".to_string());
    }
    if !drift
        .drift_signals
        .iter()
        .any(|signal| signal.aspect == "rhythm")
    {
        errors.push("drift diagnostic should compare chapter text rhythm".to_string());
    }
    eval_result(
        "writer_agent:style_drift_diagnostic_links_evidence",
        format!(
            "driftSignals={} severity={} evidence={}",
            drift.drift_signals.len(),
            drift.overall_severity,
            drift.evidence_links.len()
        ),
        errors,
    )
}
