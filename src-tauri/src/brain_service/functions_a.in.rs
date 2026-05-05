pub fn cross_reference_project_brain_nodes(
    app: &tauri::AppHandle,
    source_node_id: &str,
    target_node_id: &str,
) -> Result<ProjectBrainCrossReferenceResult, String> {
    let source_node_id = source_node_id.trim();
    let target_node_id = target_node_id.trim();
    if source_node_id.is_empty() || target_node_id.is_empty() {
        return Err("Both source and target node IDs are required for cross-reference".to_string());
    }
    if source_node_id == target_node_id {
        return Err("Cannot cross-reference a node with itself".to_string());
    }

    let index = load_project_brain_knowledge_index(app)?;
    let source = index
        .nodes
        .iter()
        .find(|node| node.id == source_node_id)
        .ok_or_else(|| {
            format!(
                "Source node '{}' not found in knowledge index",
                source_node_id
            )
        })?;
    let target = index
        .nodes
        .iter()
        .find(|node| node.id == target_node_id)
        .ok_or_else(|| {
            format!(
                "Target node '{}' not found in knowledge index",
                target_node_id
            )
        })?;

    let shared_keywords: Vec<String> = source
        .keywords
        .iter()
        .filter(|keyword| target.keywords.contains(keyword))
        .cloned()
        .collect();

    let confidence = if shared_keywords.len() >= 3 {
        0.85
    } else if !shared_keywords.is_empty() {
        0.55
    } else {
        0.25
    };

    let relation = if source.kind == target.kind {
        "extends"
    } else if shared_keywords.len() >= 2 {
        "supports"
    } else if shared_keywords.is_empty() {
        "references"
    } else {
        "relates_to"
    };

    let suggested_action = if confidence >= 0.7 {
        format!(
            "Strong cross-reference: '{}' {} '{}' ({} shared keywords). Consider linking in context.",
            source.label,
            relation,
            target.label,
            shared_keywords.len()
        )
    } else if confidence >= 0.4 {
        format!(
            "Weak cross-reference: '{}' {} '{}'. Review connection before linking.",
            source.label, relation, target.label
        )
    } else {
        format!(
            "Minimal cross-reference: '{}' and '{}' share little evidence. Manual review recommended.",
            source.label, target.label
        )
    };

    let reference = ProjectBrainCrossReference {
        source_node_id: source_node_id.to_string(),
        target_node_id: target_node_id.to_string(),
        relation: relation.to_string(),
        confidence,
        evidence_keywords: shared_keywords.clone(),
        created_at_ms: crate::agent_runtime::now_ms(),
    };

    Ok(ProjectBrainCrossReferenceResult {
        reference,
        source_label: source.label.clone(),
        target_label: target.label.clone(),
        shared_keywords,
        suggested_action,
    })
}

pub fn ingest_external_research_source(
    app: &tauri::AppHandle,
    provider: &str,
    url_or_path: &str,
    title: &str,
    content: &str,
    author_approved: bool,
    approval_reason: &str,
) -> Result<ExternalResearchIngestResult, String> {
    let provider = provider.trim();
    let url_or_path = url_or_path.trim();
    let title = title.trim();
    let content = content.trim();
    let approval_reason = approval_reason.trim();

    if provider.is_empty() || title.is_empty() || content.is_empty() {
        return Err("External research provider, title, and content are all required".to_string());
    }
    validate_external_research_ingest_approval(author_approved, approval_reason)?;

    let revision = crate::storage::content_revision(content);
    let source_ref = format!("external:{}:{}", provider, revision);
    let source_kind = "external_research";

    let content_char_count = content.chars().count();
    let snippet_chars = content_char_count.min(480);
    let content_snippet: String = content.chars().take(snippet_chars).collect();

    let source = ExternalResearchSource {
        provider: provider.to_string(),
        url_or_path: url_or_path.to_string(),
        title: title.to_string(),
        content_snippet,
        relevance_score: 0.7,
        source_kind: source_kind.to_string(),
        ingestion_mode: "manual_author_approved".to_string(),
        author_approved,
    };

    let brain_path = storage::brain_path(app)?;
    let mut brain = VectorDB::load(&brain_path).map_err(|error| {
        format!(
            "Project Brain index at '{}' is unreadable: {}",
            brain_path.display(),
            error
        )
    })?;

    let chunks: Vec<String> = content
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.trim().to_string())
        .collect();
    let chunk_count = chunks.len();
    let mut node_ids = Vec::new();

    for (idx, chunk_text) in chunks.iter().enumerate() {
        let node_id = format!("{}:chunk:{}", source_ref, idx);
        let keywords: Vec<String> = chunk_text
            .split_whitespace()
            .filter(|w| w.chars().count() >= 2)
            .take(8)
            .map(|w| w.to_string())
            .collect();
        brain.chunks.push(Chunk {
            id: node_id.clone(),
            chapter: title.to_string(),
            text: chunk_text.clone(),
            embedding: Vec::new(),
            keywords: keywords.clone(),
            topic: None,
            source_ref: Some(source_ref.clone()),
            source_revision: Some(revision.clone()),
            source_kind: Some(source_kind.to_string()),
            chunk_index: Some(idx),
            archived: false,
        });

        node_ids.push(node_id);
    }

    let json = serde_json::to_string_pretty(&brain.chunks).map_err(|e| e.to_string())?;
    storage::atomic_write(&brain_path, &json)?;
    rebuild_project_brain_knowledge_index(app)?;

    let evidence_refs = node_ids
        .iter()
        .map(|id| format!("knowledge_node:{}", id))
        .collect();

    let result = ExternalResearchIngestResult {
        source,
        chunk_count,
        node_ids,
        evidence_refs,
        created_at_ms: crate::agent_runtime::now_ms(),
    };

    Ok(result)
}

