mod evals;
mod evals_extra;
mod evals_extra2;
mod fixtures;
mod product_scenarios;

use crate::evals::*;
use crate::evals_extra::*;
use crate::evals_extra2::*;
use crate::fixtures::*;
use crate::product_scenarios::*;

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
    results.push(run_context_source_trend_eval());
    results.push(run_context_window_guard_eval());
    results.push(run_compaction_latest_user_anchor_eval());
    results.push(run_tool_permission_guard_eval());
    results.push(run_effective_tool_inventory_eval());
    results.push(run_manual_request_tool_boundary_eval());
    results.push(run_manual_request_kernel_owns_run_loop_eval());
    results.push(run_operation_feedback_requires_durable_save_eval());
    results.push(run_write_operation_lifecycle_trace_eval());
    results.push(run_product_metrics_trace_eval());
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
    results.extend(run_product_scenario_evals());
    results.push(run_context_recall_tracking_eval());
    results.push(run_character_conflict_flag_eval());
    results.push(run_style_continuity_learning_eval());
    results.push(run_mission_drift_flag_eval());
    results.push(run_ghost_quality_confidence_eval());
    results.push(run_promise_object_cross_chapter_tracking_eval());
    results.push(run_canon_false_positive_suppression_eval());
    results.push(run_context_mandatory_sources_survive_tight_budget_eval());
    results.push(run_story_debt_priority_ordering_eval());
    results.push(run_promise_kind_extraction_from_text_eval());
    results.push(run_memory_candidate_quality_validation_eval());
    results.push(run_style_memory_validation_eval());
    results.push(run_promise_related_entities_extraction_eval());
    results.push(run_promise_dedup_against_existing_eval());
    results.push(run_same_entity_attribute_merge_eval());
    results.push(run_vague_memory_candidate_rejected_eval());
    results.push(run_duplicate_memory_candidate_deduped_eval());
    results.push(run_conflicting_memory_candidate_requires_review_eval());
    results.push(run_context_pack_explainability_eval());
    results.push(run_current_plot_relevance_prioritizes_same_name_entity_eval());
    results.push(run_promise_relevance_beats_plain_similarity_eval());
    results.push(run_project_brain_writing_relevance_rerank_eval());
    results.push(run_scene_type_relevance_signal_eval());
    results.push(run_project_brain_uses_writer_memory_focus_eval());
    results.push(run_project_brain_long_session_candidate_recall_eval());
    results.push(run_project_brain_avoid_terms_preserve_payoff_eval());
    results.push(run_project_brain_must_not_boundary_eval());
    results.push(run_project_brain_author_fixture_rerank_eval());
    results.push(run_end_to_end_ghost_pipeline_eval());
    results.push(run_end_to_end_contract_guard_eval());
    results.push(run_end_to_end_mission_drift_detection_eval());
    results.push(run_trajectory_product_metrics_present_eval());
    results.push(run_ghost_task_packet_foundation_coverage_eval());

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
