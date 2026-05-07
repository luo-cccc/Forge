#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::promise_planner::{
    promise_subject_pressure, reader_expectation_boost,
};
use std::path::Path;

pub fn run_reader_planner_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();

    // Add a promise whose expected payoff matches the current chapter (Chapter-5)
    let pid = memory
        .add_promise(
            "plot_promise",
            "失落权杖",
            "权杖在远古遗迹深处",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();
    let ps = memory.get_open_promise_summaries().unwrap();
    let matching_promise = ps.iter().find(|p| p.id == pid).unwrap();

    // Test: boost should be > 1.0 when expected payoff matches current chapter
    let boost_matching = reader_expectation_boost(matching_promise, "Chapter-5");
    if boost_matching <= 1.0 {
        errors.push(format!(
            "boost should be > 1.0 for matching promise, got {}",
            boost_matching
        ));
    }

    // Test: boost should be > 1.0 when expected payoff is one chapter ahead
    let boost_next = reader_expectation_boost(matching_promise, "Chapter-4");
    if boost_next <= 1.0 {
        errors.push(format!(
            "boost should be > 1.0 when expected payoff is one chapter ahead, got {}",
            boost_next
        ));
    }

    // Test: boost should be 1.0 when expected payoff is far from current chapter
    let boost_far = reader_expectation_boost(matching_promise, "Chapter-10");
    if (boost_far - 1.0).abs() > f64::EPSILON {
        errors.push(format!(
            "boost should be 1.0 for non-matching promise, got {}",
            boost_far
        ));
    }

    // Test: boost should handle empty expected_payoff gracefully
    let pid2 = memory
        .add_promise(
            "plot_promise",
            "无名线索",
            "一条无名线索",
            "Chapter-1",
            "",
            3,
        )
        .unwrap();
    let ps2 = memory.get_open_promise_summaries().unwrap();
    let empty_promise = ps2.iter().find(|p| p.id == pid2).unwrap();
    let boost_empty = reader_expectation_boost(empty_promise, "Chapter-5");
    if (boost_empty - 1.0).abs() > f64::EPSILON {
        errors.push(format!(
            "boost should be 1.0 for empty expected_payoff, got {}",
            boost_empty
        ));
    }

    // Verify that promise_subject_pressure includes the boost
    let pressure = promise_subject_pressure(matching_promise, &memory, "Chapter-5");
    if pressure <= 0.0 {
        errors.push(format!(
            "pressure should be positive for matching promise, got {}",
            pressure
        ));
    }

    eval_result(
        "writer_agent:reader_planner",
        format!(
            "boost_matching={} boost_next={} boost_far={} boost_empty={} pressure={}",
            boost_matching, boost_next, boost_far, boost_empty, pressure
        ),
        errors,
    )
}
