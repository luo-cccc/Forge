//! Writer agent Tauri commands — status, ledger, proposals, feedback.

use crate::agent_runtime;
use crate::AppState;

#[tauri::command]
pub fn get_agent_tools() -> Result<Vec<crate::agent_runtime::AgentToolDescriptor>, String> {
    Ok(crate::agent_runtime::registered_tools())
}

#[tauri::command]
pub fn get_effective_agent_tool_inventory(
) -> Result<agent_harness_core::EffectiveToolInventory, String> {
    Ok(crate::agent_runtime::effective_tool_inventory())
}

#[tauri::command]
pub fn get_agent_kernel_status() -> Result<crate::AgentKernelStatus, String> {
    let registry = agent_harness_core::default_writing_tool_registry();
    let tools = registry.list();
    let inventory = crate::agent_runtime::effective_tool_inventory();
    let domain = agent_harness_core::writing_domain_profile();

    Ok(crate::AgentKernelStatus {
        tool_generation: registry.generation(),
        tool_count: tools.len(),
        effective_tool_count: inventory.allowed.len(),
        blocked_tool_count: inventory.blocked.len(),
        model_callable_tool_count: inventory.openai_callable_allowed_count(),
        approval_required_tool_count: tools.iter().filter(|tool| tool.requires_approval).count(),
        write_tool_count: tools
            .iter()
            .filter(|tool| tool.side_effect_level == agent_harness_core::ToolSideEffectLevel::Write)
            .count(),
        domain_id: domain.id,
        capability_count: domain.capabilities.len(),
        quality_gate_count: domain.quality_gates.len(),
        trace_enabled: true,
    })
}

#[tauri::command]
pub fn get_agent_domain_profile() -> Result<agent_harness_core::AgentDomainProfile, String> {
    Ok(agent_harness_core::writing_domain_profile())
}

#[tauri::command]
pub fn get_writer_agent_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::writer_agent::WriterAgentStatus, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.status())
}

#[tauri::command]
pub fn get_writer_agent_ledger(
    state: tauri::State<'_, AppState>,
) -> Result<crate::writer_agent::kernel::WriterAgentLedgerSnapshot, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.ledger_snapshot())
}

#[tauri::command]
pub fn get_writer_agent_pending_proposals(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::writer_agent::proposal::AgentProposal>, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.pending_proposals())
}

#[tauri::command]
pub fn get_story_review_queue(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::writer_agent::kernel::StoryReviewQueueEntry>, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.story_review_queue())
}

#[tauri::command]
pub fn get_story_debt_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<crate::writer_agent::kernel::StoryDebtSnapshot, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.story_debt_snapshot())
}

#[tauri::command]
pub fn get_writer_agent_trace(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> Result<crate::writer_agent::kernel::WriterAgentTraceSnapshot, String> {
    let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    Ok(kernel.trace_snapshot(limit.unwrap_or(20).min(100)))
}

#[tauri::command]
pub fn apply_proposal_feedback(
    state: tauri::State<'_, AppState>,
    feedback: crate::writer_agent::ProposalFeedback,
) -> Result<crate::writer_agent::WriterAgentStatus, String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.apply_feedback(feedback)?;
    Ok(kernel.status())
}

#[tauri::command]
pub fn record_implicit_ghost_rejection(
    state: tauri::State<'_, AppState>,
    proposal_id: String,
    created_at: u64,
) -> Result<bool, String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.record_implicit_ghost_rejection(&proposal_id, created_at)
}

#[tauri::command]
pub fn record_writer_operation_durable_save(
    state: tauri::State<'_, AppState>,
    proposal_id: Option<String>,
    operation: crate::writer_agent::operation::WriterOperation,
    save_result: String,
) -> Result<(), String> {
    let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
    kernel.record_operation_durable_save(proposal_id, operation, save_result)
}
