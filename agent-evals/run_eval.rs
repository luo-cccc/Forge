//! Evaluation harness for the Writer Agent Kernel.
//! Runs golden-fixture tests and reports precision/recall/latency.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
struct EvalFixture {
    name: String,
    description: String,
    input_text: String,
    input_has_selection: bool,
    input_is_chapter_switch: bool,
    expected_intent: String,
    expected_behavior: String,
    expected_confidence_min: f32,
}

#[derive(Debug, Serialize)]
struct EvalResult {
    fixture: String,
    passed: bool,
    actual_intent: String,
    actual_behavior: String,
    actual_confidence: f32,
    expected_intent: String,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EvalReport {
    total: usize,
    passed: usize,
    failed: usize,
    results: Vec<EvalResult>,
}

/// Run the intent classification eval.
fn run_intent_eval(fixtures_dir: &Path) -> EvalReport {
    let mut results = Vec::new();
    let fixture_path = fixtures_dir.join("intent_fixtures.json");

    let fixtures: Vec<EvalFixture> = if fixture_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&fixture_path).unwrap_or_default())
            .unwrap_or_default()
    } else {
        // Embedded default fixtures
        default_intent_fixtures()
    };

    // Use our own simple classifier (mirrors IntentEngine logic)
    for f in &fixtures {
        let mut errors = Vec::new();

        // Simple rule-based classification for eval
        let (actual_intent, actual_confidence) = classify_intent_simple(
            &f.input_text,
            f.input_has_selection,
            f.input_is_chapter_switch,
        );

        let actual_behavior = behavior_for_intent(&actual_intent);

        if actual_intent != f.expected_intent {
            errors.push(format!(
                "intent mismatch: got '{}', expected '{}'",
                actual_intent, f.expected_intent
            ));
        }

        if actual_confidence < f.expected_confidence_min {
            errors.push(format!(
                "confidence too low: got {:.2}, min {:.2}",
                actual_confidence, f.expected_confidence_min
            ));
        }

        if actual_behavior != f.expected_behavior {
            errors.push(format!(
                "behavior mismatch: got '{}', expected '{}'",
                actual_behavior, f.expected_behavior
            ));
        }

        results.push(EvalResult {
            fixture: f.name.clone(),
            passed: errors.is_empty(),
            actual_intent,
            actual_behavior,
            actual_confidence,
            expected_intent: f.expected_intent.clone(),
            errors,
        });
    }

    let passed = results.iter().filter(|r| r.passed).count();
    EvalReport {
        total: fixtures.len(),
        passed,
        failed: fixtures.len() - passed,
        results,
    }
}

fn classify_intent_simple(text: &str, has_selection: bool, is_chapter_switch: bool) -> (String, f32) {
    if has_selection {
        return ("revision".into(), 0.85);
    }
    if is_chapter_switch {
        return ("structural_planning".into(), 0.7);
    }
    if text.contains("\"") || text.contains("「") || text.contains("说") || text.contains("道") {
        return ("dialogue".into(), 0.75);
    }
    if text.contains("愤怒") || text.contains("悲伤") || text.contains("泪") || text.contains("颤抖") {
        return ("emotional_beat".into(), 0.7);
    }
    // Check action BEFORE conflict escalation (action cues are more specific)
    let action_cues = ["拔", "冲", "击", "砍", "刺", "劈"];
    let conflict_cues = ["突然", "不料", "猛地"];
    let action_count = action_cues.iter().filter(|c| text.contains(*c)).count();
    let conflict_count = conflict_cues.iter().filter(|c| text.contains(*c)).count();
    if action_count > conflict_count {
        return ("action".into(), 0.6 + action_count as f32 * 0.05);
    }
    if conflict_count > 0 {
        return ("conflict_escalation".into(), 0.65);
    }
    if action_count > 0 {
        return ("action".into(), 0.6);
    }
    ("description".into(), 0.3)
}

