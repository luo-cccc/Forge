//! Manual ask and inline writer operation command.

use agent_harness_core::{provider::openai_compat::OpenAiCompatProvider, AgentLoopEvent};
use tauri::{Emitter, Manager};

use crate::{
    agent_runtime, events, llm_runtime, manual_agent, storage, tool_bridge, writer_agent, AppState,
    AskAgentContext, AskAgentMode, HarnessState, InlineWriterOperationEvent, ManualAgentTurn,
};

pub(crate) fn writer_agent_inline_operation_messages(
    message: &str,
    observation: &writer_agent::observation::WriterObservation,
    pack: &writer_agent::context::WritingContextPack,
) -> Vec<serde_json::Value> {
    let context = crate::render_writer_context_pack(pack);
    let selected = observation.selected_text();
    vec![
        serde_json::json!({
            "role": "system",
            "content": "你是 Forge 的 Cursor 式中文小说写作 Agent。你只为当前光标生成可执行的正文改写或插入文本，不聊天，不解释，不输出 Markdown，不输出 XML action 标签。必须尊重 ContextPack、设定、伏笔和光标后文。输出必须是可直接进入小说正文的中文文本。"
        }),
        serde_json::json!({
            "role": "user",
            "content": format!(
                "作者指令: {}\n章节: {}\n光标文本位置: {}\n选中文本:\n{}\n\n光标前文:\n{}\n\n光标后文:\n{}\n\nContextPack:\n{}\n\n请只输出要应用到正文中的文本:",
                message,
                observation.chapter_title.as_deref().unwrap_or("current chapter"),
                observation.cursor.as_ref().map(|c| c.to).unwrap_or(0),
                selected,
                observation.prefix,
                observation.suffix,
                context
            )
        }),
    ]
}

#[tauri::command]
pub async fn ask_agent(
    app: tauri::AppHandle,
    message: String,
    context: String,
    paragraph: String,
    selected_text: String,
    context_payload: Option<AskAgentContext>,
) -> Result<(), String> {
    if context_payload
        .as_ref()
        .and_then(|payload| payload.mode.as_ref())
        .is_some_and(|mode| *mode == AskAgentMode::InlineOperation)
    {
        return run_inline_writer_operation(
            app,
            message,
            context,
            paragraph,
            selected_text,
            context_payload,
        )
        .await;
    }

    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();

    let state = app.state::<AppState>();
    let truncated_context = agent_harness_core::truncate_context(&context, 2000).to_string();
    let project_id = storage::active_project_id(&app)?;
    let runtime_manual_history = {
        let history = manual_agent::lock_manual_agent_history(&state)?;
        history.recent_for_project(
            &project_id,
            manual_agent::MANUAL_AGENT_HISTORY_MAX_TURNS,
            manual_agent::MANUAL_AGENT_HISTORY_MAX_CHARS,
        )
    };
    let persisted_manual_history = {
        let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        kernel
            .memory
            .list_manual_agent_turns(
                &project_id,
                manual_agent::MANUAL_AGENT_PERSISTED_HISTORY_LOOKBACK,
            )
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(ManualAgentTurn::from)
            .collect::<Vec<_>>()
    };
    let manual_history = manual_agent::merge_manual_agent_history(
        &project_id,
        persisted_manual_history,
        runtime_manual_history,
        manual_agent::MANUAL_AGENT_HISTORY_MAX_TURNS,
        manual_agent::MANUAL_AGENT_HISTORY_MAX_CHARS,
    );
    let manual_observation = crate::build_manual_writer_observation(
        &message,
        &context,
        &paragraph,
        &selected_text,
        context_payload.as_ref(),
        &project_id,
    );
    let (mut prepared_run, emitted_proposals, _has_lore, _has_outline) = {
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        crate::refresh_kernel_canon_from_lorebook(&app, &mut kernel);
        let request = writer_agent::kernel::WriterAgentRunRequest {
            task: writer_agent::kernel::WriterAgentTask::ManualRequest,
            observation: manual_observation.clone(),
            user_instruction: message.clone(),
            frontend_state: writer_agent::kernel::WriterAgentFrontendState {
                truncated_context,
                paragraph: paragraph.clone(),
                selected_text: selected_text.clone(),
                memory_context: crate::build_context_injection(&app, &message),
                has_lore: storage::load_lorebook(&app)
                    .map(|l| !l.is_empty())
                    .unwrap_or(false),
                has_outline: storage::load_outline(&app)
                    .map(|o| !o.is_empty())
                    .unwrap_or(false),
            },
            approval_mode: writer_agent::kernel::WriterAgentApprovalMode::SurfaceProposals,
            stream_mode: writer_agent::kernel::WriterAgentStreamMode::Text,
            manual_history: manual_agent::manual_agent_history_messages(&manual_history),
        };
        let has_lore = request.frontend_state.has_lore;
        let has_outline = request.frontend_state.has_outline;
        let provider = std::sync::Arc::new(OpenAiCompatProvider::new(
            &settings.api_base,
            &settings.api_key,
            &settings.model,
        ));
        let prepared = kernel.prepare_task_run(
            request,
            provider,
            tool_bridge::TauriToolBridge { app: app.clone() },
            &model,
        )?;
        let proposals = prepared.proposals().to_vec();
        (prepared, proposals, has_lore, has_outline)
    };

    for proposal in emitted_proposals {
        app.emit(events::AGENT_PROPOSAL, proposal)
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    let app_handle = app.clone();
    prepared_run.set_event_callback(std::sync::Arc::new(move |event| match event {
        AgentLoopEvent::Intent { intent } => {
            let _ = app_handle.emit(
                events::AGENT_CHAIN_OF_THOUGHT,
                serde_json::json!({
                    "step": 1,
                    "total": 3,
                    "description": format!("Intent: {}", intent),
                    "status": "done",
                }),
            );
        }
        AgentLoopEvent::TextChunk { content } => {
            let _ = app_handle.emit(
                events::AGENT_STREAM_CHUNK,
                serde_json::json!({"content": content}),
            );
        }
        AgentLoopEvent::Error { message } => {
            let _ = app_handle.emit(
                events::AGENT_ERROR,
                serde_json::json!({"message": message, "source": "agent_loop"}),
            );
        }
        AgentLoopEvent::Complete { .. } => {
            let _ = app_handle.emit(
                events::AGENT_STREAM_END,
                serde_json::json!({"reason": "complete"}),
            );
        }
        _ => {
            let _ = app_handle.emit("agent-loop-event", serde_json::json!(event));
        }
    }));

    {
        let db = crate::lock_hermes(&state)?;
        let _ = db.log_interaction("user", &message);
    }

    let run_request = prepared_run.request().clone();
    match prepared_run.run().await {
        Ok(run_result) => {
            let final_text = run_result.answer.clone();
            {
                let db = crate::lock_hermes(&state)?;
                let _ = db.log_interaction("assistant", &final_text);
            }
            {
                let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
                kernel.record_run_completion(&run_request, &run_result)?;
            }
            {
                let mut history = manual_agent::lock_manual_agent_history(&state)?;
                history.append(ManualAgentTurn {
                    project_id: project_id.clone(),
                    created_at: agent_runtime::now_ms(),
                    observation_id: manual_observation.id.clone(),
                    chapter_title: manual_observation.chapter_title.clone(),
                    user: message,
                    assistant: final_text,
                    source_refs: run_result.source_refs,
                });
            }

            let app_clone = app.clone();
            tokio::spawn(async move {
                crate::extract_skills_from_recent(&app_clone).await;
            });

            {
                let mut s = crate::lock_harness_state(&state)?;
                *s = HarnessState::Idle;
            }
            Ok(())
        }
        Err(e) => {
            {
                let mut s = crate::lock_harness_state(&state)?;
                *s = HarnessState::Idle;
            }
            Err(e)
        }
    }
}

fn ask_agent_request_id(context_payload: Option<&AskAgentContext>) -> String {
    context_payload
        .and_then(|payload| payload.request_id.clone())
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("ask-{}", agent_runtime::now_ms()))
}

