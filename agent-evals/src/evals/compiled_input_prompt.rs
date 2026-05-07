#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    build_chapter_generation_spine, ChapterContextSpine, ChapterTarget,
};
use agent_writer_lib::writer_agent::input_governance::CompiledInput;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_compiled_input_prompt_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();

    let target = ChapterTarget {
        title: "Chapter-1".to_string(),
        filename: "chapter-1.md".to_string(),
        number: Some(1),
        summary: "Opening chapter".to_string(),
        status: "draft".to_string(),
    };

    let compiled = CompiledInput {
        intent_text: "Write action scene".to_string(),
        selected_evidence: vec!["evidence-1".to_string(), "evidence-2".to_string()],
        rule_stack: vec!["rule-1".to_string()],
        trace_hint: "test".to_string(),
        compiled_at_ms: now_ms(),
    };

    let spine = build_chapter_generation_spine(
        &target,
        None,
        None,
        None,
        Some(&compiled),
        &memory,
    );

    let focus = spine.focus_pack;
    let has_intent = focus.contains("Write action scene");
    let has_evidence = focus.contains("evidence-1") && focus.contains("evidence-2");
    let has_rules = focus.contains("rule-1");

    let ok = has_intent && has_evidence && has_rules;
    EvalResult::pass_if(
        "compiled_input_prompt",
        ok,
        format!(
            "intent={} evidence={} rules={} focusChars={}",
            has_intent,
            has_evidence,
            has_rules,
            focus.chars().count()
        ),
    )
}
