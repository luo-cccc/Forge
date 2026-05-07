use crate::fixtures::*;
use agent_writer_lib::writer_agent::feedback::FeedbackAction;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_trust_stats_contract_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "test",
            "fantasy",
            "A hero must choose between duty and love.",
            "The hero faces a moral dilemma.",
            "",
        )
        .unwrap();
    memory
        .upsert_character("Hero", &[], "protagonist", "The main hero")
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    // Inject feedback events: 8 accepted, 2 snoozed (80% accept, 20% ignore)
    for _ in 0..8 {
        kernel.add_feedback_event(FeedbackAction::Accepted);
    }
    for _ in 0..2 {
        kernel.add_feedback_event(FeedbackAction::Snoozed);
    }
    let summary = kernel.today_five_summary();
    let guard = summary
        .items
        .iter()
        .find(|i| i.slot == "guard")
        .expect("guard item missing");
    // Verify the feedback stats appear in the guard detail
    let has_stats = guard.detail.contains("你的写作习惯")
        && guard.detail.contains("接受建议")
        && guard.detail.contains("忽略提醒");
    // Verify valid ratio numbers (80% accept, 20% snooze)
    let has_accept_pct = guard.detail.contains("80%") || guard.detail.contains("80");
    let has_ignore_pct = guard.detail.contains("20%") || guard.detail.contains("20");
    let ok = has_stats && has_accept_pct && has_ignore_pct;
    EvalResult::pass_if(
        "writer_agent:trust_stats_contract",
        ok,
        format!(
            "hasStats={} hasAcceptPct={} hasIgnorePct={}",
            has_stats, has_accept_pct, has_ignore_pct
        ),
    )
}
