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

struct ContextComposer {
    max_chars: usize,
    text: String,
    sources: Vec<ChapterContextSource>,
    warnings: Vec<String>,
}

impl ContextComposer {
    fn new(max_chars: usize) -> Self {
        Self {
            max_chars,
            text: String::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn remaining_chars(&self) -> usize {
        self.max_chars.saturating_sub(char_count(&self.text))
    }

    fn add_source(
        &mut self,
        source_type: &str,
        id: &str,
        label: &str,
        content: &str,
        source_cap: usize,
        score: Option<f32>,
    ) {
        if content.trim().is_empty() || self.remaining_chars() == 0 {
            return;
        }

        let header = format!("## {}\n", label);
        let footer = "\n\n";
        let overhead = char_count(&header) + char_count(footer);
        let remaining = self.remaining_chars();
        if remaining <= overhead {
            self.warnings
                .push(format!("Context budget exhausted before adding {}.", label));
            return;
        }

        let allowed = source_cap.min(remaining - overhead);
        let original_chars = char_count(content);
        let (included, included_chars, truncated) = truncate_text_report(content, allowed);

        self.text.push_str(&header);
        self.text.push_str(&included);
        self.text.push_str(footer);

        if truncated {
            self.warnings.push(format!(
                "{} truncated from {} to {} chars.",
                label, original_chars, included_chars
            ));
        }

        self.sources.push(ChapterContextSource {
            source_type: source_type.to_string(),
            id: id.to_string(),
            label: label.to_string(),
            original_chars,
            included_chars,
            truncated,
            score,
        });
    }

    fn finish(
        self,
    ) -> (
        String,
        Vec<ChapterContextSource>,
        ChapterContextBudgetReport,
    ) {
        let included_chars = char_count(&self.text);
        let truncated_source_count = self
            .sources
            .iter()
            .filter(|source| source.truncated)
            .count();
        let report = ChapterContextBudgetReport {
            max_chars: self.max_chars,
            included_chars,
            source_count: self.sources.len(),
            truncated_source_count,
            warnings: self.warnings,
        };
        (self.text, self.sources, report)
    }
}

fn select_previous_nodes(
    outline: &[storage::OutlineNode],
    target_index: usize,
    max_count: usize,
) -> Vec<&storage::OutlineNode> {
    let start = target_index.saturating_sub(max_count);
    outline[start..target_index].iter().collect()
}

fn select_next_nodes(
    outline: &[storage::OutlineNode],
    target_index: usize,
    max_count: usize,
) -> Vec<&storage::OutlineNode> {
    outline
        .iter()
        .skip(target_index + 1)
        .take(max_count)
        .collect()
}

fn build_adjacent_chapter_context(
    app: &tauri::AppHandle,
    nodes: Vec<&storage::OutlineNode>,
) -> String {
    if nodes.is_empty() {
        return "None (first chapter or no previous outline nodes).".to_string();
    }

    nodes
        .iter()
        .map(|node| {
            let text = storage::load_chapter(app, node.chapter_title.clone()).unwrap_or_default();
            if text.trim().is_empty() {
                format!("[{}]\nSummary: {}", node.chapter_title, node.summary)
            } else {
                format!(
                    "[{}]\nSummary: {}\nExisting text:\n{}",
                    node.chapter_title, node.summary, text
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_next_chapter_context(nodes: Vec<&storage::OutlineNode>) -> String {
    if nodes.is_empty() {
        return "No next chapter outline node.".to_string();
    }

    nodes
        .iter()
        .map(|node| format!("[{}]\n{}", node.chapter_title, node.summary))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn select_lore_entries<'a>(
    entries: &'a [storage::LoreEntry],
    query: &str,
    max_count: usize,
) -> Vec<(f32, &'a storage::LoreEntry)> {
    let mut scored = entries
        .iter()
        .map(|entry| {
            let haystack = format!("{}\n{}", entry.keyword, entry.content);
            (relevance_score(query, &haystack), entry)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max_count);
    scored
}

fn select_rag_chunks(
    app: &tauri::AppHandle,
    query: &str,
    max_count: usize,
) -> Vec<(f32, Vec<String>, agent_harness_core::Chunk)> {
    let Ok(path) = storage::brain_path(app) else {
        return vec![];
    };
    let db = match VectorDB::load(&path) {
        Ok(db) => db,
        Err(e) => {
            tracing::warn!(
                "Skipping Project Brain chunks because '{}' is unreadable: {}",
                path.display(),
                e
            );
            return vec![];
        }
    };

    let scored = db
        .chunks
        .into_iter()
        .map(|chunk| {
            let haystack = format!(
                "{}\n{}\n{}\n{}",
                chunk.chapter,
                chunk.keywords.join("\n"),
                chunk.topic.clone().unwrap_or_default(),
                chunk.text
            );
            (relevance_score(query, &haystack), chunk)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect::<Vec<_>>();
    rerank_text_chunks(scored, query, |chunk| {
        format!(
            "{}\n{}\n{}\n{}",
            chunk.chapter,
            chunk.keywords.join("\n"),
            chunk.topic.clone().unwrap_or_default(),
            chunk.text
        )
    })
    .into_iter()
    .take(max_count)
    .collect()
}

fn relevance_score(query: &str, haystack: &str) -> f32 {
    let haystack = haystack.to_lowercase();
    let mut score = 0f32;
    for needle in query_needles(query) {
        if haystack.contains(&needle.to_lowercase()) {
            score += needle.chars().count().max(1) as f32;
        }
    }
    score
}

fn query_needles(query: &str) -> Vec<String> {
    let mut needles = Vec::new();
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_alphanumeric() || is_cjk(ch) {
            current.push(ch);
        } else if !current.is_empty() {
            push_needle(&mut needles, &current);
            current.clear();
        }
    }
    if !current.is_empty() {
        push_needle(&mut needles, &current);
    }
    needles.truncate(64);
    needles
}

fn push_needle(needles: &mut Vec<String>, token: &str) {
    if char_count(token) >= 2 {
        needles.push(token.to_string());
    }

    let chars = token.chars().collect::<Vec<_>>();
    if chars.len() >= 4 && chars.iter().any(|ch| is_cjk(*ch)) {
        for window in chars.windows(2).take(16) {
            needles.push(window.iter().collect());
        }
    }
}

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline() -> Vec<storage::OutlineNode> {
        vec![
            storage::OutlineNode {
                chapter_title: "第一章".to_string(),
                summary: "林墨抵达破庙。".to_string(),
                status: "drafted".to_string(),
            },
            storage::OutlineNode {
                chapter_title: "第二章".to_string(),
                summary: "林墨发现壁画。".to_string(),
                status: "drafted".to_string(),
            },
            storage::OutlineNode {
                chapter_title: "第三章".to_string(),
                summary: "林墨发现密道并遭遇毒雾机关。".to_string(),
                status: "empty".to_string(),
            },
        ]
    }

    #[test]
    fn counts_unicode_chars_instead_of_bytes_for_chinese_text() {
        assert_eq!(char_count("破庙密道"), 4);
        assert_eq!("破庙密道".len(), 12);
    }

    #[test]
    fn truncates_chinese_at_valid_utf8_boundary() {
        let (text, included, truncated) = truncate_text_report("林墨推开破庙石门", 4);
        assert_eq!(text, "林墨推开");
        assert_eq!(included, 4);
        assert!(truncated);
    }

    #[test]
    fn prefers_chinese_sentence_boundary_when_truncating() {
        let (text, _, truncated) =
            truncate_text_report("林墨停下脚步。毒雾从密道深处涌来，像潮水一样。", 16);
        assert_eq!(text, "林墨停下脚步。");
        assert!(truncated);
    }

    #[test]
    fn handles_mixed_chinese_english_and_emoji_without_corruption() {
        let (text, included, truncated) = truncate_text_report("AI提醒林墨：run！🔥继续。", 10);
        assert_eq!(char_count(&text), included);
        assert!(text.is_char_boundary(text.len()));
        assert!(truncated);
    }

    #[test]
    fn resolves_target_chapter_by_outline_number_and_returns_metadata() {
        let target = resolve_target_from_outline(&outline(), None, Some(3), None).unwrap();
        assert_eq!(target.title, "第三章");
        assert_eq!(target.number, Some(3));
        assert!(target.summary.contains("密道"));
    }

    #[test]
    fn rejects_missing_target_chapter_with_typed_error() {
        let err = resolve_target_from_outline(&outline(), Some("第九章"), None, None).unwrap_err();
        assert_eq!(err.code, "TARGET_CHAPTER_NOT_FOUND");
    }

    #[test]
    fn rejects_ambiguous_target_chapter_with_typed_error() {
        let mut data = outline();
        data.push(storage::OutlineNode {
            chapter_title: "第三章".to_string(),
            summary: "重复节点".to_string(),
            status: "empty".to_string(),
        });
        let err = resolve_target_from_outline(&data, Some("第三章"), None, None).unwrap_err();
        assert_eq!(err.code, "TARGET_CHAPTER_AMBIGUOUS");
    }

    #[test]
    fn replaces_chapter_when_revision_matches_and_frontend_is_clean() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "abc",
            Some(&FrontendChapterStateSnapshot {
                open_chapter_title: Some("第三章".to_string()),
                open_chapter_revision: Some("abc".to_string()),
                dirty: false,
            }),
        );
        assert!(matches!(decision, SaveDecision::WriteTarget));
    }

    #[test]
    fn rejects_dirty_open_target_chapter_without_writing() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "abc",
            Some(&FrontendChapterStateSnapshot {
                open_chapter_title: Some("第三章".to_string()),
                open_chapter_revision: Some("abc".to_string()),
                dirty: true,
            }),
        );
        match decision {
            SaveDecision::Conflict(conflict) => {
                assert_eq!(conflict.reason, "frontend_dirty_open_chapter");
            }
            _ => panic!("expected conflict"),
        }
    }

    #[test]
    fn rejects_revision_mismatch_without_writing() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "def",
            None,
        );
        match decision {
            SaveDecision::Conflict(conflict) => {
                assert_eq!(conflict.reason, "revision_mismatch");
            }
            _ => panic!("expected conflict"),
        }
    }

    #[test]
    fn saves_draft_copy_on_conflict_only_when_requested() {
        let decision = decide_save_action(
            "第三章",
            "request-abcdef",
            SaveMode::SaveAsDraft,
            "abc",
            "def",
            None,
        );
        match decision {
            SaveDecision::WriteDraft {
                draft_title,
                conflict,
            } => {
                assert!(draft_title.contains("第三章 draft"));
                assert_eq!(conflict.reason, "revision_mismatch");
            }
            _ => panic!("expected draft decision"),
        }
    }

    #[test]
    fn rejects_empty_generated_content_with_content_empty() {
        let err = validate_generated_content("  ").unwrap_err();
        assert_eq!(err.code, "MODEL_OUTPUT_EMPTY");
    }

    #[test]
    fn maps_http_429_to_provider_rate_limited() {
        let err = map_provider_error("API error 429: too many requests".to_string());
        assert_eq!(err.code, "PROVIDER_RATE_LIMITED");
    }

    #[test]
    fn provider_budget_error_preserves_report_evidence() {
        let target = ChapterTarget {
            title: "第三章".to_string(),
            filename: "第三章.md".to_string(),
            number: Some(3),
            summary: "林墨发现密道。".to_string(),
            status: "empty".to_string(),
        };
        let receipt = build_chapter_generation_receipt(
            "budget-test-1",
            &target,
            "rev-1",
            "写第三章。",
            &[ChapterContextSource {
                source_type: "instruction".to_string(),
                id: "user-instruction".to_string(),
                label: "User instruction".to_string(),
                original_chars: 5,
                included_chars: 5,
                truncated: false,
                score: None,
            }],
            10,
        );
        let report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
            WriterProviderBudgetTask::ChapterGeneration,
            "gpt-4o",
            90_000,
            24_000,
        ));

        let error = provider_budget_error("budget-test-1", &receipt, report);

        assert_eq!(error.code, "PROVIDER_BUDGET_APPROVAL_REQUIRED");
        let evidence = error.evidence.expect("budget error has evidence");
        assert_eq!(evidence.category, WriterFailureCategory::ProviderFailed);
        assert!(evidence.details.get("providerBudget").is_some());
        assert!(!evidence.remediation.is_empty());
    }
}
