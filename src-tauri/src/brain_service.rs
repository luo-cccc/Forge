use agent_harness_core::{
    chunk_text,
    vector_db::{Chunk, VectorDB},
};

use crate::writer_agent::context_relevance::{
    format_text_chunk_relevance, score_text_for_writing_focus,
};
use crate::writer_agent::kernel::WriterAgentKernel;
use crate::{llm_runtime, storage};

const CHUNK_MAX_CHARS: usize = 500;
const MIN_CHUNK_CHARS: usize = 20;
const TOP_K: usize = 5;
const RERANK_CANDIDATE_MULTIPLIER: usize = 6;

#[derive(Debug, Clone)]
pub struct ProjectBrainFocus {
    query_text: String,
    memory_text: String,
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

    fn search_str(&self) -> String {
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
    let search_text = focus.search_str();
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
    let results = rerank_project_brain_results_with_focus(
        db.search_hybrid(
            &search_text,
            &query_embedding,
            TOP_K * RERANK_CANDIDATE_MULTIPLIER,
        ),
        focus,
    );
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
