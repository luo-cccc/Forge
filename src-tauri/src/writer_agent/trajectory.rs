use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Value};

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
    pub trace_viewer_schema: String,
    pub trace_viewer_event_count: usize,
    pub trace_viewer_jsonl: String,
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

    let redaction_warning = redaction_warning();
    let trace_viewer_lines = trace_viewer_jsonl_lines(
        project_id,
        session_id,
        &trace_id,
        &lines,
        &redaction_warning,
    );
    WriterTrajectoryExport {
        schema: TRAJECTORY_SCHEMA.to_string(),
        schema_version: SCHEMA_VERSION,
        project_id: project_id.to_string(),
        session_id: session_id.to_string(),
        redaction_warning,
        local_only: true,
        event_count: lines.len(),
        jsonl: if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        },
        trace_viewer_schema: "claude-code-jsonl-for-hf-agent-trace-viewer".to_string(),
        trace_viewer_event_count: trace_viewer_lines.len(),
        trace_viewer_jsonl: if trace_viewer_lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", trace_viewer_lines.join("\n"))
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

fn trace_viewer_jsonl_lines(
    project_id: &str,
    session_id: &str,
    trace_id: &str,
    forge_lines: &[String],
    redaction_warning: &str,
) -> Vec<String> {
    let mut lines = Vec::new();
    let metadata_uuid = stable_uuid(session_id, "metadata", 0);
    push_trace_viewer_event(
        &mut lines,
        json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "model": "forge-writer-agent",
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Forge writer trajectory export for project {}. {}",
                        project_id,
                        redaction_warning
                    ),
                }],
            },
            "uuid": metadata_uuid,
            "parentUuid": Value::Null,
            "sessionId": session_id,
            "timestamp": iso_timestamp(0),
            "forgeEventType": "forge.export_metadata",
            "forgeTraceId": trace_id,
            "projectId": project_id,
            "localOnly": true,
            "redactionWarning": redaction_warning,
        }),
    );
    let mut parent_uuid = Some(metadata_uuid);

    for (index, line) in forge_lines.iter().enumerate() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event_type = string_field(&event, &["eventType"]).unwrap_or("writer.unknown");
        let seq = event
            .get("seq")
            .and_then(Value::as_u64)
            .unwrap_or(index as u64 + 1);
        let ts_ms = event.get("tsMs").and_then(Value::as_u64).unwrap_or(0);
        let source_refs = event
            .get("sourceRefs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let data = event.get("data").cloned().unwrap_or(Value::Null);
        let message_type = trace_viewer_message_type(event_type);
        let role = if message_type == "user" {
            "user"
        } else {
            "assistant"
        };
        let uuid = stable_uuid(session_id, event_type, seq);
        let parent = parent_uuid
            .as_ref()
            .map(|uuid| Value::String(uuid.clone()))
            .unwrap_or(Value::Null);
        let content_text = trace_viewer_summary(event_type, &data);
        let message = if role == "user" {
            json!({
                "role": "user",
                "content": content_text,
            })
        } else {
            json!({
                "role": "assistant",
                "model": "forge-writer-agent",
                "content": [{
                    "type": "text",
                    "text": content_text,
                }],
            })
        };

        push_trace_viewer_event(
            &mut lines,
            json!({
                "type": message_type,
                "message": message,
                "uuid": uuid,
                "parentUuid": parent,
                "sessionId": session_id,
                "timestamp": iso_timestamp(ts_ms),
                "forgeEventType": event_type,
                "forgeSeq": seq,
                "forgeTraceId": trace_id,
                "projectId": project_id,
                "sourceRefs": source_refs,
                "localOnly": true,
                "forgeEvent": event,
            }),
        );
        parent_uuid = Some(uuid);
    }

    lines
}

fn push_trace_viewer_event(lines: &mut Vec<String>, event: Value) {
    if let Ok(line) = serde_json::to_string(&event) {
        lines.push(line);
    }
}

fn trace_viewer_message_type(event_type: &str) -> &'static str {
    match event_type {
        "writer.observation" | "writer.feedback" | "writer.context_recall" => "user",
        "writer.run_event" => "assistant",
        _ => "assistant",
    }
}

