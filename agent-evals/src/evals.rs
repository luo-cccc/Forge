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
mod chapter_settlement;
mod context;
mod e2e_scenarios;
mod foundation;
mod ghost_feedback;
mod intent;
mod memory_quality;
mod metacognition;
mod mission;
mod product_scenarios;
mod project_brain_knowledge;
mod project_intake;
mod promise;
mod provider_budget;
mod research_subtask;
mod rewrite_impact;
mod run_loop;
mod run_preflight;
mod story_debt;
mod story_impact;
mod supervised_sprint;
mod task_packet;
mod tool_policy;
mod trajectory;
mod writing_relevance;
mod reader_compensation;
mod context_spine;
pub use context_spine::*;
pub use reader_compensation::*;

pub use author_voice::*;
pub use belief_conflict::*;
pub use canon::*;
pub use chapter_settlement::*;
pub use context::*;
pub use e2e_scenarios::*;
pub use foundation::*;
pub use ghost_feedback::*;
pub use intent::*;
pub use memory_quality::*;
pub use metacognition::*;
pub use mission::*;
pub use product_scenarios::*;
pub use project_brain_knowledge::*;
pub use project_intake::*;
pub use promise::*;
pub use provider_budget::*;
pub use research_subtask::*;
pub use rewrite_impact::*;
pub use run_loop::*;
pub use run_preflight::*;
pub use story_debt::*;
pub use story_impact::*;
pub use supervised_sprint::*;
pub use task_packet::*;
pub use tool_policy::*;
pub use trajectory::*;
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
