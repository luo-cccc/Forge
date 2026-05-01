//! WriterAgentKernel — persistent project agent that owns observations,
//! proposals, memory, canon, and feedback.

use super::observation::WriterObservation;
use super::proposal::{AgentProposal, ProposalKind, ProposalPriority, EvidenceRef, EvidenceSource};
use super::operation::{WriterOperation, OperationResult, execute_text_operation};
use super::feedback::{ProposalFeedback, FeedbackAction};
use super::memory::WriterMemory;
use super::canon::CanonEngine;

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

pub struct WriterAgentKernel {
    pub project_id: String,
    pub session_id: String,
    pub memory: WriterMemory,
    pub canon: CanonEngine,
    observations: Vec<WriterObservation>,
    proposals: Vec<AgentProposal>,
    feedback_events: Vec<ProposalFeedback>,
    observation_counter: u64,
    proposal_counter: u64,
    pub active_chapter: Option<String>,
}

impl WriterAgentKernel {
    pub fn new(project_id: &str, memory: WriterMemory) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: format!("session-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()),
            memory,
            canon: CanonEngine::new(),
            observations: Vec::new(),
            proposals: Vec::new(),
            feedback_events: Vec::new(),
            observation_counter: 0,
            proposal_counter: 0,
            active_chapter: None,
        }
    }

    pub async fn observe(&mut self, observation: WriterObservation) -> Result<Vec<AgentProposal>, String> {
        self.observation_counter += 1;
        let mut proposals = Vec::new();
        let obs_id = observation.id.clone();

        let canon_checks = self.canon.check_paragraph(&observation.paragraph, &self.memory);
        for check in &canon_checks {
            if check.conflict {
                proposals.push(AgentProposal {
                    id: format!("prop_{}", self.proposal_counter),
                    observation_id: obs_id.clone(),
                    kind: ProposalKind::ContinuityWarning,
                    priority: ProposalPriority::Urgent,
                    target: observation.cursor.clone().map(|c| {
                        super::observation::TextRange { from: c.from, to: c.to }
                    }),
                    preview: format!("{}: canon认为 {} = {}, 但文中出现 {}",
                        check.entity_name, check.mentioned_attribute,
                        check.canon_value.as_deref().unwrap_or("?"),
                        check.mentioned_value),
                    operations: vec![],
                    rationale: format!("canon冲突: {}.{}", check.entity_name, check.mentioned_attribute),
                    evidence: vec![EvidenceRef {
                        source: EvidenceSource::Canon,
                        reference: check.entity_name.clone(),
                        snippet: format!("canon: {} = {}",
                            check.mentioned_attribute,
                            check.canon_value.as_deref().unwrap_or("?")),
                    }],
                    risks: vec!["修改文本以匹配canon".into()],
                    confidence: check.confidence,
                    expires_at: None,
                });
                self.proposal_counter += 1;
            }
        }

        if let Ok(promises) = self.memory.get_open_promises() {
            for (_kind, title, desc, chapter) in &promises {
                if observation.reason == super::observation::ObservationReason::ChapterSwitch {
                    proposals.push(AgentProposal {
                        id: format!("prop_{}", self.proposal_counter),
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
                        confidence: 0.7,
                        expires_at: None,
                    });
                    self.proposal_counter += 1;
                }
            }
        }

        if observation.reason == super::observation::ObservationReason::Idle
            && observation.paragraph.chars().count() >= 32
        {
            proposals.push(AgentProposal {
                id: format!("prop_{}", self.proposal_counter),
                observation_id: obs_id.clone(),
                kind: ProposalKind::Ghost,
                priority: ProposalPriority::Ambient,
                target: observation.cursor.clone().map(|c| {
                    super::observation::TextRange { from: c.to, to: c.to }
                }),
                preview: "续写...".into(),
                operations: vec![],
                rationale: "段落暂停，可提供续写建议".into(),
                evidence: vec![],
                risks: vec![],
                confidence: 0.55,
                expires_at: Some(observation.created_at + 30_000),
            });
            self.proposal_counter += 1;
        }

        self.observations.push(observation);
        self.proposals.extend(proposals.clone());
        Ok(proposals)
    }

    pub async fn apply_feedback(&mut self, feedback: ProposalFeedback) -> Result<(), String> {
        self.memory.record_feedback(
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
        ).map_err(|e| format!("feedback: {}", e))?;

        if feedback.is_positive() {
            if let Some(prop) = self.proposals.iter().find(|p| p.id == feedback.proposal_id) {
                self.memory.upsert_style_preference(
                    &format!("accepted_{:?}", prop.kind), &prop.rationale, true,
                ).ok();
            }
        }

        self.feedback_events.push(feedback);
        Ok(())
    }

    pub async fn execute_operation(
        &mut self,
        operation: WriterOperation,
        current_content: &str,
        current_revision: &str,
    ) -> Result<OperationResult, String> {
        match &operation {
            WriterOperation::TextInsert { .. } | WriterOperation::TextReplace { .. } => {
                match execute_text_operation(&operation, current_content, current_revision) {
                    Ok((_new_content, new_revision)) => Ok(OperationResult {
                        success: true, operation, error: None,
                        revision_after: Some(new_revision),
                    }),
                    Err(e) => Ok(OperationResult {
                        success: false, operation, error: Some(e), revision_after: None,
                    }),
                }
            }
            WriterOperation::CanonUpsertEntity { entity } => {
                self.memory.upsert_canon_entity(
                    &entity.kind, &entity.name, &entity.aliases,
                    &entity.summary, &entity.attributes, entity.confidence,
                ).map_err(|e| format!("canon: {}", e))?;
                Ok(OperationResult { success: true, operation, error: None, revision_after: None })
            }
            WriterOperation::PromiseAdd { promise } => {
                self.memory.add_promise(
                    &promise.kind, &promise.title, &promise.description,
                    &promise.introduced_chapter, &promise.expected_payoff, promise.priority,
                ).map_err(|e| format!("promise: {}", e))?;
                Ok(OperationResult { success: true, operation, error: None, revision_after: None })
            }
            _ => Ok(OperationResult {
                success: false, operation,
                error: Some(super::operation::OperationError::invalid("not implemented")),
                revision_after: None,
            }),
        }
    }

    pub fn status(&self) -> WriterAgentStatus {
        let open = self.memory.get_open_promises().map(|p| p.len()).unwrap_or(0);
        WriterAgentStatus {
            project_id: self.project_id.clone(),
            session_id: self.session_id.clone(),
            active_chapter: self.active_chapter.clone(),
            observation_count: self.observation_counter,
            proposal_count: self.proposal_counter,
            open_promise_count: open,
            pending_proposals: self.proposals.iter().filter(|p| {
                !self.feedback_events.iter().any(|f| f.proposal_id == p.id)
            }).count(),
            total_feedback_events: self.feedback_events.len() as u64,
        }
    }
}
