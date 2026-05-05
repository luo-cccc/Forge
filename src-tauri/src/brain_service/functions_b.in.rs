pub fn safe_knowledge_index_file_path(
    project_data_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let requested = Path::new(relative_path);
    if requested.is_absolute()
        || requested
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "Knowledge index path must stay inside the active project: {}",
            relative_path
        ));
    }
    let joined = project_data_dir.join(requested);
    let root = project_data_dir
        .canonicalize()
        .unwrap_or_else(|_| project_data_dir.to_path_buf());
    let parent = joined
        .parent()
        .unwrap_or(project_data_dir)
        .canonicalize()
        .unwrap_or_else(|_| project_data_dir.to_path_buf());
    if !parent.starts_with(&root) {
        return Err(format!(
            "Knowledge index path escapes active project: {}",
            relative_path
        ));
    }
    Ok(joined)
}

pub fn project_brain_embedding_batch_status(
    requested_count: usize,
    embedded_count: usize,
    skipped_count: usize,
    errors: &[String],
) -> ProjectBrainEmbeddingBatchStatus {
    if embedded_count == 0 {
        ProjectBrainEmbeddingBatchStatus::Empty
    } else if embedded_count == requested_count && skipped_count == 0 && errors.is_empty() {
        ProjectBrainEmbeddingBatchStatus::Complete
    } else {
        ProjectBrainEmbeddingBatchStatus::Partial
    }
}

pub fn project_brain_source_revision(content: &str) -> String {
    storage::content_revision(content)
}

pub fn build_project_brain_knowledge_index(
    project_id: &str,
    brain: &VectorDB,
    outline: &[storage::OutlineNode],
    lorebook: &[storage::LoreEntry],
) -> ProjectBrainKnowledgeIndex {
    let mut nodes = Vec::new();

    for entry in lorebook {
        nodes.push(ProjectBrainKnowledgeNode {
            id: format!("lore:{}", stable_node_id(&entry.id, &entry.keyword)),
            kind: "lore".to_string(),
            label: entry.keyword.clone(),
            source_ref: format!("lorebook:{}", entry.id),
            source_revision: None,
            source_kind: Some("lorebook".to_string()),
            chunk_index: None,
            archived: false,
            keywords: unique_keywords(vec![entry.keyword.clone()], &entry.content),
            summary: snippet_text(&entry.content, 220),
        });
    }

    for node in outline {
        nodes.push(ProjectBrainKnowledgeNode {
            id: format!(
                "outline:{}",
                stable_node_id(&node.chapter_title, &node.summary)
            ),
            kind: "outline".to_string(),
            label: node.chapter_title.clone(),
            source_ref: format!("outline:{}", node.chapter_title),
            source_revision: None,
            source_kind: Some("outline".to_string()),
            chunk_index: None,
            archived: false,
            keywords: unique_keywords(vec![node.chapter_title.clone()], &node.summary),
            summary: snippet_text(&node.summary, 220),
        });
    }

    for chunk in &brain.chunks {
        let label = if chunk.chapter.trim().is_empty() {
            chunk.id.clone()
        } else {
            chunk.chapter.clone()
        };
        let source_ref = chunk
            .source_ref
            .clone()
            .unwrap_or_else(|| format!("project_brain:{}", chunk.id));
        nodes.push(ProjectBrainKnowledgeNode {
            id: format!("chunk:{}", stable_node_id(&chunk.id, &chunk.chapter)),
            kind: "chunk".to_string(),
            label,
            source_ref,
            source_revision: chunk.source_revision.clone(),
            source_kind: chunk.source_kind.clone(),
            chunk_index: chunk.chunk_index,
            archived: chunk.archived,
            keywords: unique_keywords(chunk.keywords.clone(), &chunk.text),
            summary: snippet_text(&chunk.text, 220),
        });
    }

    let edges = build_knowledge_edges(&nodes);
    let source_history = build_source_history(&nodes);
    ProjectBrainKnowledgeIndex {
        project_id: project_id.to_string(),
        source_count: lorebook.len() + outline.len() + brain.chunks.len(),
        nodes,
        edges,
        source_history,
    }
}

