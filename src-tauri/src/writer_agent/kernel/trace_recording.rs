use super::*;

pub struct SaveCompletedEventContext {
    pub observation_id: String,
    pub chapter_title: Option<String>,
    pub chapter_revision: Option<String>,
    pub save_result: String,
}

pub struct ModelStartedEventContext {
    pub task_id: String,
    pub task: crate::writer_agent::provider_budget::WriterProviderBudgetTask,
    pub model: String,
    pub provider: String,
    pub stream: bool,
}

include!("trace_recording/task_packet.in.rs");
include!("trace_recording/event_recording.in.rs");
include!("trace_recording/helpers.in.rs");
