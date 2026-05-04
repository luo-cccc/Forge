use super::*;
use std::collections::BTreeMap;

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
            memory_reliability: memory_reliability_summary(
                self.memory.list_memory_feedback(200).unwrap_or_default(),
            ),
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
        let recent_proposals = if persisted_proposals.is_empty() {
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
                .collect::<Vec<_>>()
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
                .collect::<Vec<_>>()
        };
        let context_source_trends = context_source_trends(&recent_proposals);

        let mut snapshot = WriterAgentTraceSnapshot {
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
            recent_proposals,
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
            run_events: self.run_events(limit),
            post_write_diagnostics: self
                .run_events(limit)
                .into_iter()
                .filter(|event| event.event_type == "writer.post_write_diagnostics")
                .filter_map(|event| serde_json::from_value(event.data).ok())
                .collect(),
            context_source_trends,
            context_recalls: self
                .memory
                .list_context_recalls(&self.project_id, limit)
                .unwrap_or_default(),
            product_metrics: self.product_metrics(),
            product_metrics_trend: self.product_metrics_trend(limit),
            metacognitive_snapshot: Default::default(),
        };
        snapshot.metacognitive_snapshot =
            crate::writer_agent::metacognition::metacognitive_snapshot_from_trace(&snapshot);
        snapshot
    }

    pub fn export_trajectory(&self, limit: usize) -> super::trajectory::WriterTrajectoryExport {
        super::trajectory::export_trace_snapshot(
            &self.project_id,
            &self.session_id,
            &self.trace_snapshot(limit),
        )
    }

    pub fn inspector_timeline(&self, limit: usize) -> WriterInspectorTimeline {
        crate::writer_agent::inspector::build_inspector_timeline(&self.trace_snapshot(limit), limit)
    }

    pub fn companion_timeline_summary(&self) -> WriterInspectorTimeline {
        crate::writer_agent::inspector::build_companion_timeline_summary(&self.trace_snapshot(20))
    }

    fn product_metrics(&self) -> WriterProductMetrics {
        product_metrics_from_trace(
            &self.observations,
            &self.proposals,
            &self.feedback_events,
            &self.operation_lifecycle,
            self.memory.list_context_recalls(&self.project_id, 50),
            self.memory.list_chapter_missions(&self.project_id, 250),
        )
    }

    fn product_metrics_trend(&self, limit: usize) -> WriterProductMetricsTrend {
        let event_limit = limit.max(20).saturating_mul(40).min(5_000);
        let events = self
            .memory
            .list_project_run_events(&self.project_id, event_limit)
            .unwrap_or_default();
        product_metrics_trend_from_run_events(&events, limit.min(12).max(3))
    }
}

#[derive(Default)]
struct MemoryReliabilityAccumulator {
    slot: String,
    category: String,
    reinforcement_count: u64,
    correction_count: u64,
    net_confidence_delta: f64,
    last_action: String,
    last_source_error: Option<String>,
    last_reason: Option<String>,
    last_proposal_id: String,
    updated_at: u64,
}

