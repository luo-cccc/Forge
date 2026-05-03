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

    pub(super) fn record_memory_candidate_created_run_event(
        &mut self,
        proposal: &AgentProposal,
        slot: String,
        created_at: u64,
    ) {
        let operation_kinds = proposal
            .operations
            .iter()
            .map(|operation| operation_kind_label(operation).to_string())
            .collect::<Vec<_>>();
        self.record_run_event(
            "memory_candidate_created",
            created_at,
            Some(proposal.id.clone()),
            proposal_source_refs(proposal),
            serde_json::json!({
                "proposalId": proposal.id,
                "observationId": proposal.observation_id,
                "kind": proposal.kind,
                "slot": slot,
                "operationKinds": operation_kinds,
                "evidenceCount": proposal.evidence.len(),
                "previewSnippet": snippet(&proposal.preview, 160),
                "requiresAuthorReview": true,
                "writesLedgerImmediately": false,
            }),
        );
    }

    pub(super) fn record_context_pack_built_run_event(
        &mut self,
        observation: &WriterObservation,
        context_pack: &WritingContextPack,
        created_at: u64,
    ) {
        let task_id = format!(
            "{}:{}:{:?}:context_pack",
            self.session_id, observation.id, context_pack.task
        );
        let source_reports = context_pack_built_reports(context_pack);
        let truncated_source_count = source_reports
            .iter()
            .filter(|report| report.truncated)
            .count();
        let mut source_refs = observation_source_refs(observation);
        source_refs.extend(
            context_pack
                .sources
                .iter()
                .map(|source| format!("context_source:{:?}", source.source)),
        );

        self.record_context_pack_built_event(
            crate::writer_agent::kernel::WriterContextPackBuiltRunEvent {
                task_id: task_id.clone(),
                task: format!("{:?}", context_pack.task),
                source_count: context_pack.sources.len(),
                total_chars: context_pack.total_chars,
                budget_limit: context_pack.budget_limit,
                wasted: context_pack.budget_report.wasted,
                truncated_source_count,
                source_reports,
            },
            source_refs,
            created_at,
        );
    }

    pub fn record_chapter_context_pack_built_run_event(
        &mut self,
        context: &crate::chapter_generation::BuiltChapterContext,
        created_at: u64,
    ) {
        let task_id = format!(
            "{}:{}:ChapterGeneration:context_pack",
            self.session_id, context.request_id
        );
        let source_reports = chapter_context_pack_built_reports(context);
        let mut source_refs = vec![
            format!("receipt:{}", context.receipt.task_id),
            format!("chapter:{}", context.target.title),
            format!("revision:{}", context.base_revision),
        ];
        source_refs.extend(
            context
                .sources
                .iter()
                .filter(|source| source.included_chars > 0)
                .map(|source| format!("{}:{}", source.source_type, source.id)),
        );
        self.record_context_pack_built_event(
            crate::writer_agent::kernel::WriterContextPackBuiltRunEvent {
                task_id: task_id.clone(),
                task: "ChapterGeneration".to_string(),
                source_count: context.budget.source_count,
                total_chars: context.budget.included_chars,
                budget_limit: context.budget.max_chars,
                wasted: context
                    .budget
                    .max_chars
                    .saturating_sub(context.budget.included_chars),
                truncated_source_count: context.budget.truncated_source_count,
                source_reports,
            },
            source_refs,
            created_at,
        );
    }

    fn record_context_pack_built_event(
        &mut self,
        event: crate::writer_agent::kernel::WriterContextPackBuiltRunEvent,
        source_refs: Vec<String>,
        created_at: u64,
    ) {
        self.record_run_event(
            "context_pack_built",
            created_at,
            Some(event.task_id.clone()),
            source_refs,
            serde_json::json!(event),
        );
    }

    pub(super) fn record_approval_decided_run_event(
        &mut self,
        operation: &WriterOperation,
        approval: Option<&super::operation::OperationApproval>,
        approved: bool,
        reason: &str,
        created_at: u64,
    ) {
        let proposal_id = approval.and_then(|context| context.proposal_id.clone());
        let mut source_refs = Vec::new();
        if let Some(proposal_id) = proposal_id.as_ref() {
            source_refs.push(format!("proposal:{}", proposal_id));
        }
        if let Some(scope) = operation_affected_scope(operation) {
            source_refs.push(scope);
        }
        if let Some(context) = approval {
            source_refs.extend(approval_sources(context));
        }
        self.record_run_event(
            "approval_decided",
            created_at,
            proposal_id.clone(),
            source_refs,
            serde_json::json!({
                "proposalId": proposal_id,
                "operationKind": operation_kind_label(operation),
                "affectedScope": operation_affected_scope(operation),
                "decision": if approved { "approved" } else { "rejected" },
                "approvalSource": approval.map(|context| context.source.clone()),
                "actor": approval.map(|context| context.actor.clone()),
                "surfacedToUser": approval.map(|context| context.surfaced_to_user).unwrap_or(false),
                "reason": reason,
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

    pub(super) fn record_save_completed_run_event(
        &mut self,
        observation_id: impl Into<String>,
        chapter_title: Option<String>,
        chapter_revision: Option<String>,
        save_result: impl Into<String>,
        proposal_id: Option<String>,
        operation_kind: Option<String>,
        report: Option<
            &crate::writer_agent::post_write_diagnostics::WriterPostWriteDiagnosticReport,
        >,
        created_at_ms: u64,
    ) {
        let observation_id = observation_id.into();
        let save_result = save_result.into();
        let mut source_refs = vec![observation_id.clone()];
        if let Some(chapter) = chapter_title.as_ref() {
            source_refs.push(format!("chapter:{}", chapter));
        }
        if let Some(revision) = chapter_revision.as_ref() {
            source_refs.push(format!("revision:{}", revision));
        }
        if let Some(proposal_id) = proposal_id.as_ref() {
            source_refs.push(format!("proposal:{}", proposal_id));
        }
        if let Some(operation_kind) = operation_kind.as_ref() {
            source_refs.push(format!("operation:{}", operation_kind));
        }
        if let Some(report) = report {
            source_refs.extend(report.source_refs.iter().cloned());
        }

        self.record_run_event(
            "save_completed",
            created_at_ms,
            Some(observation_id.clone()),
            source_refs,
            serde_json::json!({
                "observationId": observation_id,
                "chapterTitle": chapter_title,
                "chapterRevision": chapter_revision,
                "saveResult": save_result,
                "proposalId": proposal_id,
                "operationKind": operation_kind,
                "postWriteReportId": report.map(|report| report.observation_id.clone()),
                "diagnosticTotalCount": report.map(|report| report.total_count).unwrap_or(0),
                "diagnosticErrorCount": report.map(|report| report.error_count).unwrap_or(0),
                "diagnosticWarningCount": report.map(|report| report.warning_count).unwrap_or(0),
            }),
        );
    }

    pub fn record_provider_budget_report(
        &mut self,
        task_id: impl Into<String>,
        report: &crate::writer_agent::provider_budget::WriterProviderBudgetReport,
        source_refs: Vec<String>,
        created_at_ms: u64,
    ) {
        let task_id = task_id.into();
        self.record_run_event(
            "provider_budget",
            created_at_ms,
            Some(task_id.clone()),
            source_refs,
            serde_json::json!({
                "taskId": task_id,
                "task": report.task,
                "model": report.model,
                "decision": report.decision,
                "approvalRequired": report.approval_required,
                "estimatedTotalTokens": report.estimated_total_tokens,
                "estimatedCostMicros": report.estimated_cost_micros,
                "reasons": &report.reasons,
                "remediation": &report.remediation,
                "providerBudget": report,
            }),
        );
    }

    pub fn record_model_started_run_event(
        &mut self,
        task_id: impl Into<String>,
        task: crate::writer_agent::provider_budget::WriterProviderBudgetTask,
        model: impl Into<String>,
        provider: impl Into<String>,
        stream: bool,
        source_refs: Vec<String>,
        report: Option<&crate::writer_agent::provider_budget::WriterProviderBudgetReport>,
        created_at_ms: u64,
    ) {
        let task_id = task_id.into();
        let model = model.into();
        let provider = provider.into();
        self.record_run_event(
            "model_started",
            created_at_ms,
            Some(task_id.clone()),
            source_refs,
            serde_json::json!({
                "taskId": task_id,
                "task": task,
                "model": model,
                "provider": provider,
                "stream": stream,
                "estimatedInputTokens": report.map(|report| report.estimated_input_tokens),
                "requestedOutputTokens": report.map(|report| report.requested_output_tokens),
                "estimatedTotalTokens": report.map(|report| report.estimated_total_tokens),
                "estimatedCostMicros": report.map(|report| report.estimated_cost_micros),
                "budgetDecision": report.map(|report| report.decision),
                "approvalRequired": report.map(|report| report.approval_required).unwrap_or(false),
            }),
        );
    }

    pub fn record_tool_called_run_event(
        &mut self,
        task_id: impl Into<String>,
        tool_name: impl Into<String>,
        phase: impl Into<String>,
        input: Option<&serde_json::Value>,
        result: Option<&agent_harness_core::ToolExecution>,
        source_refs: Vec<String>,
        created_at_ms: u64,
    ) {
        let task_id = task_id.into();
        let tool_name = tool_name.into();
        let phase = phase.into();
        let input_keys = input.map(json_object_keys).unwrap_or_default();
        let input_bytes = input
            .and_then(|value| serde_json::to_vec(value).ok())
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        let output_bytes = result
            .and_then(|execution| serde_json::to_vec(&execution.output).ok())
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        let remediation_codes = result
            .map(|execution| {
                execution
                    .remediation
                    .iter()
                    .map(|item| item.code.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let success = result.map(|execution| execution.error.is_none());
        self.record_run_event(
            "tool_called",
            created_at_ms,
            Some(task_id.clone()),
            source_refs,
            serde_json::json!({
                "taskId": task_id,
                "toolName": tool_name,
                "phase": phase,
                "success": success,
                "durationMs": result.map(|execution| execution.duration_ms),
                "inputKeys": input_keys,
                "inputBytes": input_bytes,
                "outputBytes": output_bytes,
                "error": result.and_then(|execution| execution.error.clone()),
                "remediationCodes": remediation_codes,
            }),
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

fn json_object_keys(value: &serde_json::Value) -> Vec<String> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

fn context_pack_built_reports(
    context_pack: &WritingContextPack,
) -> Vec<crate::writer_agent::kernel::WriterContextPackBuiltSourceReport> {
    context_pack
        .budget_report
        .source_reports
        .iter()
        .map(
            |source| crate::writer_agent::kernel::WriterContextPackBuiltSourceReport {
                source: source.source.clone(),
                id: None,
                label: None,
                requested: Some(source.requested),
                original_chars: None,
                provided: source.provided,
                truncated: source.truncated,
                required: context_pack
                    .task
                    .required_source_budgets()
                    .iter()
                    .any(|(required, _)| format!("{:?}", required) == source.source),
                reason: Some(source.reason.clone()).filter(|reason| !reason.trim().is_empty()),
                truncation_reason: source.truncation_reason.clone(),
            },
        )
        .collect()
}

fn chapter_context_pack_built_reports(
    context: &crate::chapter_generation::BuiltChapterContext,
) -> Vec<crate::writer_agent::kernel::WriterContextPackBuiltSourceReport> {
    context
        .sources
        .iter()
        .map(
            |source| crate::writer_agent::kernel::WriterContextPackBuiltSourceReport {
                source: source.source_type.clone(),
                id: Some(source.id.clone()).filter(|id| !id.trim().is_empty()),
                label: Some(source.label.clone()).filter(|label| !label.trim().is_empty()),
                requested: None,
                original_chars: Some(source.original_chars),
                provided: source.included_chars,
                truncated: source.truncated,
                required: matches!(
                    source.source_type.as_str(),
                    "instruction"
                        | "outline"
                        | "target_beat"
                        | "previous_chapters"
                        | "lorebook"
                        | "project_brain"
                ),
                reason: None,
                truncation_reason: source.truncated.then(|| {
                    format!(
                        "Chapter context budget included {} of {} chars.",
                        source.included_chars, source.original_chars
                    )
                }),
            },
        )
        .collect()
}