fn build_source_history(nodes: &[ProjectBrainKnowledgeNode]) -> Vec<ProjectBrainSourceHistory> {
    #[derive(Default)]
    struct SourceAccumulator {
        source_kind: Option<String>,
        revisions: BTreeMap<String, ProjectBrainSourceRevision>,
        node_count: usize,
        chunk_count: usize,
        active_revisions: HashSet<String>,
        latest_summary: String,
    }

    let mut by_source = BTreeMap::<String, SourceAccumulator>::new();
    for node in nodes {
        let source_ref = node.source_ref.trim();
        if source_ref.is_empty() {
            continue;
        }
        let entry = by_source.entry(source_ref.to_string()).or_default();
        entry.node_count += 1;
        if node.kind == "chunk" {
            entry.chunk_count += 1;
        }
        if entry.source_kind.is_none() {
            entry.source_kind = node
                .source_kind
                .clone()
                .filter(|kind| !kind.trim().is_empty())
                .or_else(|| Some(node.kind.clone()));
        }
        if !node.summary.trim().is_empty() {
            entry.latest_summary = node.summary.clone();
        }
        if let Some(revision) = node
            .source_revision
            .as_deref()
            .map(str::trim)
            .filter(|revision| !revision.is_empty())
        {
            if !node.archived {
                entry.active_revisions.insert(revision.to_string());
            }
            let revision_entry = entry
                .revisions
                .entry(revision.to_string())
                .or_insert_with(|| ProjectBrainSourceRevision {
                    revision: revision.to_string(),
                    node_count: 0,
                    chunk_indexes: Vec::new(),
                    active: false,
                });
            revision_entry.node_count += 1;
            if let Some(chunk_index) = node.chunk_index {
                revision_entry.chunk_indexes.push(chunk_index);
            }
        }
    }

    by_source
        .into_iter()
        .map(|(source_ref, entry)| {
            let mut revisions = entry.revisions.into_values().collect::<Vec<_>>();
            for revision in &mut revisions {
                revision.chunk_indexes.sort_unstable();
                revision.chunk_indexes.dedup();
                revision.active = entry.active_revisions.contains(&revision.revision);
            }
            ProjectBrainSourceHistory {
                source_ref,
                source_kind: entry.source_kind.unwrap_or_else(|| "unknown".to_string()),
                revisions,
                node_count: entry.node_count,
                chunk_count: entry.chunk_count,
                latest_summary: snippet_text(&entry.latest_summary, 220),
            }
        })
        .collect()
}

fn build_knowledge_edges(nodes: &[ProjectBrainKnowledgeNode]) -> Vec<ProjectBrainKnowledgeEdge> {
    let mut keyword_to_nodes = BTreeMap::<String, Vec<&ProjectBrainKnowledgeNode>>::new();
    for node in nodes {
        for keyword in &node.keywords {
            keyword_to_nodes
                .entry(keyword.to_string())
                .or_default()
                .push(node);
        }
    }

    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for (keyword, linked_nodes) in keyword_to_nodes {
        if linked_nodes.len() < 2 {
            continue;
        }
        for left in 0..linked_nodes.len() {
            for right in left + 1..linked_nodes.len() {
                let from = &linked_nodes[left].id;
                let to = &linked_nodes[right].id;
                let key = if from <= to {
                    format!("{}|{}|{}", from, to, keyword)
                } else {
                    format!("{}|{}|{}", to, from, keyword)
                };
                if !seen.insert(key) {
                    continue;
                }
                edges.push(ProjectBrainKnowledgeEdge {
                    from: from.clone(),
                    to: to.clone(),
                    relation: format!("shared_keyword:{}", keyword),
                    evidence_ref: keyword.clone(),
                });
            }
        }
    }
    edges
}

fn unique_keywords(mut seed: Vec<String>, text: &str) -> Vec<String> {
    seed.extend(agent_harness_core::extract_keywords(text));
    let mut seen = HashSet::new();
    seed.into_iter()
        .map(|keyword| keyword.trim().to_string())
        .filter(|keyword| keyword.chars().count() >= 2 && seen.insert(keyword.to_lowercase()))
        .take(12)
        .collect()
}

