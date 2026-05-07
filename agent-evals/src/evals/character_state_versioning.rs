#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_character_state_versioning_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();

    let char_id = memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();

    // Create state version at Chapter-1
    memory
        .upsert_character_state(
            char_id,
            "Chapter-1",
            &serde_json::json!(["protect the ring"]),
            &serde_json::json!({"goal": "find truth"}),
            &serde_json::json!({"identity": "wanderer"}),
            &[],
            "settlement:Chapter-1",
        )
        .unwrap();

    // Create state version at Chapter-3 (close prior states before Chapter-3)
    memory
        .close_active_states_for_character(char_id, "Chapter-2")
        .unwrap();
    memory
        .upsert_character_state(
            char_id,
            "Chapter-3",
            &serde_json::json!(["reveal the secret"]),
            &serde_json::json!({"goal": "confront enemy"}),
            &serde_json::json!({"identity": "avenger"}),
            &[],
            "settlement:Chapter-3",
        )
        .unwrap();

    // Chapter-1 query returns first version, Chapter-3 returns second
    let s1 = memory.get_active_state(char_id, "Chapter-1").unwrap();
    let s3 = memory.get_active_state(char_id, "Chapter-3").unwrap();

    if s1.is_none() {
        errors.push("expected active state at Chapter-1".to_string());
    }
    if s3.is_none() {
        errors.push("expected active state at Chapter-3".to_string());
    }

    let v1_chapter = s1
        .as_ref()
        .map(|s| s.valid_from_chapter.as_str())
        .unwrap_or("");
    let v3_chapter = s3
        .as_ref()
        .map(|s| s.valid_from_chapter.as_str())
        .unwrap_or("");

    if v1_chapter != "Chapter-1" {
        errors.push(format!(
            "expected s1 valid_from_chapter=Chapter-1, got={}",
            v1_chapter
        ));
    }
    if v3_chapter != "Chapter-3" {
        errors.push(format!(
            "expected s3 valid_from_chapter=Chapter-3, got={}",
            v3_chapter
        ));
    }

    let versioning_works = errors.is_empty();

    eval_result(
        "writer_agent:character_state_versioning",
        format!("characterStateVersioning={}", versioning_works),
        errors,
    )
}
