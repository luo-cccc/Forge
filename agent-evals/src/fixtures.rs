//! Evaluation harness for the real Writer Agent Kernel.
//! These are product-behavior checks, not mirror implementations.

use agent_writer_lib::writer_agent::observation::{
    ObservationReason, ObservationSource, TextRange, WriterObservation,
};
use agent_writer_lib::writer_agent::operation::OperationApproval;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct EvalResult {
    pub fixture: String,
    pub passed: bool,
    pub actual: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<EvalResult>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn observation(paragraph: &str) -> WriterObservation {
    observation_in_chapter(paragraph, "Chapter-1")
}

pub fn observation_in_chapter(paragraph: &str, chapter_title: &str) -> WriterObservation {
    let cursor = paragraph.chars().count();
    WriterObservation {
        id: format!("eval-{}", now_ms()),
        created_at: now_ms(),
        source: ObservationSource::Editor,
        reason: ObservationReason::Idle,
        project_id: "eval".to_string(),
        chapter_title: Some(chapter_title.to_string()),
        chapter_revision: Some("rev-1".to_string()),
        cursor: Some(TextRange {
            from: cursor,
            to: cursor,
        }),
        selection: None,
        prefix: paragraph.to_string(),
        suffix: String::new(),
        paragraph: paragraph.to_string(),
        full_text_digest: None,
        editor_dirty: true,
    }
}

pub fn eval_result(fixture: &str, actual: String, errors: Vec<String>) -> EvalResult {
    EvalResult {
        fixture: fixture.to_string(),
        passed: errors.is_empty(),
        actual,
        errors,
    }
}

pub fn eval_approval(source: &str) -> OperationApproval {
    OperationApproval {
        source: source.to_string(),
        actor: "eval_author".to_string(),
        reason: "eval simulates an author accepting a surfaced operation".to_string(),
        proposal_id: Some(format!("eval-proposal-{}", now_ms())),
        surfaced_to_user: true,
        created_at: now_ms(),
    }
}
