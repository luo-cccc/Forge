use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::{
    build_chapter_generation_receipt, build_chapter_generation_task_packet,
    failure_bundle_from_chapter_error, BuiltChapterContext, ChapterContextBudgetReport,
    ChapterContextSource, ChapterGenerationError, ChapterTarget,
};
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::intent::{AgentBehavior, IntentEngine, WritingIntent};
use agent_writer_lib::writer_agent::kernel::{
    StoryDebtCategory, StoryReviewQueueStatus, WriterAgentApprovalMode, WriterAgentFrontendState,
    WriterAgentRunRequest, WriterAgentStreamMode, WriterAgentTask,
};
use agent_writer_lib::writer_agent::memory::{
    PromiseKind, StoryContractQuality, StoryContractSummary, WriterMemory,
};
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::operation::WriterOperation;
use agent_writer_lib::writer_agent::proposal::{EvidenceSource, ProposalKind, ProposalPriority};
use agent_writer_lib::writer_agent::WriterAgentKernel;

mod anchor_regression;
mod author_voice;
mod belief_conflict;
mod canon;
mod chapter_contract;
mod chapter_settlement;
mod character_state_versioning;
mod chronology_preservation;
mod companion_contract;
mod compiled_input_prompt;
mod context;
mod context_spine;
mod e2e_scenarios;
mod emotional_debt_diagnostics;
mod emotional_debt_extraction;
mod emotional_debt_planner;
mod emotional_debt_todayfive;
mod entity_apply_perf;
mod entity_repair_state;
mod entity_settlement;
mod fact_dedup;
mod false_belief_preservation;
mod feedback_diagnostics;
mod feedback_ghost;
mod feedback_planner;
mod flashback_identity;
mod focus_rebuild;
mod foundation;
mod ghost_feedback;
mod hook_triage;
mod identity_reveal;
mod impact_scoped_recall;
mod input_compiler;
mod inspect_boundary_contract;
mod intent;
mod interrupt_contract;
mod knowledge_visibility;
mod length_telemetry;
mod memory_quality;
mod metacognition;
mod mission;
mod onboarding_contract;
mod planner_fallback;
mod previous_fulltext_gate;
mod product_scenarios;
mod project_brain_knowledge;
mod project_intake;
mod promise;
mod promise_subject;
mod provider_budget;
mod reader_compensation;
mod reader_planner;
mod reader_takeaway;
mod reader_todayfive;
mod relationship_validity;
mod repair_confirm_contract;
mod research_subtask;
mod retrospective_contract;
mod rewrite_impact;
mod risk_prompt_contract;
mod run_loop;
mod run_preflight;
mod save_path_consistency;
mod save_perf;
mod scene_obligation;
mod scene_obligation_diagnostic;
mod scene_result;
mod scene_sequence;
mod settlement_replay;
mod spine_telemetry;
mod stable_prefix_reuse;
mod story_debt;
mod story_impact;
mod story_snapshot_contract;
mod story_time_mapping;
mod strategy_selection;
mod supervised_sprint;
mod task_packet;
mod tiered_memory;
mod timeline_event_order;
mod todayfive_sort_contract;
mod tool_policy;
mod trajectory;
mod trust_stats_contract;
mod typed_context_filter;
mod volume_scope;
mod writing_relevance;
pub use context_spine::*;
pub use reader_compensation::*;
pub use reader_planner::*;
pub use reader_takeaway::*;
pub use reader_todayfive::*;
pub use relationship_validity::*;

pub use anchor_regression::*;
pub use author_voice::*;
pub use belief_conflict::*;
pub use canon::*;
pub use chapter_contract::*;
pub use chapter_settlement::*;
pub use character_state_versioning::*;
pub use chronology_preservation::*;
pub use companion_contract::*;
pub use compiled_input_prompt::*;
pub use context::*;
pub use e2e_scenarios::*;
pub use emotional_debt_diagnostics::*;
pub use emotional_debt_extraction::*;
pub use emotional_debt_planner::*;
pub use emotional_debt_todayfive::*;
pub use entity_apply_perf::*;
pub use entity_repair_state::*;
pub use entity_settlement::*;
pub use fact_dedup::*;
pub use false_belief_preservation::*;
pub use feedback_diagnostics::*;
pub use feedback_ghost::*;
pub use feedback_planner::*;
pub use flashback_identity::*;
pub use focus_rebuild::*;
pub use foundation::*;
pub use ghost_feedback::*;
pub use hook_triage::*;
pub use identity_reveal::*;
pub use impact_scoped_recall::*;
pub use input_compiler::*;
pub use inspect_boundary_contract::*;
pub use intent::*;
pub use interrupt_contract::*;
pub use knowledge_visibility::*;
pub use length_telemetry::*;
pub use memory_quality::*;
pub use metacognition::*;
pub use mission::*;
pub use onboarding_contract::*;
pub use planner_fallback::*;
pub use previous_fulltext_gate::*;
pub use product_scenarios::*;
pub use project_brain_knowledge::*;
pub use project_intake::*;
pub use promise::*;
pub use promise_subject::*;
pub use provider_budget::*;
pub use repair_confirm_contract::*;
pub use research_subtask::*;
pub use retrospective_contract::*;
pub use rewrite_impact::*;
pub use risk_prompt_contract::*;
pub use run_loop::*;
pub use run_preflight::*;
pub use save_path_consistency::*;
pub use save_perf::*;
pub use scene_obligation::*;
pub use scene_obligation_diagnostic::*;
pub use scene_result::*;
pub use scene_sequence::*;
pub use settlement_replay::*;
pub use spine_telemetry::*;
pub use stable_prefix_reuse::*;
pub use story_debt::*;
pub use story_impact::*;
pub use story_snapshot_contract::*;
pub use story_time_mapping::*;
pub use strategy_selection::*;
pub use supervised_sprint::*;
pub use task_packet::*;
pub use tiered_memory::*;
pub use timeline_event_order::*;
pub use todayfive_sort_contract::*;
pub use tool_policy::*;
pub use trajectory::*;
pub use trust_stats_contract::*;
pub use typed_context_filter::*;
pub use volume_scope::*;
pub use writing_relevance::*;

fn eval_llm_message(role: &str, content: &str) -> agent_harness_core::provider::LlmMessage {
    agent_harness_core::provider::LlmMessage {
        role: role.to_string(),
        content: Some(content.to_string()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }
}

struct EvalToolHandler;

#[async_trait::async_trait]
impl agent_harness_core::ToolHandler for EvalToolHandler {
    async fn execute(
        &self,
        tool_name: &str,
        _args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"reachedHandler": true, "tool": tool_name}))
    }
}
