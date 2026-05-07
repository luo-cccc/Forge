use crate::fixtures::*;
use agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_voice_drift_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed a protagonist with taciturn ("寡言") summary
    memory
        .upsert_character("云逸", &[], "protagonist", "寡言少语，沉默内敛的青年剑客")
        .unwrap();

    // Build a paragraph with very long sentences (avg > 80 chars) about this character.
    // Each sentence must be well over 80 Chinese characters to trigger voice drift.
    let paragraph = "云逸静静地站在那里看着远方的山峦和云雾因为他知道这一切都即将发生改变而他必须做出那个最重要的选择无论前方等待着什么样未知的命运与艰难险阻他都已经做好了迎接一切挑战与牺牲的充分准备云逸的目光依然深邃而坚定仿佛穿透了时光的迷雾注视着那些已经被岁月掩埋的过往记忆与未竟的誓言。";

    let engine = DiagnosticsEngine::new();
    let results = engine.diagnose(paragraph, 0, "ch1", "eval", &memory);

    let voice_drift = results
        .iter()
        .find(|r| r.message.contains("声音漂移"));
    if voice_drift.is_none() {
        errors.push(format!(
            "expected voice drift diagnostic for taciturn protagonist with long sentences, got {} results",
            results.len()
        ));
    } else {
        let r = voice_drift.unwrap();
        if !r.message.contains("云逸") {
            errors.push(format!("voice drift message should mention character name, got: {}", r.message));
        }
    }

    eval_result(
        "writer_agent:voice_drift",
        format!("results={} found={}", results.len(), voice_drift.is_some()),
        errors,
    )
}
