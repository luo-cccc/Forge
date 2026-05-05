//! Generation, analysis, and creative drafting Tauri commands.

use crate::chapter_generation::{
    ChapterGenerationEvent, FrontendChapterStateSnapshot, GenerateChapterAutonomousPayload,
    PipelineTerminal, SaveMode,
};
use crate::llm_runtime;
use crate::writer_agent::kernel::ModelStartedEventContext;
use crate::writer_agent::provider_budget::WriterProviderBudgetApproval;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReviewItem {
    quote: String,
    #[serde(rename = "type")]
    review_type: String,
    issue: String,
    suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReviewReport {
    reviews: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParallelDraft {
    id: String,
    label: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ParallelDraftPayload {
    prefix: String,
    suffix: String,
    paragraph: String,
    selected_text: String,
    chapter_title: Option<String>,
    #[serde(default)]
    mission_context: String,
    #[serde(default)]
    promise_context: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskProjectBrainPayload {
    query: String,
    provider_budget_approval: Option<WriterProviderBudgetApproval>,
}

#[derive(Serialize, Clone)]
struct BatchStatus {
    chapter_title: String,
    status: String,
    error: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterGenerationStart {
    request_id: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn trim_parallel_draft(text: &str) -> String {
    text.trim_matches(|c: char| c == '`' || c.is_whitespace())
        .chars()
        .take(1200)
        .collect::<String>()
}

fn parse_parallel_drafts(raw: &str) -> Vec<ParallelDraft> {
    let labels = ["A 顺势推进", "B 冲突加压", "C 情绪转折"];
    let ids = ["a", "b", "c"];
    let mut drafts = Vec::new();
    let mut current_idx: Option<usize> = None;
    let mut current_text = String::new();

    let flush = |drafts: &mut Vec<ParallelDraft>,
                 current_idx: &mut Option<usize>,
                 current_text: &mut String| {
        let Some(idx) = current_idx.take() else {
            current_text.clear();
            return;
        };
        let text = trim_parallel_draft(current_text);
        current_text.clear();
        if text.is_empty() {
            return;
        }
        drafts.push(ParallelDraft {
            id: ids[idx].to_string(),
            label: labels[idx].to_string(),
            text,
        });
    };

    for line in raw.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .split_once(':')
            .or_else(|| trimmed.split_once('：'))
            .and_then(|(head, body)| {
                let idx = match head.trim().chars().next().map(|c| c.to_ascii_uppercase()) {
                    Some('A') => 0,
                    Some('B') => 1,
                    Some('C') => 2,
                    _ => return None,
                };
                Some((idx, body.trim_start()))
            });

        if let Some((idx, body)) = marker {
            flush(&mut drafts, &mut current_idx, &mut current_text);
            current_idx = Some(idx);
            current_text.push_str(body);
        } else if current_idx.is_some() {
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        }
    }
    flush(&mut drafts, &mut current_idx, &mut current_text);
    drafts.truncate(3);
    drafts
}

fn record_writer_failure_bundle(
    app: &tauri::AppHandle,
    bundle: &crate::writer_agent::task_receipt::WriterFailureEvidenceBundle,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    kernel.record_failure_evidence_bundle(bundle);
}

fn record_chapter_provider_budget_report(
    app: &tauri::AppHandle,
    context: &crate::chapter_generation::BuiltChapterContext,
    report: &crate::writer_agent::provider_budget::WriterProviderBudgetReport,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    let mut source_refs = vec![
        format!("receipt:{}", context.receipt.task_id),
        format!("chapter:{}", context.target.title),
    ];
    source_refs.extend(
        context
            .sources
            .iter()
            .filter(|source| source.included_chars > 0)
            .map(|source| format!("{}:{}", source.source_type, source.id)),
    );
    kernel.record_provider_budget_report(
        context.request_id.clone(),
        report,
        source_refs,
        crate::agent_runtime::now_ms(),
    );
}

fn chapter_model_source_refs(
    context: &crate::chapter_generation::BuiltChapterContext,
    report: &crate::writer_agent::provider_budget::WriterProviderBudgetReport,
) -> Vec<String> {
    let mut source_refs = vec![
        format!("receipt:{}", context.receipt.task_id),
        format!("chapter:{}", context.target.title),
        format!("model:{}", report.model),
        format!("estimated_tokens:{}", report.estimated_total_tokens),
        format!("estimated_cost_micros:{}", report.estimated_cost_micros),
    ];
    source_refs.extend(
        context
            .sources
            .iter()
            .filter(|source| source.included_chars > 0)
            .map(|source| format!("{}:{}", source.source_type, source.id)),
    );
    source_refs
}

fn record_chapter_model_started(
    app: &tauri::AppHandle,
    context: &crate::chapter_generation::BuiltChapterContext,
    report: &crate::writer_agent::provider_budget::WriterProviderBudgetReport,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    kernel.record_model_started_run_event(
        ModelStartedEventContext {
            task_id: context.request_id.clone(),
            task: report.task,
            model: report.model.clone(),
            provider: "openai-compatible".to_string(),
            stream: false,
        },
        chapter_model_source_refs(context, report),
        Some(report),
        crate::agent_runtime::now_ms(),
    );
}

fn record_chapter_context_pack_built(
    app: &tauri::AppHandle,
    context: &crate::chapter_generation::BuiltChapterContext,
    created_at_ms: u64,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    kernel.record_chapter_context_pack_built_run_event(context, created_at_ms);
}

// ── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn batch_generate_chapter(
    app: tauri::AppHandle,
    chapter_title: String,
    summary: String,
    frontend_state: Option<FrontendChapterStateSnapshot>,
) -> Result<(), String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let user_profile_entries = crate::collect_user_profile_entries(&app).unwrap_or_default();
    let request_id = crate::chapter_generation::make_request_id("batch");

    let app_clone = app.clone();
    let title_clone = chapter_title.clone();

    tokio::spawn(async move {
        let _ = app_clone.emit(
            crate::events::BATCH_STATUS,
            BatchStatus {
                chapter_title: title_clone.clone(),
                status: "generating".to_string(),
                error: String::new(),
            },
        );

        let trace_request_id = request_id.clone();
        let payload = GenerateChapterAutonomousPayload {
            request_id: Some(request_id.clone()),
            target_chapter_title: Some(title_clone.clone()),
            target_chapter_number: None,
            user_instruction: format!("帮我写《{}》这一章的完整初稿。", title_clone),
            budget: None,
            frontend_state,
            save_mode: SaveMode::ReplaceIfClean,
            chapter_summary_override: Some(summary),
            provider_budget_approval: None,
        };
        let user_instruction = payload.user_instruction.clone();
        let trace_app = app_clone.clone();
        let budget_app = app_clone.clone();
        let model_app = app_clone.clone();

        let terminal = crate::chapter_generation::run_chapter_generation_pipeline(
            crate::chapter_generation::ChapterGenerationConfig {
                app: app_clone.clone(),
                settings,
                payload,
                user_profile_entries,
            },
            |event| {
                let _ = app_clone.emit(crate::events::CHAPTER_GENERATION, event);
            },
            move |context| {
                let created_at_ms = crate::agent_runtime::now_ms();
                record_chapter_context_pack_built(&trace_app, context, created_at_ms);
                let state = trace_app.state::<crate::AppState>();
                let Ok(mut kernel) = state.writer_kernel.lock() else {
                    return;
                };
                let packet = crate::chapter_generation::build_chapter_generation_task_packet(
                    &kernel.project_id,
                    &kernel.session_id,
                    context,
                    &user_instruction,
                    created_at_ms,
                );
                if let Err(error) = kernel.record_task_packet(
                    context.request_id.clone(),
                    "ChapterGeneration",
                    packet,
                ) {
                    tracing::warn!(
                        "WriterAgent chapter-generation task packet rejected: {}",
                        error
                    );
                }
            },
            move |context, report| {
                record_chapter_provider_budget_report(&budget_app, context, report);
            },
            move |context, report| {
                record_chapter_model_started(&model_app, context, report);
            },
        )
        .await;

        match terminal {
            PipelineTerminal::Completed {
                saved,
                generated_content,
            } => {
                crate::observe_generated_chapter_result(&app_clone, &saved, &generated_content);
                let embed_app = app_clone.clone();
                let embed_title = title_clone.clone();
                tokio::spawn(async move {
                    crate::auto_embed_chapter(&embed_app, &embed_title, &generated_content).await;
                });
                let _ = app_clone.emit(
                    crate::events::BATCH_STATUS,
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "complete".to_string(),
                        error: String::new(),
                    },
                );
            }
            PipelineTerminal::Conflict(conflict) => {
                let bundle = crate::writer_agent::task_receipt::WriterFailureEvidenceBundle::new(
                    crate::writer_agent::task_receipt::WriterFailureCategory::SaveFailed,
                    "SAVE_CONFLICT",
                    format!("Save blocked by {}.", conflict.reason),
                    true,
                    Some(trace_request_id.clone()),
                    vec![
                        format!("base_revision:{}", conflict.base_revision),
                        format!("current_revision:{}", conflict.current_revision),
                        format!("save_conflict:{}", conflict.reason),
                    ],
                    serde_json::json!({ "conflict": conflict }),
                    vec![
                        "Resolve editor/storage revision mismatch or save as a draft copy."
                            .to_string(),
                    ],
                    crate::agent_runtime::now_ms(),
                );
                record_writer_failure_bundle(&app_clone, &bundle);
                let _ = app_clone.emit(
                    crate::events::BATCH_STATUS,
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "error".to_string(),
                        error: format!("save conflict: {}", conflict.reason),
                    },
                );
            }
            PipelineTerminal::Failed(error) => {
                let bundle = crate::chapter_generation::failure_bundle_from_chapter_error(
                    &trace_request_id,
                    &error,
                    crate::agent_runtime::now_ms(),
                );
                record_writer_failure_bundle(&app_clone, &bundle);
                let _ = app_clone.emit(
                    crate::events::BATCH_STATUS,
                    BatchStatus {
                        chapter_title: title_clone,
                        status: "error".to_string(),
                        error: error.message,
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn generate_chapter_autonomous(
    app: tauri::AppHandle,
    payload: GenerateChapterAutonomousPayload,
) -> Result<ChapterGenerationStart, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let user_profile_entries = crate::collect_user_profile_entries(&app).unwrap_or_default();
    let request_id = payload
        .request_id
        .clone()
        .unwrap_or_else(|| crate::chapter_generation::make_request_id("chapter"));
    let payload = GenerateChapterAutonomousPayload {
        request_id: Some(request_id.clone()),
        ..payload
    };
    let app_clone = app.clone();
    let trace_request_id = request_id.clone();

    tokio::spawn(async move {
        let user_instruction = payload.user_instruction.clone();
        let trace_app = app_clone.clone();
        let budget_app = app_clone.clone();
        let model_app = app_clone.clone();
        let terminal = crate::chapter_generation::run_chapter_generation_pipeline(
            crate::chapter_generation::ChapterGenerationConfig {
                app: app_clone.clone(),
                settings,
                payload,
                user_profile_entries,
            },
            |event: ChapterGenerationEvent| {
                let _ = app_clone.emit(crate::events::CHAPTER_GENERATION, event);
            },
            move |context| {
                let created_at_ms = crate::agent_runtime::now_ms();
                record_chapter_context_pack_built(&trace_app, context, created_at_ms);
                let state = trace_app.state::<crate::AppState>();
                let Ok(mut kernel) = state.writer_kernel.lock() else {
                    return;
                };
                let packet = crate::chapter_generation::build_chapter_generation_task_packet(
                    &kernel.project_id,
                    &kernel.session_id,
                    context,
                    &user_instruction,
                    created_at_ms,
                );
                if let Err(error) = kernel.record_task_packet(
                    context.request_id.clone(),
                    "ChapterGeneration",
                    packet,
                ) {
                    tracing::warn!(
                        "WriterAgent chapter-generation task packet rejected: {}",
                        error
                    );
                }
            },
            move |context, report| {
                record_chapter_provider_budget_report(&budget_app, context, report);
            },
            move |context, report| {
                record_chapter_model_started(&model_app, context, report);
            },
        )
        .await;

        match terminal {
            PipelineTerminal::Completed {
                saved,
                generated_content,
            } => {
                crate::observe_generated_chapter_result(&app_clone, &saved, &generated_content);
                let embed_app = app_clone.clone();
                tokio::spawn(async move {
                    crate::auto_embed_chapter(&embed_app, &saved.chapter_title, &generated_content)
                        .await;
                });
            }
            PipelineTerminal::Conflict(conflict) => {
                let bundle = crate::writer_agent::task_receipt::WriterFailureEvidenceBundle::new(
                    crate::writer_agent::task_receipt::WriterFailureCategory::SaveFailed,
                    "SAVE_CONFLICT",
                    format!("Save blocked by {}.", conflict.reason),
                    true,
                    Some(trace_request_id.clone()),
                    vec![
                        format!("base_revision:{}", conflict.base_revision),
                        format!("current_revision:{}", conflict.current_revision),
                        format!("save_conflict:{}", conflict.reason),
                    ],
                    serde_json::json!({ "conflict": conflict }),
                    vec![
                        "Resolve editor/storage revision mismatch or save as a draft copy."
                            .to_string(),
                    ],
                    crate::agent_runtime::now_ms(),
                );
                record_writer_failure_bundle(&app_clone, &bundle);
            }
            PipelineTerminal::Failed(error) => {
                let bundle = crate::chapter_generation::failure_bundle_from_chapter_error(
                    &trace_request_id,
                    &error,
                    crate::agent_runtime::now_ms(),
                );
                record_writer_failure_bundle(&app_clone, &bundle);
            }
        }
    });

    Ok(ChapterGenerationStart { request_id })
}

#[tauri::command]
pub async fn analyze_chapter(
    _app: tauri::AppHandle,
    content: String,
) -> Result<Vec<ReviewItem>, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);

    let system_prompt = r#"You are a professional novel editor. Analyze the chapter and output a JSON object with a "reviews" array.

Each review must have:
- "quote": exact text from the chapter (copy verbatim, at least 10 characters)
- "type": one of "logic" | "ooc" | "pacing" | "prose"
- "issue": what the problem is
- "suggestion": how to fix it (in Chinese, specific rewrite suggestion)

Output ONLY the JSON object, no explanation outside. Example:
{"reviews":[{"quote":"他走出了房间","type":"prose","issue":"缺乏画面感","suggestion":"他推开吱呀作响的木门，幽暗的走廊里只有自己的脚步声在回荡。"}]}"#;

    let truncated = agent_harness_core::truncate_context(&content, 8000);
    let body = llm_runtime::chat_json(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": format!("Analyze this chapter:\n\n{}", truncated)}),
        ],
        60,
    )
    .await?;

    let report: ReviewReport =
        serde_json::from_value(body).map_err(|e| format!("Failed to parse review JSON: {}", e))?;

    Ok(report.reviews)
}

#[tauri::command]
pub async fn ask_project_brain(
    app: tauri::AppHandle,
    query: String,
    payload: Option<AskProjectBrainPayload>,
) -> Result<(), String> {
    let query = payload
        .as_ref()
        .map(|payload| payload.query.as_str())
        .unwrap_or(&query);
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let focus = {
        let state = app.state::<crate::AppState>();
        let kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        crate::brain_service::ProjectBrainFocus::from_kernel(query, &kernel)
    };

    crate::brain_service::answer_query_with_focus(
        &app,
        &settings,
        query,
        &focus,
        payload
            .as_ref()
            .and_then(|payload| payload.provider_budget_approval.as_ref()),
        |content| {
            let _ = app.emit(
                crate::events::AGENT_STREAM_CHUNK,
                crate::StreamChunk { content },
            );
            Ok(llm_runtime::StreamControl::Continue)
        },
    )
    .await?;

    let _ = app.emit(
        crate::events::AGENT_STREAM_END,
        crate::StreamEnd {
            reason: "complete".to_string(),
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn generate_parallel_drafts(
    payload: ParallelDraftPayload,
) -> Result<Vec<ParallelDraft>, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let chapter = payload
        .chapter_title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or("当前章节");
    let focus = if payload.selected_text.trim().is_empty() {
        payload.paragraph.trim()
    } else {
        payload.selected_text.trim()
    };

    let mission_block = if payload.mission_context.trim().is_empty() {
        String::new()
    } else {
        format!("\n## 本章任务约束\n{}\n", payload.mission_context)
    };
    let promise_block = if payload.promise_context.trim().is_empty() {
        String::new()
    } else {
        format!("\n## 未兑现伏笔参考\n{}\n", payload.promise_context)
    };

    let prompt = format!(
        "你是中文小说共创写手。请顺着用户已有文本，生成三个不同方向的平行草稿。\n\
         输出格式必须严格为：\n\
         A: ...\nB: ...\nC: ...\n\
         每个版本 2-5 句，可以分段；每个版本末尾用括号标注关联的创作依据。不要解释，不要 Markdown。\n\
         A 偏顺势推进，B 偏冲突加压，C 偏情绪转折。\n\
         ## 章节\n{}{}{}\n## 光标前文\n{}\n## 光标后文\n{}\n## 当前焦点\n{}",
        chapter,
        mission_block,
        promise_block,
        agent_harness_core::truncate_context(&payload.prefix, 3000),
        agent_harness_core::truncate_context(&payload.suffix, 1000),
        focus,
    );

    let text = llm_runtime::chat_text(
        &settings,
        vec![serde_json::json!({"role": "user", "content": prompt})],
        false,
        45,
    )
    .await?;
    let drafts = parse_parallel_drafts(&text);
    if drafts.is_empty() {
        let fallback = trim_parallel_draft(&text);
        if fallback.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![ParallelDraft {
            id: "a".to_string(),
            label: "A 顺势推进".to_string(),
            text: fallback,
        }]);
    }
    Ok(drafts)
}

#[tauri::command]
pub async fn analyze_pacing(summaries: String) -> Result<String, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let text = llm_runtime::chat_text(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": "You are a structural editor. Analyze the chapter sequence for pacing issues, slow sections, abrupt transitions, and unresolved arcs. Be specific and concise."}),
            serde_json::json!({"role": "user", "content": format!("Chapter summaries:\n{}", summaries)}),
        ],
        false,
        60,
    )
    .await?;

    Ok(if text.is_empty() {
        "No analysis generated".to_string()
    } else {
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_parallel_drafts_keeps_multiline_branches() {
        let drafts = parse_parallel_drafts(
            "A: 林墨没有立刻回答。\n他只是把刀压低。\nB：门外忽然传来脚步声。\nC: 她看见他眼里的犹豫。",
        );

        assert_eq!(drafts.len(), 3);
        assert_eq!(drafts[0].id, "a");
        assert!(drafts[0].text.contains("把刀压低"));
        assert_eq!(drafts[1].label, "B 冲突加压");
        assert_eq!(drafts[2].id, "c");
    }

    #[test]
    fn trim_parallel_draft_removes_markdown_fence_noise() {
        assert_eq!(
            trim_parallel_draft("```\n林墨停下脚步。\n```"),
            "林墨停下脚步。"
        );
    }
}
