use crate::fixtures::*;
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::memory::{
    ChapterMissionSummary, ChapterResultSummary, WriterMemory,
};
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::proposal::{EvidenceSource, ProposalKind};
use agent_writer_lib::writer_agent::task_receipt::{
    WriterFailureCategory, WriterFailureEvidenceBundle,
};
use agent_writer_lib::writer_agent::trajectory::export_trace_snapshot;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_product_scenario_evals() -> Vec<EvalResult> {
    vec![
        run_multi_chapter_scenario_eval(),
        run_scenario_chapter_save_feedback_handoff_eval(),
        run_scenario_promise_payoff_nearby_eval(),
        run_scenario_resolved_promise_stays_quiet_eval(),
        run_scenario_object_whereabouts_context_priority_eval(),
        run_scenario_mission_drift_save_eval(),
        run_scenario_canon_conflict_no_autowrite_eval(),
        run_scenario_style_feedback_affects_ghost_context_eval(),
        run_scenario_manual_ask_records_decision_eval(),
        run_scenario_context_explainability_for_longform_eval(),
        run_continuous_writing_fixture_20_chapters_eval(),
        run_real_author_long_session_calibration_eval(),
    ]
}

include!("product_scenarios/part_a.in.rs");
include!("product_scenarios/part_b.in.rs");
include!("product_scenarios/part_c.in.rs");
