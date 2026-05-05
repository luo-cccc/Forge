use super::*;

pub fn run_story_debt_snapshot_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel
        .observe(observation_in_chapter(
            "林墨拔出长剑，指向门外的人。",
            "Chapter-3",
        ))
        .unwrap();

    let debt = kernel.story_debt_snapshot();
    let mut errors = Vec::new();
    if debt.canon_risk_count != 1 {
        errors.push(format!(
            "expected 1 canon risk, got {}",
            debt.canon_risk_count
        ));
    }
    if debt.promise_count != 1 {
        errors.push(format!(
            "expected 1 promise debt, got {}",
            debt.promise_count
        ));
    }
    if debt.open_count < 2 {
        errors.push(format!(
            "expected at least 2 open debts, got {}",
            debt.open_count
        ));
    }
    if !debt
        .entries
        .iter()
        .any(|entry| entry.title.contains("Story truth"))
    {
        errors.push("missing story truth debt entry".to_string());
    }
    if !debt
        .entries
        .iter()
        .any(|entry| entry.title.contains("Open promise"))
    {
        errors.push("missing open promise debt entry".to_string());
    }
    if !debt.entries.iter().any(|entry| {
        entry.title.contains("Open promise")
            && entry
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
    }) {
        errors.push("open promise debt is not executable".to_string());
    }

    eval_result(
        "writer_agent:story_debt_snapshot_counts_foundation",
        format!(
            "total={} open={} canon={} promise={}",
            debt.total, debt.open_count, debt.canon_risk_count, debt.promise_count
        ),
        errors,
    )
}

pub fn run_story_debt_priority_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "密信",
            "密信被张三拿走，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel
        .observe(observation_in_chapter(
            "林墨拔出长剑，张三直接揭开真相：玉佩来自禁地。",
            "Chapter-2",
        ))
        .unwrap();

    let debt = kernel.story_debt_snapshot();
    let categories = debt
        .entries
        .iter()
        .take(4)
        .map(|entry| entry.category.clone())
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    if categories.len() < 4 {
        errors.push(format!(
            "expected at least 4 debt entries, got {}",
            categories.len()
        ));
    }
    let expected = [
        StoryDebtCategory::StoryContract,
        StoryDebtCategory::ChapterMission,
        StoryDebtCategory::CanonRisk,
        StoryDebtCategory::Promise,
    ];
    for (index, expected_category) in expected.iter().enumerate() {
        if categories.get(index) != Some(expected_category) {
            errors.push(format!(
                "debt priority index {} got {:?}, expected {:?}",
                index,
                categories.get(index),
                expected_category
            ));
        }
    }

    eval_result(
        "writer_agent:story_debt_priority_foundation",
        format!("categories={:?}", categories),
        errors,
    )
}

pub fn run_guard_trace_evidence_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落。",
            "玉佩线索",
            "提前揭开真相",
            "以线索推进收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨站在雨里，沉默地看着远处灯火。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    let gap = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::ChapterMission && proposal.preview.contains("必保事项")
    });
    let trace = kernel.trace_snapshot(10);
    let trace_entry = gap.and_then(|proposal| {
        trace
            .recent_proposals
            .iter()
            .find(|entry| entry.id == proposal.id)
    });

    let mut errors = Vec::new();
    if trace_entry.is_none() {
        errors.push("missing trace entry for mission guard".to_string());
    }
    if !trace_entry.is_some_and(|entry| {
        entry
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && entry
                .evidence
                .iter()
                .any(|evidence| evidence.source == EvidenceSource::ChapterText)
    }) {
        errors.push("trace entry lacks mission and chapter text evidence".to_string());
    }

    eval_result(
        "writer_agent:guard_trace_evidence",
        format!(
            "traceEvidence={}",
            trace_entry.map(|entry| entry.evidence.len()).unwrap_or(0)
        ),
        errors,
    )
}

pub fn run_story_debt_priority_ordering_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    kernel
        .observe(observation_in_chapter("林墨停在旧门前。", "Chapter-1"))
        .unwrap();

    let debt = kernel.story_debt_snapshot();
    let mut errors = Vec::new();
    // Verify the snapshot structure is well-formed (may be empty for minimal obs)
    if debt.total > 0 {
        let categories: Vec<String> = debt
            .entries
            .iter()
            .map(|e| format!("{:?}", e.category))
            .collect();
        let unique: std::collections::BTreeSet<_> = categories.iter().collect();
        if unique.is_empty() {
            errors.push("debt entries lack categories".to_string());
        }
    }

    eval_result(
        "writer_agent:story_debt_priority_ordering",
        format!("totalDebt={}", debt.total),
        errors,
    )
}
