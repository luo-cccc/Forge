//! WriterAgentKernel — persistent project agent that owns observations,
//! proposals, memory, canon, and feedback.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_harness_core::agent_loop::EventCallback;
use agent_harness_core::provider::{LlmMessage, Provider};
use agent_harness_core::{
    default_writing_tool_registry, AgentLoop, AgentLoopConfig, EffectiveToolInventory,
    FeedbackContract, RequiredContext, TaskBelief, TaskPacket, TaskScope, ToolFilter, ToolHandler,
    ToolPolicyContract, ToolSideEffectLevel,
};
use agent_harness_core::{PermissionMode, PermissionPolicy};

use super::canon::CanonEngine;
use super::context::{
    assemble_observation_context, assemble_observation_context_with_default_budget, AgentTask,
    ContextSource, WritingContextPack,
};
use super::diagnostics::{
    DiagnosticCategory, DiagnosticResult, DiagnosticSeverity, DiagnosticsEngine,
};
use super::feedback::{FeedbackAction, ProposalFeedback};
use super::intent::{AgentBehavior, IntentEngine};
use super::memory::{
    ChapterMissionSummary, ChapterResultSummary, ContextBudgetTrace, ContextRecallSummary,
    ContextSourceBudgetTrace, ManualAgentTurnSummary, NextBeatSummary, PromiseKind,
    StoryContractQuality, StoryContractSummary, WriterMemory,
};
use super::observation::WriterObservation;
use super::operation::{
    execute_text_operation, CanonEntityOp, OperationResult, PlotPromiseOp, WriterOperation,
};
use super::proposal::{
    AgentProposal, EvidenceRef, EvidenceSource, ProposalAlternative, ProposalKind, ProposalPriority,
};

