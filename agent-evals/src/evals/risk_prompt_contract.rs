use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_risk_prompt_contract_eval() -> EvalResult {
    // Test with canon risk debt -> guard tone should be danger
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .upsert_character_with_attrs(
            "林墨",
            &[],
            "protagonist",
            "主角，惯用寒影刀。",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.95,
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落。",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    // Observe text that creates canon risk: character known for 寒影刀 uses a different weapon
    kernel
        .observe(observation_in_chapter(
            "林墨拔出长剑，指向门外的人。",
            "Chapter-1",
        ))
        .unwrap();

    let summary_with_debt = kernel.today_five_summary();
    let guard_with_debt = summary_with_debt.items.iter().find(|i| i.slot == "guard");
    let tone_with_debt = guard_with_debt.map(|i| i.tone.as_str()).unwrap_or("none");

    // Test with no debt -> guard tone should be success or accent
    let clean_memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    clean_memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    let clean_kernel = WriterAgentKernel::new("eval", clean_memory);
    let summary_clean = clean_kernel.today_five_summary();
    let guard_clean = summary_clean.items.iter().find(|i| i.slot == "guard");
    let tone_no_debt = guard_clean.map(|i| i.tone.as_str()).unwrap_or("none");

    let debt_danger = tone_with_debt == "⚠️ 需要注意";
    let no_debt_safe = tone_no_debt == "✅ 一切正常" || tone_no_debt == "📝 提个醒";
    let ok = debt_danger && no_debt_safe;
    EvalResult::pass_if(
        "writer_agent:risk_prompt_contract",
        ok,
        format!(
            "toneWithDebt={} toneNoDebt={} danger={} safe={}",
            tone_with_debt, tone_no_debt, debt_danger, no_debt_safe
        ),
    )
}
