//! Writer run preflight types — readiness report without executing provider or tools.
//!
//! The preflight checks are implemented as WriterAgentKernel::preflight()
//! in kernel/run_loop.rs, mirroring the first half of prepare_task_run.
//! This module defines the report shape.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WriterRunPreflightReport {
    pub task: String,
    pub observation_id: String,
    pub readiness: String, // "ready" | "warning" | "blocked"
    pub blocks: Vec<PreflightItem>,
    pub warnings: Vec<PreflightItem>,
    pub context_source_count: usize,
    pub context_total_chars: usize,
    pub context_budget_limit: usize,
    pub story_impact_truncated: bool,
    pub story_impact_risk: String,
    pub story_contract_quality: String,
    pub tool_allowed_count: usize,
    pub tool_blocked_count: usize,
    pub estimated_input_tokens: u64,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreflightItem {
    pub code: String,
    pub reason: String,
}
