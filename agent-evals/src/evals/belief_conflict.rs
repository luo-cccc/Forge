use super::*;
use agent_harness_core::Chunk;
use agent_writer_lib::writer_agent::belief_conflict::{
    explain_memory_belief_conflicts, project_brain_chunk_belief_evidence, BeliefConflictKind,
    BeliefSource,
};

pub fn run_belief_conflict_explains_sources_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查寒玉戒指真相，但真相必须延后回收。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露寒玉戒指来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-5",
            "林墨只确认寒玉戒指仍是悬念，不揭示来源。",
            "保留寒玉戒指来源悬念",
            "不得揭示寒玉戒指来源",
            "林墨带着未解的戒指线索离开旧门",
            "mission:chapter-5",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "寒玉戒指",
            &[],
            "寒玉戒指的来源仍未知。",
            &serde_json::json!({"来源": "未知"}),
            0.92,
        )
        .unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "寒玉戒指来源",
            "戒指来源必须保持悬念。",
            "Chapter-2",
            "第8章再揭示寒玉戒指来源",
            9,
        )
        .unwrap();

    let project_brain = Chunk {
        id: "ring-origin-revealed".to_string(),
        chapter: "Chapter-5".to_string(),
        text: "寒玉戒指来源已经揭示，来自皇宫禁库。".to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec!["寒玉戒指".to_string(), "来源".to_string()],
        topic: Some("ring origin".to_string()),
        source_ref: Some("chapter:Chapter-5".to_string()),
        source_revision: Some("rev-revealed".to_string()),
        source_kind: Some("chapter".to_string()),
        chunk_index: Some(0),
        archived: false,
    };
    let brain_evidence = vec![project_brain_chunk_belief_evidence(&project_brain, 0.84)];

    let conflicts =
        explain_memory_belief_conflicts(&memory, "eval", Some("Chapter-5"), &brain_evidence)
            .unwrap();

    let mut errors = Vec::new();
    let reveal_conflict = conflicts
        .iter()
        .find(|conflict| conflict.kind == BeliefConflictKind::ForbiddenReveal);
    let fact_conflict = conflicts
        .iter()
        .find(|conflict| conflict.kind == BeliefConflictKind::FactContradiction);

    let Some(conflict) = reveal_conflict else {
        return eval_result(
            "writer_agent:belief_conflict_explains_sources",
            format!("conflicts={}", conflicts.len()),
            vec!["missing forbidden reveal conflict".to_string()],
        );
    };
    let Some(fact_conflict) = fact_conflict else {
        return eval_result(
            "writer_agent:belief_conflict_explains_sources",
            format!("conflicts={}", conflicts.len()),
            vec!["missing fact contradiction conflict".to_string()],
        );
    };

    for expected_source in [
        BeliefSource::StoryContract,
        BeliefSource::ChapterMission,
        BeliefSource::PromiseLedger,
        BeliefSource::ProjectBrain,
    ] {
        if !conflict
            .evidence
            .iter()
            .any(|item| item.source == expected_source)
        {
            errors.push(format!("missing evidence source {:?}", expected_source));
        }
    }
    for expected_source in [BeliefSource::Canon, BeliefSource::ProjectBrain] {
        if !fact_conflict
            .evidence
            .iter()
            .any(|item| item.source == expected_source)
        {
            errors.push(format!(
                "missing fact contradiction evidence source {:?}",
                expected_source
            ));
        }
    }

    if conflict
        .evidence
        .iter()
        .any(|item| item.reference.is_empty())
        || fact_conflict
            .evidence
            .iter()
            .any(|item| item.reference.is_empty())
    {
        errors.push("some conflict evidence lacks reference".to_string());
    }
    if conflict.evidence.iter().any(|item| item.snippet.is_empty())
        || fact_conflict
            .evidence
            .iter()
            .any(|item| item.snippet.is_empty())
    {
        errors.push("some conflict evidence lacks snippet".to_string());
    }
    if conflict
        .evidence
        .iter()
        .any(|item| item.confidence <= 0.0 || item.confidence > 1.0)
        || fact_conflict
            .evidence
            .iter()
            .any(|item| item.confidence <= 0.0 || item.confidence > 1.0)
    {
        errors.push("some conflict evidence confidence is out of range".to_string());
    }
    if !(0.75..=1.0).contains(&conflict.confidence) {
        errors.push(format!(
            "conflict confidence should be high but bounded, got {}",
            conflict.confidence
        ));
    }
    if !conflict.rationale.contains("forbidden or deferred")
        || conflict.resolution_hint.trim().is_empty()
    {
        errors.push("conflict lacks actionable rationale/resolution hint".to_string());
    }
    if !conflict
        .evidence
        .iter()
        .any(|item| item.reference == "project_brain:ring-origin-revealed")
    {
        errors.push("project brain chunk reference not preserved".to_string());
    }

    eval_result(
        "writer_agent:belief_conflict_explains_sources",
        format!(
            "conflicts={} guardEvidence={} factEvidence={} confidence={:.2}/{:.2}",
            conflicts.len(),
            conflict.evidence.len(),
            fact_conflict.evidence.len(),
            conflict.confidence,
            fact_conflict.confidence
        ),
        errors,
    )
}
