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

mod author_voice;
mod belief_conflict;
mod canon;
mod chapter_contract;
mod chapter_settlement;
mod character_state_versioning;
mod chronology_preservation;
mod context;
mod context_spine;
mod e2e_scenarios;
mod entity_settlement;
mod false_belief_preservation;
mod flashback_identity;
mod foundation;
mod ghost_feedback;
mod identity_reveal;
mod intent;
mod knowledge_visibility;
mod memory_quality;
mod metacognition;
mod mission;
mod product_scenarios;
mod project_brain_knowledge;
mod project_intake;
mod promise;
mod promise_subject;
mod provider_budget;
mod reader_compensation;
mod planner_fallback;
mod relationship_validity;
mod research_subtask;
mod rewrite_impact;
mod run_loop;
mod run_preflight;
mod save_path_consistency;
mod scene_obligation;
mod scene_obligation_diagnostic;
mod scene_result;
mod scene_sequence;
mod settlement_replay;
mod story_debt;
mod story_impact;
mod story_time_mapping;
mod supervised_sprint;
mod task_packet;
mod tiered_memory;
mod timeline_event_order;
mod typed_context_filter;
mod tool_policy;
mod trajectory;
mod volume_scope;
mod writing_relevance;
pub use context_spine::*;
pub use reader_compensation::*;
pub use relationship_validity::*;

pub use author_voice::*;
pub use belief_conflict::*;
pub use canon::*;
pub use chapter_contract::*;
pub use chapter_settlement::*;
pub use character_state_versioning::*;
pub use chronology_preservation::*;
pub use context::*;
pub use e2e_scenarios::*;
pub use entity_settlement::*;
pub use false_belief_preservation::*;
pub use flashback_identity::*;
pub use foundation::*;
pub use ghost_feedback::*;
pub use identity_reveal::*;
pub use intent::*;
pub use knowledge_visibility::*;
pub use memory_quality::*;
pub use metacognition::*;
pub use mission::*;
pub use product_scenarios::*;
pub use project_brain_knowledge::*;
pub use planner_fallback::*;
pub use project_intake::*;
pub use promise::*;
pub use promise_subject::*;
pub use provider_budget::*;
pub use research_subtask::*;
pub use rewrite_impact::*;
pub use run_loop::*;
pub use run_preflight::*;
pub use save_path_consistency::*;
pub use scene_obligation_diagnostic::*;
pub use scene_obligation::*;
pub use scene_result::*;
pub use scene_sequence::*;
pub use settlement_replay::*;
pub use story_debt::*;
pub use story_impact::*;
pub use story_time_mapping::*;
pub use supervised_sprint::*;
pub use task_packet::*;
pub use tiered_memory::*;
pub use typed_context_filter::*;
pub use timeline_event_order::*;
pub use tool_policy::*;
pub use trajectory::*;
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
