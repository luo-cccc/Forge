pub async fn generate_chapter_draft(
    settings: &llm_runtime::LlmSettings,
    context: &BuiltChapterContext,
    provider_budget_approval: Option<&WriterProviderBudgetApproval>,
    mut record_model_started: impl FnMut(&BuiltChapterContext, &WriterProviderBudgetReport) + Send,
) -> Result<GenerateChapterDraftOutput, ChapterGenerationError> {
    if context.prompt_context.trim().is_empty() {
        return Err(ChapterGenerationError::new(
            "CONTEXT_INVALID",
            "The built chapter context is empty.",
            true,
        ));
    }

    let system_prompt = format!(
        "You are a professional Chinese novelist drafting a complete chapter. \
Use the provided project context, preserve continuity, and write only chapter prose. \
Do not include analysis, markdown fences, action tags, or meta commentary. \
Preserve the named anchors, unresolved debts, and chapter mission constraints from the context; \
do not silently drop active named entities, artifacts, promises, or reader-debt payoffs unless the context says they are resolved. \
If the context names active anchors, carry the relevant anchors into the scene through action, dialogue, consequence, or payoff pressure; \
do not merely mention them in passing. \
Unless the chapter plan explicitly narrows scope, at least three active anchors from the context must materially participate in the scene, \
and at least one of them must change the immediate choice, pressure, or consequence of the chapter. \
Aim for up to {} Chinese characters unless the beat clearly requires less.",
        DEFAULT_OUTPUT_SOFT_CAP_CHARS
    );
    let user_prompt = format!(
        "Task: {}\n\nTarget chapter: {}\n\nProject context:\n{}",
        context
            .sources
            .iter()
            .find(|s| s.source_type == "instruction")
            .map(|_| "Draft this chapter from the user's instruction.")
            .unwrap_or("Draft this chapter."),
        context.target.title,
        context.prompt_context
    );
    let messages = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
        serde_json::json!({"role": "user", "content": user_prompt}),
    ];
    let budget_report = apply_provider_budget_approval(
        chapter_generation_provider_budget(settings, &messages),
        provider_budget_approval,
    );
    if budget_report.decision == WriterProviderBudgetDecision::ApprovalRequired {
        return Err(provider_budget_error(
            &context.request_id,
            &context.receipt,
            budget_report,
        ));
    }
    record_model_started(context, &budget_report);

    let content = llm_runtime::chat_text_profile(
        settings,
        messages,
        llm_runtime::LlmRequestProfile::ChapterDraft,
        PROVIDER_TIMEOUT_SECS,
    )
    .await
    .map_err(map_provider_error)?;

    let content = content.trim().to_string();
    validate_generated_content(&content)?;

    Ok(GenerateChapterDraftOutput {
        output_chars: char_count(&content),
        content,
        finish_reason: "complete".to_string(),
        model: settings.model.clone(),
        provider: "openai-compatible".to_string(),
        base_revision: context.base_revision.clone(),
        provider_budget: budget_report,
    })
}

pub fn chapter_generation_provider_budget(
    settings: &llm_runtime::LlmSettings,
    messages: &[serde_json::Value],
) -> WriterProviderBudgetReport {
    let converted = messages
        .iter()
        .map(|message| LlmMessage {
            role: message
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or("user")
                .to_string(),
            content: message
                .get("content")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        })
        .collect::<Vec<_>>();
    let estimated_input_tokens =
        agent_harness_core::context_window_guard::estimate_request_tokens(&converted, None);
    evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        settings.model.clone(),
        estimated_input_tokens,
        u64::from(
            llm_runtime::request_options(settings, llm_runtime::LlmRequestProfile::ChapterDraft)
                .max_tokens,
        ),
    ))
}