fn trace_viewer_summary(event_type: &str, data: &Value) -> String {
    match event_type {
        "writer.observation" => format!(
            "Observation: {}{}{}",
            string_field(data, &["reason"]).unwrap_or("unknown reason"),
            optional_labeled(
                " chapter ",
                string_field(data, &["chapterTitle", "chapter_title"])
            ),
            optional_labeled(
                " - ",
                string_field(data, &["paragraphSnippet", "paragraph_snippet"])
            ),
        ),
        "writer.task_packet" => format!(
            "Task packet: {} | objective: {} | scope: {} | required context: {}",
            string_field(data, &["task"]).unwrap_or("unknown task"),
            string_field(data, &["objective"]).unwrap_or("unknown objective"),
            string_field(data, &["scope"]).unwrap_or("unknown scope"),
            number_field(data, &["requiredContextCount", "required_context_count"]).unwrap_or(0),
        ),
        "writer.proposal" => format!(
            "Proposal: {} priority={} confidence={}{}",
            string_field(data, &["kind"]).unwrap_or("unknown"),
            string_field(data, &["priority"]).unwrap_or("unknown"),
            number_or_float_field(data, &["confidence"]).unwrap_or_else(|| "unknown".to_string()),
            optional_labeled(
                " - ",
                string_field(data, &["previewSnippet", "preview_snippet"])
            ),
        ),
        "writer.feedback" => format!(
            "Feedback: proposal={} action={}{}",
            string_field(data, &["proposalId", "proposal_id"]).unwrap_or("unknown"),
            string_field(data, &["action"]).unwrap_or("unknown"),
            optional_labeled(" reason=", string_field(data, &["reason"])),
        ),
        "writer.operation_lifecycle" => format!(
            "Operation lifecycle: {} {}{}",
            string_field(data, &["operationKind", "operation_kind"]).unwrap_or("operation"),
            string_field(data, &["state"]).unwrap_or("unknown"),
            optional_labeled(
                " proposal=",
                string_field(data, &["proposalId", "proposal_id"])
            ),
        ),
        "writer.run_event" => summarize_run_event(data),
        "writer.post_write_diagnostics" => format!(
            "Post-write diagnostics: total={} errors={} warnings={}{}",
            number_field(data, &["totalCount", "total_count"]).unwrap_or(0),
            number_field(data, &["errorCount", "error_count"]).unwrap_or(0),
            number_field(data, &["warningCount", "warning_count"]).unwrap_or(0),
            optional_labeled(
                " chapter=",
                string_field(data, &["chapterTitle", "chapter_title"])
            ),
        ),
        "writer.context_recall" => format!(
            "Context recall: {}:{}{}",
            string_field(data, &["source"]).unwrap_or("source"),
            string_field(data, &["reference"]).unwrap_or("reference"),
            optional_labeled(" - ", string_field(data, &["snippet"])),
        ),
        "writer.product_metrics" => format!(
            "Product metrics: proposals={} feedback={} acceptance={} durable_save={}",
            number_field(data, &["proposalCount", "proposal_count"]).unwrap_or(0),
            number_field(data, &["feedbackCount", "feedback_count"]).unwrap_or(0),
            number_or_float_field(
                data,
                &["proposalAcceptanceRate", "proposal_acceptance_rate"]
            )
            .unwrap_or_else(|| "unknown".to_string()),
            number_or_float_field(
                data,
                &["durableSaveSuccessRate", "durable_save_success_rate"]
            )
            .unwrap_or_else(|| "unknown".to_string()),
        ),
        _ => format!("{}: {}", event_type, compact_json_snippet(data, 600)),
    }
}

fn summarize_run_event(data: &Value) -> String {
    let run_event_type = string_field(data, &["eventType", "event_type"]).unwrap_or("writer.event");
    let payload = data.get("data").unwrap_or(&Value::Null);
    let task_id = string_field(data, &["taskId", "task_id"]);
    let headline = if let Some(message) = string_field(payload, &["message"]) {
        message.to_string()
    } else if let Some(code) = string_field(payload, &["code"]) {
        code.to_string()
    } else if let Some(decision) = string_field(payload, &["decision"]) {
        format!("decision={}", decision)
    } else if let Some(summary) = string_field(payload, &["summary"]) {
        summary.to_string()
    } else {
        compact_json_snippet(payload, 500)
    };
    format!(
        "Run event: {}{} - {}",
        run_event_type,
        optional_labeled(" task=", task_id),
        headline
    )
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
}

fn number_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn number_or_float_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        value
            .as_u64()
            .map(|number| number.to_string())
            .or_else(|| value.as_i64().map(|number| number.to_string()))
            .or_else(|| value.as_f64().map(|number| format!("{:.2}", number)))
    })
}

fn optional_labeled(label: &str, value: Option<&str>) -> String {
    value
        .map(|value| format!("{}{}", label, snippet(value, 220)))
        .unwrap_or_default()
}

fn compact_json_snippet(value: &Value, max_chars: usize) -> String {
    serde_json::to_string(value)
        .map(|text| snippet(&text, max_chars))
        .unwrap_or_default()
}

fn snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn iso_timestamp(ts_ms: u64) -> String {
    let secs = (ts_ms / 1_000) as i64;
    let nanos = ((ts_ms % 1_000) * 1_000_000) as u32;
    let datetime = DateTime::<Utc>::from_timestamp(secs, nanos).unwrap_or_else(|| {
        DateTime::<Utc>::from_timestamp(0, 0).expect("unix epoch timestamp is valid")
    });
    datetime.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn stable_uuid(session_id: &str, kind: &str, seq: u64) -> String {
    let input = format!("{}::{}::{}", session_id, kind, seq);
    let first = stable_hash64(input.as_bytes(), 0xcbf29ce484222325);
    let second = stable_hash64(input.as_bytes(), 0x84222325cbf29ce4);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (first >> 32) as u32,
        ((first >> 16) & 0xffff) as u16,
        (first & 0xffff) as u16,
        ((second >> 48) & 0xffff) as u16,
        second & 0x0000_ffff_ffff_ffff
    )
}

fn stable_hash64(bytes: &[u8], seed: u64) -> u64 {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
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
