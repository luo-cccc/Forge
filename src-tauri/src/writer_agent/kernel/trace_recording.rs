use super::*;

impl WriterAgentKernel {
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
}
