use agent_harness_core::agent_loop::EventCallback;
use agent_harness_core::provider::{LlmMessage, Provider};
use agent_harness_core::{AgentLoop, EffectiveToolInventory, TaskPacket, ToolHandler};

use super::context::AgentTask;
use super::observation::WriterObservation;
use super::operation::WriterOperation;
use super::proposal::AgentProposal;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriterAgentTask {
    ManualRequest,
    InlineRewrite,
    GhostWriting,
    ChapterGeneration,
    PlanningReview,
    ContinuityDiagnostic,
    CanonMaintenance,
    ProposalEvaluation,
}

impl WriterAgentTask {
    pub fn as_agent_task(&self) -> AgentTask {
        match self {
            WriterAgentTask::ManualRequest => AgentTask::ManualRequest,
            WriterAgentTask::InlineRewrite => AgentTask::InlineRewrite,
            WriterAgentTask::GhostWriting => AgentTask::GhostWriting,
            WriterAgentTask::ChapterGeneration => AgentTask::ChapterGeneration,
            WriterAgentTask::PlanningReview => AgentTask::PlanningReview,
            WriterAgentTask::ContinuityDiagnostic => AgentTask::ContinuityDiagnostic,
            WriterAgentTask::CanonMaintenance => AgentTask::CanonMaintenance,
            WriterAgentTask::ProposalEvaluation => AgentTask::ProposalEvaluation,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriterAgentApprovalMode {
    ReadOnly,
    SurfaceProposals,
    ApprovedWrites,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriterAgentStreamMode {
    None,
    Text,
}

#[derive(Debug, Clone)]
pub struct WriterAgentRunRequest {
    pub task: WriterAgentTask,
    pub observation: WriterObservation,
    pub user_instruction: String,
    pub frontend_state: WriterAgentFrontendState,
    pub approval_mode: WriterAgentApprovalMode,
    pub stream_mode: WriterAgentStreamMode,
    pub manual_history: Vec<LlmMessage>,
}

#[derive(Debug, Clone, Default)]
pub struct WriterAgentFrontendState {
    pub truncated_context: String,
    pub paragraph: String,
    pub selected_text: String,
    pub memory_context: String,
    pub has_lore: bool,
    pub has_outline: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterAgentRunResult {
    pub answer: String,
    pub proposals: Vec<AgentProposal>,
    pub operations: Vec<WriterOperation>,
    pub task_packet: TaskPacket,
    pub context_pack_summary: WriterAgentContextPackSummary,
    pub tool_inventory: EffectiveToolInventory,
    pub trace_refs: Vec<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterAgentContextPackSummary {
    pub task: AgentTask,
    pub source_count: usize,
    pub total_chars: usize,
    pub budget_limit: usize,
    pub source_refs: Vec<String>,
}

pub struct WriterAgentPreparedRun<P: Provider, H: ToolHandler> {
    pub(crate) request: WriterAgentRunRequest,
    pub(crate) agent: AgentLoop<P, H>,
    pub(crate) proposals: Vec<AgentProposal>,
    pub(crate) operations: Vec<WriterOperation>,
    pub(crate) task_packet: TaskPacket,
    pub(crate) context_pack_summary: WriterAgentContextPackSummary,
    pub(crate) tool_inventory: EffectiveToolInventory,
    pub(crate) source_refs: Vec<String>,
    pub(crate) trace_refs: Vec<String>,
}

impl<P, H> WriterAgentPreparedRun<P, H>
where
    P: Provider,
    H: ToolHandler,
{
    pub fn set_event_callback(&mut self, cb: EventCallback) {
        self.agent.set_event_callback(cb);
    }

    pub fn request(&self) -> &WriterAgentRunRequest {
        &self.request
    }

    pub fn proposals(&self) -> &[AgentProposal] {
        &self.proposals
    }

    pub fn context_pack_summary(&self) -> &WriterAgentContextPackSummary {
        &self.context_pack_summary
    }

    pub fn tool_inventory(&self) -> &EffectiveToolInventory {
        &self.tool_inventory
    }

    pub async fn run(mut self) -> Result<WriterAgentRunResult, String> {
        self.agent
            .add_user_message(self.request.user_instruction.clone());
        let answer = self
            .agent
            .run(
                &self.request.user_instruction,
                self.request.frontend_state.has_lore,
                self.request.frontend_state.has_outline,
            )
            .await?;
        self.trace_refs.push(self.task_packet.id.clone());
        Ok(WriterAgentRunResult {
            answer,
            proposals: self.proposals,
            operations: self.operations,
            task_packet: self.task_packet,
            context_pack_summary: self.context_pack_summary,
            tool_inventory: self.tool_inventory,
            trace_refs: self.trace_refs,
            source_refs: self.source_refs,
        })
    }
}