fn normalized_limited_keywords(seed: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    seed.into_iter()
        .flat_map(|keyword| unique_keywords(vec![keyword.clone()], &keyword))
        .map(|keyword| keyword.trim().to_string())
        .filter(|keyword| keyword.chars().count() >= 2 && seen.insert(keyword.to_lowercase()))
        .take(limit)
        .collect()
}

fn normalized_keyword_set(seed: &[String]) -> BTreeSet<String> {
    normalized_limited_keywords(seed.to_vec(), 64)
        .into_iter()
        .collect()
}

fn compare_summary_terms(primary: &str, baseline: &str) -> Vec<String> {
    let baseline_terms = normalized_keyword_set(&agent_harness_core::extract_keywords(baseline));
    normalized_limited_keywords(agent_harness_core::extract_keywords(primary), 24)
        .into_iter()
        .filter(|term| !baseline_terms.contains(term))
        .take(8)
        .collect()
}

fn stable_node_id(primary: &str, fallback: &str) -> String {
    let source = if primary.trim().is_empty() {
        fallback
    } else {
        primary
    };
    storage::content_revision(source)
        .split('-')
        .next()
        .unwrap_or("0000000000000000")
        .to_string()
}

fn snippet_text(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn openai_embedding_model_specs() -> Vec<ProjectBrainEmbeddingModelSpec> {
    vec![
        ProjectBrainEmbeddingModelSpec {
            model: "text-embedding-3-large".to_string(),
            dimensions: 3072,
        },
        ProjectBrainEmbeddingModelSpec {
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
        },
        ProjectBrainEmbeddingModelSpec {
            model: "text-embedding-ada-002".to_string(),
            dimensions: 1536,
        },
    ]
}

fn registry_provider_for_api_base<'a>(
    registry: &'a ProjectBrainEmbeddingProviderRegistry,
    api_base: &str,
) -> Option<&'a ProjectBrainEmbeddingProviderSpec> {
    let lower = api_base.to_ascii_lowercase();
    registry.providers.iter().find(|provider| {
        provider
            .api_base_markers
            .iter()
            .any(|marker| lower.contains(marker))
    })
}

fn registry_model_for_name<'a>(
    registry: &'a ProjectBrainEmbeddingProviderRegistry,
    provider: Option<&'a ProjectBrainEmbeddingProviderSpec>,
    model: &str,
) -> Option<&'a ProjectBrainEmbeddingModelSpec> {
    provider
        .and_then(|provider| provider.models.iter().find(|spec| spec.model == model))
        .or_else(|| {
            registry
                .providers
                .iter()
                .flat_map(|provider| provider.models.iter())
                .find(|spec| spec.model == model)
        })
}

fn validate_embedding_dimensions(
    profile: &ProjectBrainEmbeddingProviderProfile,
    embedding: &[f32],
) -> Result<(), String> {
    if embedding.is_empty() {
        return Err("embedding is empty".to_string());
    }
    if profile.dimensions > 0 && embedding.len() != profile.dimensions {
        return Err(format!(
            "expected {} dimensions from {}:{}, got {}",
            profile.dimensions,
            profile.provider_id,
            profile.model,
            embedding.len()
        ));
    }
    Ok(())
}

async fn embed_project_brain_input_with_retry(
    settings: &llm_runtime::LlmSettings,
    profile: &ProjectBrainEmbeddingProviderProfile,
    input: &str,
    timeout_secs: u64,
) -> Result<Vec<f32>, String> {
    let attempts = profile.retry_limit.saturating_add(1).max(1);
    let mut last_error = String::new();
    for attempt in 1..=attempts {
        match llm_runtime::embed(settings, input, timeout_secs).await {
            Ok(embedding) => return Ok(embedding),
            Err(error) => {
                last_error = error;
                if attempt < attempts {
                    tracing::warn!(
                        "Project Brain embedding attempt {}/{} failed for provider={} model={}",
                        attempt,
                        attempts,
                        profile.provider_id,
                        profile.model
                    );
                }
            }
        }
    }

    Err(format!(
        "Project Brain embedding failed after {} attempt(s): {}",
        attempts, last_error
    ))
}

