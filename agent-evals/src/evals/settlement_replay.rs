#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::{
    build_basic_chapter_settlement_delta, replay_settlement_extraction,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_settlement_replay_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let project_id = "eval-settlement-replay";
    memory
        .ensure_story_contract_seed(
            project_id,
            "Settlement Replay Test",
            "fantasy",
            "promise",
            "journey",
            "",
        )
        .unwrap();

    let content = "第四章内容：主角发现了古剑的秘密，剑身泛着蓝色微光。这把剑曾经属于北境宗主。江湖上流传着关于这把剑的传说。";

    let delta1 = build_basic_chapter_settlement_delta(
        project_id,
        "第四章",
        "aaaa0002",
        content,
        1000,
        &memory,
        Vec::new(),
    );

    let replay = replay_settlement_extraction(&delta1, content, &memory);

    if !replay.matches_original {
        errors.push(format!(
            "replay mismatches: {}",
            replay.mismatches.join("; ")
        ));
    }

    eval_result(
        "writer_agent:settlement_replay_produces_identical_delta",
        format!(
            "matches={} mismatches={} originalHash={} replayedHash={}",
            replay.matches_original,
            replay.mismatches.join("; "),
            replay.original_hash,
            replay.replayed_hash,
        ),
        errors,
    )
}
