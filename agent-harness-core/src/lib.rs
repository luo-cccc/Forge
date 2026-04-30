pub mod actions;
pub mod config;
pub mod hermes_memory;
pub mod llm;
pub mod vector_db;

pub use actions::{parse_actions, Action};
pub use config::HarnessConfig;
pub use hermes_memory::HermesDB;
pub use llm::LLMClient;
pub use vector_db::{chunk_text, cosine_similarity, extract_keywords, Chunk, VectorDB};

/// 通用文本截断 — 取最后 max_chars 字符，从词边界断开
pub fn truncate_context(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        return text;
    }
    let start = text.len().saturating_sub(max_chars);
    let slice = &text[start..];
    if let Some(idx) = slice.find(' ') {
        &slice[idx + 1..]
    } else {
        slice
    }
}
