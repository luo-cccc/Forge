use crate::fixtures::*;
use agent_writer_lib::writer_agent::anchor_carry::score_anchor_carry;

pub fn run_anchor_regression_eval() -> EvalResult {
    let sample_text = "Lin Mo raised the blade. The cold hilt hummed against his palm. \
        Zhang San stepped back, eyes flickering toward the door. The old debt hung between them \
        like smoke. 'You still carry that ring,' Zhang San said. Lin Mo did not answer. \
        The sealing gate trembled in the distance.";

    let anchors = vec![
        "Lin Mo".to_string(),
        "Zhang San".to_string(),
        "寒影刀".to_string(),
        "寒玉戒指".to_string(),
    ];

    let report = score_anchor_carry(sample_text, &anchors);

    // Smoke test: the report should have valid structure.
    let ok = report.anchor_count == anchors.len() as u64
        && report.mention_rate >= 0.0
        && report.carry_rate >= 0.0
        && !report.items.is_empty();

    EvalResult::pass_if(
        "anchor_regression",
        ok,
        format!(
            "anchors={} mentioned={} carried={} mention_rate={:.2} carry_rate={:.2}",
            report.anchor_count,
            report.mentioned_count,
            report.carried_count,
            report.mention_rate,
            report.carry_rate,
        ),
    )
}