pub fn validate_external_research_ingest_approval(
    author_approved: bool,
    approval_reason: &str,
) -> Result<(), String> {
    if !author_approved {
        return Err(
            "External research ingestion writes to Project Brain and requires explicit author approval"
                .to_string(),
        );
    }
    if approval_reason.trim().is_empty() {
        return Err("External research ingestion requires an author approval reason".to_string());
    }
    Ok(())
}

pub fn project_brain_embedding_profile(
    settings: &llm_runtime::LlmSettings,
) -> ProjectBrainEmbeddingProviderProfile {
    project_brain_embedding_profile_from_config(
        &settings.api_base,
        &settings.embedding_model,
        settings.embedding_input_limit_chars,
    )
}

pub fn project_brain_embedding_profile_from_config(
    api_base: &str,
    embedding_model: &str,
    input_limit_chars: usize,
) -> ProjectBrainEmbeddingProviderProfile {
    resolve_project_brain_embedding_profile(api_base, embedding_model, Some(input_limit_chars))
}

pub fn resolve_project_brain_embedding_profile(
    api_base: &str,
    embedding_model: &str,
    input_limit_chars: Option<usize>,
) -> ProjectBrainEmbeddingProviderProfile {
    let registry = project_brain_embedding_provider_registry();
    let provider_spec = registry_provider_for_api_base(&registry, api_base);
    let model_spec = registry_model_for_name(&registry, provider_spec, embedding_model);
    let provider_status = if provider_spec.is_some() {
        ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    } else {
        ProjectBrainEmbeddingRegistryStatus::CompatibilityFallback
    };
    let model_status = if model_spec.is_some() {
        ProjectBrainEmbeddingRegistryStatus::RegistryKnown
    } else {
        ProjectBrainEmbeddingRegistryStatus::CompatibilityFallback
    };
    let provider_id = provider_spec
        .map(|provider| provider.provider_id.clone())
        .unwrap_or_else(|| registry.fallback_provider_id.clone());
    let dimensions = model_spec
        .map(|model| model.dimensions)
        .unwrap_or(registry.fallback_dimensions);
    let input_limit_chars = input_limit_chars
        .filter(|limit| *limit > 0)
        .unwrap_or_else(|| {
            provider_spec
                .map(|provider| provider.default_input_limit_chars)
                .unwrap_or(registry.fallback_input_limit_chars)
        });
    let batch_limit = provider_spec
        .map(|provider| provider.batch_limit)
        .unwrap_or(registry.fallback_batch_limit);
    let retry_limit = provider_spec
        .map(|provider| provider.retry_limit)
        .unwrap_or(registry.fallback_retry_limit);

    ProjectBrainEmbeddingProviderProfile {
        provider_id,
        model: embedding_model.to_string(),
        dimensions,
        input_limit_chars,
        batch_limit,
        retry_limit,
        provider_status,
        model_status,
    }
}