pub(crate) use super::kernel_chapters::*;
pub use super::kernel_helpers::*;
pub(crate) use super::kernel_prompts::*;
pub(crate) use super::kernel_review::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WriterAgentStatus {
    pub project_id: String,
    pub session_id: String,
    pub active_chapter: Option<String>,
    pub observation_count: u64,
    pub proposal_count: u64,
    pub open_promise_count: usize,
    pub pending_proposals: usize,
    pub total_feedback_events: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterAgentLedgerSnapshot {
    pub story_contract: Option<super::memory::StoryContractSummary>,
    pub active_chapter_mission: Option<super::memory::ChapterMissionSummary>,
    pub chapter_missions: Vec<super::memory::ChapterMissionSummary>,
    pub recent_chapter_results: Vec<super::memory::ChapterResultSummary>,
    pub next_beat: Option<super::memory::NextBeatSummary>,
    pub canon_entities: Vec<super::memory::CanonEntitySummary>,
    pub canon_rules: Vec<super::memory::CanonRuleSummary>,
    pub open_promises: Vec<super::memory::PlotPromiseSummary>,
    pub recent_decisions: Vec<super::memory::CreativeDecisionSummary>,
    pub memory_audit: Vec<super::memory::MemoryAuditSummary>,
    pub context_recalls: Vec<ContextRecallSummary>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryReviewQueueEntry {
    pub id: String,
    pub proposal_id: String,
    pub category: ProposalKind,
    pub severity: StoryReviewSeverity,
    pub title: String,
    pub message: String,
    pub target: Option<super::observation::TextRange>,
    pub evidence: Vec<EvidenceRef>,
    pub operations: Vec<WriterOperation>,
    pub status: StoryReviewQueueStatus,
    pub created_at: u64,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StoryReviewSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StoryReviewQueueStatus {
    Pending,
    Accepted,
    Ignored,
    Snoozed,
    Expired,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryDebtSnapshot {
    pub chapter_title: Option<String>,
    pub total: usize,
    pub open_count: usize,
    pub contract_count: usize,
    pub mission_count: usize,
    pub canon_risk_count: usize,
    pub promise_count: usize,
    pub pacing_count: usize,
    pub entries: Vec<StoryDebtEntry>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryDebtEntry {
    pub id: String,
    pub chapter_title: Option<String>,
    pub category: StoryDebtCategory,
    pub severity: StoryReviewSeverity,
    pub status: StoryDebtStatus,
    pub title: String,
    pub message: String,
    pub evidence: Vec<EvidenceRef>,
    pub related_review_ids: Vec<String>,
    pub operations: Vec<WriterOperation>,
    pub created_at: u64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StoryDebtCategory {
    StoryContract,
    ChapterMission,
    CanonRisk,
    TimelineRisk,
    Promise,
    Pacing,
    Memory,
    Question,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StoryDebtStatus {
    Open,
    Snoozed,
    Stale,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterAgentTraceSnapshot {
    pub recent_observations: Vec<WriterObservationTrace>,
    pub task_packets: Vec<WriterTaskPacketTrace>,
    pub recent_proposals: Vec<WriterProposalTrace>,
    pub recent_feedback: Vec<WriterFeedbackTrace>,
    pub operation_lifecycle: Vec<WriterOperationLifecycleTrace>,
    pub context_recalls: Vec<ContextRecallSummary>,
    pub product_metrics: WriterProductMetrics,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterProductMetrics {
    pub proposal_count: u64,
    pub feedback_count: u64,
    pub accepted_count: u64,
    pub rejected_count: u64,
    pub edited_count: u64,
    pub snoozed_count: u64,
    pub explained_count: u64,
    pub ignored_count: u64,
    pub positive_feedback_count: u64,
    pub negative_feedback_count: u64,
    pub proposal_acceptance_rate: f64,
    pub ignored_repeated_suggestion_rate: f64,
    pub manual_ask_converted_to_operation_rate: f64,
    pub promise_recall_hit_rate: f64,
    pub canon_false_positive_rate: f64,
    pub chapter_mission_completion_rate: f64,
    pub durable_save_success_rate: f64,
    pub average_save_to_feedback_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterTaskPacketTrace {
    pub id: String,
    pub observation_id: String,
    pub task: String,
    pub objective: String,
    pub scope: String,
    pub intent: Option<String>,
    pub required_context_count: usize,
    pub belief_count: usize,
    pub success_criteria_count: usize,
    pub max_side_effect_level: String,
    pub feedback_checkpoint_count: usize,
    pub foundation_complete: bool,
    pub packet: TaskPacket,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterObservationTrace {
    pub id: String,
    pub created_at: u64,
    pub reason: String,
    pub chapter_title: Option<String>,
    pub paragraph_snippet: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterProposalTrace {
    pub id: String,
    pub observation_id: String,
    pub kind: String,
    pub priority: String,
    pub state: String,
    pub confidence: f64,
    pub preview_snippet: String,
    pub evidence: Vec<EvidenceRef>,
    pub context_budget: Option<ContextBudgetTrace>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterFeedbackTrace {
    pub proposal_id: String,
    pub action: String,
    pub reason: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriterOperationLifecycleState {
    Proposed,
    Approved,
    Applied,
    DurablySaved,
    FeedbackRecorded,
    Rejected,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterOperationLifecycleTrace {
    pub proposal_id: Option<String>,
    pub operation_kind: String,
    pub source_task: Option<String>,
    pub approval_source: Option<String>,
    pub affected_scope: Option<String>,
    pub state: WriterOperationLifecycleState,
    pub save_result: Option<String>,
    pub feedback_result: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WriterAgentTask {
    ManualRequest,
    InlineRewrite,
    GhostWriting,
    ChapterGeneration,
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
    request: WriterAgentRunRequest,
    agent: AgentLoop<P, H>,
    proposals: Vec<AgentProposal>,
    operations: Vec<WriterOperation>,
    task_packet: TaskPacket,
    context_pack_summary: WriterAgentContextPackSummary,
    tool_inventory: EffectiveToolInventory,
    source_refs: Vec<String>,
    trace_refs: Vec<String>,
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

pub struct WriterAgentKernel {
    pub project_id: String,
    pub session_id: String,
    pub memory: WriterMemory,
    pub canon: CanonEngine,
    diagnostics: DiagnosticsEngine,
    intent: IntentEngine,
    observations: Vec<WriterObservation>,
    proposals: Vec<AgentProposal>,
    proposal_context_budgets: HashMap<String, ContextBudgetTrace>,
    task_packets: Vec<WriterTaskPacketTrace>,
    feedback_events: Vec<ProposalFeedback>,
    operation_lifecycle: Vec<WriterOperationLifecycleTrace>,
    superseded_proposals: HashSet<String>,
    suppressed_slots: Vec<SuppressedProposalSlot>,
    ignored_ghost_slots: Vec<IgnoredGhostSlot>,
    observation_counter: u64,
    proposal_counter: u64,
    pub active_chapter: Option<String>,
}

struct SuppressedProposalSlot {
    slot: String,
    until: u64,
}

struct IgnoredGhostSlot {
    slot: String,
    count: u8,
    last_seen: u64,
}

impl WriterAgentKernel {
    pub fn new(project_id: &str, memory: WriterMemory) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: format!(
                "session-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ),
            memory,
            canon: CanonEngine::new(),
            diagnostics: DiagnosticsEngine::new(),
            intent: IntentEngine::new(),
            observations: Vec::new(),
            proposals: Vec::new(),
            proposal_context_budgets: HashMap::new(),
            task_packets: Vec::new(),
            feedback_events: Vec::new(),
            operation_lifecycle: Vec::new(),
            superseded_proposals: HashSet::new(),
            suppressed_slots: Vec::new(),
            ignored_ghost_slots: Vec::new(),
            observation_counter: 0,
            proposal_counter: 0,
            active_chapter: None,
        }
    }

    pub fn observe(
        &mut self,
        observation: WriterObservation,
    ) -> Result<Vec<AgentProposal>, String> {
        self.observation_counter += 1;
        let mut proposals = Vec::new();
        let mut proposal_context_budgets = HashMap::new();
        let obs_id = observation.id.clone();
        self.active_chapter = observation.chapter_title.clone();
        self.memory
            .record_observation_trace(
                &observation.id,
                observation.created_at,
                &format!("{:?}", observation.reason),
                observation.chapter_title.as_deref(),
                &snippet(&observation.paragraph, 120),
            )
            .ok();

        let intent = self.intent.classify(
            &observation.paragraph,
            observation.has_selection(),
            observation.reason == super::observation::ObservationReason::ChapterSwitch,
        );

        if observation.reason == super::observation::ObservationReason::Save {
            let result = chapter_result_from_observation(&observation, &self.memory);
            if !result.is_empty() {
                self.memory.record_chapter_result(&result).ok();
                self.calibrate_chapter_mission(&observation, &result).ok();
                self.touch_promise_last_seen_from_result(&result).ok();
                proposals.extend(chapter_mission_result_proposals(
                    &observation,
                    &result,
                    &self.memory,
                    &obs_id,
                    &mut self.proposal_counter,
                    &self.session_id,
                ));
            }
        }

        if let Ok(promises) = self.memory.get_open_promises() {
            for (_kind, title, desc, chapter) in &promises {
                if observation.reason == super::observation::ObservationReason::ChapterSwitch {
                    proposals.push(AgentProposal {
                        id: proposal_id(&self.session_id, self.proposal_counter),
                        observation_id: obs_id.clone(),
                        kind: ProposalKind::PlotPromise,
                        priority: ProposalPriority::Normal,
                        target: None,
                        preview: format!("未回收伏笔: {} ({}章)", title, chapter),
                        operations: vec![],
                        rationale: format!("{}: {}", title, desc),
                        evidence: vec![EvidenceRef {
                            source: EvidenceSource::PromiseLedger,
                            reference: title.clone(),
                            snippet: desc.clone(),
                        }],
                        risks: vec![],
                        alternatives: vec![],
                        confidence: 0.7,
                        expires_at: None,
                    });
                    self.proposal_counter += 1;
                }
            }
        }

        if matches!(
            observation.reason,
            super::observation::ObservationReason::Save
                | super::observation::ObservationReason::ChapterSwitch
        ) {
            for candidate in memory_candidates_from_observation(
                &observation,
                &self.memory,
                &obs_id,
                &mut self.proposal_counter,
                &self.session_id,
            ) {
                proposals.push(candidate);
            }
        }

        if matches!(
            observation.reason,
            super::observation::ObservationReason::Idle
                | super::observation::ObservationReason::ChapterSwitch
                | super::observation::ObservationReason::Save
        ) {
            let paragraph_offset = observation
                .cursor
                .as_ref()
                .map(|cursor| {
                    cursor
                        .from
                        .saturating_sub(observation.paragraph.chars().count())
                })
                .unwrap_or(0);
            let chapter_id = observation.chapter_title.as_deref().unwrap_or("Chapter-1");
            for diagnostic in self.diagnostics.diagnose(
                &observation.paragraph,
                paragraph_offset,
                chapter_id,
                &observation.project_id,
                &self.memory,
            ) {
                proposals.push(diagnostic_to_proposal(
                    diagnostic,
                    &observation,
                    &obs_id,
                    &proposal_id(&self.session_id, self.proposal_counter),
                ));
                self.proposal_counter += 1;
            }
        }

        let should_offer_continuation = matches!(
            &intent.desired_behavior,
            AgentBehavior::SuggestContinuation | AgentBehavior::GenerateDraft
        );

        if observation.paragraph.chars().count() >= 32
            && should_offer_continuation
            && matches!(
                observation.reason,
                super::observation::ObservationReason::Idle
                    | super::observation::ObservationReason::Typed
            )
        {
            let context_pack = assemble_observation_context_with_default_budget(
                AgentTask::GhostWriting,
                &observation,
                &self.memory,
            );
            self.record_task_packet_for(
                AgentTask::GhostWriting,
                &observation,
                &context_pack,
                "Continue from the current cursor while preserving chapter mission, canon, and open promises.",
                vec![
                    "Continuation fits the local paragraph without forcing a broad rewrite."
                        .to_string(),
                    "Continuation does not introduce canon or promise-ledger conflicts."
                        .to_string(),
                ],
            );
            let continuation = draft_continuation(&intent.primary, &observation, &context_pack);
            let insert_at = observation.cursor.as_ref().map(|c| c.to).unwrap_or(0);
            let chapter = observation
                .chapter_title
                .clone()
                .or_else(|| self.active_chapter.clone())
                .unwrap_or_else(|| "Chapter-1".to_string());
            let revision = observation
                .chapter_revision
                .clone()
                .unwrap_or_else(|| "missing".to_string());
            let alternatives = ghost_alternatives(
                &intent.primary,
                &observation,
                &context_pack,
                &chapter,
                insert_at,
                &revision,
            );

            let proposal_id_value = proposal_id(&self.session_id, self.proposal_counter);
            proposal_context_budgets.insert(
                proposal_id_value.clone(),
                context_budget_trace(&context_pack),
            );
            proposals.push(AgentProposal {
                id: proposal_id_value,
                observation_id: obs_id.clone(),
                kind: ProposalKind::Ghost,
                priority: ProposalPriority::Ambient,
                target: observation
                    .cursor
                    .clone()
                    .map(|c| super::observation::TextRange {
                        from: c.to,
                        to: c.to,
                    }),
                preview: continuation.clone(),
                operations: vec![WriterOperation::TextInsert {
                    chapter,
                    at: insert_at,
                    text: continuation,
                    revision,
                }],
                rationale: format!(
                    "意图识别: {:?} ({:.0}%). ContextPack: {} sources, {}/{} chars.",
                    intent.primary,
                    intent.confidence * 100.0,
                    context_pack.sources.len(),
                    context_pack.total_chars,
                    context_pack.budget_limit
                ),
                evidence: context_pack_evidence(&context_pack, &observation),
                risks: vec![],
                alternatives,
                confidence: intent.confidence.max(0.55) as f64,
                expires_at: Some(observation.created_at + 30_000),
            });
            self.proposal_counter += 1;
        }

        self.observations.push(observation);
        Ok(self.register_proposals(proposals, &proposal_context_budgets))
    }

    pub fn ghost_context_pack(&self, observation: &WriterObservation) -> WritingContextPack {
        assemble_observation_context_with_default_budget(
            AgentTask::GhostWriting,
            observation,
            &self.memory,
        )
    }

    pub fn context_pack_for(
        &self,
        task: AgentTask,
        observation: &WriterObservation,
        total_budget: usize,
    ) -> WritingContextPack {
        assemble_observation_context(task, observation, &self.memory, total_budget)
    }

    pub fn context_pack_for_default(
        &self,
        task: AgentTask,
        observation: &WriterObservation,
    ) -> WritingContextPack {
        assemble_observation_context_with_default_budget(task, observation, &self.memory)
    }

    pub fn record_manual_exchange(
        &mut self,
        observation: &WriterObservation,
        message: &str,
        response: &str,
        source_refs: &[String],
    ) -> Result<(), String> {
        let scope = observation
            .chapter_title
            .as_deref()
            .unwrap_or("manual request");
        let title = format!("ManualRequest: {}", snippet(message, 48));
        let rationale = format!(
            "用户显式请求: {}\nAgent回应摘要: {}",
            snippet(message, 160),
            snippet(response, 240)
        );
        self.memory
            .record_decision(scope, &title, "answered", &[], &rationale, source_refs)
            .map_err(|e| e.to_string())?;
        self.memory
            .record_manual_agent_turn(&ManualAgentTurnSummary {
                project_id: observation.project_id.clone(),
                observation_id: observation.id.clone(),
                chapter_title: observation.chapter_title.clone(),
                user: message.to_string(),
                assistant: response.to_string(),
                source_refs: source_refs.to_vec(),
                created_at: crate::agent_runtime::now_ms(),
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn prepare_task_run<P, H>(
        &mut self,
        request: WriterAgentRunRequest,
        provider: Arc<P>,
        handler: H,
        model: &str,
    ) -> Result<WriterAgentPreparedRun<P, H>, String>
    where
        P: Provider + 'static,
        H: ToolHandler + 'static,
    {
        let task = request.task.as_agent_task();
        let proposals = self.observe(request.observation.clone())?;
        let operations = proposals
            .iter()
            .flat_map(|proposal| proposal.operations.clone())
            .collect::<Vec<_>>();
        let context_pack = self.context_pack_for_default(task.clone(), &request.observation);
        let mut task_packet = build_task_packet_for_observation(
            &self.project_id,
            &self.session_id,
            task.clone(),
            &request.observation,
            &context_pack,
            &objective_for_run_task(&request.task),
            success_criteria_for_run_task(&request.task),
        );
        task_packet.validate().map_err(|error| error.to_string())?;
        self.push_task_packet_trace(
            request.observation.id.clone(),
            format!("{:?}", task),
            task_packet.clone(),
        );

        if request.task == WriterAgentTask::ChapterGeneration {
            let quality = self.contract_quality();
            if quality <= StoryContractQuality::Vague {
                task_packet.beliefs.push(TaskBelief {
                    subject: "Story Contract Quality".to_string(),
                    statement: format!(
                        "StoryContract quality is {:?}: generated chapter may lack story-level grounding. Consider strengthening the Story Contract in settings.",
                        quality
                    ),
                    confidence: 0.9f32,
                    source: Some("story_contract_quality_gate".to_string()),
                });
            }
        }

        let tool_filter = tool_filter_for_run_request(task.clone(), &request.approval_mode);
        let registry = default_writing_tool_registry();
        let tool_inventory = registry.effective_inventory(
            &tool_filter,
            &PermissionPolicy::new(PermissionMode::WorkspaceWrite),
        );
        let source_refs = source_refs_from_context_pack(&context_pack);
        let context_pack_summary = WriterAgentContextPackSummary {
            task: task.clone(),
            source_count: context_pack.sources.len(),
            total_chars: context_pack.total_chars,
            budget_limit: context_pack.budget_limit,
            source_refs: source_refs.clone(),
        };
        let system_prompt = render_run_system_prompt(&request, &context_pack, self);
        tracing::debug!(
            "WriterAgent {:?} ContextPack: {} sources, {}/{} chars",
            task,
            context_pack.sources.len(),
            context_pack.total_chars,
            context_pack.budget_limit
        );

        let mut agent = AgentLoop::new(
            AgentLoopConfig {
                max_rounds: 10,
                system_prompt,
                context_limit_tokens: Some(
                    agent_harness_core::resolve_context_window_info(model).tokens,
                ),
                tool_filter: Some(tool_filter),
            },
            provider,
            registry,
            handler,
        );
        agent.messages.extend(request.manual_history.clone());

        Ok(WriterAgentPreparedRun {
            request,
            agent,
            proposals,
            operations,
            task_packet,
            context_pack_summary,
            tool_inventory,
            source_refs,
            trace_refs: vec![],
        })
    }

    pub async fn run_task<P, H>(
        &mut self,
        request: WriterAgentRunRequest,
        provider: Arc<P>,
        handler: H,
        model: &str,
        on_event: Option<EventCallback>,
    ) -> Result<WriterAgentRunResult, String>
    where
        P: Provider + 'static,
        H: ToolHandler + 'static,
    {
        let completion_request = request.clone();
        let mut prepared = self.prepare_task_run(request, provider, handler, model)?;
        if let Some(callback) = on_event {
            prepared.set_event_callback(callback);
        }
        let result = prepared.run().await?;
        self.record_run_completion(&completion_request, &result)?;
        Ok(result)
    }

    pub fn record_run_completion(
        &mut self,
        request: &WriterAgentRunRequest,
        result: &WriterAgentRunResult,
    ) -> Result<(), String> {
        if request.task == WriterAgentTask::ManualRequest {
            self.record_manual_exchange(
                &request.observation,
                &request.user_instruction,
                &result.answer,
                &result.source_refs,
            )?;
        }
        Ok(())
    }

    pub fn create_llm_ghost_proposal(
        &mut self,
        observation: WriterObservation,
        continuation: String,
        model: &str,
    ) -> Result<AgentProposal, String> {
        let continuation = sanitize_continuation(&continuation);
        if continuation.is_empty() {
            return Err("empty LLM continuation".to_string());
        }

        let intent = self.intent.classify(
            &observation.paragraph,
            observation.has_selection(),
            observation.reason == super::observation::ObservationReason::ChapterSwitch,
        );
        let context_pack = self.ghost_context_pack(&observation);
        self.record_task_packet_for(
            AgentTask::GhostWriting,
            &observation,
            &context_pack,
            "Generate an LLM-backed ghost continuation grounded in the active writing context.",
            vec![
                "LLM ghost is short enough to review inline.".to_string(),
                "LLM ghost cites the same required context pack used for generation.".to_string(),
            ],
        );
        let insert_at = observation.cursor.as_ref().map(|c| c.to).unwrap_or(0);
        let chapter = observation
            .chapter_title
            .clone()
            .or_else(|| self.active_chapter.clone())
            .unwrap_or_else(|| "Chapter-1".to_string());
        let revision = observation
            .chapter_revision
            .clone()
            .unwrap_or_else(|| "missing".to_string());

        let proposal = AgentProposal {
            id: proposal_id(&self.session_id, self.proposal_counter),
            observation_id: observation.id.clone(),
            kind: ProposalKind::Ghost,
            priority: ProposalPriority::Ambient,
            target: observation
                .cursor
                .clone()
                .map(|c| super::observation::TextRange {
                    from: c.to,
                    to: c.to,
                }),
            preview: continuation.clone(),
            operations: vec![WriterOperation::TextInsert {
                chapter,
                at: insert_at,
                text: continuation,
                revision,
            }],
            rationale: format!(
                "LLM增强续写: {}. 意图识别: {:?} ({:.0}%). ContextPack: {} sources, {}/{} chars.",
                model,
                intent.primary,
                intent.confidence * 100.0,
                context_pack.sources.len(),
                context_pack.total_chars,
                context_pack.budget_limit
            ),
            evidence: context_pack_evidence(&context_pack, &observation),
            risks: vec!["LLM draft should be reviewed before keeping.".into()],
            alternatives: vec![],
            confidence: ghost_confidence(intent.confidence, &self.memory, &self.project_id),
            expires_at: Some(observation.created_at + 60_000),
        };

        self.proposal_counter += 1;
        self.register_proposal(proposal, Some(context_budget_trace(&context_pack)))
            .ok_or_else(|| "duplicate LLM continuation suppressed".to_string())
    }

    pub fn create_inline_operation_proposal(
        &mut self,
        observation: WriterObservation,
        instruction: &str,
        draft: String,
        model: &str,
    ) -> Result<AgentProposal, String> {
        let draft = sanitize_continuation(&draft);
        if draft.is_empty() {
            return Err("empty inline operation draft".to_string());
        }

        let context_pack = assemble_observation_context_with_default_budget(
            AgentTask::InlineRewrite,
            &observation,
            &self.memory,
        );
        let chapter = observation
            .chapter_title
            .clone()
            .or_else(|| self.active_chapter.clone())
            .unwrap_or_else(|| "Chapter-1".to_string());
        let revision = observation
            .chapter_revision
            .clone()
            .unwrap_or_else(|| "missing".to_string());
        let operation = if let Some(selection) = observation.selection.as_ref() {
            if selection.from < selection.to {
                WriterOperation::TextReplace {
                    chapter: chapter.clone(),
                    from: selection.from,
                    to: selection.to,
                    text: draft.clone(),
                    revision,
                }
            } else {
                WriterOperation::TextInsert {
                    chapter: chapter.clone(),
                    at: observation
                        .cursor
                        .as_ref()
                        .map(|c| c.to)
                        .unwrap_or(selection.to),
                    text: draft.clone(),
                    revision,
                }
            }
        } else {
            WriterOperation::TextInsert {
                chapter: chapter.clone(),
                at: observation.cursor.as_ref().map(|c| c.to).unwrap_or(0),
                text: draft.clone(),
                revision,
            }
        };

        let target = match &operation {
            WriterOperation::TextReplace { from, to, .. } => Some(super::observation::TextRange {
                from: *from,
                to: *to,
            }),
            WriterOperation::TextInsert { at, .. } => {
                Some(super::observation::TextRange { from: *at, to: *at })
            }
            _ => None,
        };

        let proposal = AgentProposal {
            id: proposal_id(&self.session_id, self.proposal_counter),
            observation_id: observation.id.clone(),
            kind: ProposalKind::ParallelDraft,
            priority: ProposalPriority::Normal,
            target,
            preview: draft.clone(),
            operations: vec![operation],
            rationale: format!(
                "Inline typed operation via {}. Instruction: {}. ContextPack: {} sources, {}/{} chars.",
                model,
                snippet(instruction, 120),
                context_pack.sources.len(),
                context_pack.total_chars,
                context_pack.budget_limit
            ),
            evidence: context_pack_evidence(&context_pack, &observation),
            risks: vec!["Inline operation should be previewed before accepting.".into()],
            alternatives: vec![],
            confidence: 0.78,
            expires_at: Some(observation.created_at + 120_000),
        };

        self.proposal_counter += 1;
        self.register_proposal(proposal, Some(context_budget_trace(&context_pack)))
            .ok_or_else(|| "duplicate inline operation suppressed".to_string())
    }

    pub fn create_llm_memory_proposals(
        &mut self,
        observation: WriterObservation,
        value: serde_json::Value,
        model: &str,
    ) -> Vec<AgentProposal> {
        let feedback = MemoryExtractionFeedback::from_memory(&self.memory);
        let candidates = llm_memory_candidates_from_value(value, &observation, model)
            .into_iter()
            .filter_map(|candidate| feedback.apply_to_candidate(candidate))
            .collect::<Vec<_>>();
        let mut proposals = Vec::new();
        for candidate in candidates {
            let proposal = match candidate {
                MemoryCandidate::Canon(entity) => canon_candidate_proposal(
                    &observation,
                    &observation.id,
                    &mut self.proposal_counter,
                    &self.session_id,
                    entity,
                    CandidateSource::Llm(model.to_string()),
                ),
                MemoryCandidate::Promise(promise) => promise_candidate_proposal(
                    &observation,
                    &observation.id,
                    &mut self.proposal_counter,
                    &self.session_id,
                    promise,
                    CandidateSource::Llm(model.to_string()),
                ),
            };

            if let Some(registered) = self.register_proposal(proposal, None) {
                proposals.push(registered);
            }
        }
        proposals
    }

    pub fn diagnose_paragraph(
        &self,
        paragraph: &str,
        paragraph_offset: usize,
        chapter_id: &str,
    ) -> Vec<DiagnosticResult> {
        self.diagnostics.diagnose(
            paragraph,
            paragraph_offset,
            chapter_id,
            &self.project_id,
            &self.memory,
        )
    }

    pub fn apply_feedback(&mut self, feedback: ProposalFeedback) -> Result<(), String> {
        let proposal = self
            .proposals
            .iter()
            .find(|p| p.id == feedback.proposal_id)
            .cloned();
        let positive_feedback_ready = proposal
            .as_ref()
            .map(|prop| !feedback.is_positive() || self.proposal_positive_feedback_ready(prop))
            .unwrap_or(true);
        let feedback_result = if feedback.is_positive() && !positive_feedback_ready {
            Some("deferred:missing_durable_save".to_string())
        } else {
            Some("recorded".to_string())
        };

        self.memory
            .record_feedback(
                &feedback.proposal_id,
                match feedback.action {
                    FeedbackAction::Accepted => "accepted",
                    FeedbackAction::Rejected => "rejected",
                    FeedbackAction::Edited => "edited",
                    FeedbackAction::Snoozed => "snoozed",
                    FeedbackAction::Explained => "explained",
                },
                feedback.reason.as_deref().unwrap_or(""),
                feedback.final_text.as_deref().unwrap_or(""),
            )
            .map_err(|e| format!("feedback: {}", e))?;

        if feedback.is_positive() && positive_feedback_ready {
            if let Some(prop) = proposal.as_ref() {
                record_memory_candidate_feedback(&self.memory, prop, true);
                record_memory_audit_event(&self.memory, prop, &feedback);
                self.memory
                    .upsert_style_preference(
                        &format!("accepted_{:?}", prop.kind),
                        &prop.rationale,
                        true,
                    )
                    .ok();
                self.memory
                    .record_decision(
                        self.active_chapter.as_deref().unwrap_or("project"),
                        &format!("{:?}", prop.kind),
                        "accepted",
                        &[],
                        &prop.rationale,
                        &prop
                            .evidence
                            .iter()
                            .map(|e| e.reference.clone())
                            .collect::<Vec<_>>(),
                    )
                    .ok();
            }
        } else if !feedback.is_positive() {
            if let Some(prop) = proposal.as_ref() {
                let action = match feedback.action {
                    FeedbackAction::Rejected => "rejected",
                    FeedbackAction::Edited => "edited",
                    FeedbackAction::Snoozed => "snoozed",
                    FeedbackAction::Explained => "explained",
                    FeedbackAction::Accepted => "accepted",
                };
                if feedback.is_negative() || matches!(feedback.action, FeedbackAction::Edited) {
                    record_memory_candidate_feedback(&self.memory, prop, false);
                    record_memory_audit_event(&self.memory, prop, &feedback);
                    self.memory
                        .record_decision(
                            self.active_chapter.as_deref().unwrap_or("project"),
                            &format!("{:?}", prop.kind),
                            action,
                            &[],
                            feedback.reason.as_deref().unwrap_or(&prop.rationale),
                            &prop
                                .evidence
                                .iter()
                                .map(|e| e.reference.clone())
                                .collect::<Vec<_>>(),
                        )
                        .ok();
                }
                if prop.kind == ProposalKind::Ghost
                    && matches!(feedback.action, FeedbackAction::Explained)
                {
                    self.memory
                        .upsert_style_preference("ignored_ghost", &prop.rationale, false)
                        .ok();
                }
            }
        }

        if let Some(prop) = proposal.as_ref() {
            self.suppress_slot_after_feedback(&prop, &feedback);
        }

        self.memory
            .record_feedback_trace(&super::memory::FeedbackTraceSummary {
                proposal_id: feedback.proposal_id.clone(),
                action: format!("{:?}", feedback.action),
                reason: feedback.reason.clone(),
                created_at: feedback.created_at,
            })
            .ok();
        self.memory
            .update_proposal_trace_state(
                &feedback.proposal_id,
                &format!("feedback:{:?}", feedback.action),
            )
            .ok();

        if let Some(prop) = proposal.as_ref() {
            for operation in prop
                .operations
                .iter()
                .filter(|operation| operation_is_write_capable(operation))
            {
                self.push_operation_lifecycle(
                    Some(prop.id.clone()),
                    operation,
                    WriterOperationLifecycleState::FeedbackRecorded,
                    None,
                    None,
                    feedback_result.clone(),
                    feedback.created_at,
                );
            }
        }

        self.feedback_events.push(feedback);
        Ok(())
    }

    pub fn record_implicit_ghost_rejection(
        &mut self,
        proposal_id: &str,
        created_at: u64,
    ) -> Result<bool, String> {
        let Some(proposal) = self.proposals.iter().find(|p| p.id == proposal_id).cloned() else {
            return Ok(false);
        };
        if proposal.kind != ProposalKind::Ghost
            || self
                .feedback_events
                .iter()
                .any(|feedback| feedback.proposal_id == proposal.id)
        {
            return Ok(false);
        }

        let slot = suppression_slot_key(&proposal);
        self.prune_ignored_ghost_slots(created_at);
        let suppressed = if let Some(index) = self
            .ignored_ghost_slots
            .iter()
            .position(|entry| entry.slot == slot)
        {
            let entry = &mut self.ignored_ghost_slots[index];
            entry.count = entry.count.saturating_add(1);
            entry.last_seen = created_at;
            if entry.count >= 3 {
                self.ignored_ghost_slots.remove(index);
                true
            } else {
                false
            }
        } else {
            self.ignored_ghost_slots.push(IgnoredGhostSlot {
                slot,
                count: 1,
                last_seen: created_at,
            });
            false
        };
        let feedback = ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: if suppressed {
                FeedbackAction::Snoozed
            } else {
                FeedbackAction::Explained
            },
            final_text: None,
            reason: Some(if suppressed {
                "Implicit rejection: author continued writing over repeated ghost text.".to_string()
            } else {
                "Implicit pass: author continued writing instead of accepting ghost text."
                    .to_string()
            }),
            created_at,
        };
        self.apply_feedback(feedback)?;
        Ok(suppressed)
    }

    fn prune_ignored_ghost_slots(&mut self, now: u64) {
        self.ignored_ghost_slots
            .retain(|entry| now.saturating_sub(entry.last_seen) <= 10 * 60 * 1_000);
    }

    fn register_proposals(
        &mut self,
        proposals: Vec<AgentProposal>,
        context_budgets: &HashMap<String, ContextBudgetTrace>,
    ) -> Vec<AgentProposal> {
        proposals
            .into_iter()
            .filter_map(|proposal| {
                let context_budget = context_budgets.get(&proposal.id).cloned();
                self.register_proposal(proposal, context_budget)
            })
            .collect()
    }

    fn register_proposal(
        &mut self,
        proposal: AgentProposal,
        context_budget: Option<ContextBudgetTrace>,
    ) -> Option<AgentProposal> {
        self.prune_suppressed_slots(now_ms());
        if self.is_slot_suppressed(&proposal) {
            return None;
        }

        let slot = proposal_slot_key(&proposal);
        let existing = self
            .proposals
            .iter()
            .rev()
            .find(|existing| {
                self.is_pending_proposal(existing) && proposal_slot_key(existing) == slot
            })
            .cloned();

        if let Some(existing) = existing {
            if should_replace_proposal(&existing, &proposal) {
                self.memory
                    .update_proposal_trace_state(&existing.id, "superseded")
                    .ok();
                self.superseded_proposals.insert(existing.id);
            } else {
                return None;
            }
        }

        if let Some(context_budget) = context_budget.clone() {
            self.proposal_context_budgets
                .insert(proposal.id.clone(), context_budget);
        }
        self.proposals.push(proposal.clone());
        let created_at = self
            .observations
            .iter()
            .find(|observation| observation.id == proposal.observation_id)
            .map(|observation| observation.created_at)
            .unwrap_or_else(now_ms);
        self.memory
            .record_proposal_trace(
                &proposal_trace_summary(&proposal, "pending", context_budget),
                created_at,
            )
            .ok();
        self.memory
            .record_context_recalls(
                &self.project_id,
                &proposal.id,
                &proposal.observation_id,
                &proposal.evidence,
                created_at,
            )
            .ok();
        for operation in proposal
            .operations
            .iter()
            .filter(|operation| operation_is_write_capable(operation))
        {
            self.push_operation_lifecycle(
                Some(proposal.id.clone()),
                operation,
                WriterOperationLifecycleState::Proposed,
                None,
                None,
                None,
                created_at,
            );
        }
        Some(proposal)
    }

    fn is_pending_proposal(&self, proposal: &AgentProposal) -> bool {
        !self.superseded_proposals.contains(&proposal.id)
            && !self
                .feedback_events
                .iter()
                .any(|f| f.proposal_id == proposal.id)
            && !proposal_expired(proposal, now_ms())
    }

    fn suppress_slot_after_feedback(
        &mut self,
        proposal: &AgentProposal,
        feedback: &ProposalFeedback,
    ) {
        let ttl_ms = match feedback.action {
            FeedbackAction::Snoozed => 10 * 60 * 1_000,
            FeedbackAction::Rejected => 5 * 60 * 1_000,
            FeedbackAction::Edited => 2 * 60 * 1_000,
            FeedbackAction::Accepted | FeedbackAction::Explained => return,
        };
        self.suppressed_slots.push(SuppressedProposalSlot {
            slot: suppression_slot_key(proposal),
            until: feedback.created_at.saturating_add(ttl_ms),
        });
    }

    fn is_slot_suppressed(&self, proposal: &AgentProposal) -> bool {
        let slot = suppression_slot_key(proposal);
        let now = now_ms();
        self.suppressed_slots
            .iter()
            .any(|entry| entry.slot == slot && entry.until > now)
    }

    fn prune_suppressed_slots(&mut self, now: u64) {
        self.suppressed_slots.retain(|entry| entry.until > now);
    }

    pub fn execute_operation(
        &mut self,
        operation: WriterOperation,
        current_content: &str,
        current_revision: &str,
    ) -> Result<OperationResult, String> {
        if operation_is_write_capable(&operation) {
            let result = OperationResult {
                success: false,
                operation,
                error: Some(super::operation::OperationError::approval_required(
                    "Write-capable operations require an explicit surfaced approval context",
                )),
                revision_after: None,
            };
            self.record_operation_result_lifecycle(&result, None, None);
            return Ok(result);
        }

        let result = self.execute_operation_inner(operation, current_content, current_revision)?;
        self.record_operation_result_lifecycle(&result, None, None);
        Ok(result)
    }

    fn execute_operation_inner(
        &mut self,
        operation: WriterOperation,
        current_content: &str,
        current_revision: &str,
    ) -> Result<OperationResult, String> {
        let result: Result<OperationResult, String> = match &operation {
            WriterOperation::TextInsert { .. } | WriterOperation::TextReplace { .. } => {
                match execute_text_operation(&operation, current_content, current_revision) {
                    Ok((_new_content, new_revision)) => Ok(OperationResult {
                        success: true,
                        operation,
                        error: None,
                        revision_after: Some(new_revision),
                    }),
                    Err(e) => Ok(OperationResult {
                        success: false,
                        operation,
                        error: Some(e),
                        revision_after: None,
                    }),
                }
            }
            WriterOperation::TextAnnotate {
                chapter,
                from,
                to,
                message,
                severity,
            } => {
                let source = format!("text:{}:{}-{}", chapter, from, to);
                self.memory
                    .record_decision(
                        chapter,
                        &format!("Annotation: {:?}", severity),
                        "annotated_text",
                        &[],
                        message,
                        &[source],
                    )
                    .map_err(|e| format!("annotation: {}", e))?;
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
            WriterOperation::CanonUpsertEntity { entity } => {
                self.memory
                    .upsert_canon_entity(
                        &entity.kind,
                        &entity.name,
                        &entity.aliases,
                        &entity.summary,
                        &entity.attributes,
                        entity.confidence,
                    )
                    .map_err(|e| format!("canon: {}", e))?;
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
            WriterOperation::CanonUpdateAttribute {
                entity,
                attribute,
                value,
                confidence,
            } => {
                let rationale = format!(
                    "Author confirmed canon update: {}.{} = {}",
                    entity, attribute, value
                );
                self.memory
                    .update_canon_attribute(entity, attribute, value, *confidence)
                    .map_err(|e| format!("canon: {}", e))?;
                self.memory
                    .record_decision(
                        self.active_chapter.as_deref().unwrap_or("project"),
                        &format!("Canon update: {}", entity),
                        "updated_canon",
                        &[],
                        &rationale,
                        &[format!("canon:{}:{}", entity, attribute)],
                    )
                    .ok();
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
            WriterOperation::CanonUpsertRule { rule } => {
                self.memory
                    .upsert_canon_rule(
                        &rule.rule,
                        &rule.category,
                        rule.priority,
                        "writer_operation",
                    )
                    .map_err(|e| format!("canon rule: {}", e))?;
                self.memory
                    .record_decision(
                        self.active_chapter.as_deref().unwrap_or("project"),
                        &format!("Canon rule: {}", rule.category),
                        "upserted_canon_rule",
                        &[],
                        &rule.rule,
                        &[format!("canon_rule:{}", rule.category)],
                    )
                    .ok();
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
            WriterOperation::PromiseAdd { promise } => {
                self.memory
                    .add_promise(
                        &promise.kind,
                        &promise.title,
                        &promise.description,
                        &promise.introduced_chapter,
                        &promise.expected_payoff,
                        promise.priority,
                    )
                    .map_err(|e| format!("promise: {}", e))?;
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
            WriterOperation::PromiseResolve {
                promise_id,
                chapter,
            } => {
                let id = promise_id
                    .parse::<i64>()
                    .map_err(|_| format!("promise: invalid promise id '{}'", promise_id))?;
                let resolved = self
                    .memory
                    .resolve_promise(id, chapter)
                    .map_err(|e| format!("promise: {}", e))?;
                Ok(OperationResult {
                    success: resolved,
                    operation,
                    error: if resolved {
                        None
                    } else {
                        Some(super::operation::OperationError::invalid(
                            "Promise is already resolved or does not exist",
                        ))
                    },
                    revision_after: None,
                })
            }
            WriterOperation::PromiseDefer {
                promise_id,
                chapter,
                expected_payoff,
            } => {
                let id = promise_id
                    .parse::<i64>()
                    .map_err(|_| format!("promise: invalid promise id '{}'", promise_id))?;
                let deferred = self
                    .memory
                    .defer_promise(id, expected_payoff)
                    .map_err(|e| format!("promise: {}", e))?;
                if deferred {
                    self.memory
                        .record_decision(
                            chapter,
                            &format!("Defer promise {}", promise_id),
                            "deferred_promise",
                            &[],
                            &format!(
                                "Author deferred promise {} to {}",
                                promise_id, expected_payoff
                            ),
                            &[format!("promise:{}", promise_id)],
                        )
                        .ok();
                }
                Ok(OperationResult {
                    success: deferred,
                    operation,
                    error: if deferred {
                        None
                    } else {
                        Some(super::operation::OperationError::invalid(
                            "Promise is already closed or does not exist",
                        ))
                    },
                    revision_after: None,
                })
            }
            WriterOperation::PromiseAbandon {
                promise_id,
                chapter,
                reason,
            } => {
                let id = promise_id
                    .parse::<i64>()
                    .map_err(|_| format!("promise: invalid promise id '{}'", promise_id))?;
                let abandoned = self
                    .memory
                    .abandon_promise(id)
                    .map_err(|e| format!("promise: {}", e))?;
                if abandoned {
                    self.memory
                        .record_decision(
                            chapter,
                            &format!("Abandon promise {}", promise_id),
                            "abandoned_promise",
                            &["resolve".to_string(), "defer".to_string()],
                            reason,
                            &[format!("promise:{}", promise_id)],
                        )
                        .ok();
                }
                Ok(OperationResult {
                    success: abandoned,
                    operation,
                    error: if abandoned {
                        None
                    } else {
                        Some(super::operation::OperationError::invalid(
                            "Promise is already closed or does not exist",
                        ))
                    },
                    revision_after: None,
                })
            }
            WriterOperation::StyleUpdatePreference { key, value } => {
                self.memory
                    .upsert_style_preference(key, value, true)
                    .map_err(|e| format!("style preference: {}", e))?;
                self.memory
                    .record_decision(
                        self.active_chapter.as_deref().unwrap_or("project"),
                        &format!("Style preference: {}", key),
                        "updated_style_preference",
                        &[],
                        value,
                        &[format!("style:{}", key)],
                    )
                    .ok();
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
            WriterOperation::StoryContractUpsert { contract } => {
                let summary = StoryContractSummary {
                    project_id: contract.project_id.clone(),
                    title: contract.title.clone(),
                    genre: contract.genre.clone(),
                    target_reader: contract.target_reader.clone(),
                    reader_promise: contract.reader_promise.clone(),
                    first_30_chapter_promise: contract.first_30_chapter_promise.clone(),
                    main_conflict: contract.main_conflict.clone(),
                    structural_boundary: contract.structural_boundary.clone(),
                    tone_contract: contract.tone_contract.clone(),
                    updated_at: String::new(),
                };
                if let Some(error) = validate_story_contract_summary(&summary) {
                    Ok(OperationResult {
                        success: false,
                        operation,
                        error: Some(super::operation::OperationError::invalid(&error)),
                        revision_after: None,
                    })
                } else {
                    self.memory
                        .upsert_story_contract(&summary)
                        .map_err(|e| format!("story contract: {}", e))?;
                    self.memory
                        .record_decision(
                            "project",
                            "Story contract",
                            "updated_story_contract",
                            &[],
                            &summary.render_for_context(),
                            &[format!("story_contract:{}", summary.project_id)],
                        )
                        .ok();
                    Ok(OperationResult {
                        success: true,
                        operation,
                        error: None,
                        revision_after: None,
                    })
                }
            }
            WriterOperation::ChapterMissionUpsert { mission } => {
                let normalized_status = normalize_chapter_mission_status(&mission.status);
                let summary = ChapterMissionSummary {
                    id: 0,
                    project_id: mission.project_id.clone(),
                    chapter_title: mission.chapter_title.clone(),
                    mission: mission.mission.clone(),
                    must_include: mission.must_include.clone(),
                    must_not: mission.must_not.clone(),
                    expected_ending: mission.expected_ending.clone(),
                    status: normalized_status,
                    source_ref: mission.source_ref.clone(),
                    updated_at: String::new(),
                };
                if let Some(error) = validate_chapter_mission_summary(&summary) {
                    Ok(OperationResult {
                        success: false,
                        operation,
                        error: Some(super::operation::OperationError::invalid(&error)),
                        revision_after: None,
                    })
                } else {
                    self.memory
                        .upsert_chapter_mission(&summary)
                        .map_err(|e| format!("chapter mission: {}", e))?;
                    self.memory
                        .record_decision(
                            &summary.chapter_title,
                            "Chapter mission",
                            "updated_chapter_mission",
                            &[],
                            &summary.render_for_context(),
                            &[format!(
                                "chapter_mission:{}:{}",
                                summary.project_id, summary.chapter_title
                            )],
                        )
                        .ok();
                    Ok(OperationResult {
                        success: true,
                        operation,
                        error: None,
                        revision_after: None,
                    })
                }
            }
            WriterOperation::OutlineUpdate { .. } => Ok(OperationResult {
                success: false,
                operation,
                error: Some(super::operation::OperationError::invalid(
                    "outline.update requires project storage runtime",
                )),
                revision_after: None,
            }),
        };
        result
    }

    pub fn approve_editor_operation(
        &mut self,
        operation: WriterOperation,
        current_revision: &str,
    ) -> Result<OperationResult, String> {
        self.approve_editor_operation_with_approval(operation, current_revision, None)
    }

    pub fn approve_editor_operation_with_approval(
        &mut self,
        operation: WriterOperation,
        current_revision: &str,
        approval: Option<&super::operation::OperationApproval>,
    ) -> Result<OperationResult, String> {
        let requires_approval = operation_is_write_capable(&operation);
        if requires_approval && !approval.is_some_and(|context| context.is_valid_for_write()) {
            let result = OperationResult {
                success: false,
                operation,
                error: Some(super::operation::OperationError::approval_required(
                    "Write-capable operations require an explicit surfaced approval context",
                )),
                revision_after: None,
            };
            self.record_operation_result_lifecycle(&result, approval, None);
            return Ok(result);
        }

        if let Some(context) = approval {
            self.memory
                .record_decision(
                    self.active_chapter.as_deref().unwrap_or("project"),
                    &format!("Approved operation: {}", operation_kind_label(&operation)),
                    "approved_writer_operation",
                    &[],
                    &format!(
                        "{} approved from {}: {}",
                        context.actor, context.source, context.reason
                    ),
                    &approval_sources(context),
                )
                .ok();
        }

        if requires_approval {
            self.push_operation_lifecycle(
                approval.and_then(|context| context.proposal_id.clone()),
                &operation,
                WriterOperationLifecycleState::Approved,
                approval.map(|context| context.source.clone()),
                None,
                None,
                now_ms(),
            );
        }

        let result = match &operation {
            WriterOperation::TextInsert { revision, .. }
            | WriterOperation::TextReplace { revision, .. } => {
                if revision != current_revision {
                    Ok(OperationResult {
                        success: false,
                        operation,
                        error: Some(super::operation::OperationError::conflict(
                            "Proposal is stale; the chapter changed since it was created",
                        )),
                        revision_after: None,
                    })
                } else {
                    Ok(OperationResult {
                        success: true,
                        operation,
                        error: None,
                        revision_after: Some(current_revision.to_string()),
                    })
                }
            }
            _ => self.execute_operation_inner(operation, "", current_revision),
        }?;
        self.record_operation_result_lifecycle(&result, approval, None);
        Ok(result)
    }

    pub fn record_operation_durable_save(
        &mut self,
        proposal_id: Option<String>,
        operation: WriterOperation,
        save_result: String,
    ) -> Result<(), String> {
        if !operation_is_write_capable(&operation) {
            return Ok(());
        }

        let normalized = if save_result.trim().is_empty() {
            "saved".to_string()
        } else {
            save_result
        };
        let state = if save_result_is_success(&normalized) {
            WriterOperationLifecycleState::DurablySaved
        } else {
            WriterOperationLifecycleState::Rejected
        };
        let resolved_proposal_id =
            proposal_id.or_else(|| self.proposal_id_for_operation(&operation));
        let approval_source = resolved_proposal_id
            .as_deref()
            .and_then(|id| self.latest_approval_source_for_operation(id, &operation));
        self.push_operation_lifecycle(
            resolved_proposal_id,
            &operation,
            state,
            approval_source,
            Some(normalized),
            None,
            now_ms(),
        );
        Ok(())
    }

    pub fn status(&self) -> WriterAgentStatus {
        let open = self
            .memory
            .get_open_promises()
            .map(|p| p.len())
            .unwrap_or(0);
        let now = now_ms();
        WriterAgentStatus {
            project_id: self.project_id.clone(),
            session_id: self.session_id.clone(),
            active_chapter: self.active_chapter.clone(),
            observation_count: self.observation_counter,
            proposal_count: self.proposal_counter,
            open_promise_count: open,
            pending_proposals: self
                .proposals
                .iter()
                .filter(|p| {
                    !self.superseded_proposals.contains(&p.id)
                        && !self.feedback_events.iter().any(|f| f.proposal_id == p.id)
                        && !proposal_expired(p, now)
                })
                .count(),
            total_feedback_events: self.feedback_events.len() as u64,
        }
    }

    pub fn pending_proposals(&self) -> Vec<AgentProposal> {
        let now = now_ms();
        let mut proposals = self
            .proposals
            .iter()
            .filter(|proposal| {
                !self.superseded_proposals.contains(&proposal.id)
                    && !self
                        .feedback_events
                        .iter()
                        .any(|feedback| feedback.proposal_id == proposal.id)
                    && !proposal_expired(proposal, now)
            })
            .cloned()
            .collect::<Vec<_>>();
        proposals.sort_by(|a, b| {
            priority_weight(&b.priority)
                .cmp(&priority_weight(&a.priority))
                .then_with(|| b.confidence.total_cmp(&a.confidence))
        });
        proposals
    }

    pub fn story_review_queue(&self) -> Vec<StoryReviewQueueEntry> {
        let now = now_ms();
        let mut entries = self
            .proposals
            .iter()
            .filter(|proposal| {
                proposal.kind != ProposalKind::Ghost
                    && !self.superseded_proposals.contains(&proposal.id)
            })
            .map(|proposal| {
                let created_at = self
                    .observations
                    .iter()
                    .find(|observation| observation.id == proposal.observation_id)
                    .map(|observation| observation.created_at)
                    .unwrap_or(0);
                let status = self.story_review_queue_status(proposal, now);
                story_review_queue_entry(proposal, created_at, status)
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            queue_status_weight(&b.status)
                .cmp(&queue_status_weight(&a.status))
                .then_with(|| {
                    queue_severity_weight(&b.severity).cmp(&queue_severity_weight(&a.severity))
                })
                .then_with(|| b.created_at.cmp(&a.created_at))
        });
        entries
    }

    pub fn story_debt_snapshot(&self) -> StoryDebtSnapshot {
        let mut entries = Vec::new();
        let chapter_title = self.active_chapter.clone();
        let review_entries = self.story_review_queue();

        for entry in review_entries.iter().filter(|entry| {
            matches!(
                entry.status,
                StoryReviewQueueStatus::Pending | StoryReviewQueueStatus::Snoozed
            )
        }) {
            entries.push(story_debt_from_review_entry(entry, &chapter_title));
        }

        let queued_promise_ids = entries
            .iter()
            .flat_map(|entry| &entry.evidence)
            .filter(|evidence| evidence.source == EvidenceSource::PromiseLedger)
            .map(|evidence| evidence.reference.clone())
            .collect::<HashSet<_>>();

        for promise in self.memory.get_open_promise_summaries().unwrap_or_default() {
            if queued_promise_ids.contains(&promise.title) {
                continue;
            }
            entries.push(story_debt_from_open_promise(&promise, &chapter_title));
        }

        entries.sort_by(|a, b| {
            story_debt_status_weight(&b.status)
                .cmp(&story_debt_status_weight(&a.status))
                .then_with(|| {
                    story_debt_category_weight(&b.category)
                        .cmp(&story_debt_category_weight(&a.category))
                })
                .then_with(|| {
                    queue_severity_weight(&b.severity).cmp(&queue_severity_weight(&a.severity))
                })
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        let open_count = entries
            .iter()
            .filter(|entry| entry.status == StoryDebtStatus::Open)
            .count();
        let contract_count = entries
            .iter()
            .filter(|entry| entry.category == StoryDebtCategory::StoryContract)
            .count();
        let mission_count = entries
            .iter()
            .filter(|entry| entry.category == StoryDebtCategory::ChapterMission)
            .count();
        let canon_risk_count = entries
            .iter()
            .filter(|entry| entry.category == StoryDebtCategory::CanonRisk)
            .count();
        let promise_count = entries
            .iter()
            .filter(|entry| entry.category == StoryDebtCategory::Promise)
            .count();
        let pacing_count = entries
            .iter()
            .filter(|entry| entry.category == StoryDebtCategory::Pacing)
            .count();

        StoryDebtSnapshot {
            chapter_title,
            total: entries.len(),
            open_count,
            contract_count,
            mission_count,
            canon_risk_count,
            promise_count,
            pacing_count,
            entries,
        }
    }

    fn contract_quality(&self) -> StoryContractQuality {
        self.memory
            .get_story_contract(&self.project_id)
            .ok()
            .flatten()
            .map(|contract| contract.quality())
            .unwrap_or(StoryContractQuality::Missing)
    }

    pub fn ledger_snapshot(&self) -> WriterAgentLedgerSnapshot {
        let active_chapter_mission = self.active_chapter.as_deref().and_then(|chapter| {
            self.memory
                .get_chapter_mission(&self.project_id, chapter)
                .ok()
                .flatten()
        });
        let recent_chapter_results = self
            .memory
            .list_recent_chapter_results(&self.project_id, 20)
            .unwrap_or_default();
        let open_promises = self.memory.get_open_promise_summaries().unwrap_or_default();
        let next_beat = derive_next_beat(
            self.active_chapter.as_deref(),
            active_chapter_mission.as_ref(),
            &recent_chapter_results,
            &open_promises,
        );

        WriterAgentLedgerSnapshot {
            story_contract: self
                .memory
                .get_story_contract(&self.project_id)
                .unwrap_or_default(),
            active_chapter_mission,
            chapter_missions: self
                .memory
                .list_chapter_missions(&self.project_id, 50)
                .unwrap_or_default(),
            recent_chapter_results,
            next_beat,
            canon_entities: self.memory.list_canon_entities().unwrap_or_default(),
            canon_rules: self.memory.list_canon_rules(20).unwrap_or_default(),
            open_promises,
            recent_decisions: self.memory.list_recent_decisions(20).unwrap_or_default(),
            memory_audit: self.memory.list_memory_audit(30).unwrap_or_default(),
            context_recalls: self
                .memory
                .list_context_recalls(&self.project_id, 30)
                .unwrap_or_default(),
        }
    }

    pub fn trace_snapshot(&self, limit: usize) -> WriterAgentTraceSnapshot {
        let now = now_ms();
        let persisted_observations = self
            .memory
            .list_observation_traces(limit)
            .unwrap_or_default();
        let persisted_proposals = self.memory.list_proposal_traces(limit).unwrap_or_default();
        let persisted_feedback = self.memory.list_feedback_traces(limit).unwrap_or_default();

        WriterAgentTraceSnapshot {
            recent_observations: if persisted_observations.is_empty() {
                self.observations
                    .iter()
                    .rev()
                    .take(limit)
                    .map(|observation| WriterObservationTrace {
                        id: observation.id.clone(),
                        created_at: observation.created_at,
                        reason: format!("{:?}", observation.reason),
                        chapter_title: observation.chapter_title.clone(),
                        paragraph_snippet: snippet(&observation.paragraph, 120),
                    })
                    .collect()
            } else {
                persisted_observations
                    .into_iter()
                    .map(|observation| WriterObservationTrace {
                        id: observation.id,
                        created_at: observation.created_at,
                        reason: observation.reason,
                        chapter_title: observation.chapter_title,
                        paragraph_snippet: observation.paragraph_snippet,
                    })
                    .collect()
            },
            task_packets: self
                .task_packets
                .iter()
                .rev()
                .take(limit)
                .cloned()
                .collect(),
            recent_proposals: if persisted_proposals.is_empty() {
                self.proposals
                    .iter()
                    .rev()
                    .take(limit)
                    .map(|proposal| WriterProposalTrace {
                        id: proposal.id.clone(),
                        observation_id: proposal.observation_id.clone(),
                        kind: format!("{:?}", proposal.kind),
                        priority: format!("{:?}", proposal.priority),
                        state: self.proposal_state(proposal, now),
                        confidence: proposal.confidence,
                        preview_snippet: snippet(&proposal.preview, 120),
                        evidence: proposal.evidence.clone(),
                        context_budget: self.proposal_context_budgets.get(&proposal.id).cloned(),
                    })
                    .collect()
            } else {
                persisted_proposals
                    .into_iter()
                    .map(|proposal| WriterProposalTrace {
                        id: proposal.id,
                        observation_id: proposal.observation_id,
                        kind: proposal.kind,
                        priority: proposal.priority,
                        state: trace_state_with_expiry(&proposal.state, proposal.expires_at, now),
                        confidence: proposal.confidence,
                        preview_snippet: proposal.preview_snippet,
                        evidence: proposal.evidence,
                        context_budget: proposal.context_budget,
                    })
                    .collect()
            },
            recent_feedback: if persisted_feedback.is_empty() {
                self.feedback_events
                    .iter()
                    .rev()
                    .take(limit)
                    .map(|feedback| WriterFeedbackTrace {
                        proposal_id: feedback.proposal_id.clone(),
                        action: format!("{:?}", feedback.action),
                        reason: feedback.reason.clone(),
                        created_at: feedback.created_at,
                    })
                    .collect()
            } else {
                persisted_feedback
                    .into_iter()
                    .map(|feedback| WriterFeedbackTrace {
                        proposal_id: feedback.proposal_id,
                        action: feedback.action,
                        reason: feedback.reason,
                        created_at: feedback.created_at,
                    })
                    .collect()
            },
            operation_lifecycle: self
                .operation_lifecycle
                .iter()
                .rev()
                .take(limit)
                .cloned()
                .collect(),
            context_recalls: self
                .memory
                .list_context_recalls(&self.project_id, limit)
                .unwrap_or_default(),
            product_metrics: self.product_metrics(),
        }
    }

    pub fn export_trajectory(&self, limit: usize) -> super::trajectory::WriterTrajectoryExport {
        super::trajectory::export_trace_snapshot(
            &self.project_id,
            &self.session_id,
            &self.trace_snapshot(limit),
        )
    }

    pub fn record_task_packet(
        &mut self,
        observation_id: impl Into<String>,
        task: impl Into<String>,
        packet: TaskPacket,
    ) -> Result<(), String> {
        packet.validate().map_err(|error| error.to_string())?;
        self.push_task_packet_trace(observation_id.into(), task.into(), packet);
        Ok(())
    }

    fn record_task_packet_for(
        &mut self,
        task: AgentTask,
        observation: &WriterObservation,
        context_pack: &WritingContextPack,
        objective: &str,
        success_criteria: Vec<String>,
    ) {
        let packet = build_task_packet_for_observation(
            &self.project_id,
            &self.session_id,
            task.clone(),
            observation,
            context_pack,
            objective,
            success_criteria,
        );
        if let Err(error) = packet.validate() {
            tracing::warn!(
                "Skipping invalid writer task packet for {:?}: {}",
                task,
                error
            );
            return;
        }
        self.push_task_packet_trace(observation.id.clone(), format!("{:?}", task), packet);
    }

    fn push_task_packet_trace(&mut self, observation_id: String, task: String, packet: TaskPacket) {
        let coverage = packet.foundation_coverage();
        self.task_packets.push(WriterTaskPacketTrace {
            id: packet.id.clone(),
            observation_id,
            task,
            objective: packet.objective.clone(),
            scope: packet.scope_label(),
            intent: packet.intent.as_ref().map(|intent| format!("{:?}", intent)),
            required_context_count: packet.required_context.len(),
            belief_count: packet.beliefs.len(),
            success_criteria_count: packet.success_criteria.len(),
            max_side_effect_level: format!("{:?}", packet.tool_policy.max_side_effect_level),
            feedback_checkpoint_count: packet.feedback.checkpoints.len(),
            foundation_complete: coverage.is_complete(),
            packet,
        });
    }

    fn push_operation_lifecycle(
        &mut self,
        proposal_id: Option<String>,
        operation: &WriterOperation,
        state: WriterOperationLifecycleState,
        approval_source: Option<String>,
        save_result: Option<String>,
        feedback_result: Option<String>,
        created_at: u64,
    ) {
        self.operation_lifecycle
            .push(WriterOperationLifecycleTrace {
                source_task: proposal_id
                    .as_deref()
                    .and_then(|id| self.proposals.iter().find(|proposal| proposal.id == id))
                    .map(|proposal| format!("{:?}", proposal.kind)),
                proposal_id,
                operation_kind: operation_kind_label(operation).to_string(),
                approval_source,
                affected_scope: operation_affected_scope(operation),
                state,
                save_result,
                feedback_result,
                created_at,
            });
    }

    fn product_metrics(&self) -> WriterProductMetrics {
        product_metrics_from_trace(
            &self.proposals,
            &self.feedback_events,
            &self.operation_lifecycle,
            self.memory.list_context_recalls(&self.project_id, 50),
            self.memory.list_chapter_missions(&self.project_id, 250),
        )
    }

    fn record_operation_result_lifecycle(
        &mut self,
        result: &OperationResult,
        approval: Option<&super::operation::OperationApproval>,
        save_result_override: Option<String>,
    ) {
        if !operation_is_write_capable(&result.operation) {
            return;
        }

        let proposal_id = approval
            .and_then(|context| context.proposal_id.clone())
            .or_else(|| self.proposal_id_for_operation(&result.operation));
        let approval_source = approval.map(|context| context.source.clone());
        let save_result = save_result_override.or_else(|| {
            result
                .error
                .as_ref()
                .map(|error| format!("{}:{}", error.code, error.message))
        });
        let state = if result.success {
            WriterOperationLifecycleState::Applied
        } else {
            WriterOperationLifecycleState::Rejected
        };
        self.push_operation_lifecycle(
            proposal_id.clone(),
            &result.operation,
            state,
            approval_source.clone(),
            save_result.clone(),
            None,
            now_ms(),
        );

        if result.success && operation_has_kernel_durable_save(&result.operation) {
            self.push_operation_lifecycle(
                proposal_id,
                &result.operation,
                WriterOperationLifecycleState::DurablySaved,
                approval_source,
                Some("kernel_write:ok".to_string()),
                None,
                now_ms(),
            );
        }
    }

    fn proposal_positive_feedback_ready(&self, proposal: &AgentProposal) -> bool {
        proposal
            .operations
            .iter()
            .filter(|operation| operation_is_write_capable(operation))
            .all(|operation| {
                if operation_requires_durable_save(operation) {
                    self.lifecycle_has_state(
                        &proposal.id,
                        operation,
                        WriterOperationLifecycleState::DurablySaved,
                    )
                } else {
                    self.lifecycle_has_state(
                        &proposal.id,
                        operation,
                        WriterOperationLifecycleState::Applied,
                    ) || self.lifecycle_has_state(
                        &proposal.id,
                        operation,
                        WriterOperationLifecycleState::DurablySaved,
                    )
                }
            })
    }

    fn proposal_id_for_operation(&self, operation: &WriterOperation) -> Option<String> {
        let kind = operation_kind_label(operation);
        let scope = operation_affected_scope(operation);
        self.proposals.iter().rev().find_map(|proposal| {
            proposal
                .operations
                .iter()
                .any(|candidate| {
                    operation_kind_label(candidate) == kind
                        && operation_affected_scope(candidate) == scope
                })
                .then(|| proposal.id.clone())
        })
    }

    fn lifecycle_has_state(
        &self,
        proposal_id: &str,
        operation: &WriterOperation,
        state: WriterOperationLifecycleState,
    ) -> bool {
        self.operation_lifecycle.iter().any(|trace| {
            trace.proposal_id.as_deref() == Some(proposal_id)
                && trace.operation_kind == operation_kind_label(operation)
                && trace.affected_scope == operation_affected_scope(operation)
                && trace.state == state
        })
    }

    fn latest_approval_source_for_operation(
        &self,
        proposal_id: &str,
        operation: &WriterOperation,
    ) -> Option<String> {
        let kind = operation_kind_label(operation);
        let scope = operation_affected_scope(operation);
        self.operation_lifecycle
            .iter()
            .rev()
            .find(|trace| {
                trace.proposal_id.as_deref() == Some(proposal_id)
                    && trace.operation_kind == kind
                    && trace.affected_scope == scope
                    && trace.state == WriterOperationLifecycleState::Approved
            })
            .and_then(|trace| trace.approval_source.clone())
    }

    fn proposal_state(&self, proposal: &AgentProposal, now: u64) -> String {
        if self.superseded_proposals.contains(&proposal.id) {
            return "superseded".to_string();
        }
        if let Some(feedback) = self
            .feedback_events
            .iter()
            .find(|feedback| feedback.proposal_id == proposal.id)
        {
            return format!("feedback:{:?}", feedback.action);
        }
        if proposal_expired(proposal, now) {
            return "expired".to_string();
        }
        "pending".to_string()
    }

    fn story_review_queue_status(
        &self,
        proposal: &AgentProposal,
        now: u64,
    ) -> StoryReviewQueueStatus {
        if proposal_expired(proposal, now) {
            return StoryReviewQueueStatus::Expired;
        }
        if let Some(feedback) = self
            .feedback_events
            .iter()
            .find(|feedback| feedback.proposal_id == proposal.id)
        {
            return match feedback.action {
                FeedbackAction::Accepted | FeedbackAction::Edited => {
                    StoryReviewQueueStatus::Accepted
                }
                FeedbackAction::Snoozed => StoryReviewQueueStatus::Snoozed,
                FeedbackAction::Rejected | FeedbackAction::Explained => {
                    StoryReviewQueueStatus::Ignored
                }
            };
        }
        StoryReviewQueueStatus::Pending
    }

    fn calibrate_chapter_mission(
        &self,
        observation: &WriterObservation,
        result: &ChapterResultSummary,
    ) -> Result<(), String> {
        let Some(chapter_title) = observation.chapter_title.as_deref() else {
            return Ok(());
        };
        let Some(mut mission) = self
            .memory
            .get_chapter_mission(&self.project_id, chapter_title)
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };

        let status = calibrated_mission_status(&mission, result);
        if mission.status == status {
            return Ok(());
        }

        mission.status = status;
        mission.source_ref = format!("result_feedback:{}", result.source_ref);
        self.memory
            .upsert_chapter_mission(&mission)
            .map_err(|e| e.to_string())?;
        self.memory
            .record_decision(
                chapter_title,
                "Chapter mission calibration",
                &format!("mission_status:{}", mission.status),
                &[],
                &mission.render_for_context(),
                &[result.source_ref.clone()],
            )
            .ok();
        Ok(())
    }

    fn touch_promise_last_seen_from_result(
        &self,
        result: &ChapterResultSummary,
    ) -> Result<(), String> {
        let haystack = mission_result_haystack(result);
        for promise in self
            .memory
            .get_open_promise_summaries()
            .map_err(|e| e.to_string())?
        {
            let title_hit =
                !promise.title.trim().is_empty() && haystack.contains(promise.title.trim());
            let description_hit = !promise.description.trim().is_empty()
                && cue_hit_score(&promise.description, &haystack) > 0;
            if title_hit || description_hit {
                self.memory
                    .touch_promise_last_seen(promise.id, &result.chapter_title, &result.source_ref)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

pub(crate) fn snippet(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn context_budget_trace(pack: &WritingContextPack) -> ContextBudgetTrace {
    ContextBudgetTrace {
        task: format!("{:?}", pack.task),
        used: pack.budget_report.used,
        total_budget: pack.budget_report.total_budget,
        wasted: pack.budget_report.wasted,
        source_reports: pack
            .budget_report
            .source_reports
            .iter()
            .map(|source| ContextSourceBudgetTrace {
                source: source.source.clone(),
                requested: source.requested,
                provided: source.provided,
                truncated: source.truncated,
                reason: source.reason.clone(),
                truncation_reason: source.truncation_reason.clone(),
            })
            .collect(),
    }
}

pub fn build_task_packet_for_observation(
    project_id: &str,
    session_id: &str,
    task: AgentTask,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
    objective: &str,
    success_criteria: Vec<String>,
) -> TaskPacket {
    let scope_ref = observation
        .chapter_title
        .clone()
        .or_else(|| {
            observation
                .cursor
                .as_ref()
                .map(|cursor| format!("{}..{}", cursor.from, cursor.to))
        })
        .unwrap_or_else(|| project_id.to_string());
    let scope = match task {
        AgentTask::GhostWriting => TaskScope::CursorWindow,
        AgentTask::InlineRewrite => TaskScope::Selection,
        AgentTask::ChapterGeneration => TaskScope::Chapter,
        AgentTask::ManualRequest => TaskScope::Chapter,
        AgentTask::ContinuityDiagnostic | AgentTask::CanonMaintenance => TaskScope::Scene,
        AgentTask::ProposalEvaluation => TaskScope::Custom,
    };
    let mut packet = TaskPacket::new(
        format!("{}:{}:{:?}", session_id, observation.id, task),
        objective,
        scope,
        observation.created_at,
    );
    packet.scope_ref = Some(scope_ref);
    packet.intent = Some(match task {
        AgentTask::GhostWriting | AgentTask::ChapterGeneration | AgentTask::InlineRewrite => {
            agent_harness_core::Intent::GenerateContent
        }
        AgentTask::ContinuityDiagnostic
        | AgentTask::CanonMaintenance
        | AgentTask::ProposalEvaluation => agent_harness_core::Intent::AnalyzeText,
        AgentTask::ManualRequest => agent_harness_core::Intent::Chat,
    });
    packet.constraints = constraints_for_task(&task);
    packet.success_criteria = success_criteria;
    packet.beliefs = beliefs_from_context_pack(context_pack);
    packet.required_context = required_context_from_pack(context_pack);
    packet.tool_policy = tool_policy_for_task(&task);
    packet.feedback = feedback_contract_for_task(&task);
    packet
}

fn constraints_for_task(task: &AgentTask) -> Vec<String> {
    let mut constraints = vec![
        "Preserve established canon unless the user explicitly approves a change.".to_string(),
        "Respect active chapter mission and story contract boundaries.".to_string(),
    ];
    match task {
        AgentTask::GhostWriting => {
            constraints.push("Keep proactive text short and easy to ignore.".to_string());
        }
        AgentTask::ManualRequest => {
            constraints
                .push("Answer the author directly before proposing broad rewrites.".to_string());
        }
        AgentTask::ChapterGeneration => {
            constraints
                .push("Generate chapter prose only; no analysis or markdown wrapper.".to_string());
        }
        AgentTask::InlineRewrite => {
            constraints
                .push("Limit edits to the selected range or cursor insertion point.".to_string());
        }
        AgentTask::ContinuityDiagnostic | AgentTask::CanonMaintenance => {
            constraints.push("Surface evidence before recommending canon changes.".to_string());
        }
        AgentTask::ProposalEvaluation => {
            constraints
                .push("Judge the proposal against evidence and feedback history.".to_string());
        }
    }
    constraints
}

fn beliefs_from_context_pack(context_pack: &WritingContextPack) -> Vec<TaskBelief> {
    let mut beliefs = Vec::new();
    for source in &context_pack.sources {
        if beliefs.len() >= 8 {
            break;
        }
        let subject = format!("{:?}", source.source);
        let statement = snippet(&source.content, 180);
        if statement.trim().is_empty() {
            continue;
        }
        beliefs.push(TaskBelief::new(
            subject,
            statement,
            belief_confidence(&source.source),
        ));
    }

    if beliefs.is_empty() {
        beliefs.push(TaskBelief::new(
            "editor_context",
            "Only the current editor observation is available for this task.",
            0.55,
        ));
    }
    beliefs
}

fn belief_confidence(source: &ContextSource) -> f32 {
    match source {
        ContextSource::SystemContract
        | ContextSource::ProjectBrief
        | ContextSource::ChapterMission
        | ContextSource::CanonSlice
        | ContextSource::PromiseSlice => 0.9,
        ContextSource::ResultFeedback | ContextSource::DecisionSlice | ContextSource::NextBeat => {
            0.8
        }
        ContextSource::CursorPrefix
        | ContextSource::CursorSuffix
        | ContextSource::SelectedText
        | ContextSource::PreviousChapter
        | ContextSource::NextChapter
        | ContextSource::NeighborText => 0.75,
        ContextSource::AuthorStyle | ContextSource::OutlineSlice | ContextSource::RagExcerpt => 0.7,
    }
}

fn required_context_from_pack(context_pack: &WritingContextPack) -> Vec<RequiredContext> {
    let mut contexts = context_pack
        .sources
        .iter()
        .take(12)
        .map(|source| {
            RequiredContext::new(
                format!("{:?}", source.source),
                context_source_purpose(&source.source),
                source.char_count.max(1),
                is_required_context_source(&context_pack.task, &source.source),
            )
        })
        .collect::<Vec<_>>();

    if !contexts.iter().any(|context| context.required) {
        if let Some(first) = contexts.first_mut() {
            first.required = true;
        } else {
            contexts.push(RequiredContext::new(
                "editor_observation",
                "Fallback sensory context for the current writing task.",
                1,
                true,
            ));
        }
    }
    contexts
}

fn context_source_purpose(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::SystemContract | ContextSource::ProjectBrief => {
            "Keep the task inside the book-level contract."
        }
        ContextSource::ChapterMission => "Preserve this chapter's active mission.",
        ContextSource::NextBeat => "Carry forward the next intended story beat.",
        ContextSource::ResultFeedback => "Use the previous chapter result feedback loop.",
        ContextSource::AuthorStyle => "Preserve learned author style preferences.",
        ContextSource::CanonSlice => "Avoid contradictions against established canon.",
        ContextSource::PromiseSlice => "Track open promises and story debts.",
        ContextSource::DecisionSlice => "Respect recent creative decisions.",
        ContextSource::OutlineSlice => "Stay aligned with the outline.",
        ContextSource::RagExcerpt => "Ground the task in retrieved project memory.",
        ContextSource::CursorPrefix => "Read the local prose before the cursor.",
        ContextSource::CursorSuffix => "Avoid clashing with local prose after the cursor.",
        ContextSource::SelectedText => "Constrain edits to the selected text.",
        ContextSource::PreviousChapter => "Maintain continuity from previous chapters.",
        ContextSource::NextChapter => "Avoid blocking the next planned chapter.",
        ContextSource::NeighborText => "Maintain nearby prose flow.",
    }
}

fn is_required_context_source(task: &AgentTask, source: &ContextSource) -> bool {
    task.required_source_budgets()
        .iter()
        .any(|(required, _)| required == source)
}

fn trace_state_with_expiry(state: &str, expires_at: Option<u64>, now: u64) -> String {
    if state == "pending" && expires_at.is_some_and(|expiry| expiry <= now) {
        "expired".to_string()
    } else {
        state.to_string()
    }
}

fn product_metrics_from_trace(
    proposals: &[AgentProposal],
    feedback_events: &[ProposalFeedback],
    operation_lifecycle: &[WriterOperationLifecycleTrace],
    context_recalls: Result<Vec<ContextRecallSummary>, rusqlite::Error>,
    chapter_missions: Result<Vec<ChapterMissionSummary>, rusqlite::Error>,
) -> WriterProductMetrics {
    let proposal_count = proposals.len() as u64;
    let feedback_count = feedback_events.len() as u64;
    let accepted_count = feedback_events
        .iter()
        .filter(|feedback| matches!(feedback.action, FeedbackAction::Accepted))
        .count() as u64;
    let rejected_count = feedback_events
        .iter()
        .filter(|feedback| matches!(feedback.action, FeedbackAction::Rejected))
        .count() as u64;
    let edited_count = feedback_events
        .iter()
        .filter(|feedback| matches!(feedback.action, FeedbackAction::Edited))
        .count() as u64;
    let snoozed_count = feedback_events
        .iter()
        .filter(|feedback| matches!(feedback.action, FeedbackAction::Snoozed))
        .count() as u64;
    let explained_count = feedback_events
        .iter()
        .filter(|feedback| matches!(feedback.action, FeedbackAction::Explained))
        .count() as u64;
    let ignored_count = rejected_count + snoozed_count + explained_count;
    let positive_feedback_count = feedback_events
        .iter()
        .filter(|feedback| feedback.is_positive())
        .count() as u64;
    let negative_feedback_count = feedback_events
        .iter()
        .filter(|feedback| feedback.is_negative())
        .count() as u64;

    let proposal_acceptance_rate = ratio(accepted_count + edited_count, feedback_count);
    let ignored_repeated_suggestion_rate = ratio(ignored_count, feedback_count);

    let manual_proposals = proposals
        .iter()
        .filter(|proposal| {
            proposal
                .evidence
                .iter()
                .any(|evidence| evidence.reference.contains("manual_request"))
        })
        .count() as u64;
    let manual_operations = proposals
        .iter()
        .filter(|proposal| {
            proposal
                .evidence
                .iter()
                .any(|evidence| evidence.reference.contains("manual_request"))
                && !proposal.operations.is_empty()
        })
        .count() as u64;
    let manual_ask_converted_to_operation_rate = ratio(manual_operations, manual_proposals);

    let recalls = context_recalls.unwrap_or_default();
    let promise_recalls = recalls
        .iter()
        .filter(|recall| recall.source == "PromiseSlice")
        .count() as u64;
    let promise_recall_hit_rate = ratio(promise_recalls, recalls.len() as u64);

    let canon_feedback = proposals
        .iter()
        .filter(|proposal| matches!(proposal.kind, ProposalKind::CanonUpdate))
        .filter(|proposal| {
            feedback_events
                .iter()
                .any(|feedback| feedback.proposal_id == proposal.id)
        })
        .count() as u64;
    let canon_negative = proposals
        .iter()
        .filter(|proposal| matches!(proposal.kind, ProposalKind::CanonUpdate))
        .filter(|proposal| {
            feedback_events.iter().any(|feedback| {
                feedback.proposal_id == proposal.id
                    && (feedback.is_negative()
                        || matches!(feedback.action, FeedbackAction::Explained))
            })
        })
        .count() as u64;
    let canon_false_positive_rate = ratio(canon_negative, canon_feedback);

    let missions = chapter_missions.unwrap_or_default();
    let completed_missions = missions
        .iter()
        .filter(|mission| mission.status == "completed")
        .count() as u64;
    let chapter_mission_completion_rate = ratio(completed_missions, missions.len() as u64);

    let durable_saves = operation_lifecycle
        .iter()
        .filter(|trace| trace.state == WriterOperationLifecycleState::DurablySaved)
        .count() as u64;
    let failed_saves = operation_lifecycle
        .iter()
        .filter(|trace| {
            trace.state == WriterOperationLifecycleState::Rejected && trace.save_result.is_some()
        })
        .count() as u64;
    let durable_save_success_rate = ratio(durable_saves, durable_saves + failed_saves);

    let mut save_to_feedback = Vec::new();
    for feedback in feedback_events {
        let Some(proposal) = proposals
            .iter()
            .find(|proposal| proposal.id == feedback.proposal_id)
        else {
            continue;
        };
        for operation in &proposal.operations {
            let Some(saved_at) = operation_lifecycle
                .iter()
                .filter(|trace| {
                    trace.proposal_id.as_deref() == Some(proposal.id.as_str())
                        && trace.operation_kind == operation_kind_label(operation)
                        && trace.affected_scope == operation_affected_scope(operation)
                        && trace.state == WriterOperationLifecycleState::DurablySaved
                })
                .map(|trace| trace.created_at)
                .max()
            else {
                continue;
            };
            if feedback.created_at >= saved_at {
                save_to_feedback.push(feedback.created_at - saved_at);
            }
        }
    }
    let average_save_to_feedback_ms = if save_to_feedback.is_empty() {
        None
    } else {
        Some(save_to_feedback.iter().sum::<u64>() / save_to_feedback.len() as u64)
    };

    WriterProductMetrics {
        proposal_count,
        feedback_count,
        accepted_count,
        rejected_count,
        edited_count,
        snoozed_count,
        explained_count,
        ignored_count,
        positive_feedback_count,
        negative_feedback_count,
        proposal_acceptance_rate,
        ignored_repeated_suggestion_rate,
        manual_ask_converted_to_operation_rate,
        promise_recall_hit_rate,
        canon_false_positive_rate,
        chapter_mission_completion_rate,
        durable_save_success_rate,
        average_save_to_feedback_ms,
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn proposal_slot_key(proposal: &AgentProposal) -> String {
    let target = proposal
        .target
        .as_ref()
        .map(|target| format!("{}:{}", target.from, target.to))
        .unwrap_or_else(|| "none".to_string());

    if proposal.kind == ProposalKind::Ghost {
        return format!("{}|{:?}|{}", proposal.observation_id, proposal.kind, target);
    }

    if let Some(memory_slot) = memory_operation_slot(proposal) {
        return memory_slot;
    }

    let evidence_key = proposal
        .evidence
        .first()
        .map(|evidence| format!("{:?}:{}", evidence.source, evidence.reference))
        .unwrap_or_default();
    let preview_key: String = proposal
        .preview
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect();

    format!(
        "{:?}|{}|{}|{}",
        proposal.kind, target, evidence_key, preview_key
    )
}

fn suppression_slot_key(proposal: &AgentProposal) -> String {
    let target = proposal
        .target
        .as_ref()
        .map(|target| format!("{}:{}", target.from, target.to))
        .unwrap_or_else(|| "none".to_string());
    let evidence_key = proposal
        .evidence
        .first()
        .map(|evidence| format!("{:?}:{}", evidence.source, evidence.reference))
        .unwrap_or_default();

    if proposal.kind == ProposalKind::Ghost {
        return ghost_suppression_slot_key(proposal);
    }

    if let Some(memory_slot) = memory_operation_slot(proposal) {
        return memory_slot;
    }

    format!(
        "{:?}|{}|{}|{}",
        proposal.kind,
        target,
        evidence_key,
        preview_fingerprint(&proposal.preview)
    )
}

fn ghost_suppression_slot_key(proposal: &AgentProposal) -> String {
    let chapter = proposal
        .operations
        .first()
        .and_then(|operation| match operation {
            WriterOperation::TextInsert { chapter, .. }
            | WriterOperation::TextReplace { chapter, .. }
            | WriterOperation::TextAnnotate { chapter, .. } => Some(chapter.as_str()),
            _ => None,
        })
        .unwrap_or("project");
    format!(
        "{:?}|{}|{}",
        proposal.kind,
        chapter,
        preview_fingerprint(&proposal.preview)
    )
}

fn memory_operation_slot(proposal: &AgentProposal) -> Option<String> {
    match proposal.operations.first()? {
        WriterOperation::CanonUpsertEntity { entity } => {
            Some(memory_candidate_slot_for_canon(entity))
        }
        WriterOperation::PromiseAdd { promise } => Some(memory_candidate_slot_for_promise(promise)),
        _ => None,
    }
}

fn memory_audit_title(proposal: &AgentProposal) -> String {
    match proposal.operations.first() {
        Some(WriterOperation::CanonUpsertEntity { entity }) => {
            format!("{} [{}]", entity.name, entity.kind)
        }
        Some(WriterOperation::PromiseAdd { promise }) => {
            format!("{} [{}]", promise.title, promise.kind)
        }
        _ => proposal.preview.clone(),
    }
}

fn record_memory_audit_event(
    memory: &WriterMemory,
    proposal: &AgentProposal,
    feedback: &ProposalFeedback,
) {
    if memory_operation_slot(proposal).is_none() {
        return;
    }
    let entry = super::memory::MemoryAuditSummary {
        proposal_id: proposal.id.clone(),
        kind: format!("{:?}", proposal.kind),
        action: format!("{:?}", feedback.action),
        title: memory_audit_title(proposal),
        evidence: proposal
            .evidence
            .first()
            .map(|evidence| evidence.snippet.clone())
            .unwrap_or_default(),
        rationale: proposal.rationale.clone(),
        reason: feedback.reason.clone(),
        created_at: feedback.created_at,
    };
    memory.record_memory_audit(&entry).ok();
}

fn memory_candidate_slot_for_canon(entity: &CanonEntityOp) -> String {
    format!("memory|canon|{}|{}", entity.kind, entity.name)
}

fn memory_candidate_slot_for_promise(promise: &PlotPromiseOp) -> String {
    format!("memory|promise|{}|{}", promise.kind, promise.title)
}

fn memory_feedback_key(slot: &str) -> String {
    format!("memory_extract:{}", slot)
}

fn record_memory_candidate_feedback(
    memory: &WriterMemory,
    proposal: &AgentProposal,
    accepted: bool,
) {
    let Some(slot) = memory_operation_slot(proposal) else {
        return;
    };
    let value = if accepted { "accepted" } else { "rejected" };
    let _ = memory.upsert_style_preference(&memory_feedback_key(&slot), value, accepted);
}

struct MemoryExtractionFeedback {
    suppressed_slots: std::collections::HashSet<String>,
    preferred_slots: std::collections::HashSet<String>,
}

impl MemoryExtractionFeedback {
    fn from_memory(memory: &WriterMemory) -> Self {
        let mut suppressed_slots = std::collections::HashSet::new();
        let mut preferred_slots = std::collections::HashSet::new();
        for preference in memory.list_style_preferences(200).unwrap_or_default() {
            let Some(slot) = preference.key.strip_prefix("memory_extract:") else {
                continue;
            };
            if preference.rejected_count >= 1
                && preference.rejected_count >= preference.accepted_count
            {
                suppressed_slots.insert(slot.to_string());
            } else if preference.accepted_count > preference.rejected_count {
                preferred_slots.insert(slot.to_string());
            }
        }
        Self {
            suppressed_slots,
            preferred_slots,
        }
    }

    fn is_suppressed(&self, slot: &str) -> bool {
        self.suppressed_slots.contains(slot)
    }

    fn is_preferred(&self, slot: &str) -> bool {
        self.preferred_slots.contains(slot)
    }

    fn apply_to_candidate(&self, candidate: MemoryCandidate) -> Option<MemoryCandidate> {
        match candidate {
            MemoryCandidate::Canon(mut entity) => {
                let slot = memory_candidate_slot_for_canon(&entity);
                if self.is_suppressed(&slot) {
                    return None;
                }
                if self.is_preferred(&slot) {
                    entity.confidence = (entity.confidence + 0.08).min(0.95);
                }
                Some(MemoryCandidate::Canon(entity))
            }
            MemoryCandidate::Promise(mut promise) => {
                let slot = memory_candidate_slot_for_promise(&promise);
                if self.is_suppressed(&slot) {
                    return None;
                }
                if self.is_preferred(&slot) {
                    promise.priority = (promise.priority + 1).min(10);
                }
                Some(MemoryCandidate::Promise(promise))
            }
        }
    }
}

fn preview_fingerprint(preview: &str) -> String {
    preview
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect()
}

fn diagnostic_to_proposal(
    diagnostic: DiagnosticResult,
    observation: &WriterObservation,
    observation_id: &str,
    proposal_id: &str,
) -> AgentProposal {
    let priority = match diagnostic.severity {
        DiagnosticSeverity::Error => ProposalPriority::Urgent,
        DiagnosticSeverity::Warning => ProposalPriority::Normal,
        DiagnosticSeverity::Info => ProposalPriority::Ambient,
    };
    let kind = match diagnostic.category {
        DiagnosticCategory::UnresolvedPromise => ProposalKind::PlotPromise,
        DiagnosticCategory::CanonConflict => ProposalKind::ContinuityWarning,
        DiagnosticCategory::StoryContractViolation => ProposalKind::StoryContract,
        DiagnosticCategory::ChapterMissionViolation => ProposalKind::ChapterMission,
        DiagnosticCategory::PacingNote => ProposalKind::StyleNote,
        DiagnosticCategory::CharacterVoiceInconsistency => ProposalKind::StyleNote,
        DiagnosticCategory::TimelineIssue => ProposalKind::ContinuityWarning,
    };
    let evidence = diagnostic
        .evidence
        .iter()
        .map(|item| EvidenceRef {
            source: match item.source.as_str() {
                "canon" => EvidenceSource::Canon,
                "promise" => EvidenceSource::PromiseLedger,
                "story_contract" => EvidenceSource::StoryContract,
                "chapter_mission" => EvidenceSource::ChapterMission,
                "outline" => EvidenceSource::Outline,
                "style" => EvidenceSource::StyleLedger,
                _ => EvidenceSource::ChapterText,
            },
            reference: item.reference.clone(),
            snippet: item.snippet.clone(),
        })
        .collect::<Vec<_>>();
    let operations = diagnostic
        .operations
        .into_iter()
        .map(|operation| operation.with_observation_revision(observation))
        .collect();
    let mut risks = Vec::new();
    if let Some(fix) = &diagnostic.fix_suggestion {
        risks.push(fix.clone());
    }

    AgentProposal {
        id: proposal_id.to_string(),
        observation_id: observation_id.to_string(),
        kind,
        priority,
        target: Some(super::observation::TextRange {
            from: diagnostic.from,
            to: diagnostic.to,
        }),
        preview: diagnostic.message,
        operations,
        rationale: format!(
            "Ambient diagnostic from {}.",
            observation
                .chapter_title
                .as_deref()
                .unwrap_or("current chapter")
        ),
        evidence,
        risks,
        alternatives: vec![],
        confidence: 0.72,
        expires_at: Some(observation.created_at + 120_000),
    }
}

trait OperationObservationRevision {
    fn with_observation_revision(self, observation: &WriterObservation) -> Self;
}

impl OperationObservationRevision for WriterOperation {
    fn with_observation_revision(self, observation: &WriterObservation) -> Self {
        let revision = observation
            .chapter_revision
            .clone()
            .unwrap_or_else(|| "missing".to_string());
        match self {
            WriterOperation::TextInsert {
                chapter,
                at,
                text,
                revision: _,
            } => WriterOperation::TextInsert {
                chapter,
                at,
                text,
                revision,
            },
            WriterOperation::TextReplace {
                chapter,
                from,
                to,
                text,
                revision: _,
            } => WriterOperation::TextReplace {
                chapter,
                from,
                to,
                text,
                revision,
            },
            other => other,
        }
    }
}

fn memory_candidates_from_observation(
    observation: &WriterObservation,
    memory: &WriterMemory,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
) -> Vec<AgentProposal> {
    let mut proposals = Vec::new();
    let mut known = memory.get_canon_entity_names().unwrap_or_default();
    known.sort();
    known.dedup();

    let feedback = MemoryExtractionFeedback::from_memory(memory);

    for mut entity in extract_new_canon_entities(&observation.paragraph, &known)
        .into_iter()
        .take(3)
    {
        let slot = memory_candidate_slot_for_canon(&entity);
        if feedback.is_suppressed(&slot) {
            continue;
        }
        if feedback.is_preferred(&slot) {
            entity.confidence = (entity.confidence + 0.08).min(0.92);
        }
        proposals.push(canon_candidate_proposal(
            observation,
            observation_id,
            proposal_counter,
            session_id,
            entity,
            CandidateSource::Local,
        ));
    }

    for mut promise in extract_plot_promises(&observation.paragraph, observation)
        .into_iter()
        .take(3)
    {
        let slot = memory_candidate_slot_for_promise(&promise);
        if feedback.is_suppressed(&slot) {
            continue;
        }
        if feedback.is_preferred(&slot) {
            promise.priority = (promise.priority + 1).min(10);
        }
        proposals.push(promise_candidate_proposal(
            observation,
            observation_id,
            proposal_counter,
            session_id,
            promise,
            CandidateSource::Local,
        ));
    }

    proposals
}

fn canon_candidate_proposal(
    observation: &WriterObservation,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
    entity: CanonEntityOp,
    source: CandidateSource,
) -> AgentProposal {
    let id = proposal_id(session_id, *proposal_counter);
    *proposal_counter += 1;
    let preview = format!("沉淀设定: {} - {}", entity.name, entity.summary);
    let snippet = entity.summary.clone();
    let (rationale, confidence, risks) = source.canon_metadata();
    AgentProposal {
        id,
        observation_id: observation_id.to_string(),
        kind: ProposalKind::CanonUpdate,
        priority: ProposalPriority::Ambient,
        target: observation.cursor.clone(),
        preview,
        operations: vec![WriterOperation::CanonUpsertEntity { entity }],
        rationale,
        evidence: vec![EvidenceRef {
            source: EvidenceSource::ChapterText,
            reference: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            snippet,
        }],
        risks,
        alternatives: vec![],
        confidence,
        expires_at: None,
    }
}

fn promise_candidate_proposal(
    observation: &WriterObservation,
    observation_id: &str,
    proposal_counter: &mut u64,
    session_id: &str,
    promise: PlotPromiseOp,
    source: CandidateSource,
) -> AgentProposal {
    let id = proposal_id(session_id, *proposal_counter);
    *proposal_counter += 1;
    let preview = format!("登记伏笔: {} - {}", promise.title, promise.description);
    let snippet = promise.description.clone();
    let (rationale, confidence, risks) = source.promise_metadata();
    AgentProposal {
        id,
        observation_id: observation_id.to_string(),
        kind: ProposalKind::PlotPromise,
        priority: ProposalPriority::Ambient,
        target: observation.cursor.clone(),
        preview,
        operations: vec![WriterOperation::PromiseAdd { promise }],
        rationale,
        evidence: vec![EvidenceRef {
            source: EvidenceSource::ChapterText,
            reference: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            snippet,
        }],
        risks,
        alternatives: vec![],
        confidence,
        expires_at: None,
    }
}

enum CandidateSource {
    Local,
    Llm(String),
}

impl CandidateSource {
    fn canon_metadata(&self) -> (String, f64, Vec<String>) {
        match self {
            CandidateSource::Local => (
                "章节保存后发现可复用人物/物件设定，建议写入长期 canon。".to_string(),
                0.62,
                vec!["自动抽取可能误把普通名词当设定，请确认后接受。".to_string()],
            ),
            CandidateSource::Llm(model) => (
                format!("LLM增强记忆抽取: {}. 建议写入长期 canon。", model),
                0.78,
                vec!["LLM 抽取仍需人工确认，避免把临场描述误记成长期设定。".to_string()],
            ),
        }
    }

    fn promise_metadata(&self) -> (String, f64, Vec<String>) {
        match self {
            CandidateSource::Local => (
                "章节保存后发现未回收信息，建议加入伏笔 ledger 以便后续提醒。".to_string(),
                0.66,
                vec!["请确认这是真伏笔，而不是只在当前场景内解决的信息。".to_string()],
            ),
            CandidateSource::Llm(model) => (
                format!("LLM增强记忆抽取: {}. 建议加入伏笔 ledger。", model),
                0.8,
                vec!["请确认这是真伏笔，而不是 LLM 过度解读。".to_string()],
            ),
        }
    }
}

pub(crate) fn extract_new_canon_entities(text: &str, known: &[String]) -> Vec<CanonEntityOp> {
    let mut entities = Vec::new();
    for sentence in split_sentences(text) {
        for cue in ["名叫", "叫做", "名为", "代号"] {
            if let Some(name) = extract_name_after(&sentence, cue) {
                if should_keep_entity(&name, known, &entities) {
                    entities.push(CanonEntityOp {
                        kind: "character".to_string(),
                        name: name.clone(),
                        aliases: vec![],
                        summary: sentence_snippet(&sentence, 120),
                        attributes: serde_json::json!({}),
                        confidence: 0.62,
                    });
                }
            }
        }

        for marker in ["寒影刀", "玉佩", "密信", "钥匙", "令牌"] {
            if sentence.contains(marker) && should_keep_entity(marker, known, &entities) {
                entities.push(CanonEntityOp {
                    kind: "object".to_string(),
                    name: marker.to_string(),
                    aliases: vec![],
                    summary: sentence_snippet(&sentence, 120),
                    attributes: serde_json::json!({ "category": "story_object" }),
                    confidence: 0.58,
                });
            }
        }
    }
    entities
}

pub fn extract_plot_promises(text: &str, observation: &WriterObservation) -> Vec<PlotPromiseOp> {
    let mut promises = Vec::new();
    for sentence in split_sentences(text) {
        if !contains_promise_cue(&sentence) {
            continue;
        }
        let title = promise_title(&sentence);
        if title.is_empty() || promises.iter().any(|p: &PlotPromiseOp| p.title == title) {
            continue;
        }
        let kind = promise_kind_from_cues(&sentence);
        let priority = match kind {
            PromiseKind::ObjectWhereabouts | PromiseKind::MysteryClue => 5,
            PromiseKind::CharacterCommitment | PromiseKind::EmotionalDebt => 4,
            _ => 3,
        };
        promises.push(PlotPromiseOp {
            kind: kind.as_kind_str().to_string(),
            title,
            description: sentence_snippet(&sentence, 140),
            introduced_chapter: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            expected_payoff: "后续章节回收或解释".to_string(),
            priority,
        });
    }
    promises
}

enum MemoryCandidate {
    Canon(CanonEntityOp),
    Promise(PlotPromiseOp),
}

fn llm_memory_candidates_from_value(
    value: serde_json::Value,
    observation: &WriterObservation,
    _model: &str,
) -> Vec<MemoryCandidate> {
    let mut candidates = Vec::new();

    if let Some(canon) = value.get("canon").and_then(|v| v.as_array()) {
        for item in canon.iter().take(5) {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if name.chars().count() < 2 || name.chars().count() > 16 {
                continue;
            }
            let summary = item
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if summary.chars().count() < 6 {
                continue;
            }
            let kind = item
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("entity")
                .trim();
            let confidence = item
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.75)
                .clamp(0.0, 1.0);
            if confidence < 0.55 {
                continue;
            }
            let aliases = item
                .get("aliases")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|alias| alias.as_str())
                        .map(str::trim)
                        .filter(|alias| !alias.is_empty())
                        .take(6)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let attributes = item
                .get("attributes")
                .cloned()
                .filter(|value| value.is_object())
                .unwrap_or_else(|| serde_json::json!({}));
            candidates.push(MemoryCandidate::Canon(CanonEntityOp {
                kind: if kind.is_empty() {
                    "entity".to_string()
                } else {
                    kind.to_string()
                },
                name: name.to_string(),
                aliases,
                summary: sentence_snippet(summary, 180),
                attributes,
                confidence,
            }));
        }
    }

    if let Some(promises) = value.get("promises").and_then(|v| v.as_array()) {
        for item in promises.iter().take(5) {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if title.chars().count() < 2 || description.chars().count() < 6 {
                continue;
            }
            let confidence = item
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.75)
                .clamp(0.0, 1.0);
            if confidence < 0.55 {
                continue;
            }
            candidates.push(MemoryCandidate::Promise(PlotPromiseOp {
                kind: item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("open_question")
                    .trim()
                    .to_string(),
                title: sentence_snippet(title, 40),
                description: sentence_snippet(description, 180),
                introduced_chapter: item
                    .get("introducedChapter")
                    .or_else(|| item.get("introduced_chapter"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        observation
                            .chapter_title
                            .as_deref()
                            .unwrap_or("current chapter")
                    })
                    .trim()
                    .to_string(),
                expected_payoff: item
                    .get("expectedPayoff")
                    .or_else(|| item.get("expected_payoff"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("后续章节回收或解释")
                    .trim()
                    .to_string(),
                priority: item
                    .get("priority")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3)
                    .clamp(0, 10) as i32,
            }));
        }
    }

    dedupe_memory_candidates(candidates)
}

fn dedupe_memory_candidates(candidates: Vec<MemoryCandidate>) -> Vec<MemoryCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        let key = match &candidate {
            MemoryCandidate::Canon(entity) => format!("canon:{}", entity.name),
            MemoryCandidate::Promise(promise) => format!("promise:{}", promise.title),
        };
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}

#[cfg(test)]
fn proposal_feedback(
    proposal_id: String,
    action: FeedbackAction,
    reason: Option<String>,
) -> ProposalFeedback {
    ProposalFeedback {
        proposal_id,
        action,
        final_text: None,
        reason,
        created_at: now_ms(),
    }
}

pub(crate) fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '\n') {
            let trimmed = current.trim();
            if trimmed.chars().count() >= 6 {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    let trimmed = current.trim();
    if trimmed.chars().count() >= 6 {
        sentences.push(trimmed.to_string());
    }
    sentences
}

fn extract_name_after(sentence: &str, cue: &str) -> Option<String> {
    let cue_byte = sentence.find(cue)?;
    let after = &sentence[cue_byte + cue.len()..];
    let name: String = after
        .chars()
        .skip_while(|c| c.is_whitespace() || matches!(c, '“' | '"' | '\'' | '：' | ':'))
        .take_while(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(c))
        .take(6)
        .collect();
    let count = name.chars().count();
    if (2..=6).contains(&count) {
        Some(name)
    } else {
        None
    }
}

fn should_keep_entity(name: &str, known: &[String], existing: &[CanonEntityOp]) -> bool {
    let name = name.trim();
    !name.is_empty()
        && !known.iter().any(|item| item == name)
        && !existing.iter().any(|item| item.name == name)
}

fn contains_promise_cue(sentence: &str) -> bool {
    [
        "还没",
        "尚未",
        "迟早",
        "总有一天",
        "秘密",
        "谜",
        "真相",
        "下落",
        "没有说出口",
        "没有告诉",
        "约定",
        "承诺",
        "发誓",
        "一定会",
        "等着",
        "留给",
        "交给",
        "带走",
        "失踪",
        "不见",
        "消失",
        "藏",
        "隐瞒",
    ]
    .iter()
    .any(|cue| sentence.contains(cue))
}

fn promise_kind_from_cues(sentence: &str) -> PromiseKind {
    let s = sentence;
    if s.contains("下落") || s.contains("不见") || s.contains("消失") || s.contains("带走")
    {
        PromiseKind::ObjectWhereabouts
    } else if s.contains("秘密") || s.contains("谜") || s.contains("真相") || s.contains("隐瞒")
    {
        PromiseKind::MysteryClue
    } else if s.contains("约定") || s.contains("承诺") || s.contains("发誓") || s.contains("等着")
    {
        PromiseKind::CharacterCommitment
    } else if s.contains("没有说出口") || s.contains("没有告诉") || s.contains("藏") {
        PromiseKind::EmotionalDebt
    } else {
        PromiseKind::PlotPromise
    }
}

fn promise_title(sentence: &str) -> String {
    for marker in [
        "玉佩", "密信", "钥匙", "令牌", "真相", "秘密", "下落", "戒指", "剑", "刀", "信物", "地图",
        "药", "毒",
    ] {
        if sentence.contains(marker) {
            return marker.to_string();
        }
    }
    sentence
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(12)
        .collect()
}

pub(crate) fn sentence_snippet(sentence: &str, limit: usize) -> String {
    sentence
        .trim_matches(|c: char| c.is_whitespace())
        .chars()
        .take(limit)
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum MemoryCandidateQuality {
    Acceptable,
    Vague { reason: String },
    Duplicate { existing_name: String },
}

pub fn validate_canon_candidate(candidate: &CanonEntityOp) -> MemoryCandidateQuality {
    let name = candidate.name.trim();
    if name.chars().count() < 2 {
        return MemoryCandidateQuality::Vague {
            reason: "entity name too short (min 2 chars)".to_string(),
        };
    }
    let summary = candidate.summary.trim();
    if summary.chars().count() < 6 {
        return MemoryCandidateQuality::Vague {
            reason: format!(
                "entity summary too short ({} chars, min 6)",
                summary.chars().count()
            ),
        };
    }
    MemoryCandidateQuality::Acceptable
}

pub fn validate_promise_candidate(candidate: &PlotPromiseOp) -> MemoryCandidateQuality {
    let title = candidate.title.trim();
    if title.chars().count() < 2 {
        return MemoryCandidateQuality::Vague {
            reason: "promise title too short (min 2 chars)".to_string(),
        };
    }
    let description = candidate.description.trim();
    if description.chars().count() < 8 {
        return MemoryCandidateQuality::Vague {
            reason: format!(
                "promise description too short ({} chars, min 8)",
                description.chars().count()
            ),
        };
    }
    MemoryCandidateQuality::Acceptable
}

fn should_replace_proposal(existing: &AgentProposal, incoming: &AgentProposal) -> bool {
    if is_llm_ghost(incoming) && !is_llm_ghost(existing) {
        return true;
    }

    if priority_weight(&incoming.priority) > priority_weight(&existing.priority) {
        return true;
    }

    incoming.confidence > existing.confidence + 0.05
}

fn is_llm_ghost(proposal: &AgentProposal) -> bool {
    proposal.kind == ProposalKind::Ghost && proposal.rationale.contains("LLM增强续写")
}

fn priority_weight(priority: &ProposalPriority) -> u8 {
    match priority {
        ProposalPriority::Ambient => 0,
        ProposalPriority::Normal => 1,
        ProposalPriority::Urgent => 2,
    }
}

fn proposal_expired(proposal: &AgentProposal, now: u64) -> bool {
    proposal
        .expires_at
        .map(|expires_at| expires_at <= now)
        .unwrap_or(false)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn draft_continuation(
    intent: &super::intent::WritingIntent,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
) -> String {
    let paragraph = observation.paragraph.trim();
    let lead = if paragraph.ends_with('。')
        || paragraph.ends_with('！')
        || paragraph.ends_with('？')
        || paragraph.ends_with('.')
        || paragraph.ends_with('!')
        || paragraph.ends_with('?')
    {
        "\n"
    } else {
        ""
    };

    let canon_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::CanonSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");
    let promise_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");

    let text = if !promise_hint.is_empty() {
        "他忽然想起那件还没交代清楚的旧事，原本要出口的话在舌尖停住了。"
    } else if canon_hint.contains("weapon") || canon_hint.contains("武器") {
        "他没有急着开口，只让手指重新落回熟悉的兵器旁，像是在确认自己仍握着选择。"
    } else {
        match intent {
            super::intent::WritingIntent::Dialogue => {
                "他没有立刻回答，只把真正想说的话压在喉咙后面。"
            }
            super::intent::WritingIntent::Action => {
                "下一瞬，他侧身避开逼近的锋芒，顺势把局面逼向更窄的角落。"
            }
            super::intent::WritingIntent::ConflictEscalation => {
                "偏在这时，门外传来第三个人的脚步声，把所有尚未出口的话都截断了。"
            }
            super::intent::WritingIntent::Description => {
                "风从缝隙里钻进来，带着潮湿的冷意，让这片沉默显得更不安稳。"
            }
            _ => "他停了半息，终于做出那个无法再撤回的决定。",
        }
    };

    format!("{lead}{text}")
}

fn ghost_alternatives(
    intent: &super::intent::WritingIntent,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
    chapter: &str,
    insert_at: usize,
    revision: &str,
) -> Vec<ProposalAlternative> {
    let candidates = ghost_candidate_texts(intent, observation, context_pack);
    let labels = ghost_candidate_labels(intent);
    candidates
        .into_iter()
        .enumerate()
        .map(|(idx, preview)| {
            let id = ["a", "b", "c"].get(idx).unwrap_or(&"x").to_string();
            ProposalAlternative {
                id: id.clone(),
                label: labels[idx].to_string(),
                operation: Some(WriterOperation::TextInsert {
                    chapter: chapter.to_string(),
                    at: insert_at,
                    text: preview.clone(),
                    revision: revision.to_string(),
                }),
                rationale: format!("multi-ghost branch {}", id.to_ascii_uppercase()),
                preview,
            }
        })
        .collect()
}

fn ghost_candidate_texts(
    intent: &super::intent::WritingIntent,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
) -> [String; 3] {
    let base = draft_continuation(intent, observation, context_pack);
    let promise_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");
    let canon_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::CanonSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");

    let branch_b = if !promise_hint.is_empty() {
        "他没有继续逼问，只把那件悬而未决的旧事重新压回心底，等对方先露出破绽。"
    } else {
        match intent {
            super::intent::WritingIntent::Dialogue => {
                "他垂下眼，像是随口一问：“你刚才避开的，究竟是哪一句？”"
            }
            super::intent::WritingIntent::Action => {
                "他故意慢了半拍，让对方以为自己占了先机，再突然切进空门。"
            }
            super::intent::WritingIntent::ConflictEscalation => {
                "他还没来得及判断局势，屋内的灯先灭了，黑暗把所有退路一并吞没。"
            }
            super::intent::WritingIntent::Description => {
                "潮气沿着墙根蔓延，旧木与灰尘的味道混在一起，像某种迟迟不肯散去的警告。"
            }
            _ => "他没有立刻推进，只把目光移向最安静的那个人，等一个真正的答案。",
        }
    };

    let branch_c = if canon_hint.contains("weapon") || canon_hint.contains("武器") {
        "他松开那句差点出口的话，先确认掌心熟悉的重量仍在，才重新抬眼看向对方。"
    } else {
        match intent {
            super::intent::WritingIntent::Dialogue => {
                "那句话到了嘴边又被他咽回去，只剩一个短促的笑，听不出是承认还是挑衅。"
            }
            super::intent::WritingIntent::Action => {
                "可就在他发力之前，身后传来一声轻响，迫使他把所有动作硬生生收住。"
            }
            super::intent::WritingIntent::ConflictEscalation => {
                "更糟的是，来人没有藏脚步，仿佛正等着他们意识到自己已经无处可躲。"
            }
            super::intent::WritingIntent::Description => {
                "远处的声响被夜色压得很低，低到像是从每个人心里慢慢渗出来的。"
            }
            _ => "他终于意识到，真正该被追问的不是眼前这句话，而是此前一直没人敢提的沉默。",
        }
    };

    [base, branch_b.to_string(), branch_c.to_string()]
}

fn ghost_candidate_labels(intent: &super::intent::WritingIntent) -> [&'static str; 3] {
    match intent {
        super::intent::WritingIntent::Dialogue => ["A 直接表态", "B 言语试探", "C 压住情绪"],
        super::intent::WritingIntent::Action => ["A 快节奏", "B 诱敌试探", "C 外部打断"],
        super::intent::WritingIntent::ConflictEscalation => {
            ["A 顺势加压", "B 黑暗反转", "C 来人压迫"]
        }
        super::intent::WritingIntent::Description => ["A 氛围推进", "B 感官细化", "C 情绪映射"],
        _ => ["A 顺势推进", "B 关系试探", "C 伏笔回扣"],
    }
}

fn sanitize_continuation(text: &str) -> String {
    text.trim()
        .trim_matches('`')
        .trim()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(260)
        .collect()
}

fn context_pack_evidence(
    pack: &WritingContextPack,
    observation: &WriterObservation,
) -> Vec<EvidenceRef> {
    let mut evidence = Vec::new();
    for source in &pack.sources {
        let evidence_source = match source.source {
            ContextSource::CursorPrefix
            | ContextSource::CursorSuffix
            | ContextSource::SelectedText => EvidenceSource::ChapterText,
            ContextSource::CanonSlice => EvidenceSource::Canon,
            ContextSource::PromiseSlice => EvidenceSource::PromiseLedger,
            ContextSource::ProjectBrief => EvidenceSource::StoryContract,
            ContextSource::ChapterMission => EvidenceSource::ChapterMission,
            ContextSource::DecisionSlice => EvidenceSource::AuthorFeedback,
            ContextSource::AuthorStyle => EvidenceSource::StyleLedger,
            ContextSource::OutlineSlice => EvidenceSource::Outline,
            ContextSource::ResultFeedback => EvidenceSource::ChapterText,
            _ => EvidenceSource::ChapterText,
        };
        evidence.push(EvidenceRef {
            source: evidence_source,
            reference: format!("{:?}", source.source),
            snippet: source.content.chars().take(140).collect(),
        });
    }

    if evidence.is_empty() {
        evidence.push(EvidenceRef {
            source: EvidenceSource::ChapterText,
            reference: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".into()),
            snippet: observation.paragraph.chars().take(120).collect(),
        });
    }

    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
    use crate::writer_agent::memory::WriterMemory;
    use crate::writer_agent::observation::{
        ObservationReason, ObservationSource, TextRange, WriterObservation,
    };

    fn observation(paragraph: &str) -> WriterObservation {
        WriterObservation {
            id: "obs-1".to_string(),
            created_at: now_ms(),
            source: ObservationSource::Editor,
            reason: ObservationReason::Idle,
            project_id: "default".to_string(),
            chapter_title: Some("Chapter-1".to_string()),
            chapter_revision: Some("rev".to_string()),
            cursor: Some(TextRange { from: 10, to: 10 }),
            selection: None,
            prefix: paragraph.to_string(),
            suffix: String::new(),
            paragraph: paragraph.to_string(),
            full_text_digest: None,
            editor_dirty: true,
        }
    }

    fn test_approval(source: &str) -> crate::writer_agent::operation::OperationApproval {
        test_approval_for_proposal(source, "proposal-test")
    }

    fn test_approval_for_proposal(
        source: &str,
        proposal_id: &str,
    ) -> crate::writer_agent::operation::OperationApproval {
        crate::writer_agent::operation::OperationApproval {
            source: source.to_string(),
            actor: "author".to_string(),
            reason: "test approval".to_string(),
            proposal_id: Some(proposal_id.to_string()),
            surfaced_to_user: true,
            created_at: now_ms(),
        }
    }

    #[test]
    fn observe_emits_intent_proposal_and_feedback_records_decision() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposals = kernel
            .observe(observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上，听见里面有人压低声音。"))
            .unwrap();

        assert!(proposals
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::Ghost));
        assert!(proposals.iter().any(|proposal| matches!(
            proposal.operations.first(),
            Some(WriterOperation::TextInsert { .. })
        )));
        assert!(proposals
            .iter()
            .any(|proposal| proposal.rationale.contains("ContextPack")));
        let proposal = proposals
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap();
        let operation = proposal.operations[0].clone();
        kernel
            .approve_editor_operation_with_approval(
                operation.clone(),
                "rev",
                Some(&test_approval_for_proposal("ghost_feedback", &proposal.id)),
            )
            .unwrap();
        kernel
            .record_operation_durable_save(
                Some(proposal.id.clone()),
                operation,
                "editor_save:rev-2".to_string(),
            )
            .unwrap();
        let proposal_id = proposal.id.clone();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id,
                action: FeedbackAction::Accepted,
                final_text: None,
                reason: None,
                created_at: 2_000,
            })
            .unwrap();

        let status = kernel.status();
        assert_eq!(status.total_feedback_events, 1);
        assert_eq!(status.pending_proposals, 0);
        assert!(kernel
            .ledger_snapshot()
            .recent_decisions
            .iter()
            .any(|decision| decision.decision == "accepted"));
    }

    #[test]
    fn approve_editor_operation_checks_revision_without_mutating_text() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let ok = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::TextInsert {
                    chapter: "Chapter-1".to_string(),
                    at: 3,
                    text: "续写".to_string(),
                    revision: "rev-1".to_string(),
                },
                "rev-1",
                Some(&test_approval("text_revision")),
            )
            .unwrap();
        assert!(ok.success);

        let conflict = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::TextInsert {
                    chapter: "Chapter-1".to_string(),
                    at: 3,
                    text: "续写".to_string(),
                    revision: "rev-1".to_string(),
                },
                "rev-2",
                Some(&test_approval("text_revision")),
            )
            .unwrap();
        assert!(!conflict.success);
        assert_eq!(conflict.error.unwrap().code, "conflict");
    }

    #[test]
    fn approve_editor_operation_requires_context_for_memory_writes() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let result = kernel
            .approve_editor_operation(
                WriterOperation::StyleUpdatePreference {
                    key: "dialogue".to_string(),
                    value: "prefers_subtext".to_string(),
                },
                "",
            )
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.error.unwrap().code, "approval_required");
    }

    #[test]
    fn execute_operation_records_annotation_without_text_revision() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let result = kernel
            .execute_operation(
                WriterOperation::TextAnnotate {
                    chapter: "Chapter-1".to_string(),
                    from: 1,
                    to: 4,
                    message: "这里与设定冲突".to_string(),
                    severity: crate::writer_agent::operation::AnnotationSeverity::Warning,
                },
                "",
                "",
            )
            .unwrap();

        assert!(result.success);
        assert!(result.revision_after.is_none());
        assert_eq!(kernel.ledger_snapshot().recent_decisions.len(), 1);
    }

    #[test]
    fn execute_operation_rejects_write_without_approval_context() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let result = kernel
            .execute_operation(
                WriterOperation::TextInsert {
                    chapter: "Chapter-1".to_string(),
                    at: 0,
                    text: "续写".to_string(),
                    revision: "rev-1".to_string(),
                },
                "",
                "rev-1",
            )
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.error.unwrap().code, "approval_required");
    }

    #[test]
    fn accepted_text_feedback_requires_durable_save() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposal = kernel
            .create_llm_ghost_proposal(
                observation("林墨停在旧门前，风从裂开的门缝里钻出来。"),
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "test-model",
            )
            .unwrap();
        let operation = proposal.operations[0].clone();

        kernel
            .approve_editor_operation_with_approval(
                operation.clone(),
                "rev",
                Some(&test_approval_for_proposal("ghost", &proposal.id)),
            )
            .unwrap();
        kernel
            .apply_feedback(proposal_feedback(
                proposal.id.clone(),
                FeedbackAction::Accepted,
                None,
            ))
            .unwrap();
        assert!(!kernel
            .memory
            .list_style_preferences(20)
            .unwrap()
            .iter()
            .any(|preference| preference.key == "accepted_Ghost"));

        kernel
            .record_operation_durable_save(
                Some(proposal.id.clone()),
                operation,
                "editor_save:rev-2".to_string(),
            )
            .unwrap();
        kernel
            .apply_feedback(proposal_feedback(
                proposal.id.clone(),
                FeedbackAction::Accepted,
                None,
            ))
            .unwrap();

        assert!(kernel
            .memory
            .list_style_preferences(20)
            .unwrap()
            .iter()
            .any(|preference| preference.key == "accepted_Ghost"));
    }

    #[test]
    fn execute_operation_upserts_canon_rule_and_style_preference() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);

        let rule = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::CanonUpsertRule {
                    rule: crate::writer_agent::operation::CanonRuleOp {
                        rule: "林墨不会主动弃刀。".to_string(),
                        category: "character_rule".to_string(),
                        priority: 8,
                    },
                },
                "",
                Some(&test_approval("canon_test")),
            )
            .unwrap();
        assert!(rule.success);

        let style = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::StyleUpdatePreference {
                    key: "dialogue".to_string(),
                    value: "prefers_subtext".to_string(),
                },
                "",
                Some(&test_approval("style_test")),
            )
            .unwrap();
        assert!(style.success);

        let ledger = kernel.ledger_snapshot();
        assert_eq!(ledger.canon_rules.len(), 1);
        assert_eq!(ledger.canon_rules[0].priority, 8);
        let preferences = kernel.memory.list_style_preferences(10).unwrap();
        assert!(preferences
            .iter()
            .any(|pref| pref.key == "dialogue" && pref.value == "prefers_subtext"));
    }

    #[test]
    fn pure_kernel_rejects_outline_update_without_project_runtime() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::OutlineUpdate {
                    node_id: "Chapter-1".to_string(),
                    patch: serde_json::json!({"summary": "new"}),
                },
                "",
                Some(&test_approval("outline_test")),
            )
            .unwrap();

        assert!(!result.success);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("project storage runtime"));
    }

    #[test]
    fn create_llm_ghost_proposal_registers_typed_operation() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposal = kernel
            .create_llm_ghost_proposal(
                observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。"),
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "test-model",
            )
            .unwrap();

        assert!(proposal.rationale.contains("LLM增强续写"));
        assert!(matches!(
            proposal.operations.first(),
            Some(WriterOperation::TextInsert { .. })
        ));
        assert_eq!(kernel.status().pending_proposals, 1);
    }

    #[test]
    fn create_inline_operation_proposal_uses_selection_replace() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("林墨握住刀柄，沉默片刻。");
        obs.reason = ObservationReason::Explicit;
        obs.selection = Some(super::super::observation::TextSelection {
            from: 2,
            to: 6,
            text: "握住刀柄".to_string(),
        });

        let proposal = kernel
            .create_inline_operation_proposal(
                obs,
                "改得更紧张",
                "指节一点点扣紧刀柄".to_string(),
                "test-model",
            )
            .unwrap();

        assert_eq!(proposal.kind, ProposalKind::ParallelDraft);
        assert!(proposal.rationale.contains("Inline typed operation"));
        assert!(matches!(
            proposal.operations.first(),
            Some(WriterOperation::TextReplace { from: 2, to: 6, .. })
        ));
    }

    #[test]
    fn create_inline_operation_proposal_without_selection_inserts_at_cursor() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("林墨停在门前。");
        obs.reason = ObservationReason::Explicit;
        obs.cursor = Some(TextRange { from: 7, to: 7 });

        let proposal = kernel
            .create_inline_operation_proposal(
                obs,
                "补一句动作",
                "他把呼吸压得更低。".to_string(),
                "test-model",
            )
            .unwrap();

        assert!(matches!(
            proposal.operations.first(),
            Some(WriterOperation::TextInsert { at: 7, .. })
        ));
    }

    #[test]
    fn duplicate_ghost_proposal_is_suppressed_for_same_observation_slot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");

        let first = kernel.observe(obs.clone()).unwrap();
        let second = kernel.observe(obs).unwrap();

        assert!(first
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::Ghost));
        assert!(!second
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::Ghost));
        assert_eq!(kernel.status().pending_proposals, 1);
    }

    #[test]
    fn implicit_ghost_rejections_snooze_repeated_ignored_slot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let paragraph = "林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。";

        let mut first_obs = observation(paragraph);
        first_obs.id = "obs-ignored-1".to_string();
        let first = kernel.observe(first_obs).unwrap();
        let first_ghost = first
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap();
        let first_id = first_ghost.id.clone();
        assert!(!kernel
            .record_implicit_ghost_rejection(&first_id, now_ms())
            .unwrap());
        assert_eq!(kernel.status().pending_proposals, 0);

        let mut second_obs = observation(paragraph);
        second_obs.id = "obs-ignored-2".to_string();
        second_obs.cursor = Some(TextRange { from: 11, to: 11 });
        let second = kernel.observe(second_obs).unwrap();
        let second_id = second
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone();
        assert!(!kernel
            .record_implicit_ghost_rejection(&second_id, now_ms())
            .unwrap());

        let mut third_obs = observation(paragraph);
        third_obs.id = "obs-ignored-3".to_string();
        third_obs.cursor = Some(TextRange { from: 12, to: 12 });
        let third = kernel.observe(third_obs).unwrap();
        let third_id = third
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone();
        assert!(kernel
            .record_implicit_ghost_rejection(&third_id, now_ms())
            .unwrap());

        let mut fourth_obs = observation(paragraph);
        fourth_obs.id = "obs-ignored-4".to_string();
        fourth_obs.cursor = Some(TextRange { from: 13, to: 13 });
        let fourth = kernel.observe(fourth_obs).unwrap();
        assert!(!fourth
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::Ghost));
    }

    #[test]
    fn llm_ghost_supersedes_local_ghost_for_same_observation_slot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");

        let local = kernel.observe(obs.clone()).unwrap();
        let local_ghost = local
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone();
        let llm = kernel
            .create_llm_ghost_proposal(
                obs,
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "test-model",
            )
            .unwrap();

        assert!(llm.rationale.contains("LLM增强续写"));
        assert!(kernel.superseded_proposals.contains(&local_ghost));
        assert_eq!(kernel.status().pending_proposals, 1);
    }

    #[test]
    fn rejected_ghost_suppresses_same_slot_temporarily() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
        let first = kernel.observe(obs.clone()).unwrap();
        let ghost = first
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap();

        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: ghost.id.clone(),
                action: FeedbackAction::Rejected,
                final_text: None,
                reason: Some("too soon".to_string()),
                created_at: now_ms(),
            })
            .unwrap();

        let mut next_obs = obs;
        next_obs.id = "obs-2".to_string();
        let second = kernel.observe(next_obs).unwrap();

        assert!(!second
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::Ghost));
    }

    #[test]
    fn pending_proposals_excludes_superseded_feedback_and_expired() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
        let local = kernel.observe(obs.clone()).unwrap();
        let local_id = local
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone();
        let llm = kernel
            .create_llm_ghost_proposal(
                obs,
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "test-model",
            )
            .unwrap();

        let pending = kernel.pending_proposals();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, llm.id);
        assert!(!pending.iter().any(|proposal| proposal.id == local_id));
    }

    #[test]
    fn trace_snapshot_records_observation_proposal_and_state() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
        let local = kernel.observe(obs.clone()).unwrap();
        let local_id = local
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::Ghost)
            .unwrap()
            .id
            .clone();
        let llm = kernel
            .create_llm_ghost_proposal(
                obs,
                "他终于听见门后有人低声念出了他的名字。".to_string(),
                "test-model",
            )
            .unwrap();

        let trace = kernel.trace_snapshot(10);
        assert_eq!(trace.recent_observations.len(), 1);
        assert!(trace
            .recent_proposals
            .iter()
            .any(|proposal| proposal.id == local_id && proposal.state == "superseded"));
        let llm_trace = trace
            .recent_proposals
            .iter()
            .find(|proposal| proposal.id == llm.id && proposal.state == "pending")
            .expect("llm proposal trace should exist");
        let budget = llm_trace
            .context_budget
            .as_ref()
            .expect("context budget should be recorded for LLM proposal");
        assert_eq!(budget.task, "GhostWriting");
        assert!(budget.used <= budget.total_budget);
        assert!(!budget.source_reports.is_empty());
    }

    #[test]
    fn trace_snapshot_survives_kernel_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "forge-trace-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let ghost_id = {
            let memory = WriterMemory::open(&db_path).unwrap();
            let mut kernel = WriterAgentKernel::new("default", memory);
            let obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
            let proposals = kernel.observe(obs).unwrap();
            let ghost = proposals
                .iter()
                .find(|proposal| proposal.kind == ProposalKind::Ghost)
                .unwrap()
                .id
                .clone();
            kernel
                .apply_feedback(ProposalFeedback {
                    proposal_id: ghost.clone(),
                    action: FeedbackAction::Rejected,
                    final_text: None,
                    reason: Some("too early".to_string()),
                    created_at: 42,
                })
                .unwrap();
            ghost
        };

        let memory = WriterMemory::open(&db_path).unwrap();
        let kernel = WriterAgentKernel::new("default", memory);
        let trace = kernel.trace_snapshot(10);
        let _ = std::fs::remove_file(&db_path);

        assert_eq!(trace.recent_observations.len(), 1);
        let ghost_trace = trace
            .recent_proposals
            .iter()
            .find(|proposal| proposal.id == ghost_id && proposal.state == "feedback:Rejected")
            .expect("persisted ghost trace should exist");
        assert!(ghost_trace.context_budget.is_some());
        assert!(trace
            .recent_feedback
            .iter()
            .any(|feedback| feedback.proposal_id == ghost_id
                && feedback.action == "Rejected"
                && feedback.reason.as_deref() == Some("too early")));
    }

    #[test]
    fn proposal_ids_do_not_collide_across_kernel_restarts() {
        let db_path = std::env::temp_dir().join(format!(
            "forge-proposal-id-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let first_id = {
            let memory = WriterMemory::open(&db_path).unwrap();
            let mut kernel = WriterAgentKernel::new("default", memory);
            let proposals = kernel
                .observe(observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。"))
                .unwrap();
            proposals
                .iter()
                .find(|proposal| proposal.kind == ProposalKind::Ghost)
                .unwrap()
                .id
                .clone()
        };

        let second_id = {
            let memory = WriterMemory::open(&db_path).unwrap();
            let mut kernel = WriterAgentKernel::new("default", memory);
            let mut obs = observation("林墨停在旧门前，风从裂开的门缝里钻出来，带着潮湿的冷意。他没有立刻推门，只把手按在刀柄上。");
            obs.id = "obs-restart-second".to_string();
            let proposals = kernel.observe(obs).unwrap();
            proposals
                .iter()
                .find(|proposal| proposal.kind == ProposalKind::Ghost)
                .unwrap()
                .id
                .clone()
        };

        let memory = WriterMemory::open(&db_path).unwrap();
        let kernel = WriterAgentKernel::new("default", memory);
        let trace = kernel.trace_snapshot(10);
        let _ = std::fs::remove_file(&db_path);

        assert_ne!(first_id, second_id);
        assert!(trace
            .recent_proposals
            .iter()
            .any(|proposal| proposal.id == first_id));
        assert!(trace
            .recent_proposals
            .iter()
            .any(|proposal| proposal.id == second_id));
    }

    #[test]
    fn observe_emits_canon_conflict_from_memory_facts() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .upsert_canon_entity(
                "character",
                "林墨",
                &[],
                "主角",
                &serde_json::json!({ "weapon": "寒影刀" }),
                0.9,
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposals = kernel
            .observe(observation("林墨拔出长剑，指向门外的人。"))
            .unwrap();

        assert!(proposals
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::ContinuityWarning));
    }

    #[test]
    fn observe_emits_and_dedupes_diagnostic_pacing_proposal() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let paragraph = "风".repeat(2001);
        let mut obs = observation(&paragraph);
        obs.cursor = Some(TextRange {
            from: paragraph.chars().count(),
            to: paragraph.chars().count(),
        });

        let first = kernel.observe(obs.clone()).unwrap();
        let second = kernel.observe(obs).unwrap();

        assert!(first
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::StyleNote
                && proposal.preview.contains("段落较长")));
        assert!(!second
            .iter()
            .any(|proposal| proposal.kind == ProposalKind::StyleNote
                && proposal.preview.contains("段落较长")));
    }

    #[test]
    fn observe_ghost_uses_context_pack_evidence() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .upsert_canon_entity(
                "character",
                "林墨",
                &[],
                "主角",
                &serde_json::json!({ "weapon": "寒影刀" }),
                0.9,
            )
            .unwrap();
        memory
            .add_promise(
                "object_in_motion",
                "玉佩",
                "张三拿走玉佩",
                "Chapter-1",
                "Chapter-4",
                4,
            )
            .unwrap();

        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposals = kernel
            .observe(observation(
                "林墨停在旧门前，风声压低。他想起张三离开时攥紧的玉佩，却没有立刻追问。",
            ))
            .unwrap();
        let ghost = proposals
            .iter()
            .find(|p| p.kind == ProposalKind::Ghost)
            .unwrap();

        assert!(ghost
            .evidence
            .iter()
            .any(|e| e.source == EvidenceSource::Canon));
        assert!(ghost
            .evidence
            .iter()
            .any(|e| e.source == EvidenceSource::PromiseLedger));
        assert!(ghost.preview.contains("旧事") || ghost.preview.contains("兵器"));
    }

    #[test]
    fn observe_records_context_recalls_from_surfaced_evidence() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .upsert_canon_entity(
                "character",
                "林墨",
                &[],
                "主角",
                &serde_json::json!({ "weapon": "寒影刀" }),
                0.9,
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let proposals = kernel
            .observe(observation("林墨拔出长剑，指向门外的人。"))
            .unwrap();
        let warning = proposals
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
            .expect("continuity warning should exist");

        let trace = kernel.trace_snapshot(10);
        let ledger = kernel.ledger_snapshot();

        assert!(trace.context_recalls.iter().any(|recall| {
            recall.source == "Canon"
                && recall.last_proposal_id == warning.id
                && recall.snippet.contains("寒影刀")
        }));
        assert!(ledger
            .context_recalls
            .iter()
            .any(|recall| recall.source == "Canon"));
    }

    #[test]
    fn observe_ghost_contains_three_parallel_branches() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let text =
            "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。";
        let mut obs = observation(text);
        let cursor = text.chars().count();
        obs.cursor = Some(TextRange {
            from: cursor,
            to: cursor,
        });
        let proposals = kernel.observe(obs).unwrap();
        let ghost = proposals
            .iter()
            .find(|p| p.kind == ProposalKind::Ghost)
            .unwrap();

        assert_eq!(ghost.alternatives.len(), 3);
        assert_eq!(ghost.alternatives[0].label, "A 直接表态");
        assert!(ghost.alternatives.iter().all(|alternative| matches!(
            alternative.operation,
            Some(WriterOperation::TextInsert { .. })
        )));
    }

    #[test]
    fn save_observation_suggests_memory_candidates_without_writing_ledgers() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs =
            observation("那个少年名叫沈照，袖中藏着一枚玉佩，却始终没有告诉任何人它的下落。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let proposals = kernel.observe(obs).unwrap();

        assert!(proposals.iter().any(|proposal| {
            proposal.kind == ProposalKind::CanonUpdate
                && matches!(
                    proposal.operations.first(),
                    Some(WriterOperation::CanonUpsertEntity { .. })
                )
        }));
        assert!(proposals.iter().any(|proposal| {
            proposal.kind == ProposalKind::PlotPromise
                && matches!(
                    proposal.operations.first(),
                    Some(WriterOperation::PromiseAdd { .. })
                )
        }));
        let ledger = kernel.ledger_snapshot();
        assert!(ledger.canon_entities.is_empty());
        assert!(ledger.open_promises.is_empty());
        assert_eq!(ledger.recent_chapter_results.len(), 1);
        assert!(ledger.recent_chapter_results[0].summary.contains("沈照"));
        assert!(ledger.recent_chapter_results[0]
            .new_clues
            .contains(&"玉佩".to_string()));
    }

    #[test]
    fn save_observation_records_chapter_result_feedback() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs =
            observation("林墨发现玉佩的下落，却开始怀疑张三。张三选择隐瞒真相，新的冲突就此埋下。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;
        obs.chapter_title = Some("第一章".to_string());
        obs.chapter_revision = Some("rev-1".to_string());
        obs.prefix = obs.paragraph.clone();

        kernel.observe(obs).unwrap();

        let ledger = kernel.ledger_snapshot();
        let result = ledger.recent_chapter_results.first().unwrap();
        assert_eq!(result.chapter_title, "第一章");
        assert_eq!(result.chapter_revision, "rev-1");
        assert!(result.summary.contains("玉佩"));
        assert!(result
            .new_conflicts
            .iter()
            .any(|line| line.contains("冲突")));
        assert!(result.new_clues.contains(&"玉佩".to_string()));
        assert!(result.source_ref.contains("chapter_save:第一章:rev-1"));
    }

    #[test]
    fn invalid_task_packet_is_rejected_before_trace() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let packet = TaskPacket::new(
            "bad-packet",
            "missing foundation fields",
            TaskScope::Chapter,
            1,
        );

        let error = kernel
            .record_task_packet("obs-1", "ChapterGeneration", packet)
            .unwrap_err();

        assert!(error.contains("scopeRef"));
        assert!(kernel.trace_snapshot(10).task_packets.is_empty());
    }

    #[test]
    fn save_observation_result_feedback_feeds_next_task_packet() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs =
            observation("林墨发现玉佩的下落，却开始怀疑张三。张三选择隐瞒真相，新的冲突就此埋下。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;
        obs.chapter_title = Some("第一章".to_string());
        obs.chapter_revision = Some("rev-1".to_string());
        obs.prefix = obs.paragraph.clone();

        kernel.observe(obs).unwrap();
        let next = observation("林墨深吸一口气，说道：“");
        kernel
            .create_llm_ghost_proposal(next, "我已经知道玉佩在哪了。".to_string(), "eval-model")
            .unwrap();

        let trace = kernel.trace_snapshot(10);
        let packet_trace = trace
            .task_packets
            .iter()
            .find(|packet| packet.task == "GhostWriting")
            .expect("next ghost task should record a task packet");
        assert!(packet_trace.foundation_complete);
        assert_eq!(packet_trace.packet.scope, TaskScope::CursorWindow);
        assert!(packet_trace
            .packet
            .required_context
            .iter()
            .any(|context| context.source_type == "ResultFeedback" && context.required));
        assert!(packet_trace
            .packet
            .beliefs
            .iter()
            .any(|belief| belief.subject == "ResultFeedback" && belief.statement.contains("玉佩")));
    }

    #[test]
    fn save_observation_calibrates_completed_chapter_mission() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .ensure_chapter_mission_seed(
                "default",
                "Chapter-1",
                "林墨追查玉佩下落。",
                "玉佩",
                "提前揭开真相",
                "下落",
                "test",
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("林墨发现玉佩的下落，但张三仍没有说出真相。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        kernel.observe(obs).unwrap();

        let mission = kernel
            .ledger_snapshot()
            .active_chapter_mission
            .expect("mission should stay active");
        assert_eq!(mission.status, "completed");
        assert!(mission.source_ref.contains("result_feedback:chapter_save"));
    }

    #[test]
    fn save_observation_marks_chapter_mission_drifted_on_must_not_hit() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .ensure_chapter_mission_seed(
                "default",
                "Chapter-1",
                "林墨追查玉佩下落。",
                "玉佩",
                "真相",
                "下落",
                "test",
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("林墨发现玉佩的下落，并当场揭开真相。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        kernel.observe(obs).unwrap();

        let mission = kernel
            .ledger_snapshot()
            .active_chapter_mission
            .expect("mission should stay active");
        assert_eq!(mission.status, "drifted");
        assert!(kernel
            .ledger_snapshot()
            .recent_decisions
            .iter()
            .any(|decision| decision.decision == "mission_status:drifted"));
    }

    #[test]
    fn ledger_snapshot_derives_next_beat_from_latest_result_and_promises() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        memory
            .add_promise(
                "object_in_motion",
                "玉佩",
                "张三拿走玉佩",
                "Chapter-1",
                "Chapter-3",
                4,
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("林墨发现玉佩的下落，却开始怀疑张三。新的冲突就此埋下。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;
        obs.chapter_title = Some("Chapter-2".to_string());

        kernel.observe(obs).unwrap();

        let next_beat = kernel
            .ledger_snapshot()
            .next_beat
            .expect("next beat should be derived from saved result");
        assert!(next_beat.goal.contains("冲突"));
        assert!(next_beat
            .carryovers
            .iter()
            .any(|line| line.contains("玉佩")));
        assert!(next_beat
            .source_refs
            .iter()
            .any(|source| source.contains("chapter_save:Chapter-2")));
    }

    #[test]
    fn accepted_memory_candidate_writes_ledger() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let proposal = kernel
            .observe(obs)
            .unwrap()
            .into_iter()
            .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
            .unwrap();
        let result = kernel
            .approve_editor_operation_with_approval(
                proposal.operations[0].clone(),
                "",
                Some(&test_approval("memory_candidate")),
            )
            .unwrap();

        assert!(result.success);
        assert!(kernel
            .ledger_snapshot()
            .canon_entities
            .iter()
            .any(|entity| entity.name == "沈照"));
    }

    #[test]
    fn promise_resolve_operation_closes_open_promise() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let promise_id = memory
            .add_promise(
                "object_in_motion",
                "玉佩",
                "张三拿走玉佩",
                "Chapter-1",
                "Chapter-4",
                4,
            )
            .unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::PromiseResolve {
                    promise_id: promise_id.to_string(),
                    chapter: "Chapter-4".to_string(),
                },
                "",
                Some(&test_approval("promise_test")),
            )
            .unwrap();

        assert!(result.success);
        assert!(kernel.ledger_snapshot().open_promises.is_empty());
    }

    #[test]
    fn story_contract_operation_updates_ledger_snapshot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("novel-a", memory);
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::StoryContractUpsert {
                    contract: crate::writer_agent::operation::StoryContractOp {
                        project_id: "novel-a".to_string(),
                        title: "寒影录".to_string(),
                        genre: "玄幻".to_string(),
                        target_reader: "长篇玄幻读者".to_string(),
                        reader_promise: "刀客追查玉佩真相。".to_string(),
                        first_30_chapter_promise: "建立宗门危机与玉佩谜团。".to_string(),
                        main_conflict: "复仇与守护的冲突。".to_string(),
                        structural_boundary: "不得提前泄露玉佩来源。".to_string(),
                        tone_contract: "克制、冷峻、少解释。".to_string(),
                    },
                },
                "",
                Some(&test_approval("contract_test")),
            )
            .unwrap();

        assert!(result.success);
        let ledger = kernel.ledger_snapshot();
        let contract = ledger.story_contract.unwrap();
        assert_eq!(contract.title, "寒影录");
        assert!(contract.render_for_context().contains("前30章承诺"));
    }

    #[test]
    fn story_contract_operation_rejects_incomplete_foundation() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("novel-a", memory);
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::StoryContractUpsert {
                    contract: crate::writer_agent::operation::StoryContractOp {
                        project_id: "novel-a".to_string(),
                        title: "寒影录".to_string(),
                        genre: "玄幻".to_string(),
                        target_reader: "".to_string(),
                        reader_promise: "爽文".to_string(),
                        first_30_chapter_promise: "".to_string(),
                        main_conflict: "复仇".to_string(),
                        structural_boundary: "".to_string(),
                        tone_contract: "".to_string(),
                    },
                },
                "",
                Some(&test_approval("contract_test")),
            )
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.error.unwrap().code, "invalid");
        assert!(kernel.ledger_snapshot().story_contract.is_none());
    }

    #[test]
    fn chapter_mission_operation_updates_ledger_snapshot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("novel-a", memory);
        kernel.active_chapter = Some("第一章".to_string());
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::ChapterMissionUpsert {
                    mission: crate::writer_agent::operation::ChapterMissionOp {
                        project_id: "novel-a".to_string(),
                        chapter_title: "第一章".to_string(),
                        mission: "林墨发现玉佩线索。".to_string(),
                        must_include: "推进玉佩线索".to_string(),
                        must_not: "不要提前揭开真相".to_string(),
                        expected_ending: "以新的疑问收束。".to_string(),
                        status: "active".to_string(),
                        source_ref: "test".to_string(),
                    },
                },
                "",
                Some(&test_approval("mission_test")),
            )
            .unwrap();

        assert!(result.success);
        let ledger = kernel.ledger_snapshot();
        assert_eq!(ledger.chapter_missions.len(), 1);
        assert_eq!(
            ledger.active_chapter_mission.unwrap().mission,
            "林墨发现玉佩线索。"
        );
        assert_eq!(
            kernel
                .ledger_snapshot()
                .active_chapter_mission
                .unwrap()
                .status,
            "in_progress"
        );
    }

    #[test]
    fn chapter_mission_operation_rejects_vague_foundation() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("novel-a", memory);
        kernel.active_chapter = Some("第一章".to_string());
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::ChapterMissionUpsert {
                    mission: crate::writer_agent::operation::ChapterMissionOp {
                        project_id: "novel-a".to_string(),
                        chapter_title: "第一章".to_string(),
                        mission: "打架".to_string(),
                        must_include: "".to_string(),
                        must_not: "剧透".to_string(),
                        expected_ending: "".to_string(),
                        status: "in_progress".to_string(),
                        source_ref: "test".to_string(),
                    },
                },
                "",
                Some(&test_approval("mission_test")),
            )
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.error.unwrap().code, "invalid");
        assert!(kernel.ledger_snapshot().active_chapter_mission.is_none());
    }

    #[test]
    fn llm_memory_candidates_parse_filter_and_dedupe() {
        let obs = observation("沈照把玉佩藏进袖中。");
        let value = serde_json::json!({
            "canon": [
                {
                    "kind": "character",
                    "name": "沈照",
                    "aliases": ["少年"],
                    "summary": "沈照把玉佩藏进袖中。",
                    "attributes": { "object": "玉佩" },
                    "confidence": 0.82
                },
                {
                    "kind": "character",
                    "name": "沈照",
                    "summary": "重复条目",
                    "confidence": 0.92
                },
                {
                    "kind": "object",
                    "name": "低",
                    "summary": "置信太低",
                    "confidence": 0.3
                }
            ],
            "promises": [
                {
                    "kind": "object_in_motion",
                    "title": "玉佩",
                    "description": "玉佩的下落需要后续交代。",
                    "introducedChapter": "Chapter-1",
                    "expectedPayoff": "说明玉佩来源",
                    "priority": 4,
                    "confidence": 0.81
                }
            ]
        });

        let candidates = llm_memory_candidates_from_value(value, &obs, "test-model");

        assert_eq!(candidates.len(), 2);
        assert!(matches!(
            &candidates[0],
            MemoryCandidate::Canon(entity) if entity.name == "沈照"
        ));
        assert!(matches!(
            &candidates[1],
            MemoryCandidate::Promise(promise) if promise.title == "玉佩"
        ));
    }

    #[test]
    fn llm_memory_proposal_replaces_local_candidate_for_same_slot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let local = kernel.observe(obs.clone()).unwrap();
        let local_canon_id = local
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
            .unwrap()
            .id
            .clone();
        let llm = kernel.create_llm_memory_proposals(
            obs,
            serde_json::json!({
                "canon": [{
                    "kind": "character",
                    "name": "沈照",
                    "summary": "沈照是本章出现的少年，袖中藏着玉佩。",
                    "attributes": { "object": "玉佩" },
                    "confidence": 0.86
                }],
                "promises": []
            }),
            "test-model",
        );

        assert_eq!(llm.len(), 1);
        assert!(llm[0].rationale.contains("LLM增强记忆抽取"));
        assert!(kernel.superseded_proposals.contains(&local_canon_id));
        assert!(kernel
            .pending_proposals()
            .iter()
            .any(|proposal| proposal.id == llm[0].id));
    }

    #[test]
    fn rejected_memory_candidate_suppresses_future_same_slot() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let first = kernel.observe(obs.clone()).unwrap();
        let canon = first
            .iter()
            .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
            .unwrap();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: canon.id.clone(),
                action: FeedbackAction::Rejected,
                final_text: None,
                reason: Some("not a durable canon item".to_string()),
                created_at: now_ms(),
            })
            .unwrap();

        let mut next = obs;
        next.id = "obs-save-2".to_string();
        let second = kernel.observe(next).unwrap();

        assert!(!second.iter().any(|proposal| {
            matches!(
                proposal.operations.first(),
                Some(WriterOperation::CanonUpsertEntity { entity }) if entity.name == "沈照"
            )
        }));
    }

    #[test]
    fn accepted_memory_candidate_records_positive_extraction_preference() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let proposal = kernel
            .observe(obs)
            .unwrap()
            .into_iter()
            .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
            .unwrap();
        let proposal_id = proposal.id.clone();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal_id.clone(),
                action: FeedbackAction::Accepted,
                final_text: None,
                reason: None,
                created_at: now_ms(),
            })
            .unwrap();

        let preferences = kernel.memory.list_style_preferences(20).unwrap();
        assert!(!preferences.iter().any(|preference| {
            preference
                .key
                .contains("memory_extract:memory|canon|character|沈照")
        }));

        let approval = test_approval_for_proposal("memory_candidate", &proposal_id);
        kernel
            .approve_editor_operation_with_approval(
                proposal.operations[0].clone(),
                "",
                Some(&approval),
            )
            .unwrap();
        kernel
            .apply_feedback(proposal_feedback(
                proposal_id,
                FeedbackAction::Accepted,
                None,
            ))
            .unwrap();

        let preferences = kernel.memory.list_style_preferences(20).unwrap();
        assert!(preferences.iter().any(|preference| {
            preference
                .key
                .contains("memory_extract:memory|canon|character|沈照")
                && preference.accepted_count == 1
        }));
    }

    #[test]
    fn ledger_snapshot_includes_memory_audit_for_candidate_feedback() {
        let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
        let mut kernel = WriterAgentKernel::new("default", memory);
        let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
        obs.reason = ObservationReason::Save;
        obs.source = ObservationSource::ChapterSave;

        let proposal = kernel
            .observe(obs)
            .unwrap()
            .into_iter()
            .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
            .unwrap();
        kernel
            .approve_editor_operation_with_approval(
                proposal.operations[0].clone(),
                "",
                Some(&test_approval_for_proposal("memory_audit", &proposal.id)),
            )
            .unwrap();
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal.id.clone(),
                action: FeedbackAction::Accepted,
                final_text: None,
                reason: Some("durable character".to_string()),
                created_at: 42,
            })
            .unwrap();

        let audit = kernel.ledger_snapshot().memory_audit;
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].proposal_id, proposal.id);
        assert_eq!(audit[0].action, "Accepted");
        assert!(audit[0].title.contains("沈照"));
        assert!(audit[0].evidence.contains("沈照"));
        assert_eq!(audit[0].reason.as_deref(), Some("durable character"));
    }

    #[test]
    fn memory_audit_survives_kernel_restart() {
        let db_path = std::env::temp_dir().join(format!(
            "forge-memory-audit-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        {
            let memory = WriterMemory::open(&db_path).unwrap();
            let mut kernel = WriterAgentKernel::new("default", memory);
            let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
            obs.reason = ObservationReason::Save;
            obs.source = ObservationSource::ChapterSave;

            let proposal = kernel
                .observe(obs)
                .unwrap()
                .into_iter()
                .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
                .unwrap();
            kernel
                .approve_editor_operation_with_approval(
                    proposal.operations[0].clone(),
                    "",
                    Some(&test_approval_for_proposal("memory_audit", &proposal.id)),
                )
                .unwrap();
            kernel
                .apply_feedback(ProposalFeedback {
                    proposal_id: proposal.id,
                    action: FeedbackAction::Accepted,
                    final_text: None,
                    reason: Some("durable character".to_string()),
                    created_at: 42,
                })
                .unwrap();
        }

        let memory = WriterMemory::open(&db_path).unwrap();
        let kernel = WriterAgentKernel::new("default", memory);
        let audit = kernel.ledger_snapshot().memory_audit;
        let _ = std::fs::remove_file(&db_path);

        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].action, "Accepted");
        assert!(audit[0].title.contains("沈照"));
        assert_eq!(audit[0].reason.as_deref(), Some("durable character"));
    }
}
