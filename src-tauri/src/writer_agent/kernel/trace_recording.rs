use super::*;

impl WriterAgentKernel {
    pub fn run_events(&self, limit: usize) -> Vec<WriterRunEvent> {
        let persisted = self
            .memory
            .list_run_events(&self.project_id, &self.session_id, limit)
            .unwrap_or_default()
            .into_iter()
            .map(|event| WriterRunEvent {
                seq: event.seq,
                ts_ms: event.ts_ms,
                project_id: event.project_id,
                session_id: event.session_id,
                task_id: event.task_id,
                event_type: event.event_type,
                source_refs: event.source_refs,
                data: event.data,
            })
            .collect::<Vec<_>>();
        if persisted.is_empty() {
            self.run_events.recent(limit)
        } else {
            persisted
        }
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

    pub(super) fn record_task_packet_for(
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

    pub(super) fn push_task_packet_trace(
        &mut self,
        observation_id: String,
        task: String,
        packet: TaskPacket,
    ) {
        let coverage = packet.foundation_coverage();
        let trace = WriterTaskPacketTrace {
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
        };
        self.record_run_event(
            "task_packet_created",
            trace.packet.created_at_ms,
            Some(trace.id.clone()),
            vec![trace.observation_id.clone()],
            serde_json::json!({
                "id": trace.id,
                "observationId": trace.observation_id,
                "task": trace.task,
                "objective": trace.objective,
                "scope": trace.scope,
                "intent": trace.intent,
                "requiredContextCount": trace.required_context_count,
                "beliefCount": trace.belief_count,
                "successCriteriaCount": trace.success_criteria_count,
                "maxSideEffectLevel": trace.max_side_effect_level,
                "feedbackCheckpointCount": trace.feedback_checkpoint_count,
                "foundationComplete": trace.foundation_complete,
                "requiredSources": trace.packet.required_context
                    .iter()
                    .filter(|context| context.required)
                    .map(|context| context.source_type.clone())
                    .collect::<Vec<_>>(),
            }),
        );
        self.task_packets.push(trace);
    }

    pub(super) fn push_operation_lifecycle(
        &mut self,
        proposal_id: Option<String>,
        operation: &WriterOperation,
        state: WriterOperationLifecycleState,
        approval_source: Option<String>,
        save_result: Option<String>,
        feedback_result: Option<String>,
        created_at: u64,
    ) {
        let source_task = proposal_id
            .as_deref()
            .and_then(|id| self.proposals.iter().find(|proposal| proposal.id == id))
            .map(|proposal| format!("{:?}", proposal.kind));
        let operation_kind = operation_kind_label(operation).to_string();
        let affected_scope = operation_affected_scope(operation);
        self.operation_lifecycle
            .push(WriterOperationLifecycleTrace {
                source_task: source_task.clone(),
                proposal_id: proposal_id.clone(),
                operation_kind: operation_kind.clone(),
                approval_source: approval_source.clone(),
                affected_scope: affected_scope.clone(),
                state: state.clone(),
                save_result: save_result.clone(),
                feedback_result: feedback_result.clone(),
                created_at,
            });
        self.record_run_event(
            "operation_lifecycle",
            created_at,
            proposal_id.clone(),
            proposal_id.iter().cloned().collect(),
            serde_json::json!({
                "proposalId": proposal_id,
                "operationKind": operation_kind,
                "sourceTask": source_task,
                "approvalSource": approval_source,
                "affectedScope": affected_scope,
                "state": state,
                "saveResult": save_result,
                "feedbackResult": feedback_result,
            }),
        );
    }

    pub(super) fn record_observation_run_event(&mut self, observation: &WriterObservation) {
        self.record_run_event(
            "observation",
            observation.created_at,
            Some(observation.id.clone()),
            observation_source_refs(observation),
            serde_json::json!({
                "id": observation.id,
                "source": observation.source,
                "reason": observation.reason,
                "chapterTitle": observation.chapter_title,
                "chapterRevision": observation.chapter_revision,
                "paragraphSnippet": snippet(&observation.paragraph, 160),
                "hasSelection": observation.has_selection(),
                "editorDirty": observation.editor_dirty,
            }),
        );
    }

    pub(super) fn record_proposal_run_event(&mut self, proposal: &AgentProposal, created_at: u64) {
        self.record_run_event(
            "proposal_created",
            created_at,
            Some(proposal.id.clone()),
            proposal_source_refs(proposal),
            serde_json::json!({
                "id": proposal.id,
                "observationId": proposal.observation_id,
                "kind": proposal.kind,
                "priority": proposal.priority,
                "confidence": proposal.confidence,
                "operationKinds": proposal.operations
                    .iter()
                    .map(|operation| operation_kind_label(operation).to_string())
                    .collect::<Vec<_>>(),
                "evidenceCount": proposal.evidence.len(),
                "previewSnippet": snippet(&proposal.preview, 160),
                "expiresAt": proposal.expires_at,
            }),
        );
    }

    pub(super) fn record_feedback_run_event(
        &mut self,
        feedback: &ProposalFeedback,
        feedback_result: Option<&str>,
    ) {
        self.record_run_event(
            "feedback_recorded",
            feedback.created_at,
            Some(feedback.proposal_id.clone()),
            vec![feedback.proposal_id.clone()],
            serde_json::json!({
                "proposalId": feedback.proposal_id,
                "action": feedback.action,
                "reason": feedback.reason,
                "hasFinalText": feedback.final_text.as_ref().is_some_and(|text| !text.trim().is_empty()),
                "feedbackResult": feedback_result,
            }),
        );
    }

    pub fn record_failure_evidence_bundle(
        &mut self,
        bundle: &crate::writer_agent::task_receipt::WriterFailureEvidenceBundle,
    ) {
        self.record_run_event(
            "error",
            bundle.created_at_ms,
            bundle.task_id.clone(),
            bundle.evidence_refs.clone(),
            serde_json::json!(bundle),
        );
    }

    pub(super) fn record_post_write_diagnostic_report(
        &mut self,
        report: &crate::writer_agent::post_write_diagnostics::WriterPostWriteDiagnosticReport,
    ) {
        self.record_run_event(
            "post_write_diagnostics",
            report.created_at_ms,
            Some(report.observation_id.clone()),
            report.source_refs.clone(),
            serde_json::json!(report),
        );
    }

    fn record_run_event(
        &mut self,
        event_type: &str,
        ts_ms: u64,
        task_id: Option<String>,
        source_refs: Vec<String>,
        data: serde_json::Value,
    ) {
        let event = self.run_events.append(
            &self.project_id,
            &self.session_id,
            format!("writer.{}", event_type),
            ts_ms,
            task_id,
            source_refs,
            data,
        );
        self.memory
            .record_run_event(&memory::RunEventSummary {
                seq: event.seq,
                project_id: event.project_id,
                session_id: event.session_id,
                task_id: event.task_id,
                event_type: event.event_type,
                source_refs: event.source_refs,
                data: event.data,
                ts_ms: event.ts_ms,
            })
            .ok();
    }
}

fn observation_source_refs(observation: &WriterObservation) -> Vec<String> {
    let mut refs = vec![observation.id.clone()];
    if let Some(chapter) = observation.chapter_title.as_ref() {
        refs.push(format!("chapter:{}", chapter));
    }
    if let Some(revision) = observation.chapter_revision.as_ref() {
        refs.push(format!("revision:{}", revision));
    }
    refs
}

fn proposal_source_refs(proposal: &AgentProposal) -> Vec<String> {
    let mut refs = vec![proposal.observation_id.clone()];
    refs.extend(
        proposal
            .evidence
            .iter()
            .map(|evidence| format!("{:?}:{}", evidence.source, evidence.reference)),
    );
    refs
}