pub fn provider_budget_error(
    request_id: &str,
    receipt: &WriterTaskReceipt,
    report: WriterProviderBudgetReport,
) -> ChapterGenerationError {
    ChapterGenerationError::new(
        "PROVIDER_BUDGET_APPROVAL_REQUIRED",
        "Chapter generation provider budget requires explicit approval before calling the model.",
        true,
    )
    .with_evidence(Box::new(WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ProviderFailed,
        "PROVIDER_BUDGET_APPROVAL_REQUIRED",
        "Chapter generation provider budget requires explicit approval before calling the model.",
        true,
        Some(request_id.to_string()),
        vec![
            format!("receipt:{}", receipt.task_id),
            format!("model:{}", report.model),
            format!("estimated_tokens:{}", report.estimated_total_tokens),
            format!("estimated_cost_micros:{}", report.estimated_cost_micros),
        ],
        serde_json::json!({
            "providerBudget": report,
            "receipt": receipt,
        }),
        vec![
            "Surface the provider token/cost estimate to the author before retrying.".to_string(),
            "Reduce context budget or requested output length if approval is not granted."
                .to_string(),
        ],
        crate::agent_runtime::now_ms(),
    )))
}

pub fn provider_budget_report_from_error(
    error: &ChapterGenerationError,
) -> Option<WriterProviderBudgetReport> {
    let budget = error
        .evidence
        .as_ref()?
        .details
        .get("providerBudget")?
        .clone();
    serde_json::from_value(budget).ok()
}

pub fn save_generated_chapter(
    app: &tauri::AppHandle,
    input: SaveGeneratedChapterInput,
) -> Result<SaveGeneratedChapterOutput, ChapterGenerationError> {
    if let Some(error) = validate_receipt_for_save(&input) {
        return Err(error);
    }

    if input.generated_content.trim().is_empty() {
        return Err(ChapterGenerationError::new(
            "CONTENT_EMPTY",
            "Generated chapter content is empty.",
            true,
        ));
    }

    if char_count(&input.generated_content) > DEFAULT_OUTPUT_HARD_CAP_CHARS {
        return Err(ChapterGenerationError::new(
            "CONTENT_TOO_LARGE",
            "Generated chapter content exceeds the hard save cap.",
            true,
        ));
    }

    let current_revision = storage::chapter_revision(app, &input.target.title).map_err(|e| {
        ChapterGenerationError::with_details(
            "STORAGE_READ_FAILED",
            "Failed to read current chapter revision.",
            true,
            e,
        )
    })?;

    match decide_save_action(
        &input.target.title,
        &input.request_id,
        input.save_mode,
        &input.base_revision,
        &current_revision,
        input.frontend_state.as_ref(),
    ) {
        SaveDecision::WriteTarget => {
            let new_revision = storage::save_chapter_content_and_revision(
                app,
                &input.target.title,
                &input.generated_content,
            )
            .map_err(|e| {
                ChapterGenerationError::with_details(
                    "STORAGE_WRITE_FAILED",
                    "Failed to save generated chapter.",
                    true,
                    e,
                )
            })?;
            Ok(SaveGeneratedChapterOutput {
                chapter_title: input.target.title,
                new_revision,
                saved_mode: if current_revision == "missing" {
                    "created".to_string()
                } else {
                    "replaced".to_string()
                },
            })
        }
        SaveDecision::WriteDraft {
            draft_title,
            conflict,
        } => {
            tracing::warn!(
                "Saving generated chapter as draft copy after conflict: {}",
                conflict.reason
            );
            let new_revision = storage::save_chapter_content_and_revision(
                app,
                &draft_title,
                &input.generated_content,
            )
            .map_err(|e| {
                ChapterGenerationError::with_details(
                    "STORAGE_WRITE_FAILED",
                    "Failed to save generated draft copy.",
                    true,
                    e,
                )
            })?;
            Ok(SaveGeneratedChapterOutput {
                chapter_title: draft_title,
                new_revision,
                saved_mode: "draft_copy".to_string(),
            })
        }
        SaveDecision::Conflict(conflict) => Err(ChapterGenerationError {
            code: "SAVE_CONFLICT".to_string(),
            message: format!("Save blocked by {}.", conflict.reason),
            recoverable: true,
            details: serde_json::to_string(&conflict).ok(),
            evidence: Some(Box::new(failure_bundle_from_save_conflict(
                &input.receipt,
                &conflict,
                crate::agent_runtime::now_ms(),
            ))),
        }),
    }
}

