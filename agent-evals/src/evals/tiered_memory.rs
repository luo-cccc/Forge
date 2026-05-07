use crate::fixtures::*;

use agent_writer_lib::writer_agent::context::{query_story_os, AgentTask, ContextSource};
use agent_writer_lib::writer_agent::kernel::WriterAgentKernel;
use agent_writer_lib::writer_agent::memory::{
    ArcSnapshotSummary, BookStateSummary, VolumeSnapshotSummary, VolumeSummary, WriterMemory,
};

fn setup_story_os_memory() -> WriterMemory {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .upsert_volume(&VolumeSummary {
            id: "volume-1".to_string(),
            project_id: "eval".to_string(),
            title: "第一卷".to_string(),
            start_chapter: 1,
            end_chapter: 40,
            contract: serde_json::json!({"focus": "setup"}),
            mission: serde_json::json!({"goal": "plant debt"}),
            status: "settled".to_string(),
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
            contract: serde_json::json!({"focus": "payoff"}),
            mission: serde_json::json!({"goal": "collect debts"}),
            status: "active".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        })
        .unwrap();
    memory
        .upsert_volume_snapshot(&VolumeSnapshotSummary {
            project_id: "eval".to_string(),
            volume_id: "volume-1".to_string(),
            snapshot: serde_json::json!({"summary": "第一卷已确认张三与寒影刀旧债相关。"}),
            created_at: String::new(),
        })
        .unwrap();
    memory
        .upsert_arc_snapshot(&ArcSnapshotSummary {
            arc_id: "arc-2a".to_string(),
            project_id: "eval".to_string(),
            volume_id: "volume-2".to_string(),
            title: "第二卷前半弧".to_string(),
            start_chapter: 41,
            end_chapter: 55,
            snapshot: serde_json::json!({"summary": "林墨必须在救张三和追账册之间做选择。"}),
            created_at: String::new(),
            updated_at: String::new(),
        })
        .unwrap();
    memory
        .upsert_book_state(&BookStateSummary {
            project_id: "eval".to_string(),
            title: "镜中墟".to_string(),
            long_term_constraints: vec!["寒影刀旧债不能被自动抹平".to_string()],
            mega_promises: vec!["封门真相必须逐卷逼近".to_string()],
            irreversible_changes: vec!["张三已暴露自己也是债务人".to_string()],
            source_ref: "eval".to_string(),
            updated_at: String::new(),
        })
        .unwrap();
    memory
}

pub fn run_tiered_memory_cold_tier_boundary_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = setup_story_os_memory();
    let obs = observation_in_chapter("林墨握着寒影刀，准备追问张三。", "Chapter-47");
    let pack = query_story_os(AgentTask::ChapterGeneration, &obs, &memory, 20_000);
    if pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::RagExcerpt)
    {
        errors.push("cold tier should not appear without explicit recall signal".to_string());
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::BookState)
    {
        errors.push("book state should be present before any cold-tier source".to_string());
    }
    eval_result(
        "writer_agent:tiered_memory_cold_tier_boundary",
        format!(
            "sources={:?}",
            pack.sources
                .iter()
                .map(|source| format!("{:?}", source.source))
                .collect::<Vec<_>>()
        ),
        errors,
    )
}

pub fn run_tiered_memory_cross_volume_promotion_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = setup_story_os_memory();
    memory
        .add_promise(
            "plot_promise",
            "旧债来源",
            "第一卷埋下的旧债要在第二卷继续逼近。",
            "Chapter-20",
            "Chapter-47",
            5,
        )
        .unwrap();
    let obs = observation_in_chapter("林墨意识到旧债和账册必须在这一章继续推进。", "Chapter-47");
    let pack = query_story_os(AgentTask::ChapterGeneration, &obs, &memory, 20_000);
    let has_volume_snapshot = pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::VolumeSnapshot);
    let has_promise = pack.sources.iter().any(|source| {
        source.source == ContextSource::PromiseSlice && source.content.contains("旧债来源")
    });
    if !has_volume_snapshot {
        errors.push("previous volume snapshot should be promoted into warm tier".to_string());
    }
    if !has_promise {
        errors.push("cross-volume promise should still surface in promise slice".to_string());
    }
    eval_result(
        "writer_agent:tiered_memory_cross_volume_promotion",
        format!(
            "volumeSnapshot={} promise={}",
            has_volume_snapshot, has_promise
        ),
        errors,
    )
}

