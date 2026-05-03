use agent_harness_core::{
    chunk_text,
    vector_db::{Chunk, VectorDB},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::writer_agent::context_relevance::{
    format_text_chunk_relevance, score_text_for_writing_focus,
};
use crate::writer_agent::kernel::WriterAgentKernel;
use crate::{llm_runtime, storage};

pub use crate::storage::{LoreEntry, OutlineNode};

const CHUNK_MAX_CHARS: usize = 500;
const MIN_CHUNK_CHARS: usize = 20;
const TOP_K: usize = 5;
const RERANK_CANDIDATE_MULTIPLIER: usize = 6;
const KNOWLEDGE_INDEX_FILENAME: &str = "knowledge_index.json";

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

    let mut embedded_chunks = Vec::new();

    for (i, (chunk_text, keywords, topic)) in chunks.iter().enumerate() {
        if chunk_text.trim().len() < MIN_CHUNK_CHARS {
            continue;
        }

        let embedding = match llm_runtime::embed(settings, chunk_text, 30).await {
            Ok(embedding) => embedding,
            Err(e) => {
                tracing::warn!(
                    "Skipping Project Brain chunk embedding for '{}#{}': {}",
                    chapter_title,
                    i,
                    e
                );
                continue;
            }
        };

        embedded_chunks.push(Chunk {
            id: format!("{}-{}", chapter_title, i),
            chapter: chapter_title.to_string(),
            text: chunk_text.to_string(),
            embedding,
            keywords: keywords.clone(),
            topic: topic.clone(),
        });
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
        on_delta,
    )
    .await
}

pub async fn answer_query_with_focus(
    app: &tauri::AppHandle,
    settings: &llm_runtime::LlmSettings,
    query: &str,
    focus: &ProjectBrainFocus,
    on_delta: impl FnMut(String) -> Result<llm_runtime::StreamControl, String>,
) -> Result<(), String> {
    let search_text = focus.search_text();
    let query_embedding = llm_runtime::embed(settings, &search_text, 30)
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

    llm_runtime::stream_chat(settings, messages, 60, on_delta).await?;

    Ok(())
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
