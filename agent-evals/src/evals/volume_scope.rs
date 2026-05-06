use crate::fixtures::*;

use agent_writer_lib::writer_agent::memory::{VolumeSummary, WriterMemory};

pub fn run_volume_isolation_context_scope_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .upsert_volume(&VolumeSummary {
            id: "volume-1".to_string(),
            project_id: "eval".to_string(),
            title: "第一卷".to_string(),
            start_chapter: 1,
            end_chapter: 40,
            contract: serde_json::json!({"scope": "setup"}),
            mission: serde_json::json!({"goal": "establish debt"}),
            status: "active".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        })
        .unwrap();
    memory
        .upsert_volume(&VolumeSummary {
            id: "volume-2".to_string(),
            project_id: "eval".to_string(),
            title: "第二卷".to_string(),
            start_chapter: 41,
            end_chapter: 80,
            contract: serde_json::json!({"scope": "payoff"}),
            mission: serde_json::json!({"goal": "escalate debt"}),
            status: "active".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        })
        .unwrap();

    let chapter_12 = memory.find_volume_for_chapter("eval", 12).unwrap();
    let chapter_57 = memory.find_volume_for_chapter("eval", 57).unwrap();

    if chapter_12.as_ref().map(|item| item.id.as_str()) != Some("volume-1") {
        errors.push("chapter 12 should resolve to volume-1".to_string());
    }
    if chapter_57.as_ref().map(|item| item.id.as_str()) != Some("volume-2") {
        errors.push("chapter 57 should resolve to volume-2".to_string());
    }

    eval_result(
        "writer_agent:volume_isolation_context_scope",
        format!(
            "chapter12={:?} chapter57={:?}",
            chapter_12.as_ref().map(|v| v.id.as_str()),
            chapter_57.as_ref().map(|v| v.id.as_str())
        ),
        errors,
    )
}