pub fn project_brain_embedding_provider_registry() -> ProjectBrainEmbeddingProviderRegistry {
    let openai_models = openai_embedding_model_specs();
    ProjectBrainEmbeddingProviderRegistry {
        providers: vec![
            ProjectBrainEmbeddingProviderSpec {
                provider_id: "openai".to_string(),
                api_base_markers: vec!["api.openai.com".to_string()],
                default_input_limit_chars: DEFAULT_EMBEDDING_INPUT_LIMIT_CHARS,
                batch_limit: 16,
                retry_limit: 1,
                models: openai_models.clone(),
            },
            ProjectBrainEmbeddingProviderSpec {
                provider_id: "openrouter".to_string(),
                api_base_markers: vec!["openrouter.ai".to_string()],
                default_input_limit_chars: DEFAULT_EMBEDDING_INPUT_LIMIT_CHARS,
                batch_limit: 16,
                retry_limit: 1,
                models: openai_models.clone(),
            },
            ProjectBrainEmbeddingProviderSpec {
                provider_id: "local-openai-compatible".to_string(),
                api_base_markers: vec![
                    "localhost".to_string(),
                    "127.0.0.1".to_string(),
                    "[::1]".to_string(),
                ],
                default_input_limit_chars: 4_000,
                batch_limit: 8,
                retry_limit: 0,
                models: openai_models,
            },
        ],
        fallback_provider_id: "openai-compatible".to_string(),
        fallback_dimensions: DEFAULT_EMBEDDING_DIMENSIONS,
        fallback_input_limit_chars: DEFAULT_EMBEDDING_INPUT_LIMIT_CHARS,
        fallback_batch_limit: 8,
        fallback_retry_limit: 0,
    }
}

pub fn trim_embedding_input(input: &str, limit: usize) -> (String, bool) {
    let trimmed = input.trim();
    if trimmed.chars().count() <= limit {
        return (trimmed.to_string(), false);
    }
    let mut output = trimmed.chars().take(limit).collect::<String>();
    while output.ends_with(char::is_whitespace) {
        output.pop();
    }
    (output, true)
}

pub async fn embed_project_brain_text(
    settings: &llm_runtime::LlmSettings,
    input: &str,
    timeout_secs: u64,
) -> Result<Vec<f32>, String> {
    let profile = project_brain_embedding_profile(settings);
    let (input, _) = trim_embedding_input(input, profile.input_limit_chars);
    if input.trim().is_empty() {
        return Err("Project Brain embedding input is empty".to_string());
    }
    let embedding =
        embed_project_brain_input_with_retry(settings, &profile, &input, timeout_secs).await?;
    validate_embedding_dimensions(&profile, &embedding)?;
    Ok(embedding)
}

pub async fn embed_chapter(
    app: &tauri::AppHandle,
    settings: &llm_runtime::LlmSettings,
    chapter_title: &str,
    content: &str,
) -> Result<(), String> {
    let chunks = chunk_text(content, CHUNK_MAX_CHARS);
    if chunks.is_empty() {
        return Ok(());
    }

    let (embedded_chunks, report) =
        embed_project_brain_chunks(settings, chapter_title, &chunks, 30).await;
    if !matches!(report.status, ProjectBrainEmbeddingBatchStatus::Complete) {
        tracing::warn!(
            "Project Brain embedding batch for '{}' finished with {:?}: embedded={} skipped={} truncated={} errors={:?}",
            chapter_title,
            report.status,
            report.embedded_count,
            report.skipped_count,
            report.truncated_count,
            report.errors
        );
    }

    if embedded_chunks.is_empty() {
        return Ok(());
    }

    let path = storage::brain_path(app)?;
    let mut db = VectorDB::load(&path).map_err(|e| {
        format!(
            "Project Brain index at '{}' is unreadable; restore a backup or rebuild the index: {}",
            path.display(),
            e
        )
    })?;
    let active_revision = embedded_chunks
        .first()
        .and_then(|chunk| chunk.source_revision.as_deref())
        .unwrap_or_default()
        .to_string();
    db.archive_chapter_revision(chapter_title, &active_revision);
    for chunk in embedded_chunks {
        db.upsert(chunk);
    }

    db.save(&path)
}

