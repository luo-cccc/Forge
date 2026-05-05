pub async fn run_chapter_generation_pipeline(
    config: ChapterGenerationConfig,
    mut emit: impl FnMut(ChapterGenerationEvent) + Send,
    mut record_task_packet: impl FnMut(&BuiltChapterContext) + Send,
    mut record_provider_budget: impl FnMut(&BuiltChapterContext, &WriterProviderBudgetReport) + Send,
    mut record_model_started: impl FnMut(&BuiltChapterContext, &WriterProviderBudgetReport) + Send,
) -> PipelineTerminal {
    let request_id = config.payload
        .request_id
        .clone()
        .unwrap_or_else(|| make_request_id("chapter"));

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_STARTED,
        "running",
        "正在理解任务并读取工程结构...",
        5,
        None,
    ));

    let build_input = BuildChapterContextInput {
        request_id: request_id.clone(),
        target_chapter_title: config.payload.target_chapter_title.clone(),
        target_chapter_number: config.payload.target_chapter_number,
        user_instruction: config.payload.user_instruction.clone(),
        budget: config.payload.budget.clone().unwrap_or_default(),
        chapter_summary_override: config.payload.chapter_summary_override.clone(),
        user_profile_entries: config.user_profile_entries,
    };

    let context = match build_chapter_context(&config.app, build_input) {
        Ok(context) => context,
        Err(error) => {
            emit(ChapterGenerationEvent::failed(&request_id, error.clone()));
            return PipelineTerminal::Failed(error);
        }
    };

    record_task_packet(&context);

    emit(ChapterGenerationEvent {
        request_id: request_id.clone(),
        phase: PHASE_CONTEXT_BUILT.to_string(),
        status: "done".to_string(),
        message: format!(
            "检索到 {} 个上下文来源，当前提示上下文 {} 字。",
            context.sources.len(),
            context.budget.included_chars
        ),
        progress: 25,
        target_chapter_title: Some(context.target.title.clone()),
        sources: Some(context.sources.clone()),
        budget: Some(context.budget.clone()),
        receipt: Some(context.receipt.clone()),
        saved: None,
        conflict: None,
        error: None,
        warnings: context.warnings.clone(),
    });

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_PROGRESS,
        "running",
        "正在撰写章节初稿...",
        45,
        Some(context.target.title.clone()),
    ));

    let draft = match generate_chapter_draft(
        &config.settings,
        &context,
        config.payload.provider_budget_approval.as_ref(),
        |context, report| record_model_started(context, report),
    )
    .await
    {
        Ok(draft) => {
            record_provider_budget(&context, &draft.provider_budget);
            draft
        }
        Err(error) => {
            if let Some(report) = provider_budget_report_from_error(&error) {
                record_provider_budget(&context, &report);
            }
            emit(ChapterGenerationEvent::failed(&request_id, error.clone()));
            return PipelineTerminal::Failed(error);
        }
    };

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_PROGRESS,
        "running",
        "正在保存章节并检查编辑器冲突...",
        70,
        Some(context.target.title.clone()),
    ));

    let save_input = SaveGeneratedChapterInput {
        request_id: request_id.clone(),
        target: context.target.clone(),
        generated_content: draft.content.clone(),
        base_revision: context.base_revision.clone(),
        save_mode: config.payload.save_mode,
        frontend_state: config.payload.frontend_state.clone(),
        receipt: context.receipt.clone(),
    };
    let saved = match save_generated_chapter(&config.app, save_input) {
        Ok(saved) => saved,
        Err(error) => {
            if let Some(conflict) = save_conflict_from_error(&error) {
                emit(ChapterGenerationEvent::conflict(
                    &request_id,
                    conflict.clone(),
                ));
                return PipelineTerminal::Conflict(conflict);
            }
            emit(ChapterGenerationEvent::failed(&request_id, error.clone()));
            return PipelineTerminal::Failed(error);
        }
    };

    emit(ChapterGenerationEvent::progress(
        &request_id,
        PHASE_PROGRESS,
        "running",
        "正在更新大纲状态...",
        85,
        Some(saved.chapter_title.clone()),
    ));

    let mut warnings = Vec::new();
    if let Err(error) = update_outline_after_generation(&config.app, &context.target, &saved) {
        warnings.push(format!("Outline update skipped: {}", error.message));
    }

    emit(ChapterGenerationEvent {
        request_id,
        phase: PHASE_COMPLETED.to_string(),
        status: "done".to_string(),
        message: format!("{} 初稿已保存。", saved.chapter_title),
        progress: 100,
        target_chapter_title: Some(saved.chapter_title.clone()),
        sources: None,
        budget: None,
        receipt: None,
        saved: Some(saved.clone()),
        conflict: None,
        error: None,
        warnings,
    });

    PipelineTerminal::Completed {
        saved,
        generated_content: draft.content,
    }
}

impl ChapterGenerationEvent {
    pub fn progress(
        request_id: &str,
        phase: &str,
        status: &str,
        message: &str,
        progress: u8,
        target_chapter_title: Option<String>,
    ) -> Self {
        Self {
            request_id: request_id.to_string(),
            phase: phase.to_string(),
            status: status.to_string(),
            message: message.to_string(),
            progress,
            target_chapter_title,
            sources: None,
            budget: None,
            receipt: None,
            saved: None,
            conflict: None,
            error: None,
            warnings: vec![],
        }
    }

    pub fn failed(request_id: &str, error: ChapterGenerationError) -> Self {
        Self {
            request_id: request_id.to_string(),
            phase: PHASE_FAILED.to_string(),
            status: "error".to_string(),
            message: error.message.clone(),
            progress: 100,
            target_chapter_title: None,
            sources: None,
            budget: None,
            receipt: None,
            saved: None,
            conflict: None,
            error: Some(error),
            warnings: vec![],
        }
    }

    pub fn conflict(request_id: &str, conflict: SaveConflict) -> Self {
        Self {
            request_id: request_id.to_string(),
            phase: PHASE_CONFLICT.to_string(),
            status: "conflict".to_string(),
            message: format!("保存被阻止：{}。", conflict.reason),
            progress: 100,
            target_chapter_title: conflict.open_chapter_title.clone(),
            sources: None,
            budget: None,
            receipt: None,
            saved: None,
            conflict: Some(conflict),
            error: None,
            warnings: vec![],
        }
    }
}

fn make_draft_title(target_title: &str, request_id: &str) -> String {
    let suffix = request_id
        .chars()
        .rev()
        .take(6)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{} draft {}", target_title, suffix)
}

pub fn make_request_id(prefix: &str) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{}", prefix, millis)
}

pub fn map_provider_error(error: String) -> ChapterGenerationError {
    let lower = error.to_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        ChapterGenerationError::with_details(
            "PROVIDER_TIMEOUT",
            "The model provider timed out.",
            true,
            error,
        )
    } else if lower.contains("429") || lower.contains("rate limit") {
        ChapterGenerationError::with_details(
            "PROVIDER_RATE_LIMITED",
            "The model provider rate-limited the request.",
            true,
            error,
        )
    } else if lower.contains("api key") || lower.contains("unauthorized") || lower.contains("401") {
        ChapterGenerationError::with_details(
            "PROVIDER_NOT_CONFIGURED",
            "The model provider is not configured.",
            true,
            error,
        )
    } else {
        ChapterGenerationError::with_details(
            "PROVIDER_CALL_FAILED",
            "The model provider call failed.",
            true,
            error,
        )
    }
}
