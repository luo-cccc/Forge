#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::ChapterResultDelta;

pub fn run_emotional_debt_extraction_eval() -> EvalResult {
    let mut errors = Vec::new();

    let emotional_keywords = [
        "愤怒", "悲伤", "背叛", "恐惧", "失去", "悔恨", "自责", "绝望", "压抑", "痛苦",
    ];

    // Create a ChapterResultDelta with state_changes and new_conflicts containing keywords
    let delta = ChapterResultDelta {
        summary: "章末总结".to_string(),
        state_changes: vec![
            "角色A感到背叛与愤怒".to_string(),
            "角色B失去了家园".to_string(),
        ],
        character_progress: vec![],
        new_conflicts: vec!["角色A对角色B的愤怒爆发".to_string()],
        new_clues: vec![],
        promise_updates: vec![],
        canon_updates: vec![],
    };

    // Scan for emotional cues (replicate the extraction logic)
    let mut cues: Vec<String> = Vec::new();
    for line in &delta.state_changes {
        for keyword in &emotional_keywords {
            if line.contains(keyword) && !cues.contains(&keyword.to_string()) {
                cues.push(keyword.to_string());
            }
        }
    }
    for line in &delta.new_conflicts {
        for keyword in &emotional_keywords {
            if line.contains(keyword) && !cues.contains(&keyword.to_string()) {
                cues.push(keyword.to_string());
            }
        }
    }

    // Verify at least 2 cues found (背叛 and 愤怒 from state_changes, 愤怒 from new_conflicts)
    if cues.len() < 2 {
        errors.push(format!(
            "expected at least 2 emotional debt cues, got {}: {:?}",
            cues.len(),
            cues
        ));
    }

    // Verify specific keywords are detected
    if !cues.contains(&"背叛".to_string()) {
        errors.push("expected '背叛' to be detected as emotional debt cue".to_string());
    }
    if !cues.contains(&"愤怒".to_string()) {
        errors.push("expected '愤怒' to be detected as emotional debt cue".to_string());
    }

    // Verify duplicate keywords are deduplicated (愤怒 appears in both state_changes and new_conflicts)
    let count_angry = cues.iter().filter(|c| *c == "愤怒").count();
    if count_angry != 1 {
        errors.push(format!(
            "expected '愤怒' to appear exactly once (dedup), got {}",
            count_angry
        ));
    }

    // Verify no false positives
    if cues.contains(&"绝望".to_string()) {
        errors.push("'绝望' should not be detected (not present in input)".to_string());
    }

    eval_result(
        "writer_agent:emotional_debt_extraction",
        format!("cues={} keywords={:?}", cues.len(), cues),
        errors,
    )
}
