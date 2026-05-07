use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_burnout_signal_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed 5 recent chapter results. list_recent_chapter_results returns DESC by created_at.
    // The burnout check does results.iter().rev().take(2) which examines the 2 oldest (lowest created_at).
    // Make the 2 lowest-created_at entries have short summaries.
    for i in 1..=5 {
        memory
            .record_chapter_result(&ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: format!("第{}章", i),
                chapter_revision: "rev-1".to_string(),
                summary: if i <= 2 {
                    "短摘要".to_string()
                } else {
                    "这是一个比较长的章节摘要，包含了更多的细节和内容描述，用于确保摘要长度超过五十个字的阈值。".to_string()
                },
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec![],
                promise_updates: vec![],
                canon_updates: vec![],
                source_ref: "test".to_string(),
                created_at: (1000 + i) as u64,
            })
            .unwrap();
    }

    let kernel = WriterAgentKernel::new("eval", memory);
    let summary = kernel.today_five_summary();

    let guard = summary.items.iter().find(|i| i.slot == "guard");
    if guard.is_none() {
        errors.push("missing guard slot in TodayFive".to_string());
        return eval_result(
            "writer_agent:burnout_signal",
            "no guard slot".to_string(),
            errors,
        );
    }
    let guard_item = guard.unwrap();

    let has_burnout = guard_item.detail.contains("💡")
        && guard_item.detail.contains("疲劳")
        && guard_item.detail.contains("摘要较短");
    if !has_burnout {
        errors.push(format!(
            "guard detail should contain burnout signal when recent summaries are short, got: {}",
            guard_item.detail
        ));
    }

    eval_result(
        "writer_agent:burnout_signal",
        format!(
            "guard_detail_len={} has_burnout={}",
            guard_item.detail.len(),
            has_burnout
        ),
        errors,
    )
}
