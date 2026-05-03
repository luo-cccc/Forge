use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentKernelStatus {
    pub(crate) tool_generation: u64,
    pub(crate) tool_count: usize,
    pub(crate) effective_tool_count: usize,
    pub(crate) blocked_tool_count: usize,
    pub(crate) model_callable_tool_count: usize,
    pub(crate) approval_required_tool_count: usize,
    pub(crate) write_tool_count: usize,
    pub(crate) domain_id: String,
    pub(crate) capability_count: usize,
    pub(crate) quality_gate_count: usize,
    pub(crate) trace_enabled: bool,
}
