use super::*;

impl WriterAgentKernel {
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
        execute_writer_operation(
            &mut self.memory,
            &self.active_chapter,
            operation,
            current_content,
            current_revision,
        )
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

    pub(super) fn lifecycle_has_state(
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
}