fn validate_receipt_for_save(input: &SaveGeneratedChapterInput) -> Option<ChapterGenerationError> {
    let mismatches = input.receipt.validate_write_attempt(
        &input.request_id,
        &input.target.title,
        &input.base_revision,
        "saved_chapter",
    );
    if mismatches.is_empty() {
        return None;
    }

    let evidence_refs = mismatches
        .iter()
        .map(|mismatch| {
            format!(
                "{}:{}->{}",
                mismatch.field, mismatch.expected, mismatch.actual
            )
        })
        .collect::<Vec<_>>();
    let evidence = WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ReceiptMismatch,
        "RECEIPT_MISMATCH",
        "Generated chapter save was blocked because the task receipt no longer matches the write attempt.",
        true,
        Some(input.receipt.task_id.clone()),
        evidence_refs,
        serde_json::json!({
            "receipt": input.receipt,
            "attempt": {
                "requestId": input.request_id,
                "chapter": input.target.title,
                "baseRevision": input.base_revision,
                "artifact": "saved_chapter",
            },
            "mismatches": mismatches,
        }),
        vec![
            "Rebuild the chapter generation context for the current target chapter.".to_string(),
            "Retry only after the frontend and storage revisions match.".to_string(),
        ],
        crate::agent_runtime::now_ms(),
    );

    Some(
        ChapterGenerationError::new(
            "RECEIPT_MISMATCH",
            "Generated chapter save was blocked because the task receipt no longer matches the write attempt.",
            true,
        )
        .with_evidence(Box::new(evidence)),
    )
}

pub fn save_conflict_from_error(error: &ChapterGenerationError) -> Option<SaveConflict> {
    if error.code != "SAVE_CONFLICT" {
        return None;
    }
    error
        .details
        .as_deref()
        .and_then(|details| serde_json::from_str(details).ok())
}

pub fn failure_bundle_from_chapter_error(
    request_id: &str,
    error: &ChapterGenerationError,
    created_at_ms: u64,
) -> WriterFailureEvidenceBundle {
    if let Some(bundle) = error.evidence.clone() {
        return *bundle;
    }
    let category = failure_category_for_error_code(&error.code);
    let mut evidence_refs = vec![format!("error:{}", error.code)];
    if let Some(details) = error
        .details
        .as_ref()
        .filter(|details| !details.trim().is_empty())
    {
        evidence_refs.push(format!("details:{}", snippet_text(details, 120)));
    }
    WriterFailureEvidenceBundle::new(
        category,
        error.code.clone(),
        error.message.clone(),
        error.recoverable,
        Some(request_id.to_string()),
        evidence_refs,
        serde_json::json!({
            "details": error.details,
        }),
        remediation_for_error_code(&error.code),
        created_at_ms,
    )
}

pub fn failure_bundle_from_save_conflict(
    receipt: &WriterTaskReceipt,
    conflict: &SaveConflict,
    created_at_ms: u64,
) -> WriterFailureEvidenceBundle {
    WriterFailureEvidenceBundle::new(
        WriterFailureCategory::SaveFailed,
        "SAVE_CONFLICT",
        format!("Save blocked by {}.", conflict.reason),
        true,
        Some(receipt.task_id.clone()),
        vec![
            format!("chapter:{}", receipt.chapter.clone().unwrap_or_default()),
            format!("base_revision:{}", conflict.base_revision),
            format!("current_revision:{}", conflict.current_revision),
            format!("save_conflict:{}", conflict.reason),
        ],
        serde_json::json!({
            "receipt": receipt,
            "conflict": conflict,
        }),
        vec![
            "Review the open editor changes before overwriting.".to_string(),
            "Regenerate from the current chapter revision or save as a draft copy.".to_string(),
        ],
        created_at_ms,
    )
}

