//! Product metric derivation from WriterAgent trace state.

use super::feedback::{FeedbackAction, ProposalFeedback};
use super::kernel::{WriterOperationLifecycleState, WriterOperationLifecycleTrace};
use super::kernel_helpers::{operation_affected_scope, operation_kind_label};
use super::memory::{ChapterMissionSummary, ContextRecallSummary};
use super::proposal::{AgentProposal, ProposalKind};

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

pub(crate) fn product_metrics_from_trace(
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

    let manual_proposals = proposals
        .iter()
        .filter(|proposal| {
            proposal
                .evidence
                .iter()
                .any(|evidence| evidence.reference.contains("manual_request"))
        })
        .count() as u64;
    let manual_operations = proposals
        .iter()
        .filter(|proposal| {
            proposal
                .evidence
                .iter()
                .any(|evidence| evidence.reference.contains("manual_request"))
                && !proposal.operations.is_empty()
        })
        .count() as u64;
    let manual_ask_converted_to_operation_rate = ratio(manual_operations, manual_proposals);

    let recalls = context_recalls.unwrap_or_default();
    let promise_recalls = recalls
        .iter()
        .filter(|recall| recall.source == "PromiseSlice")
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

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}
