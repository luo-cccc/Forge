use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub chapter: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub keywords: Vec<String>,
    pub topic: Option<String>,
}

pub struct VectorDB {
    pub chunks: Vec<Chunk>,
    avg_text_len: f32,
}

impl VectorDB {
    pub fn new() -> Self {
        Self {
            chunks: vec![],
            avg_text_len: 1.0,
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let chunks: Vec<Chunk> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        let avg_text_len = if chunks.is_empty() {
            1.0
        } else {
            chunks.iter().map(|c| c.text.split_whitespace().count() as f32).sum::<f32>()
                / chunks.len() as f32
        };
        Ok(Self {
            chunks,
            avg_text_len,
        })
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.chunks).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn upsert(&mut self, chunk: Chunk) {
        if let Some(idx) = self.chunks.iter().position(|c| c.id == chunk.id) {
            self.chunks[idx] = chunk;
        } else {
            self.chunks.push(chunk);
        }
        self.avg_text_len = self
            .chunks
            .iter()
            .map(|c| c.text.split_whitespace().count() as f32)
            .sum::<f32>()
            / self.chunks.len().max(1) as f32;
    }

    pub fn remove_chapter(&mut self, chapter: &str) {
        self.chunks.retain(|c| c.chapter != chapter);
        self.avg_text_len = if self.chunks.is_empty() {
            1.0
        } else {
            self.chunks
                .iter()
                .map(|c| c.text.split_whitespace().count() as f32)
                .sum::<f32>()
                / self.chunks.len() as f32
        };
    }

    // ── BM25 lexical scoring ────────────────────────────────────────
    fn bm25_score(&self, query: &str, chunk: &Chunk) -> f32 {
        let terms: Vec<&str> = query.split_whitespace().collect();
        let doc_count = self.chunks.len().max(1);
        let doc_len = chunk.text.split_whitespace().count() as f32;
        let k1 = 1.5;
        let b = 0.75;

        terms
            .iter()
            .map(|term| {
                let term_lower = term.to_lowercase();
                let tf = chunk
                    .text
                    .to_lowercase()
                    .split_whitespace()
                    .filter(|w| *w == term_lower)
                    .count() as f32;
                if tf == 0.0 {
                    return 0.0;
                }
                let df = self
                    .chunks
                    .iter()
                    .filter(|c| {
                        c.text
                            .to_lowercase()
                            .split_whitespace()
                            .any(|w| w == term_lower)
                    })
                    .count()
                    .max(1) as f32;
                let idf = ((doc_count as f32 - df + 0.5) / (df + 0.5)).ln().max(0.0);
                idf * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * doc_len / self.avg_text_len))
            })
            .sum()
    }

    // ── Pure cosine (backward compat) ───────────────────────────────
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(f32, &Chunk)> {
        let mut scored: Vec<(f32, &Chunk)> = self
            .chunks
            .iter()
            .map(|c| (cosine_similarity(query_embedding, &c.embedding), c))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    // ── Hybrid: Sem(cosine) + Lex(BM25) + Sym(metadata match) ──────
    pub fn search_hybrid(
        &self,
        query: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Vec<(f32, &Chunk)> {
        let mut scored: Vec<(f32, &Chunk)> = self
            .chunks
            .iter()
            .map(|c| {
                let sem = cosine_similarity(query_embedding, &c.embedding);
                let lex = self.bm25_score(query, c) * 0.3; // BM25 权重 0.3
                let sym = if c.keywords.iter().any(|kw| {
                    query.to_lowercase().contains(&kw.to_lowercase())
                }) {
                    0.5
                } else {
                    0.0
                };
                (sem + lex + sym, c)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

pub fn extract_keywords(text: &str) -> Vec<String> {
    let stopwords: &[&str] = &["the", "a", "an", "is", "are", "was", "were", "be", "been",
        "being", "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "shall", "to", "of", "in", "for", "on", "with",
        "at", "by", "from", "as", "into", "through", "during", "before", "after",
        "above", "below", "between", "and", "but", "or", "nor", "not", "so", "yet",
        "both", "either", "neither", "each", "every", "all", "any", "few", "more",
        "most", "other", "some", "such", "no", "only", "own", "same", "than", "too",
        "very", "just", "because", "about", "over", "under", "again", "further",
        "then", "once", "here", "there", "when", "where", "why", "how", "which",
        "who", "whom", "this", "that", "these", "those", "it", "its", "he", "she",
        "they", "them", "their", "we", "us", "our", "i", "me", "my", "you", "your",
        "的","了","在","是","我","有","和","就","不","人","都","一","一个","上","也","很",
        "到","说","要","去","你","会","着","没有","看","好","自己","这","他","她","它","们"];
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| {
            w.len() >= 4 && !stopwords.contains(&w.as_str()) && seen.insert(w.clone())
        })
        .take(8)
        .collect()
}

pub fn chunk_text(text: &str, max_chars: usize) -> Vec<(String, Vec<String>, Option<String>)> {
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        if current.len() + trimmed.len() > max_chars && !current.is_empty() {
            let full = current.trim().to_string();
            let keywords = extract_keywords(&full);
            let topic = full.chars().take(30).collect::<String>();
            chunks.push((full, keywords, Some(topic)));
            current = String::new();
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(trimmed);
    }

    if !current.trim().is_empty() {
        let full = current.trim().to_string();
        let keywords = extract_keywords(&full);
        let topic = full.chars().take(30).collect::<String>();
        chunks.push((full, keywords, Some(topic)));
    }

    chunks
}
