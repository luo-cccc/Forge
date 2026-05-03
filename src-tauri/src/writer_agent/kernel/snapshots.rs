use super::*;

impl WriterAgentKernel {
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

    pub(super) fn contract_quality(&self) -> StoryContractQuality {
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

    fn product_metrics(&self) -> WriterProductMetrics {
        product_metrics_from_trace(
            &self.proposals,
            &self.feedback_events,
            &self.operation_lifecycle,
            self.memory.list_context_recalls(&self.project_id, 50),
            self.memory.list_chapter_missions(&self.project_id, 250),
        )
    }
}
