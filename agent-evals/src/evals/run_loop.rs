use super::*;

pub fn run_manual_request_kernel_owns_run_loop_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "玉佩线推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation("林墨停在旧门前，想起张三带走的玉佩。");
    obs.source = ObservationSource::ManualRequest;
    obs.reason = ObservationReason::Explicit;
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "这段接下来应该怎么推进？".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            paragraph: "林墨停在旧门前，想起张三带走的玉佩。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o-mini",
        ),
    );
    let prepared = kernel.prepare_task_run(request, provider, EvalToolHandler, "gpt-4o-mini");
    let trace = kernel.trace_snapshot(10);
    let packet = trace
        .task_packets
        .iter()
        .find(|packet| packet.task == "ManualRequest");

    let mut errors = Vec::new();
    if let Err(error) = &prepared {
        errors.push(format!("kernel failed to prepare manual run: {}", error));
    }
    if packet.is_none() {
        errors.push("manual request did not create kernel task packet before run loop".to_string());
    }
    if !packet.is_some_and(|packet| {
        packet.packet.intent == Some(agent_harness_core::Intent::Chat)
            && packet.max_side_effect_level == "ProviderCall"
            && !packet.packet.tool_policy.allow_approval_required
            && packet
                .packet
                .feedback
                .memory_writes
                .iter()
                .any(|write| write == "manual_agent_turn")
    }) {
        errors.push(
            "manual request packet does not own chat intent/tool/feedback policy".to_string(),
        );
    }
    if let Ok(prepared) = &prepared {
        let names = prepared
            .proposals()
            .iter()
            .map(|proposal| proposal.id.clone())
            .collect::<Vec<_>>();
        if !names.is_empty() && trace.recent_proposals.is_empty() {
            errors.push("prepared run proposals were not registered in kernel trace".to_string());
        }
    }

    eval_result(
        "writer_agent:manual_request_kernel_owns_run_loop",
        format!(
            "prepared={} taskPackets={}",
            prepared.is_ok(),
            trace.task_packets.len()
        ),
        errors,
    )
}
