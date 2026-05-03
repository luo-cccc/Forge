use super::*;

impl WriterAgentKernel {
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
        self.record_observation_run_event(&observation);

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
            let diagnostics = self.diagnostics.diagnose(
                &observation.paragraph,
                paragraph_offset,
                chapter_id,
                &observation.project_id,
                &self.memory,
            );
            if observation.reason == super::observation::ObservationReason::Save {
                let report =
                    crate::writer_agent::post_write_diagnostics::build_post_write_diagnostic_report(
                        &observation,
                        &diagnostics,
                        observation.created_at,
                    );
                self.record_post_write_diagnostic_report(&report);
                self.record_save_completed_run_event(
                    observation.id.clone(),
                    observation.chapter_title.clone(),
                    observation.chapter_revision.clone(),
                    "chapter_save:observed",
                    None,
                    None,
                    Some(&report),
                    observation.created_at,
                );
            }
            for diagnostic in diagnostics {
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
