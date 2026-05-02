mod evals;
mod fixtures;

use crate::evals::*;
use crate::fixtures::*;

use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

fn main() {
    let mut results = Vec::new();
    results.extend(run_intent_eval());
    results.push(run_canon_conflict_eval());
    results.push(run_canon_conflict_update_canon_eval());
    results.push(run_canon_conflict_apply_eval());
    results.push(run_story_review_queue_canon_eval());
    results.push(run_multi_ghost_eval());
    results.push(run_feedback_suppression_eval());
    results.push(run_context_budget_eval());
    results.push(run_context_budget_trace_eval());
    results.push(run_context_window_guard_eval());
    results.push(run_compaction_latest_user_anchor_eval());
    results.push(run_tool_permission_guard_eval());
    results.push(run_effective_tool_inventory_eval());
    results.push(run_manual_request_tool_boundary_eval());
    results.push(run_manual_request_kernel_owns_run_loop_eval());
    results.push(run_operation_feedback_requires_durable_save_eval());
    results.push(run_write_operation_lifecycle_trace_eval());
    results.push(run_task_packet_foundation_eval());
    results.push(run_chapter_generation_task_packet_eval());
    results.push(run_result_feedback_tight_budget_eval());
    results.push(run_context_decision_slice_eval());
    results.push(run_story_contract_context_eval());
    results.push(run_foundation_write_validation_eval());
    results.push(run_story_contract_quality_nominal_eval());
    results.push(run_story_contract_vague_excluded_from_context_eval());
    results.push(run_story_contract_quality_chapter_gen_eval());
    results.push(run_story_contract_guard_eval());
    results.push(run_story_contract_negated_guard_eval());
    results.push(run_chapter_mission_result_feedback_eval());
    results.push(run_chapter_mission_partial_progress_eval());
    results.push(run_chapter_mission_guard_eval());
    results.push(run_chapter_mission_negated_guard_eval());
    results.push(run_chapter_mission_save_gap_eval());
    results.push(run_chapter_mission_drifted_no_duplicate_save_gap_eval());
    results.push(run_next_beat_context_eval());
    results.push(run_timeline_contradiction_eval());
    results.push(run_promise_opportunity_eval());
    results.push(run_promise_opportunity_apply_eval());
    results.push(run_promise_stale_eval());
    results.push(run_promise_defer_operation_eval());
    results.push(run_promise_abandon_operation_eval());
    results.push(run_promise_resolve_operation_eval());
    results.push(run_promise_last_seen_context_eval());
    results.push(run_promise_kind_classification_eval());
    results.push(run_story_review_queue_promise_eval());
    results.push(run_story_debt_snapshot_eval());
    results.push(run_story_debt_priority_eval());
    results.push(run_guard_trace_evidence_eval());
    results.push(run_trajectory_export_eval());
    results.push(run_task_packet_trace_eval());
    results.push(run_chapter_generation_task_packet_trace_eval());
    fn run_multi_chapter_scenario_eval() -> EvalResult {
        let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
        memory
            .ensure_story_contract_seed(
                "eval",
                "寒影录",
                "玄幻",
                "刀客追查玉佩真相，在复仇与守护之间做出最终选择。",
                "林墨必须在复仇和守护之间做艰难选择。",
                "不得提前泄露玉佩来源。",
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        let mut errors = Vec::new();

        kernel.active_chapter = Some("Chapter-1".to_string());
        kernel
            .memory
            .add_promise(
                "mystery_clue",
                "玉佩线索",
                "张三拿走了刻有龙纹的玉佩",
                "Chapter-1",
                "Chapter-4",
                5,
            )
            .unwrap();
        let p1 = kernel
            .observe(observation_in_chapter(
                "林墨发现张三留下的玉佩盒子里是空的。",
                "Chapter-1",
            ))
            .unwrap();

        kernel.active_chapter = Some("Chapter-2".to_string());
        let p2 = kernel
            .observe(observation_in_chapter(
                "林墨握紧刀柄，终于决定不再逃避。",
                "Chapter-2",
            ))
            .unwrap();

        kernel.active_chapter = Some("Chapter-3".to_string());
        let p3 = kernel
            .observe(observation_in_chapter(
                "一个戴斗笠的神秘人递给林墨另一块完全相同的玉佩。",
                "Chapter-3",
            ))
            .unwrap();

        kernel.active_chapter = Some("Chapter-4".to_string());
        kernel
            .memory
            .ensure_chapter_mission_seed(
                "eval",
                "Chapter-4",
                "林墨找到玉佩的真正主人。",
                "玉佩主人现身",
                "提前揭开玉佩来源",
                "林墨将玉佩归还主人",
                "eval",
            )
            .unwrap();
        let mut save =
            observation_in_chapter("林墨终于见到了玉佩的真正主人——他的父亲。", "Chapter-4");
        save.reason = ObservationReason::Save;
        save.source = ObservationSource::ChapterSave;
        kernel.observe(save).unwrap();

        kernel.active_chapter = Some("Chapter-5".to_string());
        let p5 = kernel
            .observe(observation_in_chapter(
                "林墨将玉佩挂回父亲的颈上，转身走入风雪。",
                "Chapter-5",
            ))
            .unwrap();
        let debt = kernel.story_debt_snapshot();
        let ledger = kernel.ledger_snapshot();

        let promise_in_context = ledger
            .open_promises
            .iter()
            .any(|p| p.title.contains("玉佩"));
        if !promise_in_context {
            errors.push("promise not tracked in ledger across chapters".to_string());
        }
        if debt.total == 0 {
            errors.push("5-chapter scenario should produce story debt".to_string());
        }
        if p5.is_empty() {
            errors.push("chapter-5 observe produced zero proposals".to_string());
        }

        eval_result(
            "writer_agent:multi_chapter_scenario",
            format!(
                "p1={} p2={} p3={} p5={} debt={} promiseInLedger={} mission={}",
                p1.len(),
                p2.len(),
                p3.len(),
                p5.len(),
                debt.total,
                promise_in_context,
                ledger.active_chapter_mission.is_some()
            ),
            errors,
        )
    }

    results.push(run_multi_chapter_scenario_eval());
    results.push(run_context_recall_tracking_eval());
    results.push(run_character_conflict_flag_eval());
    results.push(run_style_continuity_learning_eval());
    results.push(run_mission_drift_flag_eval());

    let passed = results.iter().filter(|result| result.passed).count();
    let report = EvalReport {
        total: results.len(),
        passed,
        failed: results.len() - passed,
        results,
    };

    println!("=== Writer Agent Eval Report ===");
    println!(
        "Total: {} | Passed: {} | Failed: {}",
        report.total, report.passed, report.failed
    );
    println!();

    for result in &report.results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        println!("[{}] {} ({})", status, result.fixture, result.actual);
        for error in &result.errors {
            println!("  -> {}", error);
        }
    }

    let report_dir = Path::new("reports");
    let _ = std::fs::create_dir_all(report_dir);
    let report_path = report_dir.join("eval_report.json");
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        std::fs::write(&report_path, json).ok();
        println!("\nReport saved to {}", report_path.display());
    }

    if report.failed > 0 {
        std::process::exit(1);
    }
}
