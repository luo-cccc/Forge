#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::LengthPhaseTelemetry;

pub fn run_length_telemetry_eval() -> EvalResult {
    let mut errors = Vec::new();

    let default_telemetry = LengthPhaseTelemetry::default();
    if default_telemetry.continuation_count != 0 {
        errors.push("default continuation_count should be 0".to_string());
    }
    if default_telemetry.compress_count != 0 {
        errors.push("default compress_count should be 0".to_string());
    }
    if default_telemetry.hard_compress_count != 0 {
        errors.push("default hard_compress_count should be 0".to_string());
    }
    if default_telemetry.continuation_latency_ms != 0 {
        errors.push("default continuation_latency_ms should be 0".to_string());
    }

    // Verify serialization
    match serde_json::to_string(&default_telemetry) {
        Ok(json) => {
            if json.is_empty() {
                errors.push("serialized json should not be empty".to_string());
            }
        }
        Err(e) => errors.push(format!("serialization failed: {}", e)),
    }

    // Verify deserialization
    let json = r#"{"continuationCount":1,"compressCount":0,"hardCompressCount":0,"continuationLatencyMs":150,"compressLatencyMs":0,"hardCompressLatencyMs":0}"#;
    match serde_json::from_str::<LengthPhaseTelemetry>(json) {
        Ok(parsed) => {
            if parsed.continuation_count != 1 {
                errors.push("deserialized continuation_count should be 1".to_string());
            }
            if parsed.continuation_latency_ms != 150 {
                errors.push("deserialized continuation_latency_ms should be 150".to_string());
            }
        }
        Err(e) => errors.push(format!("deserialization failed: {}", e)),
    }

    eval_result(
        "writer_agent:length_telemetry",
        format!(
            "defaultsValid={} canSerialize={}",
            errors.is_empty(),
            errors.is_empty()
        ),
        errors,
    )
}
