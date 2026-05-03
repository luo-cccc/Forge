use serde::Serialize;

use super::kernel::{
    WriterAgentTraceSnapshot, WriterFeedbackTrace, WriterObservationTrace,
    WriterOperationLifecycleTrace, WriterProposalTrace, WriterTaskPacketTrace,
};
use super::memory::ContextRecallSummary;
use super::run_events::WriterRunEvent;

const TRAJECTORY_SCHEMA: &str = "forge-writer-agent-trajectory";
const SCHEMA_VERSION: u8 = 1;
const MAX_EVENT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterTrajectoryExport {
    pub schema: String,
    pub schema_version: u8,
    pub project_id: String,
    pub session_id: String,
    pub redaction_warning: String,
    pub local_only: bool,
    pub event_count: usize,
    pub jsonl: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrajectoryEvent<'a, T: Serialize> {
    trace_schema: &'static str,
    schema_version: u8,
    trace_id: &'a str,
    project_id: &'a str,
    session_id: &'a str,
    seq: usize,
    event_type: &'static str,
    ts_ms: u64,
    data: T,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TruncatedEvent {
    truncated: bool,
    original_bytes: usize,
    limit_bytes: usize,
    reason: &'static str,
}

pub fn export_trace_snapshot(
    project_id: &str,
    session_id: &str,
    snapshot: &WriterAgentTraceSnapshot,
) -> WriterTrajectoryExport {
    let mut lines = Vec::new();
    let mut seq = 0usize;
    let trace_id = format!("{}:{}", project_id, session_id);

    for observation in &snapshot.recent_observations {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.observation",
                ts_ms: observation.created_at,
                data: observation,
            },
        );
    }
    for task_packet in &snapshot.task_packets {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.task_packet",
                ts_ms: task_packet.packet.created_at_ms,
                data: task_packet,
            },
        );
    }
    for proposal in &snapshot.recent_proposals {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.proposal",
                ts_ms: 0,
                data: proposal,
            },
        );
    }
    for feedback in &snapshot.recent_feedback {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.feedback",
                ts_ms: feedback.created_at,
                data: feedback,
            },
        );
    }
    for lifecycle in &snapshot.operation_lifecycle {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.operation_lifecycle",
                ts_ms: lifecycle.created_at,
                data: lifecycle,
            },
        );
    }
    for run_event in &snapshot.run_events {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.run_event",
                ts_ms: run_event.ts_ms,
                data: run_event,
            },
        );
    }
    for report in &snapshot.post_write_diagnostics {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.post_write_diagnostics",
                ts_ms: report.created_at_ms,
                data: report,
            },
        );
    }
    for recall in &snapshot.context_recalls {
        seq += 1;
        push_event(
            &mut lines,
            TrajectoryEvent {
                trace_schema: TRAJECTORY_SCHEMA,
                schema_version: SCHEMA_VERSION,
                trace_id: &trace_id,
                project_id,
                session_id,
                seq,
                event_type: "writer.context_recall",
                ts_ms: recall.last_recalled_at,
                data: recall,
            },
        );
    }
    seq += 1;
    push_event(
        &mut lines,
        TrajectoryEvent {
            trace_schema: TRAJECTORY_SCHEMA,
            schema_version: SCHEMA_VERSION,
            trace_id: &trace_id,
            project_id,
            session_id,
            seq,
            event_type: "writer.product_metrics",
            ts_ms: snapshot
                .recent_feedback
                .iter()
                .map(|feedback| feedback.created_at)
                .max()
                .unwrap_or(0),
            data: &snapshot.product_metrics,
        },
    );

    WriterTrajectoryExport {
        schema: TRAJECTORY_SCHEMA.to_string(),
        schema_version: SCHEMA_VERSION,
        project_id: project_id.to_string(),
        session_id: session_id.to_string(),
        redaction_warning: redaction_warning(),
        local_only: true,
        event_count: lines.len(),
        jsonl: if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        },
    }
}

fn redaction_warning() -> String {
    "Trajectory export may contain manuscript text, project memory, author feedback, prompts, tool results, and internal reasoning metadata. Review or redact before sharing; exports are local-only by default.".to_string()
}

fn push_event<T: Serialize>(lines: &mut Vec<String>, event: TrajectoryEvent<'_, T>) {
    let Ok(line) = serde_json::to_string(&event) else {
        return;
    };
    if line.len() <= MAX_EVENT_BYTES {
        lines.push(line);
        return;
    }

    let truncated = TrajectoryEvent {
        trace_schema: TRAJECTORY_SCHEMA,
        schema_version: SCHEMA_VERSION,
        trace_id: event.trace_id,
        project_id: event.project_id,
        session_id: event.session_id,
        seq: event.seq,
        event_type: event.event_type,
        ts_ms: event.ts_ms,
        data: TruncatedEvent {
            truncated: true,
            original_bytes: line.len(),
            limit_bytes: MAX_EVENT_BYTES,
            reason: "trajectory-event-size-limit",
        },
    };
    if let Ok(line) = serde_json::to_string(&truncated) {
        lines.push(line);
    }
}

#[allow(dead_code)]
fn _assert_trace_types(
    _observation: &WriterObservationTrace,
    _task_packet: &WriterTaskPacketTrace,
    _proposal: &WriterProposalTrace,
    _feedback: &WriterFeedbackTrace,
    _lifecycle: &WriterOperationLifecycleTrace,
    _run_event: &WriterRunEvent,
    _recall: &ContextRecallSummary,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_snapshot_as_jsonl_events() {
        let snapshot = WriterAgentTraceSnapshot {
            recent_observations: vec![WriterObservationTrace {
                id: "obs_1".to_string(),
                created_at: 10,
                reason: "Idle".to_string(),
                chapter_title: Some("Chapter-1".to_string()),
                paragraph_snippet: "林墨停下。".to_string(),
            }],
            task_packets: Vec::new(),
            recent_proposals: Vec::new(),
            recent_feedback: vec![WriterFeedbackTrace {
                proposal_id: "prop_1".to_string(),
                action: "Accepted".to_string(),
                reason: Some("fits".to_string()),
                created_at: 20,
            }],
            operation_lifecycle: Vec::new(),
            run_events: Vec::new(),
            post_write_diagnostics: Vec::new(),
            context_source_trends: Vec::new(),
            context_recalls: Vec::new(),
            product_metrics: Default::default(),
        };

        let export = export_trace_snapshot("novel-a", "session-a", &snapshot);
        let lines = export.jsonl.lines().collect::<Vec<_>>();

        assert_eq!(export.schema, TRAJECTORY_SCHEMA);
        assert_eq!(export.event_count, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("\"eventType\":\"writer.observation\""));
        assert!(lines[1].contains("\"eventType\":\"writer.feedback\""));
        assert!(lines[2].contains("\"eventType\":\"writer.product_metrics\""));
    }
}
