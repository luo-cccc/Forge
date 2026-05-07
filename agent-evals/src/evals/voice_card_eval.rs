use crate::fixtures::*;
use std::path::Path;

use agent_writer_lib::chapter_generation::character_voice_cards;
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_voice_card_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "Test", "fantasy", "promise", "journey", "")
        .unwrap();

    // Seed multiple characters
    memory
        .upsert_character("云逸", &[], "protagonist", "踏上寻找真相的旅途，内心充满疑问")
        .ok();
    memory
        .upsert_character("墨尘", &["黑衣客".to_string()], "antagonist", "暗中策划阴谋，身份成谜")
        .ok();
    memory
        .upsert_character("青璃", &[], "supporting", "陪伴云逸，提供关键线索支撑")
        .ok();

    let cards = character_voice_cards(&memory);
    if cards.is_empty() {
        errors.push("voice cards should not be empty with seeded characters".to_string());
    }
    if !cards.contains("角色速写") {
        errors.push("voice cards should contain '角色速写' header".to_string());
    }
    // Should include at least one character
    if !cards.contains("云逸") {
        errors.push("voice cards should include seeded character '云逸'".to_string());
    }

    eval_result(
        "writer_agent:voice_card",
        format!("len={}", cards.len()),
        errors,
    )
}
