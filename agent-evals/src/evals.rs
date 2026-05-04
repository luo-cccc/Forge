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

mod belief_conflict;
mod canon;
mod context;
mod e2e_scenarios;
mod foundation;
mod ghost_feedback;
mod intent;
mod metacognition;
mod mission;
mod project_brain_knowledge;
mod promise;
mod provider_budget;
mod research_subtask;
mod run_loop;
mod story_debt;
mod story_impact;
mod task_packet;
mod tool_policy;
mod trajectory;
mod writing_relevance;

pub use belief_conflict::*;
pub use canon::*;
pub use context::*;
pub use e2e_scenarios::*;
pub use foundation::*;
pub use ghost_feedback::*;
pub use intent::*;
pub use metacognition::*;
pub use mission::*;
pub use project_brain_knowledge::*;
pub use promise::*;
pub use provider_budget::*;
pub use research_subtask::*;
pub use run_loop::*;
pub use story_debt::*;
pub use story_impact::*;
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
