#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::context_relevance::apply_typed_filter;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_typed_context_filter_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "typed_filter", "fantasy", "p", "j", "")
        .unwrap();
    // Create 10 characters, one protagonist
    memory
        .upsert_character("林墨", &[], "protagonist", "主角")
        .unwrap();
    memory
        .upsert_character("苏婉", &[], "supporting", "配角")
        .unwrap();
    memory
        .upsert_character("铁虎", &[], "supporting", "配角")
        .unwrap();
    memory
        .upsert_character("苍云", &[], "background", "背景")
        .unwrap();
    memory
        .upsert_character("夜影", &[], "background", "背景")
        .unwrap();
    memory
        .upsert_character("清风", &[], "background", "背景")
        .unwrap();
    memory
        .upsert_character("明月", &[], "background", "背景")
        .unwrap();
    memory
        .upsert_character("红尘", &[], "background", "背景")
        .unwrap();
    memory
        .upsert_character("白云", &[], "background", "背景")
        .unwrap();
    memory
        .upsert_character("飞燕", &[], "background", "背景")
        .unwrap();
    // Create 3 knowledge items
    memory
        .upsert_knowledge_item("寒玉戒指", "objective", "ch1")
        .unwrap();
    memory
        .upsert_knowledge_item("远古龙脉的秘密", "objective", "ch2")
        .unwrap();
    memory
        .upsert_knowledge_item("北境势力的动向", "subjective", "ch3")
        .unwrap();
    // Create 2 scenes
    memory
        .upsert_scene("Chapter-1", 0, "dialogue", "测试场景一")
        .unwrap();
    memory
        .upsert_scene("Chapter-1", 1, "action", "测试场景二")
        .unwrap();

    // Call apply_typed_filter with text mentioning protagonist and knowledge topic
    let text = "林墨握紧了手中的寒玉戒指，他知道这枚戒指的下落至关重要。";
    let filter = apply_typed_filter(text, "Chapter-1", &memory);

    let entity_boosted = filter.entity_boost > 1.0;
    let knowledge_boosted = filter.knowledge_boost > 1.0;

    if !entity_boosted {
        errors.push(format!(
            "entity_boost should be >1.0, got {}",
            filter.entity_boost
        ));
    }
    if !knowledge_boosted {
        errors.push(format!(
            "knowledge_boost should be >1.0, got {}",
            filter.knowledge_boost
        ));
    }

    eval_result(
        "writer_agent:typed_context_filter",
        format!(
            "entityBoost={} knowledgeBoost={} sceneBoost={} reasons={:?}",
            filter.entity_boost, filter.knowledge_boost, filter.scene_boost, filter.reasons
        ),
        errors,
    )
}
