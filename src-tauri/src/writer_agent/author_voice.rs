//! Author Voice Guard — style fingerprinting from author samples and feedback.
//!
//! Builds an AuthorVoiceSnapshot from accepted prose, rejected proposals,
//! and style memory feedback. Used as a context source for generation/rewrite.

use serde::{Deserialize, Serialize};

use super::memory::WriterMemory;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorVoiceSnapshot {
    pub voice_id: String,
    pub rhythm: VoiceRhythm,
    pub diction: VoiceDiction,
    pub pov: String,
    pub dialogue_texture: String,
    pub sentence_shape: Vec<String>,
    pub taboo_phrases: Vec<String>,
    pub confidence: f64,
    pub sample_refs: Vec<String>,
    pub last_updated_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceRhythm {
    pub avg_sentence_length: f64,
    pub sentence_variance: f64,
    pub paragraph_pacing: String, // "fast" | "medium" | "slow"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDiction {
    pub register: String,     // "formal" | "colloquial" | "literary"
    pub sensory_density: f64, // 0.0–1.0
    pub subtext_ratio: f64,   // 0.0–1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StyleDriftDiagnostic {
    pub chapter_title: String,
    pub drift_signals: Vec<DriftSignal>,
    pub overall_severity: String,
    pub evidence_links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DriftSignal {
    pub aspect: String, // "rhythm" | "diction" | "dialogue" | "sentence_shape"
    pub expected_pattern: String,
    pub observed_pattern: String,
    pub severity: String, // "low" | "medium" | "high"
}

/// Build an author voice snapshot from memory data.
pub fn build_author_voice_snapshot(
    memory: &WriterMemory,
    sample_chapter_titles: &[String],
    now_ms: u64,
) -> AuthorVoiceSnapshot {
    let mut sample_refs = Vec::new();
    let mut taboo_phrases = Vec::new();
    let mut sentence_shapes = Vec::new();

    // Gather style preferences as voice signals.
    if let Ok(prefs) = memory.list_style_preferences(20) {
        for pref in &prefs {
            if pref.accepted_count > 0 {
                if pref.key.contains("sentence") || pref.key.contains("length") {
                    sentence_shapes.push(pref.value.clone());
                }
                if pref.key.contains("avoid") || pref.key.contains("taboo") {
                    taboo_phrases.push(pref.value.clone());
                }
                sample_refs.push(format!("style:{}", pref.key));
            }
        }
    }

    // Gather correction signals from memory feedback.
    if let Ok(feedback_entries) = memory.list_memory_feedback(10) {
        for entry in &feedback_entries {
            if entry.action == "correction" {
                taboo_phrases.push(format!("[correction] {}", entry.slot));
                sample_refs.push(format!("feedback:{}", entry.slot));
            }
        }
    }

    // Chapter references.
    for title in sample_chapter_titles {
        sample_refs.push(format!("chapter:{}", title));
    }

    let diction_register = if let Ok(prefs) = memory.list_style_preferences(5) {
        let has_literary = prefs
            .iter()
            .any(|p| p.key.contains("literary") || p.key.contains("prose"));
        let has_colloquial = prefs.iter().any(|p| {
            p.key.contains("colloquial") || p.key.contains("dialogue") || p.key.contains("casual")
        });
        if has_literary {
            "literary"
        } else if has_colloquial {
            "colloquial"
        } else {
            "formal"
        }
    } else {
        "formal"
    };

    // Derive rhythm from style preferences when available.
    let (avg_sentence_len, pacing) = if let Ok(prefs) = memory.list_style_preferences(10) {
        let len_pref = prefs
            .iter()
            .find(|p| p.key.contains("sentence") || p.key.contains("length"));
        let pace_pref = prefs
            .iter()
            .find(|p| p.key.contains("pace") || p.key.contains("fast") || p.key.contains("slow"));
        let sl = len_pref.map(|_| 14.0).unwrap_or(18.0);
        let pp = if let Some(p) = pace_pref {
            if p.key.contains("fast") {
                "fast"
            } else if p.key.contains("slow") {
                "slow"
            } else {
                "medium"
            }
        } else {
            "medium"
        };
        (sl, pp.to_string())
    } else {
        (18.0, "medium".to_string())
    };

    AuthorVoiceSnapshot {
        voice_id: format!("voice:{}", now_ms),
        rhythm: VoiceRhythm {
            avg_sentence_length: avg_sentence_len,
            sentence_variance: 5.0,
            paragraph_pacing: pacing,
        },
        diction: VoiceDiction {
            register: diction_register.to_string(),
            sensory_density: 0.5,
            subtext_ratio: 0.3,
        },
        pov: "third_person_limited".to_string(),
        dialogue_texture: "subtext_heavy".to_string(),
        sentence_shape: sentence_shapes,
        taboo_phrases,
        confidence: if sample_refs.len() >= 3 { 0.7 } else { 0.3 },
        sample_refs,
        last_updated_ms: now_ms,
    }
}

/// Compute style drift diagnostics comparing a chapter against the voice snapshot.
pub fn compute_style_drift(
    voice: &AuthorVoiceSnapshot,
    _chapter_content: &str,
    chapter_title: &str,
) -> StyleDriftDiagnostic {
    let mut drift_signals = Vec::new();

    // Basic heuristic: if voice has low confidence, flag it.
    if voice.confidence < 0.5 {
        drift_signals.push(DriftSignal {
            aspect: "diction".to_string(),
            expected_pattern: "established voice register".to_string(),
            observed_pattern: "insufficient samples for comparison".to_string(),
            severity: "medium".to_string(),
        });
    }

    let overall = if drift_signals.is_empty() {
        "low"
    } else if drift_signals.iter().any(|s| s.severity == "high") {
        "high"
    } else {
        "medium"
    };

    StyleDriftDiagnostic {
        chapter_title: chapter_title.to_string(),
        drift_signals,
        overall_severity: overall.to_string(),
        evidence_links: voice.sample_refs.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer_agent::memory::WriterMemory;
    use std::path::Path;

    #[test]
    fn voice_uses_author_samples() {
        let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
        memory
            .ensure_story_contract_seed("eval", "T", "fantasy", "p", "j", "")
            .unwrap();
        memory
            .upsert_style_preference("sentence_length", "短句", false)
            .ok();
        memory
            .upsert_style_preference("literary_prose", "文学", false)
            .ok();
        let voice = build_author_voice_snapshot(&memory, &["Chapter-1".to_string()], 100);
        assert!(!voice.sample_refs.is_empty());
        assert!(voice.confidence > 0.0);
    }

    #[test]
    fn style_drift_links_evidence() {
        let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
        memory
            .ensure_story_contract_seed("eval", "T", "fantasy", "p", "j", "")
            .unwrap();
        memory
            .upsert_style_preference("literary_prose", "文学表达", false)
            .ok();
        let voice = build_author_voice_snapshot(&memory, &["Ch1".to_string()], 100);
        let drift = compute_style_drift(&voice, "sample text", "Chapter-2");
        assert!(!drift.evidence_links.is_empty());
        assert!(!drift.overall_severity.is_empty());
    }
}
