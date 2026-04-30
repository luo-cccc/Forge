use agent_harness_core::{
    chunk_text,
    vector_db::{Chunk, VectorDB},
};

use crate::{llm_runtime, storage};

const CHUNK_MAX_CHARS: usize = 500;
const MIN_CHUNK_CHARS: usize = 20;
const TOP_K: usize = 5;

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
    let mut db = VectorDB::load(&path).unwrap_or_else(|_| VectorDB::new());
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
    mut on_delta: impl FnMut(String) -> Result<llm_runtime::StreamControl, String>,
) -> Result<(), String> {
    let query_embedding = llm_runtime::embed(settings, query, 30)
        .await
        .map_err(|e| format!("Embed error: {}", e))?;

    let brain_path = storage::brain_path(app)?;
    let db = VectorDB::load(&brain_path).unwrap_or_else(|_| VectorDB::new());
    let results = db.search_hybrid(query, &query_embedding, TOP_K);
    let context = build_context(&results);

    let messages = vec![
        serde_json::json!({"role": "system", "content": format!(
            "You are an expert on this novel. Answer the user's question using ONLY the provided book excerpts. \
             If the excerpts don't contain relevant information, say so honestly.\n\nBook excerpts:\n{}",
            context
        )}),
        serde_json::json!({"role": "user", "content": query}),
    ];

    llm_runtime::stream_chat(settings, messages, 60, |content| on_delta(content)).await?;

    Ok(())
}

fn build_context(results: &[(f32, &Chunk)]) -> String {
    if results.is_empty() {
        return "No relevant chunks found in the book.".to_string();
    }

    results
        .iter()
        .enumerate()
        .map(|(i, (score, chunk))| {
            format!(
                "[Chunk {} · {} · score {:.3}]\n{}",
                i + 1,
                chunk.chapter,
                score,
                chunk.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
