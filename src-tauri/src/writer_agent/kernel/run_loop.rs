use super::*;

impl WriterAgentKernel {
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
        self.record_context_pack_built_run_event(&request.observation, &context_pack, now_ms());
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
        let task_receipt = (request.task == WriterAgentTask::ContinuityDiagnostic).then(|| {
            crate::writer_agent::task_receipt::build_continuity_diagnostic_receipt(
                task_packet.id.clone(),
                &request.observation,
                &task_packet.objective,
                &context_pack,
                now_ms(),
            )
        });
        if let Some(receipt) = task_receipt.as_ref() {
            self.record_task_receipt_run_event(receipt);
        }

        if task_requires_story_grounding(&request.task) {
            let quality = self.contract_quality();
            if quality <= StoryContractQuality::Vague {
                task_packet.beliefs.push(TaskBelief {
                    subject: "Story Contract Quality".to_string(),
                    statement: format!(
                        "StoryContract quality is {:?}: this task may lack story-level grounding. Consider strengthening the Story Contract in settings.",
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
            task_receipt,
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
        if request.task == WriterAgentTask::ContinuityDiagnostic {
            if let Some(receipt) = result.task_receipt.as_ref() {
                let artifact =
                    crate::writer_agent::task_receipt::build_diagnostic_report_artifact(
                        receipt,
                        &result.answer,
                        now_ms(),
                    )
                    .map_err(|mismatches| {
                        format!(
                            "ContinuityDiagnostic diagnostic_report artifact failed receipt validation: {:?}",
                            mismatches
                        )
                    })?;
                self.record_task_artifact_run_event(&artifact);
            }
        }
        Ok(())
    }
}