pub async fn embed_project_brain_chunks(
    settings: &llm_runtime::LlmSettings,
    chapter_title: &str,
    chunks: &[(String, Vec<String>, Option<String>)],
    timeout_secs: u64,
) -> (Vec<Chunk>, ProjectBrainEmbeddingBatchReport) {
    let source_revision = storage::content_revision(
        &chunks
            .iter()
            .map(|(chunk_text, _, _)| chunk_text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    let source_ref = format!("chapter:{}", chapter_title);
    let profile = project_brain_embedding_profile(settings);
    let mut embedded_chunks = Vec::new();
    let mut report = ProjectBrainEmbeddingBatchReport {
        profile: profile.clone(),
        requested_count: chunks.len(),
        embedded_count: 0,
        skipped_count: 0,
        truncated_count: 0,
        status: ProjectBrainEmbeddingBatchStatus::Empty,
        errors: Vec::new(),
    };

    for (i, (chunk_text, keywords, topic)) in chunks.iter().enumerate() {
        if chunk_text.trim().chars().count() < MIN_CHUNK_CHARS {
            report.skipped_count += 1;
            continue;
        }
        let (limited_text, truncated) = trim_embedding_input(chunk_text, profile.input_limit_chars);
        if truncated {
            report.truncated_count += 1;
        }

        let embedding = match embed_project_brain_input_with_retry(
            settings,
            &profile,
            &limited_text,
            timeout_secs,
        )
        .await
        {
            Ok(embedding) => embedding,
            Err(error) => {
                report.skipped_count += 1;
                report.errors.push(format!(
                    "{}#{} embed request failed: {}",
                    chapter_title, i, error
                ));
                continue;
            }
        };
        if let Err(error) = validate_embedding_dimensions(&profile, &embedding) {
            report.skipped_count += 1;
            report.errors.push(format!(
                "{}#{} invalid embedding: {}",
                chapter_title, i, error
            ));
            continue;
        }

        embedded_chunks.push(Chunk {
            id: format!("{}-{}-{}", chapter_title, source_revision, i),
            chapter: chapter_title.to_string(),
            text: limited_text,
            embedding,
            keywords: keywords.clone(),
            topic: topic.clone(),
            source_ref: Some(source_ref.clone()),
            source_revision: Some(source_revision.clone()),
            source_kind: Some("chapter".to_string()),
            chunk_index: Some(i),
            archived: false,
        });
        report.embedded_count += 1;
    }

    report.status = project_brain_embedding_batch_status(
        report.requested_count,
        report.embedded_count,
        report.skipped_count,
        &report.errors,
    );

    (embedded_chunks, report)
}

pub fn knowledge_index_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(storage::active_project_data_dir(app)?.join(KNOWLEDGE_INDEX_FILENAME))
}

pub fn rebuild_project_brain_knowledge_index(
    app: &tauri::AppHandle,
) -> Result<ProjectBrainKnowledgeIndex, String> {
    let project_id = storage::active_project_id(app)?;
    let brain_path = storage::brain_path(app)?;
    let brain = VectorDB::load(&brain_path).map_err(|e| {
        format!(
            "Project Brain index at '{}' is unreadable; restore a backup or rebuild the index: {}",
            brain_path.display(),
            e
        )
    })?;
    let outline = storage::load_outline(app)?;
    let lorebook = storage::load_lorebook(app)?;
    let index = build_project_brain_knowledge_index(&project_id, &brain, &outline, &lorebook);
    save_project_brain_knowledge_index(app, &index)?;
    Ok(index)
}

pub fn load_project_brain_knowledge_index(
    app: &tauri::AppHandle,
) -> Result<ProjectBrainKnowledgeIndex, String> {
    let path = knowledge_index_path(app)?;
    if !path.exists() {
        return rebuild_project_brain_knowledge_index(app);
    }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read knowledge index '{}': {}", path.display(), e))?;
    serde_json::from_str(&data).map_err(|e| {
        format!(
            "Failed to parse knowledge index '{}': {}",
            path.display(),
            e
        )
    })
}

pub fn save_project_brain_knowledge_index(
    app: &tauri::AppHandle,
    index: &ProjectBrainKnowledgeIndex,
) -> Result<(), String> {
    let path = knowledge_index_path(app)?;
    let json = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    storage::atomic_write(&path, &json)
}

pub fn compare_project_brain_source_revisions(
    app: &tauri::AppHandle,
    source_ref: &str,
) -> Result<ProjectBrainSourceCompare, String> {
    let source_ref = source_ref.trim();
    if source_ref.is_empty() {
        return Err("Project Brain source ref is required for revision compare".to_string());
    }

    let brain_path = storage::brain_path(app)?;
    let brain = VectorDB::load(&brain_path).map_err(|e| {
        format!(
            "Project Brain index at '{}' is unreadable; restore a backup or rebuild the index: {}",
            brain_path.display(),
            e
        )
    })?;
    Ok(compare_project_brain_source_revisions_from_db(
        source_ref, &brain,
    ))
}

pub fn restore_project_brain_source_revision(
    app: &tauri::AppHandle,
    source_ref: &str,
    revision: &str,
) -> Result<ProjectBrainSourceRevisionRestore, String> {
    let source_ref = source_ref.trim();
    let revision = revision.trim();
    if source_ref.is_empty() {
        return Err("Project Brain source ref is required for revision restore".to_string());
    }
    if revision.is_empty() {
        return Err("Project Brain source revision is required for revision restore".to_string());
    }

    let brain_path = storage::brain_path(app)?;
    let mut brain = VectorDB::load(&brain_path).map_err(|e| {
        format!(
            "Project Brain index at '{}' is unreadable; restore a backup or rebuild the index: {}",
            brain_path.display(),
            e
        )
    })?;
    let report = restore_project_brain_source_revision_in_db(source_ref, revision, &mut brain)?;
    let json = serde_json::to_string_pretty(&brain.chunks).map_err(|e| e.to_string())?;
    storage::atomic_write(&brain_path, &json)?;
    rebuild_project_brain_knowledge_index(app)?;
    Ok(report)
}

pub fn restore_project_brain_source_revision_in_db(
    source_ref: &str,
    revision: &str,
    brain: &mut VectorDB,
) -> Result<ProjectBrainSourceRevisionRestore, String> {
    let source_ref = source_ref.trim();
    let revision = revision.trim();
    if source_ref.is_empty() {
        return Err("Project Brain source ref is required for revision restore".to_string());
    }
    if revision.is_empty() {
        return Err("Project Brain source revision is required for revision restore".to_string());
    }

    let mut source_kind = "unknown".to_string();
    let mut previous_active_revisions = BTreeSet::new();
    let mut has_requested_revision = false;
    let mut total_source_chunk_count = 0usize;
    let mut active_chunk_count = 0usize;
    let mut archived_chunk_count = 0usize;
    let mut changed_chunk_count = 0usize;

    for chunk in &brain.chunks {
        if chunk.source_ref.as_deref() != Some(source_ref) {
            continue;
        }
        total_source_chunk_count += 1;
        if let Some(kind) = chunk
            .source_kind
            .as_deref()
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
        {
            source_kind = kind.to_string();
        }
        if !chunk.archived {
            if let Some(active_revision) = chunk
                .source_revision
                .as_deref()
                .map(str::trim)
                .filter(|active_revision| !active_revision.is_empty())
            {
                previous_active_revisions.insert(active_revision.to_string());
            }
        }
        if chunk.source_revision.as_deref().map(str::trim) == Some(revision) {
            has_requested_revision = true;
        }
    }

    if total_source_chunk_count == 0 {
        return Err(format!(
            "Project Brain source '{}' has no indexed chunks to restore",
            source_ref
        ));
    }
    if !has_requested_revision {
        return Err(format!(
            "Project Brain source '{}' has no revision '{}'",
            source_ref, revision
        ));
    }

    for chunk in &mut brain.chunks {
        if chunk.source_ref.as_deref() != Some(source_ref) {
            continue;
        }
        let should_archive = chunk.source_revision.as_deref().map(str::trim) != Some(revision);
        if chunk.archived != should_archive {
            changed_chunk_count += 1;
            chunk.archived = should_archive;
        }
        if chunk.archived {
            archived_chunk_count += 1;
        } else {
            active_chunk_count += 1;
        }
    }

    Ok(ProjectBrainSourceRevisionRestore {
        source_ref: source_ref.to_string(),
        source_kind,
        restored_revision: revision.to_string(),
        previous_active_revisions: previous_active_revisions.into_iter().collect(),
        changed_chunk_count,
        active_chunk_count,
        archived_chunk_count,
        total_source_chunk_count,
        evidence_refs: vec![
            format!("source_ref:{}", source_ref),
            format!("source_revision:{}", revision),
        ],
    })
}

pub fn compare_project_brain_source_revisions_from_db(
    source_ref: &str,
    brain: &VectorDB,
) -> ProjectBrainSourceCompare {
    #[derive(Default)]
    struct RevisionAccumulator {
        active: bool,
        node_count: usize,
        chunk_count: usize,
        chunk_indexes: Vec<usize>,
        keywords: Vec<String>,
        summary_parts: Vec<String>,
    }

    let source_ref = source_ref.trim();
    let mut source_kind = "unknown".to_string();
    let mut by_revision = BTreeMap::<String, RevisionAccumulator>::new();
    for chunk in brain
        .chunks
        .iter()
        .filter(|chunk| chunk.source_ref.as_deref() == Some(source_ref))
    {
        if let Some(kind) = chunk
            .source_kind
            .as_deref()
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
        {
            source_kind = kind.to_string();
        }
        let revision = chunk
            .source_revision
            .as_deref()
            .map(str::trim)
            .filter(|revision| !revision.is_empty())
            .unwrap_or("unknown");
        let entry = by_revision.entry(revision.to_string()).or_default();
        entry.node_count += 1;
        entry.chunk_count += 1;
        if !chunk.archived {
            entry.active = true;
        }
        if let Some(chunk_index) = chunk.chunk_index {
            entry.chunk_indexes.push(chunk_index);
        }
        entry.keywords.extend(chunk.keywords.iter().cloned());
        if !chunk.text.trim().is_empty() {
            entry.summary_parts.push(chunk.text.clone());
        }
    }

    let mut revisions = by_revision
        .into_iter()
        .map(|(revision, mut entry)| {
            entry.chunk_indexes.sort_unstable();
            entry.chunk_indexes.dedup();
            let summary = snippet_text(
                &entry
                    .summary_parts
                    .iter()
                    .map(|part| part.trim())
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n"),
                360,
            );
            ProjectBrainSourceCompareRevision {
                revision,
                active: entry.active,
                node_count: entry.node_count,
                chunk_count: entry.chunk_count,
                chunk_indexes: entry.chunk_indexes,
                keywords: normalized_limited_keywords(entry.keywords, 16),
                summary,
            }
        })
        .collect::<Vec<_>>();
    revisions.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| left.revision.cmp(&right.revision))
    });

    let active_revision = revisions
        .iter()
        .find(|revision| revision.active)
        .map(|revision| revision.revision.clone());
    let active_keywords = revisions
        .iter()
        .find(|revision| revision.active)
        .map(|revision| normalized_keyword_set(&revision.keywords))
        .unwrap_or_default();
    let archived_keywords = revisions
        .iter()
        .filter(|revision| !revision.active)
        .flat_map(|revision| revision.keywords.iter().cloned())
        .collect::<Vec<_>>();
    let archived_keywords = normalized_keyword_set(&archived_keywords);

    let added_keywords = active_keywords
        .difference(&archived_keywords)
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    let removed_keywords = archived_keywords
        .difference(&active_keywords)
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    let shared_keywords = active_keywords
        .intersection(&archived_keywords)
        .take(12)
        .cloned()
        .collect::<Vec<_>>();

    let active_summary = revisions
        .iter()
        .find(|revision| revision.active)
        .map(|revision| revision.summary.clone())
        .unwrap_or_default();
    let archived_summary = revisions
        .iter()
        .filter(|revision| !revision.active)
        .map(|revision| revision.summary.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    ProjectBrainSourceCompare {
        source_ref: source_ref.to_string(),
        source_kind,
        active_revision,
        revisions,
        added_keywords: added_keywords.into_iter().collect(),
        removed_keywords: removed_keywords.into_iter().collect(),
        shared_keywords: shared_keywords.into_iter().collect(),
        added_summary: compare_summary_terms(&active_summary, &archived_summary),
        removed_summary: compare_summary_terms(&archived_summary, &active_summary),
        evidence_refs: vec![format!("source_ref:{}", source_ref)],
    }
}

pub fn read_knowledge_index_file(
    project_data_dir: &Path,
    relative_path: &str,
) -> Result<String, String> {
    let path = safe_knowledge_index_file_path(project_data_dir, relative_path)?;
    std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "Failed to read knowledge index file '{}': {}",
            path.display(),
            e
        )
    })
}

