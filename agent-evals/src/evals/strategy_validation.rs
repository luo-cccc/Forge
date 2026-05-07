#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    select_generation_strategy, BuiltChapterContext, ChapterContextBudgetReport, ChapterContract,
    ChapterIntentArtifact, ChapterRuleStackArtifact, ChapterTarget, ChapterTraceArtifact,
    GenerationStrategy,
};
use agent_writer_lib::writer_agent::task_receipt::WriterTaskReceipt;

fn minimal_context(included_chars: usize, impact_truncated: bool) -> BuiltChapterContext {
    BuiltChapterContext {
        request_id: "strategy-validation".to_string(),
        target: ChapterTarget {
            title: "Test".to_string(),
            filename: "test.md".to_string(),
            number: None,
            summary: "test".to_string(),
            status: "draft".to_string(),
        },
        base_revision: "rev-1".to_string(),
        chapter_contract: ChapterContract::default(),
        prompt_context: String::new(),
        sources: vec![],
        budget: ChapterContextBudgetReport {
            max_chars: 24_000,
            included_chars,
            source_count: 0,
            truncated_source_count: 0,
            warnings: vec![],
        },
        warnings: vec![],
        receipt: WriterTaskReceipt {
            task_id: "test".to_string(),
            task_kind: "ChapterGeneration".to_string(),
            chapter: None,
            objective: "test".to_string(),
            required_evidence: vec![],
            expected_artifacts: vec![],
            must_not: vec![],
            source_refs: vec![],
            base_revision: None,
            created_at_ms: 0,
        },
        intent_artifact: ChapterIntentArtifact::default(),
        selected_evidence: vec![],
        rule_stack: ChapterRuleStackArtifact::default(),
        trace_artifact: ChapterTraceArtifact::default(),
        scene_plan: vec![],
        compiled_input: None,
        stable_prefix_chars: 0,
        dynamic_tail_chars: 0,
        focus_pack_rebuild_count: 0,
        previous_fulltext_upgrade_count: 0,
        previous_fulltext_upgrade_reason: String::new(),
        impact_scoped: false,
        impact_filtered_count: 0,
        impact_truncated,
        generation_strategy: GenerationStrategy::default(),
    }
}

pub fn run_strategy_validation_eval() -> EvalResult {
    // Test: large chapter → BackgroundLongChapter
    let ctx1 = minimal_context(20_000, false);
    let s1 = select_generation_strategy(&ctx1, 0);
    // Test: normal mid-range → InteractiveSafeDraft
    let ctx2 = minimal_context(10_000, false);
    let s2 = select_generation_strategy(&ctx2, 0);
    // Test: small chapter with no truncation → InteractiveFastDraft
    let ctx3 = minimal_context(5_000, false);
    let s3 = select_generation_strategy(&ctx3, 0);
    // Test: high repair count → RepairHeavyMode
    let ctx4 = minimal_context(10_000, false);
    let s4 = select_generation_strategy(&ctx4, 3);
    // Test: impact truncated → BackgroundLongChapter (even mid-range)
    let ctx5 = minimal_context(10_000, true);
    let s5 = select_generation_strategy(&ctx5, 0);

    let valid = matches!(s1, GenerationStrategy::BackgroundLongChapter)
        && matches!(s2, GenerationStrategy::InteractiveSafeDraft)
        && matches!(s3, GenerationStrategy::InteractiveFastDraft)
        && matches!(s4, GenerationStrategy::RepairHeavyMode)
        && matches!(s5, GenerationStrategy::BackgroundLongChapter);

    EvalResult::pass_if(
        "strategy_validation",
        valid,
        format!(
            "s1={:?} s2={:?} s3={:?} s4={:?} s5={:?}",
            s1, s2, s3, s4, s5
        ),
    )
}
