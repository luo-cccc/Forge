use super::*;

pub fn run_intent_eval() -> Vec<EvalResult> {
    let engine = IntentEngine::new();
    let fixtures = [
        (
            "intent:dialogue",
            "林墨深吸一口气，说道：“我不能再替你隐瞒了。”",
            false,
            false,
            WritingIntent::Dialogue,
            AgentBehavior::SuggestContinuation,
            0.15,
        ),
        (
            "intent:emotional_silence",
            "她沉默着，眼泪无声滑落，手指微微颤抖。",
            false,
            false,
            WritingIntent::EmotionalBeat,
            AgentBehavior::StaySilent,
            0.10,
        ),
        (
            "intent:revision_selection",
            "一些被选中的文本",
            true,
            false,
            WritingIntent::Revision,
            AgentBehavior::OfferRevision,
            0.70,
        ),
        (
            "intent:chapter_switch",
            "",
            false,
            true,
            WritingIntent::StructuralPlanning,
            AgentBehavior::ProposeStructure,
            0.60,
        ),
    ];

    fixtures
        .into_iter()
        .map(
            |(
                name,
                text,
                has_selection,
                chapter_switch,
                expected_intent,
                expected_behavior,
                min_conf,
            )| {
                let estimate = engine.classify(text, has_selection, chapter_switch);
                let mut errors = Vec::new();
                if estimate.primary != expected_intent {
                    errors.push(format!(
                        "intent mismatch: got {:?}, expected {:?}",
                        estimate.primary, expected_intent
                    ));
                }
                if estimate.desired_behavior != expected_behavior {
                    errors.push(format!(
                        "behavior mismatch: got {:?}, expected {:?}",
                        estimate.desired_behavior, expected_behavior
                    ));
                }
                if estimate.confidence < min_conf {
                    errors.push(format!(
                        "confidence too low: got {:.2}, min {:.2}",
                        estimate.confidence, min_conf
                    ));
                }
                eval_result(
                    name,
                    format!(
                        "{:?} {:?} conf={:.2}",
                        estimate.primary, estimate.desired_behavior, estimate.confidence
                    ),
                    errors,
                )
            },
        )
        .collect()
}
