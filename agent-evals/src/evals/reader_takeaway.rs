#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    ChapterSettlementDelta, ReaderTakeaway,
};
use std::path::Path;

pub fn run_reader_takeaway_eval() -> EvalResult {
    let mut errors = Vec::new();

    // Build a ReaderTakeaway with all 3 fields non-empty
    let takeaway = ReaderTakeaway {
        emotional_beat: "紧张".to_string(),
        expectation: "Chapter-5 揭示真相".to_string(),
        unresolved_lack: "神秘玉牌的下落".to_string(),
    };

    // Verify all 3 fields are non-empty
    if takeaway.emotional_beat.is_empty() {
        errors.push("emotional_beat should not be empty".to_string());
    }
    if takeaway.expectation.is_empty() {
        errors.push("expectation should not be empty".to_string());
    }
    if takeaway.unresolved_lack.is_empty() {
        errors.push("unresolved_lack should not be empty".to_string());
    }

    // Test serialization round-trip
    let json = serde_json::to_string(&takeaway).unwrap();
    let parsed: ReaderTakeaway = serde_json::from_str(&json).unwrap();
    if parsed.emotional_beat != takeaway.emotional_beat {
        errors.push(format!(
            "round-trip emotional_beat mismatch: {} vs {}",
            parsed.emotional_beat, takeaway.emotional_beat
        ));
    }
    if parsed.expectation != takeaway.expectation {
        errors.push(format!(
            "round-trip expectation mismatch: {} vs {}",
            parsed.expectation, takeaway.expectation
        ));
    }
    if parsed.unresolved_lack != takeaway.unresolved_lack {
        errors.push(format!(
            "round-trip unresolved_lack mismatch: {} vs {}",
            parsed.unresolved_lack, takeaway.unresolved_lack
        ));
    }

    // Verify ChapterSettlementDelta can hold a ReaderTakeaway
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-3".to_string(),
        reader_takeaway: Some(takeaway.clone()),
        ..Default::default()
    };
    if delta.reader_takeaway.is_none() {
        errors.push("ChapterSettlementDelta should hold reader_takeaway".to_string());
    }
    let stored = delta.reader_takeaway.as_ref().unwrap();
    if stored.emotional_beat.is_empty()
        || stored.expectation.is_empty()
        || stored.unresolved_lack.is_empty()
    {
        errors.push("reader_takeaway fields should survive in delta".to_string());
    }

    // Test serialization round-trip of the full delta
    let delta_json = serde_json::to_string(&delta).unwrap();
    let delta_parsed: ChapterSettlementDelta = serde_json::from_str(&delta_json).unwrap();
    if delta_parsed.reader_takeaway.is_none() {
        errors.push("reader_takeaway should survive delta round-trip".to_string());
    }

    eval_result(
        "writer_agent:reader_takeaway",
        format!(
            "emotional_beat={} expectation={} unresolved_lack={} roundtrip_ok={}",
            takeaway.emotional_beat,
            takeaway.expectation,
            takeaway.unresolved_lack,
            errors.is_empty()
        ),
        errors,
    )
}
