use super::*;

use agent_harness_core::provider::{
    LlmMessage, LlmRequest, LlmResponse, Provider, StreamEvent, ToolCall, ToolCallFunction,
    UsageInfo,
};
use agent_writer_lib::writer_agent::provider_budget::{
    apply_provider_budget_approval, estimate_provider_cost_micros, evaluate_provider_budget,
    WriterProviderBudgetApproval, WriterProviderBudgetDecision, WriterProviderBudgetRequest,
    WriterProviderBudgetTask,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

include!("provider_budget/part_a.in.rs");
include!("provider_budget/part_b.in.rs");