pub async fn answer_query(
    app: &tauri::AppHandle,
    settings: &llm_runtime::LlmSettings,
    query: &str,
    on_delta: impl FnMut(String) -> Result<llm_runtime::StreamControl, String>,
) -> Result<(), String> {
    answer_query_with_focus(
        app,
        settings,
        query,
        &ProjectBrainFocus::from_query(query),
        None,
        on_delta,
    )
    .await
}

pub async fn answer_query_with_focus(
    app: &tauri::AppHandle,
    settings: &llm_runtime::LlmSettings,
    query: &str,
    focus: &ProjectBrainFocus,
    provider_budget_approval: Option<&WriterProviderBudgetApproval>,
    on_delta: impl FnMut(String) -> Result<llm_runtime::StreamControl, String>,
) -> Result<(), String> {
    let search_text = focus.search_text();
    let query_embedding = embed_project_brain_text(settings, &search_text, 30)
        .await
        .map_err(|e| format!("Embed error: {}", e))?;

    let brain_path = storage::brain_path(app)?;
    let db = VectorDB::load(&brain_path).map_err(|e| {
        format!(
            "Project Brain index at '{}' is unreadable; restore a backup or rebuild the index: {}",
            brain_path.display(),
            e
        )
    })?;
    let results = search_project_brain_results_with_focus(&db, focus, &query_embedding);
    let context = build_context(&results);

    let messages = vec![
        serde_json::json!({"role": "system", "content": format!(
            "You are an expert on this novel. Answer the user's question using ONLY the provided book excerpts. \
             If the excerpts don't contain relevant information, say so honestly.\n\nBook excerpts:\n{}",
            context
        )}),
        serde_json::json!({"role": "user", "content": query}),
    ];
    let budget_report = apply_provider_budget_approval(
        project_brain_query_provider_budget(settings, &messages),
        provider_budget_approval,
    );
    let created_at_ms = crate::agent_runtime::now_ms();
    let task_id = format!(
        "project-brain-query-{}",
        storage::content_revision(&format!("{}:{}", query, created_at_ms))
            .split('-')
            .next()
            .unwrap_or("0000000000000000")
    );
    let source_refs = project_brain_query_source_refs(query, &results, &budget_report);
    record_project_brain_provider_budget(
        app,
        &task_id,
        &budget_report,
        source_refs.clone(),
        created_at_ms,
    );
    if budget_report.approval_required {
        record_project_brain_budget_failure(
            app,
            task_id,
            source_refs,
            budget_report.clone(),
            created_at_ms,
        );
        emit_project_brain_provider_budget_error(app, &budget_report);
        return Err("PROJECT_BRAIN_PROVIDER_BUDGET_APPROVAL_REQUIRED".to_string());
    }
    record_project_brain_model_started(
        app,
        &task_id,
        &budget_report,
        source_refs,
        crate::agent_runtime::now_ms(),
    );

    llm_runtime::stream_chat(settings, messages, 60, on_delta).await?;

    Ok(())
}

pub fn project_brain_query_provider_budget(
    settings: &llm_runtime::LlmSettings,
    messages: &[serde_json::Value],
) -> WriterProviderBudgetReport {
    project_brain_query_provider_budget_for_model(settings.model.clone(), messages)
}

pub fn project_brain_query_provider_budget_for_model(
    model: impl Into<String>,
    messages: &[serde_json::Value],
) -> WriterProviderBudgetReport {
    let converted = messages
        .iter()
        .map(|message| agent_harness_core::provider::LlmMessage {
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
        WriterProviderBudgetTask::ProjectBrainQuery,
        model.into(),
        estimated_input_tokens,
        PROJECT_BRAIN_QUERY_OUTPUT_TOKENS,
    ))
}