pub fn run_arc_snapshot_available_in_warm_tier_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = setup_story_os_memory();
    let obs = observation_in_chapter("林墨必须在这一章做选择。", "Chapter-47");
    let pack = query_story_os(AgentTask::PlanningReview, &obs, &memory, 6_000);
    let arc = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::ArcSnapshot);
    if arc.is_none() {
        errors.push("arc snapshot missing from warm tier".to_string());
    }
    eval_result(
        "writer_agent:arc_snapshot_available_in_warm_tier",
        format!(
            "arcIncluded={} chars={}",
            arc.is_some(),
            arc.map(|a| a.char_count).unwrap_or(0)
        ),
        errors,
    )
}

pub fn run_book_state_present_without_cold_recall_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = setup_story_os_memory();
    let obs = observation_in_chapter("林墨知道封门真相不能一次说破。", "Chapter-47");
    let pack = query_story_os(AgentTask::GhostWriting, &obs, &memory, 3_000);
    let has_book_state = pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::BookState);
    let has_rag = pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::RagExcerpt);
    if !has_book_state {
        errors.push("book state should be available in project-stable tier".to_string());
    }
    if has_rag {
        errors.push("rag excerpt should not appear without explicit cold-tier recall".to_string());
    }
    eval_result(
        "writer_agent:book_state_present_without_cold_recall",
        format!("bookState={} rag={}", has_book_state, has_rag),
        errors,
    )
}

pub fn run_incremental_update_bounded_entries_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = setup_story_os_memory();
    for i in 1..=80 {
        memory
            .add_promise(
                "plot_promise",
                &format!("Promise-{i}"),
                "大量开放承诺",
                &format!("Chapter-{i}"),
                &format!("Chapter-{}", i + 5),
                3,
            )
            .unwrap();
        memory
            .upsert_character(
                &format!("角色{i}"),
                &[],
                "protagonist",
                "大量实体",
            )
            .unwrap();
    }
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-47".to_string());
    let ledger = kernel.ledger_snapshot();
    if ledger.open_promises.len() > 50 {
        errors.push(format!(
            "open promises should be bounded, got {}",
            ledger.open_promises.len()
        ));
    }
    if ledger.canon_entities.len() > 50 {
        errors.push(format!(
            "canon entities should be bounded, got {}",
            ledger.canon_entities.len()
        ));
    }
    eval_result(
        "writer_agent:incremental_update_bounded_entries",
        format!(
            "promises={} canon={} results={}",
            ledger.open_promises.len(),
            ledger.canon_entities.len(),
            ledger.recent_chapter_results.len()
        ),
        errors,
    )
}

pub fn run_ledger_snapshot_tiered_latency_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = setup_story_os_memory();
    for i in 1..=120 {
        memory
            .add_promise(
                "plot_promise",
                &format!("Promise-{i}"),
                "压测 promise",
                &format!("Chapter-{i}"),
                &format!("Chapter-{}", i + 3),
                2,
            )
            .unwrap();
    }
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-47".to_string());
    let started = std::time::Instant::now();
    let ledger = kernel.ledger_snapshot();
    let latency_ms = started.elapsed().as_millis();
    if latency_ms > 500 {
        errors.push(format!("ledger snapshot too slow: {}ms", latency_ms));
    }
    if ledger.active_volume.as_ref().map(|v| v.id.as_str()) != Some("volume-2") {
        errors.push("active volume should resolve for current chapter".to_string());
    }
    eval_result(
        "writer_agent:ledger_snapshot_tiered_latency",
        format!(
            "latencyMs={} activeVolume={:?} arcSnapshots={} volumeSnapshots={}",
            latency_ms,
            ledger.active_volume.as_ref().map(|v| v.id.as_str()),
            ledger.arc_snapshots.len(),
            ledger.volume_snapshots.len()
        ),
        errors,
    )
}