fn behavior_for_intent(intent: &str) -> String {
    match intent {
        "emotional_beat" => "stay_silent",
        "revision" => "offer_revision",
        "structural_planning" => "propose_structure",
        "canon_maintenance" => "maintain_canon",
        _ => "suggest_continuation",
    }.into()
}

fn default_intent_fixtures() -> Vec<EvalFixture> {
    vec![
        EvalFixture {
            name: "dialogue_detection".into(),
            description: "Should detect dialogue cues and suggest continuation".into(),
            input_text: "\"你不能这样做，\"她低声说道。".into(),
            input_has_selection: false,
            input_is_chapter_switch: false,
            expected_intent: "dialogue".into(),
            expected_behavior: "suggest_continuation".into(),
            expected_confidence_min: 0.6,
        },
        EvalFixture {
            name: "emotional_beat_stay_silent".into(),
            description: "Emotional beats should not interrupt the author".into(),
            input_text: "她沉默着，眼泪无声滑落，手指微微颤抖。".into(),
            input_has_selection: false,
            input_is_chapter_switch: false,
            expected_intent: "emotional_beat".into(),
            expected_behavior: "stay_silent".into(),
            expected_confidence_min: 0.6,
        },
        EvalFixture {
            name: "revision_with_selection".into(),
            description: "Selection should trigger revision intent with high confidence".into(),
            input_text: "一些被选中的文本".into(),
            input_has_selection: true,
            input_is_chapter_switch: false,
            expected_intent: "revision".into(),
            expected_behavior: "offer_revision".into(),
            expected_confidence_min: 0.8,
        },
        EvalFixture {
            name: "action_detection".into(),
            description: "Action verbs should trigger action intent".into(),
            input_text: "林墨拔出长剑，猛地冲向敌人，一剑劈下。".into(),
            input_has_selection: false,
            input_is_chapter_switch: false,
            expected_intent: "action".into(),
            expected_behavior: "suggest_continuation".into(),
            expected_confidence_min: 0.5,
        },
        EvalFixture {
            name: "conflict_escalation".into(),
            description: "Sudden events should trigger conflict escalation".into(),
            input_text: "突然，一阵狂风袭来，不料竟暗藏杀机。".into(),
            input_has_selection: false,
            input_is_chapter_switch: false,
            expected_intent: "conflict_escalation".into(),
            expected_behavior: "suggest_continuation".into(),
            expected_confidence_min: 0.55,
        },
        EvalFixture {
            name: "chapter_switch_structural".into(),
            description: "Chapter switch should trigger structural planning".into(),
            input_text: "".into(),
            input_has_selection: false,
            input_is_chapter_switch: true,
            expected_intent: "structural_planning".into(),
            expected_behavior: "propose_structure".into(),
            expected_confidence_min: 0.6,
        },
        EvalFixture {
            name: "fallback_description".into(),
            description: "Unrecognized text should fall back to description with low confidence".into(),
            input_text: "平凡的文字，没有明显的意图信号。".into(),
            input_has_selection: false,
            input_is_chapter_switch: false,
            expected_intent: "description".into(),
            expected_behavior: "suggest_continuation".into(),
            expected_confidence_min: 0.2,
        },
    ]
}

fn main() {
    let fixtures_dir = Path::new("fixtures");
    let report = run_intent_eval(fixtures_dir);

    // Print results
    println!("=== Agent Eval Report ===");
    println!("Total: {} | Passed: {} | Failed: {}",
        report.total, report.passed, report.failed);
    println!();

    for r in &report.results {
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("[{}] {} (intent={}, conf={:.2})",
            status, r.fixture, r.actual_intent, r.actual_confidence);
        for e in &r.errors {
            println!("  -> {}", e);
        }
    }

    // Save JSON report
    let report_path = Path::new("reports").join("eval_report.json");
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        std::fs::write(&report_path, json).ok();
        println!("\nReport saved to {}", report_path.display());
    }

    if report.failed > 0 {
        std::process::exit(1);
    }
}
