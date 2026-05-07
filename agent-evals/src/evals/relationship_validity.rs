#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::writer_agent::memory::WriterMemory;
use std::path::Path;

pub fn run_relationship_validity_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let a = memory
        .upsert_character("张三", &[], "protagonist", "hero")
        .unwrap();
    let b = memory
        .upsert_character("李四", &[], "supporting", "rival")
        .unwrap();

    // Enemy at Chapter-1
    let rel1 = memory
        .upsert_relationship(a, b, "enemy", "public", "Chapter-1", "src1")
        .unwrap();
    let active = memory.get_active_relationships(a, "Chapter-3").unwrap();
    let found = active.iter().any(|r| r.id == rel1);
    if !found {
        errors.push("relationship should be active at Chapter-3".to_string());
    }

    // Close at Chapter-5
    memory.close_relationship(rel1, "Chapter-5").unwrap();
    let after = memory.get_active_relationships(a, "Chapter-6").unwrap();
    let closed = !after.iter().any(|r| r.id == rel1);
    if !closed {
        errors.push("relationship should be closed by Chapter-6".to_string());
    }

    // Reopen as ally at Chapter-7 (new row)
    let rel2 = memory
        .upsert_relationship(a, b, "ally", "public", "Chapter-7", "src2")
        .unwrap();
    let reopened = rel2 != rel1;
    if !reopened {
        errors.push("reopened relationship should have new id".to_string());
    }

    eval_result(
        "writer_agent:relationship_validity",
        format!(
            "validityWindow={} closed={} reopened={}",
            found, closed, reopened
        ),
        errors,
    )
}
