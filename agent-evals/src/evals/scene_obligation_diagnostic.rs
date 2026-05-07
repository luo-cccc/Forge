#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_scene_obligation_diagnostic_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "scene_diag", "fantasy", "p", "j", "")
        .unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();
    let pid = memory
        .add_promise(
            "plot_promise",
            "test_obl_promise",
            "obligation linked promise",
            "Chapter-0",
            "Chapter-3",
            4,
        )
        .unwrap();
    let sid = memory
        .upsert_scene("Chapter-1", 0, "scene", "test_scene")
        .unwrap();
    memory
        .upsert_scene_obligations(
            sid,
            &[pid],
            &["mission-obl".to_string()],
            &["payoff-obl".to_string()],
        )
        .unwrap();

    // Verify obligation is detectable
    let obl = memory.get_scene_obligations(sid).unwrap();
    let has_promise = obl.as_ref().is_some_and(|o| o.promise_ids.contains(&pid));

    // Verify the promise is in open summaries
    let open_promises = memory.get_open_promise_summaries().unwrap();
    let promise_found = open_promises.iter().any(|p| p.id == pid);

    // Run diagnostics on a paragraph mentioning the promise title
    let engine = DiagnosticsEngine::new();
    let diags = engine.diagnose(
        "林墨握紧test_obl_promise的线索",
        0,
        "Chapter-1",
        "eval",
        &memory,
    );

    let has_diag = !diags.is_empty();

    if !has_promise {
        errors.push("scene obligation not bound to promise".to_string());
    }
    if !promise_found {
        errors.push("promise not found in open summaries".to_string());
    }
    if !has_diag {
        errors.push(format!(
            "diagnostics returned {} results with promise present",
            diags.len()
        ));
    }

    EvalResult {
        fixture: "writer_agent:scene_obligation_diagnostic".to_string(),
        passed: has_promise && promise_found && has_diag,
        actual: format!(
            "obligationDetected={} promiseFound={} diagCount={}",
            has_promise,
            promise_found,
            diags.len()
        ),
        errors,
    }
}
