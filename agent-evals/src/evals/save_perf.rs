#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::ChapterSettlementDelta;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::settlement_apply::apply_chapter_settlement_delta;
use std::path::Path;
use std::time::Instant;

pub fn run_save_perf_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "")
        .unwrap();
    for i in 0..100 {
        let _ = memory.upsert_character(&format!("char_{}", i), &[], "supporting", "filler");
    }
    let start = Instant::now();
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-1".to_string(),
        chapter_revision: "aaaa0001".to_string(),
        ..Default::default()
    };
    let result = apply_chapter_settlement_delta(&memory, "eval", &delta).unwrap();
    let ms = start.elapsed().as_millis();
    EvalResult::pass_if(
        "save_perf",
        result.applied && ms < 500,
        format!("applyMs={}", ms),
    )
}