fn project_brain_query_source_refs(
    query: &str,
    results: &[(f32, Vec<String>, &Chunk)],
    report: &WriterProviderBudgetReport,
) -> Vec<String> {
    let query_hash = storage::content_revision(query)
        .split('-')
        .next()
        .unwrap_or("0000000000000000")
        .to_string();
    let mut refs = vec![
        format!("project_brain_query:{}", query_hash),
        format!("model:{}", report.model),
        format!("estimated_tokens:{}", report.estimated_total_tokens),
        format!("estimated_cost_micros:{}", report.estimated_cost_micros),
    ];
    refs.extend(results.iter().flat_map(|(_, _, chunk)| {
        let mut refs = vec![
            format!("project_brain:{}", chunk.id),
            format!("chapter:{}", chunk.chapter),
        ];
        if let Some(source_ref) = chunk.source_ref.as_deref() {
            refs.push(format!("source_ref:{}", source_ref));
        }
        if let Some(source_revision) = chunk.source_revision.as_deref() {
            refs.push(format!("source_revision:{}", source_revision));
        }
        refs
    }));
    refs
}

fn record_project_brain_provider_budget(
    app: &tauri::AppHandle,
    task_id: &str,
    report: &WriterProviderBudgetReport,
    source_refs: Vec<String>,
    created_at_ms: u64,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    kernel.record_provider_budget_report(task_id.to_string(), report, source_refs, created_at_ms);
}

fn record_project_brain_model_started(
    app: &tauri::AppHandle,
    task_id: &str,
    report: &WriterProviderBudgetReport,
    source_refs: Vec<String>,
    created_at_ms: u64,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    kernel.record_model_started_run_event(
        ModelStartedEventContext {
            task_id: task_id.to_string(),
            task: report.task,
            model: report.model.clone(),
            provider: "openai-compatible".to_string(),
            stream: true,
        },
        source_refs,
        Some(report),
        created_at_ms,
    );
}

fn record_project_brain_budget_failure(
    app: &tauri::AppHandle,
    task_id: String,
    source_refs: Vec<String>,
    report: WriterProviderBudgetReport,
    created_at_ms: u64,
) {
    let state = app.state::<crate::AppState>();
    let Ok(mut kernel) = state.writer_kernel.lock() else {
        return;
    };
    let bundle = WriterFailureEvidenceBundle::new(
        WriterFailureCategory::ProviderFailed,
        "PROJECT_BRAIN_PROVIDER_BUDGET_APPROVAL_REQUIRED",
        "Project Brain answer provider budget requires explicit approval before calling the model.",
        true,
        Some(task_id),
        source_refs,
        serde_json::json!({ "providerBudget": report }),
        vec![
            "Surface the Project Brain token/cost estimate to the author before retrying."
                .to_string(),
            "Narrow the query or reduce Project Brain context if approval is not granted."
                .to_string(),
        ],
        created_at_ms,
    );
    kernel.record_failure_evidence_bundle(&bundle);
}

fn emit_project_brain_provider_budget_error(
    app: &tauri::AppHandle,
    report: &WriterProviderBudgetReport,
) {
    let _ = app.emit(
        crate::events::AGENT_ERROR,
        serde_json::json!({
            "message": "Project Brain provider budget requires explicit approval before calling the model.",
            "source": "provider_budget",
            "error": {
                "code": "PROJECT_BRAIN_PROVIDER_BUDGET_APPROVAL_REQUIRED",
                "message": "Project Brain provider budget requires explicit approval before calling the model.",
                "recoverable": true,
                "details": {
                    "providerBudget": report,
                },
            },
        }),
    );
}

pub fn rerank_project_brain_results<'a>(
    results: Vec<(f32, &'a Chunk)>,
    writing_focus: &str,
) -> Vec<(f32, Vec<String>, &'a Chunk)> {
    rerank_project_brain_results_with_focus(
        results,
        &ProjectBrainFocus {
            query_text: writing_focus.to_string(),
            memory_text: String::new(),
            active_chapter: None,
        },
    )
}

pub fn search_project_brain_results_with_focus<'a>(
    db: &'a VectorDB,
    focus: &ProjectBrainFocus,
    query_embedding: &[f32],
) -> Vec<(f32, Vec<String>, &'a Chunk)> {
    let search_text = focus.search_text();
    rerank_project_brain_results_with_focus(
        db.search_hybrid(
            &search_text,
            query_embedding,
            TOP_K * RERANK_CANDIDATE_MULTIPLIER,
        ),
        focus,
    )
}

