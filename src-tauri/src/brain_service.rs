use agent_harness_core::{
    chunk_text,
    vector_db::{Chunk, VectorDB},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

use crate::writer_agent::context_relevance::{
    format_text_chunk_relevance, score_text_for_writing_focus,
};
use crate::writer_agent::kernel::WriterAgentKernel;
use crate::writer_agent::provider_budget::{
    apply_provider_budget_approval, evaluate_provider_budget, WriterProviderBudgetApproval,
    WriterProviderBudgetReport, WriterProviderBudgetRequest, WriterProviderBudgetTask,
};
use crate::writer_agent::task_receipt::{WriterFailureCategory, WriterFailureEvidenceBundle};
use crate::{llm_runtime, storage};

pub use crate::storage::{LoreEntry, OutlineNode};

const CHUNK_MAX_CHARS: usize = 500;
const MIN_CHUNK_CHARS: usize = 20;
const TOP_K: usize = 5;
const RERANK_CANDIDATE_MULTIPLIER: usize = 6;
const KNOWLEDGE_INDEX_FILENAME: &str = "knowledge_index.json";
const PROJECT_BRAIN_QUERY_OUTPUT_TOKENS: u64 = 4_096;
const DEFAULT_EMBEDDING_DIMENSIONS: usize = 1536;

#[derive(Debug, Clone)]
pub struct ProjectBrainFocus {
    query_text: String,
    memory_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrainKnowledgeIndex {
    pub project_id: String,
    pub nodes: Vec<ProjectBrainKnowledgeNode>,
    pub edges: Vec<ProjectBrainKnowledgeEdge>,
    pub source_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrainKnowledgeNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub source_ref: String,
    pub keywords: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrainKnowledgeEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectBrainEmbeddingBatchStatus {
    Complete,
    Partial,
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrainEmbeddingProviderProfile {
    pub provider_id: String,
    pub model: String,
    pub dimensions: usize,
    pub input_limit_chars: usize,
    pub batch_limit: usize,
    pub retry_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrainEmbeddingBatchReport {
    pub profile: ProjectBrainEmbeddingProviderProfile,
    pub requested_count: usize,
    pub embedded_count: usize,
    pub skipped_count: usize,
    pub truncated_count: usize,
    pub status: ProjectBrainEmbeddingBatchStatus,
    pub errors: Vec<String>,
}

impl ProjectBrainFocus {
    pub fn from_query(query: &str) -> Self {
        Self {
            query_text: query.trim().to_string(),
            memory_text: String::new(),
        }
    }

    pub fn from_kernel(query: &str, kernel: &WriterAgentKernel) -> Self {
        let mut memory_parts = Vec::new();
        if let Some(chapter) = kernel.active_chapter.as_deref() {
            memory_parts.push(format!("active chapter: {}", chapter));
            if let Ok(Some(mission)) = kernel
                .memory
                .get_chapter_mission(&kernel.project_id, chapter)
            {
                memory_parts.push(mission.render_for_context());
            }
        }
        let recent_results = kernel
            .memory
            .list_recent_chapter_results(&kernel.project_id, 4)
            .unwrap_or_default();
        if let Some(next_beat) = crate::writer_agent::kernel_chapters::derive_next_beat(
            kernel.active_chapter.as_deref(),
            None,
            &recent_results,
            &kernel
                .memory
                .get_open_promise_summaries()
                .unwrap_or_default(),
        ) {
            memory_parts.push(next_beat.render_for_context());
        }
        for result in recent_results.iter().take(2) {
            memory_parts.push(result.render_for_context());
        }
        for decision in kernel.memory.list_recent_decisions(4).unwrap_or_default() {
            memory_parts.push(format!(
                "recent decision: {} {} {}",
                decision.title, decision.decision, decision.rationale
            ));
        }
        Self {
            query_text: query.trim().to_string(),
            memory_text: memory_parts
                .into_iter()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    pub fn as_str(&self) -> &str {
        if self.memory_text.trim().is_empty() {
            self.query_text.as_str()
        } else {
            self.memory_text.as_str()
        }
    }

    fn query_str(&self) -> &str {
        self.query_text.as_str()
    }

    fn memory_str(&self) -> &str {
        self.memory_text.as_str()
    }

    pub fn search_text(&self) -> String {
        [self.query_str(), self.memory_str()]
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
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
    ProjectBrainEmbeddingProviderProfile {
        provider_id: provider_id_from_api_base(api_base),
        model: embedding_model.to_string(),
        dimensions: embedding_dimensions_for_model(embedding_model),
        input_limit_chars,
        batch_limit: 16,
        retry_limit: 1,
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
    db.remove_chapter(chapter_title);
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
            id: format!("{}-{}", chapter_title, i),
            chapter: chapter_title.to_string(),
            text: limited_text,
            embedding,
            keywords: keywords.clone(),
            topic: topic.clone(),
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
        nodes.push(ProjectBrainKnowledgeNode {
            id: format!("chunk:{}", stable_node_id(&chunk.id, &chunk.chapter)),
            kind: "chunk".to_string(),
            label,
            source_ref: format!("project_brain:{}", chunk.id),
            keywords: unique_keywords(chunk.keywords.clone(), &chunk.text),
            summary: snippet_text(&chunk.text, 220),
        });
    }

    let edges = build_knowledge_edges(&nodes);
    ProjectBrainKnowledgeIndex {
        project_id: project_id.to_string(),
        source_count: lorebook.len() + outline.len() + brain.chunks.len(),
        nodes,
        edges,
    }
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

fn provider_id_from_api_base(api_base: &str) -> String {
    let lower = api_base.to_ascii_lowercase();
    if lower.contains("openrouter") {
        "openrouter".to_string()
    } else if lower.contains("openai") {
        "openai".to_string()
    } else {
        "openai-compatible".to_string()
    }
}

fn embedding_dimensions_for_model(model: &str) -> usize {
    match model {
        "text-embedding-3-large" => 3072,
        "text-embedding-3-small" | "text-embedding-ada-002" => 1536,
        _ => DEFAULT_EMBEDDING_DIMENSIONS,
    }
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
        [
            format!("project_brain:{}", chunk.id),
            format!("chapter:{}", chunk.chapter),
        ]
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
        task_id.to_string(),
        report.task,
        report.model.clone(),
        "openai-compatible",
        true,
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
            let combined = memory_score * 1.8 + query_score * 0.45 + base_score * 0.1;
            let mut reasons = memory_reasons;
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
}