fn failure_category_for_error_code(code: &str) -> WriterFailureCategory {
    match code {
        "INSTRUCTION_EMPTY"
        | "TARGET_CHAPTER_NOT_FOUND"
        | "TARGET_CHAPTER_AMBIGUOUS"
        | "CONTEXT_INVALID" => WriterFailureCategory::ContextMissing,
        "PROVIDER_TIMEOUT"
        | "PROVIDER_RATE_LIMITED"
        | "PROVIDER_NOT_CONFIGURED"
        | "PROVIDER_CALL_FAILED"
        | "PROVIDER_BUDGET_APPROVAL_REQUIRED"
        | "MODEL_OUTPUT_EMPTY"
        | "MODEL_OUTPUT_TOO_LARGE" => WriterFailureCategory::ProviderFailed,
        "SAVE_CONFLICT"
        | "CONTENT_EMPTY"
        | "CONTENT_TOO_LARGE"
        | "STORAGE_READ_FAILED"
        | "STORAGE_WRITE_FAILED"
        | "OUTLINE_LOAD_FAILED"
        | "OUTLINE_SAVE_FAILED" => WriterFailureCategory::SaveFailed,
        "RECEIPT_MISMATCH" => WriterFailureCategory::ReceiptMismatch,
        _ => WriterFailureCategory::ProviderFailed,
    }
}

fn remediation_for_error_code(code: &str) -> Vec<String> {
    match code {
        "INSTRUCTION_EMPTY" => {
            vec!["Provide a concrete chapter generation instruction.".to_string()]
        }
        "TARGET_CHAPTER_NOT_FOUND" | "TARGET_CHAPTER_AMBIGUOUS" => {
            vec!["Select a concrete target chapter or fix duplicate outline entries.".to_string()]
        }
        "PROVIDER_NOT_CONFIGURED" => vec!["Configure a valid model provider API key.".to_string()],
        "PROVIDER_BUDGET_APPROVAL_REQUIRED" => vec![
            "Review and approve the estimated provider token/cost budget before retrying."
                .to_string(),
            "Reduce context budget or requested output length if approval is not granted."
                .to_string(),
        ],
        "PROVIDER_TIMEOUT" | "PROVIDER_RATE_LIMITED" | "PROVIDER_CALL_FAILED" => vec![
            "Retry after provider recovery or switch to another configured provider.".to_string(),
        ],
        "MODEL_OUTPUT_EMPTY" | "MODEL_OUTPUT_TOO_LARGE" => vec![
            "Regenerate with a narrower chapter objective or smaller output budget.".to_string(),
        ],
        "RECEIPT_MISMATCH" => {
            vec!["Rebuild the task receipt from the latest context before saving.".to_string()]
        }
        "SAVE_CONFLICT" => {
            vec!["Resolve editor/storage revision mismatch or save as a draft copy.".to_string()]
        }
        _ => vec![
            "Inspect the failure evidence bundle and retry from the last safe phase.".to_string(),
        ],
    }
}

pub fn update_outline_after_generation(
    app: &tauri::AppHandle,
    target: &ChapterTarget,
    saved: &SaveGeneratedChapterOutput,
) -> Result<OutlineUpdateOutput, ChapterGenerationError> {
    let mut outline = storage::load_outline(app).map_err(|e| {
        ChapterGenerationError::with_details(
            "OUTLINE_NOT_FOUND",
            "Failed to read outline for status update.",
            true,
            e,
        )
    })?;

    let mut changed = false;
    if let Some(node) = outline.iter_mut().find(|node| {
        node.chapter_title == saved.chapter_title || node.chapter_title == target.title
    }) {
        if node.status != "drafted" {
            node.status = "drafted".to_string();
            changed = true;
        }
    } else {
        outline.push(storage::OutlineNode {
            chapter_title: saved.chapter_title.clone(),
            summary: target.summary.clone(),
            status: "drafted".to_string(),
        });
        changed = true;
    }

    if changed {
        storage::save_outline(app, &outline).map_err(|e| {
            ChapterGenerationError::with_details(
                "OUTLINE_UPDATE_FAILED",
                "Failed to update outline after chapter save.",
                true,
                e,
            )
        })?;
    }

    let outline_json = serde_json::to_string(&outline).unwrap_or_default();
    Ok(OutlineUpdateOutput {
        outline_revision: storage::content_revision(&outline_json),
        changed,
        warnings: vec![],
    })
}

pub struct ChapterGenerationConfig {
    pub app: tauri::AppHandle,
    pub settings: llm_runtime::LlmSettings,
    pub payload: GenerateChapterAutonomousPayload,
    pub user_profile_entries: Vec<String>,
}
