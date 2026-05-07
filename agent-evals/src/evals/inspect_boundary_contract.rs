use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_inspect_boundary_contract_eval() -> EvalResult {
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
        .upsert_character_with_attrs(
            "Hero",
            &[],
            "protagonist",
            "The main hero, struggles with choices.",
            &serde_json::json!({"weapon": "sword"}),
            0.95,
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "Ancient Sword",
            "The hero must retrieve the ancient sword.",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel
        .observe(observation_in_chapter(
            "The hero stands before the ancient gate.",
            "Chapter-1",
        ))
        .unwrap();
    kernel
        .observe(observation_in_chapter(
            "A shadow moves in the darkness beyond.",
            "Chapter-1",
        ))
        .unwrap();
    kernel
        .observe(observation_in_chapter(
            "The sword glows with an eerie light.",
            "Chapter-1",
        ))
        .unwrap();

    let companion = kernel.companion_timeline_summary();
    let inspector = kernel.inspector_timeline(20);
    let companion_count = companion.events.len();
    let inspector_count = inspector.events.len();
    let companion_is_subset = companion_count <= inspector_count;
    let ok = companion_is_subset && inspector_count > 0;
    EvalResult::pass_if(
        "writer_agent:inspect_mode_boundary",
        ok,
        format!(
            "companionEvents={} inspectorEvents={} subset={}",
            companion_count, inspector_count, companion_is_subset
        ),
    )
}
