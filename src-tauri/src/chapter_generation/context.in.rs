pub fn build_chapter_context(
    app: &tauri::AppHandle,
    input: BuildChapterContextInput,
) -> Result<BuiltChapterContext, ChapterGenerationError> {
    let instruction = input.user_instruction.trim();
    if instruction.is_empty() {
        return Err(ChapterGenerationError::new(
            "INSTRUCTION_EMPTY",
            "The chapter generation instruction is empty.",
            true,
        ));
    }

    let outline = storage::load_outline(app).map_err(|e| {
        ChapterGenerationError::with_details(
            "STORAGE_READ_FAILED",
            "Failed to read outline.",
            true,
            e,
        )
    })?;

    let target = resolve_target_from_outline(
        &outline,
        input.target_chapter_title.as_deref(),
        input.target_chapter_number,
        input.chapter_summary_override.as_deref(),
    )?;

    let base_revision = storage::chapter_revision(app, &target.title).map_err(|e| {
        ChapterGenerationError::with_details(
            "STORAGE_READ_FAILED",
            "Failed to read target chapter revision.",
            true,
            e,
        )
    })?;

    let query = format!("{}\n{}\n{}", instruction, target.title, target.summary);
    let mut composer = ContextComposer::new(input.budget.total_chars);
    composer.add_source(
        "instruction",
        "user-instruction",
        "User instruction",
        instruction,
        input.budget.instruction_chars,
        None,
    );

    let outline_text = if outline.is_empty() {
        "No outline nodes found.".to_string()
    } else {
        outline
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                format!(
                    "{}. {} [{}]\n{}",
                    idx + 1,
                    node.chapter_title,
                    node.status,
                    node.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    composer.add_source(
        "outline",
        "outline.json",
        "Outline / beat sheet",
        &outline_text,
        input.budget.outline_chars,
        None,
    );

    composer.add_source(
        "target_beat",
        &target.title,
        "Current chapter beat",
        &target.summary,
        input.budget.outline_chars.min(2_000),
        None,
    );

    if let Some(target_index) = target.number.map(|n| n - 1) {
        let previous_nodes =
            select_previous_nodes(&outline, target_index, input.budget.previous_chapter_count);
        let previous_text = build_adjacent_chapter_context(app, previous_nodes);
        composer.add_source(
            "previous_chapters",
            "previous",
            "Previous chapter continuity",
            &previous_text,
            input.budget.previous_chapters_chars,
            None,
        );

        let next_nodes = select_next_nodes(&outline, target_index, input.budget.next_chapter_count);
        let next_text = build_next_chapter_context(next_nodes);
        composer.add_source(
            "next_chapter",
            "next",
            "Next chapter direction",
            &next_text,
            input.budget.next_chapter_chars,
            None,
        );
    }

    if let Ok(existing) = storage::load_chapter(app, target.title.clone()) {
        if !existing.trim().is_empty() {
            composer.add_source(
                "target_existing_text",
                &target.title,
                "Existing target chapter text",
                &existing,
                input.budget.target_existing_chars,
                None,
            );
        }
    }

    let lore_entries = storage::load_lorebook(app)
        .map_err(|e| ChapterGenerationError::new("lorebook_load_failed", e, true))?;
    let selected_lore =
        select_lore_entries(&lore_entries, &query, input.budget.lorebook_entry_count);
    let lore_text = if selected_lore.is_empty() {
        "No directly relevant lorebook entries found.".to_string()
    } else {
        selected_lore
            .iter()
            .map(|(score, entry)| {
                format!("[{}] score {:.1}\n{}", entry.keyword, score, entry.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    composer.add_source(
        "lorebook",
        "lorebook.json",
        "Relevant lorebook entries",
        &lore_text,
        input.budget.lorebook_chars,
        None,
    );

    let rag_chunks = select_rag_chunks(app, &query, input.budget.rag_chunk_count);
    if !rag_chunks.is_empty() {
        let rag_text = rag_chunks
            .iter()
            .map(|(score, reasons, chunk)| {
                format!(
                    "[{} · {} · score {:.1}]\n{}\n{}",
                    chunk.id,
                    chunk.chapter,
                    score,
                    format_text_chunk_relevance(reasons),
                    chunk.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        composer.add_source(
            "project_brain",
            "project_brain.json",
            "Project Brain relevant chunks",
            &rag_text,
            input.budget.rag_chars,
            Some(
                rag_chunks
                    .first()
                    .map(|(score, _, _)| *score)
                    .unwrap_or_default(),
            ),
        );
    }

    let profile_text = input
        .user_profile_entries
        .iter()
        .take(input.budget.user_profile_entry_count)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if !profile_text.trim().is_empty() {
        composer.add_source(
            "user_profile",
            "user_drift_profile",
            "User style preferences",
            &profile_text,
            input.budget.user_profile_chars,
            None,
        );
    }

    let (prompt_context, sources, budget_report) = composer.finish();
    let warnings = budget_report.warnings.clone();

    let request_id = input.request_id;
    let receipt = build_chapter_generation_receipt(
        &request_id,
        &target,
        &base_revision,
        instruction,
        &sources,
        crate::agent_runtime::now_ms(),
    );

    Ok(BuiltChapterContext {
        request_id,
        target,
        base_revision,
        prompt_context,
        sources,
        budget: budget_report,
        warnings,
        receipt,
    })
}

pub fn build_chapter_generation_task_packet(
    project_id: &str,
    session_id: &str,
    context: &BuiltChapterContext,
    user_instruction: &str,
    created_at_ms: u64,
) -> TaskPacket {
    let instruction = user_instruction.trim();
    let instruction_summary = if instruction.is_empty() {
        "Draft the target chapter from the built chapter context.".to_string()
    } else {
        snippet_text(instruction, 180)
    };
    let target_title = snippet_text(&context.target.title, 180);
    let objective = snippet_text(
        &format!(
            "Draft '{}' from the chapter generation context. Instruction: {}",
            target_title, instruction_summary
        ),
        560,
    );
    let mut packet = TaskPacket::new(
        format!("{}:{}:ChapterGeneration", session_id, context.request_id),
        objective,
        TaskScope::Chapter,
        created_at_ms,
    );
    packet.scope_ref = Some(context.target.title.clone());
    packet.intent = Some(Intent::GenerateContent);
    packet.constraints = vec![
        "Preserve established canon unless the author explicitly approves a change.".to_string(),
        "Respect the book contract, chapter mission, outline beat, and known promise ledger."
            .to_string(),
        "Generate chapter prose only; no analysis, markdown fences, or meta commentary."
            .to_string(),
        "Saving generated content must pass revision/conflict checks before overwriting chapters."
            .to_string(),
    ];
    packet.success_criteria = vec![
        "Generated prose passes non-empty and output-size validation.".to_string(),
        "Context sources include the instruction plus chapter/continuity memory before drafting."
            .to_string(),
        "Save completes, or a concrete save conflict is surfaced to the author.".to_string(),
        "Chapter result feedback can be recorded after a successful save.".to_string(),
    ];
    packet.beliefs = chapter_context_beliefs(context, project_id);
    packet.required_context = chapter_required_context(context);
    packet.tool_policy = ToolPolicyContract {
        max_side_effect_level: ToolSideEffectLevel::Write,
        allow_approval_required: true,
        required_tool_tags: vec!["generation".to_string()],
    };
    packet.feedback = FeedbackContract {
        expected_signals: vec![
            PHASE_CONTEXT_BUILT.to_string(),
            PHASE_COMPLETED.to_string(),
            PHASE_CONFLICT.to_string(),
            "chapter_result_summary".to_string(),
        ],
        checkpoints: vec![
            "record chapter generation context sources".to_string(),
            "validate generated content before save".to_string(),
            "check target revision before overwrite".to_string(),
            "record result feedback after successful save".to_string(),
        ],
        memory_writes: vec![
            "chapter_result_summary".to_string(),
            "outline_status".to_string(),
        ],
    };
    packet
}

pub fn build_chapter_generation_receipt(
    request_id: &str,
    target: &ChapterTarget,
    base_revision: &str,
    user_instruction: &str,
    sources: &[ChapterContextSource],
    created_at_ms: u64,
) -> WriterTaskReceipt {
    let instruction = user_instruction.trim();
    let objective = if instruction.is_empty() {
        format!("Draft '{}' from the built chapter context.", target.title)
    } else {
        format!(
            "Draft '{}' from the built chapter context. Instruction: {}",
            target.title,
            snippet_text(instruction, 180)
        )
    };
    let mut required_evidence = vec!["instruction".to_string()];
    for source in sources.iter().filter(|source| source.included_chars > 0) {
        if is_required_chapter_source(&source.source_type)
            && !required_evidence
                .iter()
                .any(|existing| existing == &source.source_type)
        {
            required_evidence.push(source.source_type.clone());
        }
    }
    let source_refs = sources
        .iter()
        .filter(|source| source.included_chars > 0)
        .map(|source| format!("{}:{}", source.source_type, source.id))
        .collect::<Vec<_>>();

    WriterTaskReceipt::new(
        request_id,
        "ChapterGeneration",
        Some(target.title.clone()),
        objective,
        required_evidence,
        vec!["chapter_draft".to_string(), "saved_chapter".to_string()],
        vec![
            "overwrite_without_revision_match".to_string(),
            "change_target_chapter_without_new_receipt".to_string(),
            "ignore_required_context_sources".to_string(),
        ],
        source_refs,
        Some(base_revision.to_string()),
        created_at_ms,
    )
}

fn chapter_context_beliefs(context: &BuiltChapterContext, project_id: &str) -> Vec<TaskBelief> {
    let mut beliefs = context
        .sources
        .iter()
        .filter(|source| source.included_chars > 0)
        .take(8)
        .map(|source| {
            let mut statement = format!(
                "{} contributes {} chars",
                source.label, source.included_chars
            );
            if source.truncated {
                statement.push_str(" after truncation");
            }
            TaskBelief::new(
                source.source_type.clone(),
                statement,
                chapter_source_confidence(&source.source_type),
            )
            .with_source(source.id.clone())
        })
        .collect::<Vec<_>>();

    if beliefs.is_empty() {
        beliefs.push(
            TaskBelief::new(
                "chapter_generation_context",
                format!(
                    "{} has no explicit context sources; fall back to project {}.",
                    context.target.title, project_id
                ),
                0.5,
            )
            .with_source(context.request_id.clone()),
        );
    }

    beliefs
}

fn chapter_required_context(context: &BuiltChapterContext) -> Vec<RequiredContext> {
    let mut required_context = context
        .sources
        .iter()
        .take(12)
        .map(|source| {
            RequiredContext::new(
                source.source_type.clone(),
                chapter_source_purpose(&source.source_type),
                source.included_chars.max(1),
                is_required_chapter_source(&source.source_type),
            )
        })
        .collect::<Vec<_>>();

    if !required_context
        .iter()
        .any(|context| context.required && !context.source_type.trim().is_empty())
    {
        required_context.push(RequiredContext::new(
            "chapter_generation_context",
            "Fallback chapter context required to draft safely.",
            1,
            true,
        ));
    }

    required_context
}

fn is_required_chapter_source(source_type: &str) -> bool {
    matches!(
        source_type,
        "instruction"
            | "outline"
            | "target_beat"
            | "previous_chapters"
            | "lorebook"
            | "project_brain"
    )
}

fn chapter_source_purpose(source_type: &str) -> &'static str {
    match source_type {
        "instruction" => "Capture the author's explicit generation request.",
        "outline" => "Keep the draft aligned with the book-level beat sheet.",
        "target_beat" => "Preserve the target chapter mission and planned payoff.",
        "previous_chapters" => "Maintain continuity from recent chapter outcomes.",
        "next_chapter" => "Avoid blocking the next planned beat.",
        "target_existing_text" => "Respect any existing prose already in the target chapter.",
        "lorebook" => "Ground character, setting, and canon details.",
        "project_brain" => "Recall relevant long-range project memory.",
        "user_profile" => "Preserve learned author style preferences.",
        _ => "Provide supporting context for chapter generation.",
    }
}

fn chapter_source_confidence(source_type: &str) -> f32 {
    match source_type {
        "instruction" | "target_beat" => 0.92,
        "outline" | "lorebook" => 0.88,
        "previous_chapters" | "project_brain" => 0.78,
        "next_chapter" | "target_existing_text" | "user_profile" => 0.70,
        _ => 0.60,
    }
}

fn snippet_text(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

