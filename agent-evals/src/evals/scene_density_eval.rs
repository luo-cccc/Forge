use crate::fixtures::*;
use agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_scene_density_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Create 7 scenes for chapter "ch1" (chapter_id used in diagnose)
    for i in 1..=7 {
        memory
            .upsert_scene("ch1", i, "action", &format!("场景{}: 激烈的冲突和转折", i))
            .unwrap();
    }

    let paragraph = "测试段落。";
    let engine = DiagnosticsEngine::new();
    let results = engine.diagnose(paragraph, 0, "ch1", "eval", &memory);

    let scene_density = results.iter().find(|r| r.message.contains("场景密度较高"));
    if scene_density.is_none() {
        errors.push(format!(
            "expected scene density warning with 7 scenes, got {} results",
            results.len()
        ));
    } else {
        let r = scene_density.unwrap();
        if !r.message.contains("7") {
            errors.push(format!(
                "scene density message should mention scene count 7, got: {}",
                r.message
            ));
        }
    }

    eval_result(
        "writer_agent:scene_density",
        format!(
            "results={} found={}",
            results.len(),
            scene_density.is_some()
        ),
        errors,
    )
}
