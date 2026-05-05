//! Product metric derivation from WriterAgent trace state.

use std::collections::{BTreeMap, HashMap};

use super::helpers::{operation_affected_scope, operation_kind_label};
use crate::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use crate::writer_agent::kernel::{WriterOperationLifecycleState, WriterOperationLifecycleTrace};
use crate::writer_agent::memory::{ChapterMissionSummary, ContextRecallSummary, RunEventSummary};
use crate::writer_agent::observation::{ObservationSource, WriterObservation};
use crate::writer_agent::proposal::{AgentProposal, ProposalKind};

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

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterProductMetricsTrend {
    pub source_event_count: usize,
    pub session_count: usize,
    pub overall_average_save_to_feedback_ms: Option<u64>,
    pub recent_average_save_to_feedback_ms: Option<u64>,
    pub previous_average_save_to_feedback_ms: Option<u64>,
    pub save_to_feedback_delta_ms: Option<i64>,
    pub overall_context_coverage_rate: f64,
    pub recent_context_coverage_rate: f64,
    pub previous_context_coverage_rate: f64,
    pub context_coverage_delta: Option<f64>,
    pub recent_sessions: Vec<WriterProductMetricSessionTrend>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterProductMetricSessionTrend {
    pub session_id: String,
    pub first_event_at: u64,
    pub last_event_at: u64,
    pub event_count: u64,
    pub proposal_count: u64,
    pub manual_ask_proposal_count: u64,
    pub manual_ask_operation_count: u64,
    pub manual_ask_converted_to_operation_rate: f64,
    pub feedback_count: u64,
    pub accepted_count: u64,
    pub rejected_count: u64,
    pub edited_count: u64,
    pub ignored_count: u64,
    pub proposal_acceptance_rate: f64,
    pub durable_save_success_rate: f64,
    pub average_save_to_feedback_ms: Option<u64>,
    pub save_feedback_sample_count: u64,
    pub context_pack_count: u64,
    pub context_requested_chars: u64,
    pub context_provided_chars: u64,
    pub context_coverage_rate: f64,
    pub context_truncated_source_count: u64,
    pub context_dropped_source_count: u64,
}

pub(crate) fn product_metrics_from_trace(
    observations: &[WriterObservation],
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

    let manual_observation_ids = observations
        .iter()
        .filter(|observation| observation.source == ObservationSource::ManualRequest)
        .map(|observation| observation.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let manual_proposals = proposals
        .iter()
        .filter(|proposal| manual_observation_ids.contains(proposal.observation_id.as_str()))
        .count() as u64;
    let manual_operations = proposals
        .iter()
        .filter(|proposal| {
            manual_observation_ids.contains(proposal.observation_id.as_str())
                && !proposal.operations.is_empty()
        })
        .count() as u64;
    let manual_ask_converted_to_operation_rate = ratio(manual_operations, manual_proposals);

    let recalls = context_recalls.unwrap_or_default();
    let promise_recalls = recalls
        .iter()
        .filter(|recall| is_promise_context_recall(&recall.source))
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

pub(crate) fn product_metrics_trend_from_run_events(
    events: &[RunEventSummary],
    session_limit: usize,
) -> WriterProductMetricsTrend {
    let mut sessions = BTreeMap::<String, SessionMetricAccumulator>::new();
    for event in events {
        let accumulator = sessions.entry(event.session_id.clone()).or_default();
        accumulator.record_event(event);
    }

    let mut session_trends = sessions
        .into_iter()
        .map(|(session_id, accumulator)| accumulator.into_trend(session_id))
        .filter(|trend| trend.event_count > 0)
        .collect::<Vec<_>>();
    session_trends.sort_by(|left, right| {
        right
            .last_event_at
            .cmp(&left.last_event_at)
            .then_with(|| right.first_event_at.cmp(&left.first_event_at))
            .then_with(|| left.session_id.cmp(&right.session_id))
    });

    let all_save_feedback_samples = session_trends
        .iter()
        .filter_map(|trend| {
            trend
                .average_save_to_feedback_ms
                .map(|average| (average, trend.save_feedback_sample_count))
        })
        .flat_map(|(average, count)| std::iter::repeat_n(average, count as usize))
        .collect::<Vec<_>>();
    let overall_average_save_to_feedback_ms = average_u64(&all_save_feedback_samples);
    let recent_average_save_to_feedback_ms = session_trends
        .iter()
        .find_map(|trend| trend.average_save_to_feedback_ms);
    let previous_average_save_to_feedback_ms = session_trends
        .iter()
        .filter_map(|trend| trend.average_save_to_feedback_ms)
        .nth(1);
    let save_to_feedback_delta_ms = recent_average_save_to_feedback_ms
        .zip(previous_average_save_to_feedback_ms)
        .map(|(recent, previous)| recent as i64 - previous as i64);
    let total_context_requested = session_trends
        .iter()
        .map(|trend| trend.context_requested_chars)
        .sum::<u64>();
    let total_context_provided = session_trends
        .iter()
        .map(|trend| trend.context_provided_chars)
        .sum::<u64>();
    let overall_context_coverage_rate = ratio(total_context_provided, total_context_requested);
    let recent_context_coverage_rate = session_trends
        .iter()
        .find(|trend| trend.context_pack_count > 0)
        .map(|trend| trend.context_coverage_rate)
        .unwrap_or_default();
    let previous_context_coverage_rate = session_trends
        .iter()
        .filter(|trend| trend.context_pack_count > 0)
        .map(|trend| trend.context_coverage_rate)
        .nth(1)
        .unwrap_or_default();
    let context_coverage_delta = session_trends
        .iter()
        .filter(|trend| trend.context_pack_count > 0)
        .map(|trend| trend.context_coverage_rate)
        .take(2)
        .collect::<Vec<_>>();
    let context_coverage_delta = context_coverage_delta
        .first()
        .zip(context_coverage_delta.get(1))
        .map(|(recent, previous)| recent - previous);
    let session_count = session_trends.len();
    let recent_sessions = session_trends
        .into_iter()
        .take(session_limit)
        .collect::<Vec<_>>();

    WriterProductMetricsTrend {
        source_event_count: events.len(),
        session_count,
        overall_average_save_to_feedback_ms,
        recent_average_save_to_feedback_ms,
        previous_average_save_to_feedback_ms,
        save_to_feedback_delta_ms,
        overall_context_coverage_rate,
        recent_context_coverage_rate,
        previous_context_coverage_rate,
        context_coverage_delta,
        recent_sessions,
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn is_promise_context_recall(source: &str) -> bool {
    matches!(source, "PromiseSlice" | "PromiseLedger")
}

#[derive(Default)]
struct SessionMetricAccumulator {
    first_event_at: Option<u64>,
    last_event_at: u64,
    event_count: u64,
    proposal_count: u64,
    manual_ask_proposal_count: u64,
    manual_ask_operation_count: u64,
    feedback_count: u64,
    accepted_count: u64,
    rejected_count: u64,
    edited_count: u64,
    snoozed_count: u64,
    explained_count: u64,
    durable_save_count: u64,
    failed_save_count: u64,
    durable_saves_by_proposal: HashMap<String, Vec<u64>>,
    feedback_by_proposal: Vec<(String, u64)>,
    context_pack_count: u64,
    context_requested_chars: u64,
    context_provided_chars: u64,
    context_truncated_source_count: u64,
    context_dropped_source_count: u64,
}

impl SessionMetricAccumulator {
    fn record_event(&mut self, event: &RunEventSummary) {
        self.event_count = self.event_count.saturating_add(1);
        self.first_event_at = Some(
            self.first_event_at
                .map(|existing| existing.min(event.ts_ms))
                .unwrap_or(event.ts_ms),
        );
        self.last_event_at = self.last_event_at.max(event.ts_ms);

        match event.event_type.as_str() {
            "writer.proposal_created" => {
                self.proposal_count = self.proposal_count.saturating_add(1);
                self.record_proposal_created_event(event);
            }
            "writer.feedback_recorded" => {
                self.record_feedback_event(event);
            }
            "writer.operation_lifecycle" => {
                self.record_operation_lifecycle_event(event);
            }
            "writer.context_pack_built" => {
                self.record_context_pack_event(event);
            }
            _ => {}
        }
    }

    fn record_proposal_created_event(&mut self, event: &RunEventSummary) {
        let is_manual_ask = json_string(&event.data, "observationSource")
            .map(|source| normalize_metric_label(&source) == "manual_request")
            .unwrap_or(false);
        if !is_manual_ask {
            return;
        }

        self.manual_ask_proposal_count = self.manual_ask_proposal_count.saturating_add(1);
        if json_number(&event.data, "operationCount").unwrap_or_default() > 0 {
            self.manual_ask_operation_count = self.manual_ask_operation_count.saturating_add(1);
        }
    }

    fn record_feedback_event(&mut self, event: &RunEventSummary) {
        self.feedback_count = self.feedback_count.saturating_add(1);
        let action = json_string(&event.data, "action")
            .as_deref()
            .map(normalize_metric_label)
            .unwrap_or_default();
        match action.as_str() {
            "accepted" => self.accepted_count = self.accepted_count.saturating_add(1),
            "rejected" => self.rejected_count = self.rejected_count.saturating_add(1),
            "edited" => self.edited_count = self.edited_count.saturating_add(1),
            "snoozed" => self.snoozed_count = self.snoozed_count.saturating_add(1),
            "explained" => self.explained_count = self.explained_count.saturating_add(1),
            _ => {}
        }
        if let Some(proposal_id) = json_string(&event.data, "proposalId") {
            self.feedback_by_proposal.push((proposal_id, event.ts_ms));
        }
    }

    fn record_operation_lifecycle_event(&mut self, event: &RunEventSummary) {
        let state = json_string(&event.data, "state")
            .map(|state| normalize_metric_label(&state))
            .unwrap_or_default();
        if state == "durably_saved" {
            self.durable_save_count = self.durable_save_count.saturating_add(1);
            if let Some(proposal_id) = json_string(&event.data, "proposalId") {
                self.durable_saves_by_proposal
                    .entry(proposal_id)
                    .or_default()
                    .push(event.ts_ms);
            }
        } else if state == "rejected"
            && json_string(&event.data, "saveResult").is_some_and(|value| !value.trim().is_empty())
        {
            self.failed_save_count = self.failed_save_count.saturating_add(1);
        }
    }

    fn record_context_pack_event(&mut self, event: &RunEventSummary) {
        self.context_pack_count = self.context_pack_count.saturating_add(1);
        let Some(source_reports) = event
            .data
            .get("sourceReports")
            .and_then(|value| value.as_array())
        else {
            self.context_requested_chars = self
                .context_requested_chars
                .saturating_add(json_number(&event.data, "budgetLimit").unwrap_or_default());
            self.context_provided_chars = self
                .context_provided_chars
                .saturating_add(json_number(&event.data, "totalChars").unwrap_or_default());
            self.context_truncated_source_count =
                self.context_truncated_source_count.saturating_add(
                    json_number(&event.data, "truncatedSourceCount").unwrap_or_default(),
                );
            return;
        };

        for report in source_reports {
            let provided = json_number(report, "provided").unwrap_or_default();
            let requested = json_number(report, "requested")
                .or_else(|| json_number(report, "originalChars"))
                .unwrap_or(provided);
            self.context_requested_chars = self.context_requested_chars.saturating_add(requested);
            self.context_provided_chars = self.context_provided_chars.saturating_add(provided);
            if json_bool(report, "truncated").unwrap_or(false) {
                self.context_truncated_source_count =
                    self.context_truncated_source_count.saturating_add(1);
            }
            if provided == 0 {
                self.context_dropped_source_count =
                    self.context_dropped_source_count.saturating_add(1);
            }
        }
    }

    fn into_trend(self, session_id: String) -> WriterProductMetricSessionTrend {
        let mut save_to_feedback = Vec::new();
        for (proposal_id, feedback_at) in &self.feedback_by_proposal {
            if let Some(saved_events) = self.durable_saves_by_proposal.get(proposal_id) {
                for saved_at in saved_events {
                    if feedback_at >= saved_at {
                        save_to_feedback.push(feedback_at - saved_at);
                    }
                }
            }
        }

        let ignored_count = self
            .rejected_count
            .saturating_add(self.snoozed_count)
            .saturating_add(self.explained_count);
        WriterProductMetricSessionTrend {
            session_id,
            first_event_at: self.first_event_at.unwrap_or_default(),
            last_event_at: self.last_event_at,
            event_count: self.event_count,
            proposal_count: self.proposal_count,
            manual_ask_proposal_count: self.manual_ask_proposal_count,
            manual_ask_operation_count: self.manual_ask_operation_count,
            manual_ask_converted_to_operation_rate: ratio(
                self.manual_ask_operation_count,
                self.manual_ask_proposal_count,
            ),
            feedback_count: self.feedback_count,
            accepted_count: self.accepted_count,
            rejected_count: self.rejected_count,
            edited_count: self.edited_count,
            ignored_count,
            proposal_acceptance_rate: ratio(
                self.accepted_count.saturating_add(self.edited_count),
                self.feedback_count,
            ),
            durable_save_success_rate: ratio(
                self.durable_save_count,
                self.durable_save_count
                    .saturating_add(self.failed_save_count),
            ),
            average_save_to_feedback_ms: average_u64(&save_to_feedback),
            save_feedback_sample_count: save_to_feedback.len() as u64,
            context_pack_count: self.context_pack_count,
            context_requested_chars: self.context_requested_chars,
            context_provided_chars: self.context_provided_chars,
            context_coverage_rate: ratio(self.context_provided_chars, self.context_requested_chars),
            context_truncated_source_count: self.context_truncated_source_count,
            context_dropped_source_count: self.context_dropped_source_count,
        }
    }
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|field| field.as_str())
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(ToOwned::to_owned)
}

fn json_number(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|field| field.as_u64())
}

fn json_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(|field| field.as_bool())
}

fn normalize_metric_label(value: &str) -> String {
    value
        .chars()
        .enumerate()
        .flat_map(|(index, ch)| {
            let mut output = Vec::new();
            if ch.is_ascii_uppercase() && index > 0 {
                output.push('_');
            }
            output.push(ch.to_ascii_lowercase());
            output
        })
        .collect()
}

fn average_u64(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<u64>() / values.len() as u64)
    }
}