async fn run_inline_writer_operation(
    app: tauri::AppHandle,
    message: String,
    context: String,
    paragraph: String,
    selected_text: String,
    context_payload: Option<AskAgentContext>,
) -> Result<(), String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();
    let project_id = storage::active_project_id(&app)?;
    let request_id = ask_agent_request_id(context_payload.as_ref());
    let observation = crate::build_manual_writer_observation(
        &message,
        &context,
        &paragraph,
        &selected_text,
        context_payload.as_ref(),
        &project_id,
    );

    let (context_pack, local_proposals) = {
        let state = app.state::<AppState>();
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        crate::refresh_kernel_canon_from_lorebook(&app, &mut kernel);
        let local_proposals = kernel.observe(observation.clone())?;
        let context_pack = kernel.context_pack_for_default(
            writer_agent::context::AgentTask::InlineRewrite,
            &observation,
        );
        (context_pack, local_proposals)
    };

    let messages = writer_agent_inline_operation_messages(&message, &observation, &context_pack);
    let draft = crate::llm_runtime::chat_text(&settings, messages, false, 30).await?;

    let (proposal, operation) = {
        let state = app.state::<AppState>();
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        let proposal =
            kernel.create_inline_operation_proposal(observation, &message, draft, &model)?;
        let operation =
            proposal.operations.first().cloned().ok_or_else(|| {
                "inline operation proposal did not include an operation".to_string()
            })?;
        (proposal, operation)
    };

    for proposal in local_proposals {
        app.emit(events::AGENT_PROPOSAL, proposal)
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    app.emit(events::AGENT_PROPOSAL, proposal.clone())
        .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    app.emit(
        events::INLINE_WRITER_OPERATION,
        InlineWriterOperationEvent {
            request_id,
            proposal,
            operation,
        },
    )
    .map_err(|e| format!("Failed to emit inline writer operation: {}", e))?;

    Ok(())
}
