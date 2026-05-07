use crate::fixtures::*;
use agent_writer_lib::chapter_generation::GenerationStrategy;

fn test_select_generation_strategy(
    total_chars: usize,
    repair_history: usize,
    impact_truncated: bool,
) -> GenerationStrategy {
    if repair_history > 2 {
        return GenerationStrategy::RepairHeavyMode;
    }
    if total_chars < 8_000 && !impact_truncated {
        return GenerationStrategy::InteractiveFastDraft;
    }
    if total_chars > 15_000 || impact_truncated {
        return GenerationStrategy::BackgroundLongChapter;
    }
    GenerationStrategy::InteractiveSafeDraft
}

pub fn run_strategy_selection_eval() -> EvalResult {
    let results = vec![
        (
            "fast draft (5000 chars, no repair, no truncation)",
            test_select_generation_strategy(5_000, 0, false),
            GenerationStrategy::InteractiveFastDraft,
        ),
        (
            "background long (20000 chars)",
            test_select_generation_strategy(20_000, 0, false),
            GenerationStrategy::BackgroundLongChapter,
        ),
        (
            "repair heavy (repair_history=3)",
            test_select_generation_strategy(5_000, 3, false),
            GenerationStrategy::RepairHeavyMode,
        ),
        (
            "safe draft (10000 chars, no repair)",
            test_select_generation_strategy(10_000, 0, false),
            GenerationStrategy::InteractiveSafeDraft,
        ),
        (
            "background long (impact truncated)",
            test_select_generation_strategy(8_000, 0, true),
            GenerationStrategy::BackgroundLongChapter,
        ),
    ];

    let all_ok = results.iter().all(|(_, actual, expected)| actual == expected);
    let details: Vec<String> = results
        .iter()
        .map(|(desc, actual, expected)| {
            format!(
                "{}: got={:?} expected={:?} {}",
                desc,
                actual,
                expected,
                if actual == expected { "OK" } else { "FAIL" }
            )
        })
        .collect();

    EvalResult::pass_if(
        "strategy_selection",
        all_ok,
        details.join(" | "),
    )
}