fn memory_reliability_summary(
    feedback: Vec<super::memory::MemoryFeedbackSummary>,
) -> Vec<WriterMemoryReliabilitySummary> {
    let mut slots = BTreeMap::<String, MemoryReliabilityAccumulator>::new();
    for event in feedback {
        let entry =
            slots
                .entry(event.slot.clone())
                .or_insert_with(|| MemoryReliabilityAccumulator {
                    slot: event.slot.clone(),
                    category: event.category.clone(),
                    ..Default::default()
                });
        if entry.category.trim().is_empty() || entry.category == "unknown" {
            entry.category = event.category.clone();
        }
        match event.action.as_str() {
            "reinforcement" => {
                entry.reinforcement_count = entry.reinforcement_count.saturating_add(1)
            }
            "correction" => entry.correction_count = entry.correction_count.saturating_add(1),
            _ => {}
        }
        entry.net_confidence_delta += event.confidence_delta;
        if event.created_at >= entry.updated_at {
            entry.updated_at = event.created_at;
            entry.last_action = event.action.clone();
            entry.last_source_error = event.source_error.clone();
            entry.last_reason = event.reason.clone();
            entry.last_proposal_id = event.proposal_id.clone();
        }
    }

    let mut summaries = slots
        .into_values()
        .map(|entry| {
            let reliability = (0.5 + entry.net_confidence_delta).clamp(0.0, 1.0);
            let status = if entry.correction_count > 0
                && entry.correction_count >= entry.reinforcement_count
            {
                "needs_review"
            } else if reliability >= 0.55 && entry.reinforcement_count > 0 {
                "trusted"
            } else {
                "unproven"
            };
            WriterMemoryReliabilitySummary {
                slot: entry.slot,
                category: entry.category,
                status: status.to_string(),
                reliability,
                reinforcement_count: entry.reinforcement_count,
                correction_count: entry.correction_count,
                net_confidence_delta: entry.net_confidence_delta,
                last_action: entry.last_action,
                last_source_error: entry.last_source_error,
                last_reason: entry.last_reason,
                last_proposal_id: entry.last_proposal_id,
                updated_at: entry.updated_at,
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        reliability_status_weight(&right.status)
            .cmp(&reliability_status_weight(&left.status))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.slot.cmp(&right.slot))
    });
    summaries
}

fn reliability_status_weight(status: &str) -> u8 {
    match status {
        "needs_review" => 3,
        "unproven" => 2,
        "trusted" => 1,
        _ => 0,
    }
}

#[derive(Default)]
struct ContextSourceTrendAccumulator {
    appearances: usize,
    provided_count: usize,
    truncated_count: usize,
    dropped_count: usize,
    total_requested: usize,
    total_provided: usize,
    last_reason: Option<String>,
    last_truncation_reason: Option<String>,
}

fn context_source_trends(proposals: &[WriterProposalTrace]) -> Vec<WriterContextSourceTrend> {
    let mut trends = BTreeMap::<String, ContextSourceTrendAccumulator>::new();
    for report in proposals
        .iter()
        .filter_map(|proposal| proposal.context_budget.as_ref())
        .flat_map(|budget| budget.source_reports.iter())
    {
        let trend = trends.entry(report.source.clone()).or_default();
        trend.appearances += 1;
        trend.total_requested += report.requested;
        trend.total_provided += report.provided;
        if report.provided > 0 {
            trend.provided_count += 1;
        } else {
            trend.dropped_count += 1;
        }
        if report.truncated {
            trend.truncated_count += 1;
        }
        if !report.reason.trim().is_empty() {
            trend.last_reason = Some(report.reason.clone());
        }
        if let Some(reason) = report
            .truncation_reason
            .as_ref()
            .filter(|reason| !reason.trim().is_empty())
        {
            trend.last_truncation_reason = Some(reason.clone());
        }
    }

    let mut trends = trends
        .into_iter()
        .map(|(source, trend)| WriterContextSourceTrend {
            source,
            appearances: trend.appearances,
            provided_count: trend.provided_count,
            truncated_count: trend.truncated_count,
            dropped_count: trend.dropped_count,
            total_requested: trend.total_requested,
            total_provided: trend.total_provided,
            average_provided: if trend.appearances == 0 {
                0.0
            } else {
                trend.total_provided as f64 / trend.appearances as f64
            },
            last_reason: trend.last_reason,
            last_truncation_reason: trend.last_truncation_reason,
        })
        .collect::<Vec<_>>();
    trends.sort_by(|left, right| {
        right
            .truncated_count
            .cmp(&left.truncated_count)
            .then_with(|| right.dropped_count.cmp(&left.dropped_count))
            .then_with(|| right.appearances.cmp(&left.appearances))
            .then_with(|| left.source.cmp(&right.source))
    });
    trends
}
