#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::promise_planner::{promise_subject_pressure, knowledge_readiness_factor, timeline_due_factor};

pub fn run_planner_fallback_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory.ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "").unwrap();
    memory.upsert_character("林墨", &[], "protagonist", "主角").unwrap();
    let pid = memory.add_promise("plot_promise", "test", "test promise", "Chapter-1", "Chapter-5", 4).unwrap();
    let ps = memory.get_open_promise_summaries().unwrap();
    let promise = ps.iter().find(|p| p.id == pid).unwrap();
    // Without knowledge/timeline — fallback should work
    let base_pressure = promise_subject_pressure(promise, &memory, "Chapter-1");
    // With knowledge readiness (no data = 1.0)
    let kr = knowledge_readiness_factor(promise, &memory, "Chapter-1");
    // With timeline due (no data = 1.0)
    let td = timeline_due_factor(promise, &memory, "Chapter-1");
    let fallback_works = base_pressure > 0.0 && kr > 0.0 && td > 0.0;
    if !fallback_works {
        errors.push(format!("fallback failed: pressure={} kr={} td={}", base_pressure, kr, td));
    }
    EvalResult {
        fixture: "writer_agent:planner_fallback".to_string(),
        passed: fallback_works,
        actual: format!("fallbackOk={} pressure={} kr={} td={}", fallback_works, base_pressure, kr, td),
        errors,
    }
}