pub fn rerank_project_brain_results_with_focus<'a>(
    results: Vec<(f32, &'a Chunk)>,
    focus: &ProjectBrainFocus,
) -> Vec<(f32, Vec<String>, &'a Chunk)> {
    let has_memory_focus = !focus.memory_str().trim().is_empty();
    let mut scored = results
        .into_iter()
        .map(|(base_score, chunk)| {
            let text = project_brain_chunk_text(chunk);
            let (query_score, query_reasons) =
                score_text_for_writing_focus(focus.query_str(), &text);
            if !has_memory_focus {
                return (base_score + query_score, query_reasons, chunk);
            }

            let (memory_score, memory_reasons) =
                score_text_for_writing_focus(focus.memory_str(), &text);
            let (chapter_score, chapter_reason) =
                project_brain_chapter_proximity_score(focus, chunk);
            let combined =
                memory_score * 1.8 + query_score * 0.45 + base_score * 0.1 + chapter_score;
            let mut reasons = Vec::new();
            if let Some(reason) = chapter_reason {
                reasons.push(reason);
            }
            for reason in memory_reasons {
                if reasons.len() >= 5 {
                    break;
                }
                if !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
            for reason in query_reasons {
                if reasons.len() >= 5 {
                    break;
                }
                if !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
            (combined, reasons, chunk)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(TOP_K).collect()
}

fn project_brain_chapter_proximity_score(
    focus: &ProjectBrainFocus,
    chunk: &Chunk,
) -> (f32, Option<String>) {
    let Some(active_chapter) = focus.active_chapter.as_deref() else {
        return (0.0, None);
    };
    let Some(active_number) = project_brain_chapter_number(active_chapter) else {
        return (0.0, None);
    };
    let Some(chunk_number) = project_brain_chapter_number(&chunk.chapter) else {
        return (0.0, None);
    };
    let distance = active_number.abs_diff(chunk_number);
    match distance {
        0 => (18.0, Some("chapter proximity current chapter".to_string())),
        1 => (12.0, Some("chapter proximity adjacent chapter".to_string())),
        2 | 3 => (6.0, Some("chapter proximity nearby chapter".to_string())),
        _ => (0.0, None),
    }
}

fn project_brain_chapter_number(chapter: &str) -> Option<u64> {
    let mut digits = String::new();
    let mut started = false;
    for ch in chapter.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            started = true;
        } else if started {
            break;
        }
    }
    digits.parse().ok()
}

fn project_brain_chunk_text(chunk: &Chunk) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        chunk.chapter,
        chunk.keywords.join("\n"),
        chunk.topic.clone().unwrap_or_default(),
        chunk.text
    )
}

fn build_context(results: &[(f32, Vec<String>, &Chunk)]) -> String {
    if results.is_empty() {
        return "No relevant chunks found in the book.".to_string();
    }

    results
        .iter()
        .enumerate()
        .map(|(i, (score, reasons, chunk))| {
            format!(
                "[Chunk {} · {} · score {:.3}]\n{}\n{}",
                i + 1,
                chunk.chapter,
                score,
                format_text_chunk_relevance(reasons),
                chunk.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_db_load_reports_corrupt_project_brain() {
        let path = std::env::temp_dir().join(format!(
            "forge-project-brain-bad-{}-{}.json",
            std::process::id(),
            crate::storage::content_revision("bad")
        ));
        std::fs::write(&path, "{bad json").unwrap();

        let err = match VectorDB::load(&path) {
            Ok(_) => panic!("corrupt project brain should fail to load"),
            Err(err) => err,
        };

        assert!(err.contains("expected"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn external_research_source_requires_author_approval() {
        let err = validate_external_research_ingest_approval(false, "author import")
            .expect_err("Project Brain ingest should require author approval");

        assert!(err.contains("requires explicit author approval"));
        assert!(
            validate_external_research_ingest_approval(true, "author approved source import")
                .is_ok()
        );
        assert!(validate_external_research_ingest_approval(true, "   ").is_err());
    }
}
