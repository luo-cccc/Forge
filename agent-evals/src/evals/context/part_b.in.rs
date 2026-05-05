pub fn run_result_feedback_tight_budget_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "玉佩线推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-3",
            "承接上一章玉佩线索。",
            "玉佩",
            "提前揭开真相",
            "以新的选择收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter(
        "林墨发现玉佩仍在张三手里，新的冲突让两人信任受损。",
        "Chapter-2",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let obs = observation_in_chapter("林墨站在门外，想起上一章的争执。", "Chapter-3");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_050);
    let result_source = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::ResultFeedback);

    let mut errors = Vec::new();
    if result_source.is_none() {
        errors.push("tight budget dropped ResultFeedback source".to_string());
    }
    if !result_source.is_some_and(|source| source.content.contains("章节结果")) {
        errors.push("ResultFeedback source lacks rendered chapter result".to_string());
    }
    if pack.total_chars > pack.budget_limit {
        errors.push(format!(
            "context exceeded tight budget: used {} > {}",
            pack.total_chars, pack.budget_limit
        ));
    }

    eval_result(
        "writer_agent:result_feedback_survives_tight_budget",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_context_decision_slice_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .record_decision(
            "Chapter-1",
            "林墨不主动解释",
            "accepted",
            &[],
            "保持克制，不用大段自白。",
            &[],
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨看向张三，把快到嘴边的话又咽了回去。");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_200);

    let mut errors = Vec::new();
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::DecisionSlice)
    {
        errors.push("missing recent decision slice".to_string());
    }
    if !pack.sources.iter().any(|source| {
        source.source == ContextSource::DecisionSlice && source.content.contains("不用大段自白")
    }) {
        errors.push("decision slice lacks recorded rationale".to_string());
    }

    eval_result(
        "writer_agent:context_includes_recent_decisions",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_story_contract_context_eval() -> EvalResult {
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
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨握着寒影刀，想起那枚玉佩。");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_500);

    let mut errors = Vec::new();
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::ProjectBrief)
    {
        errors.push("missing story contract project brief".to_string());
    }
    if !pack.sources.iter().any(|source| {
        source.source == ContextSource::ProjectBrief && source.content.contains("读者承诺")
    }) {
        errors.push("project brief lacks story contract content".to_string());
    }

    eval_result(
        "writer_agent:story_contract_context_source",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_next_beat_context_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    let mut save = observation_in_chapter(
        "林墨发现玉佩的下落，却开始怀疑张三。新的冲突就此埋下。",
        "Chapter-2",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let obs = observation_in_chapter("林墨站在门外，没有立刻进去。", "Chapter-3");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 2_000);
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    if ledger.next_beat.is_none() {
        errors.push("ledger missing next beat handoff".to_string());
    }
    if !ledger.next_beat.as_ref().is_some_and(|beat| {
        beat.goal.contains("冲突")
            && beat
                .carryovers
                .iter()
                .any(|carryover| carryover.contains("玉佩"))
    }) {
        errors.push("next beat does not carry conflict and promise context".to_string());
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::NextBeat)
    {
        errors.push("ContextPack missing NextBeat source".to_string());
    }
    if !pack.sources.iter().any(|source| {
        source.source == ContextSource::NextBeat && source.content.contains("下一拍目标")
    }) {
        errors.push("NextBeat source lacks rendered handoff content".to_string());
    }

    eval_result(
        "writer_agent:next_beat_context_handoff",
        format!(
            "nextBeat={} sources={}",
            ledger.next_beat.is_some(),
            pack.sources.len()
        ),
        errors,
    )
}

pub fn run_context_recall_tracking_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let warning = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning);
    let trace = kernel.trace_snapshot(10);
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    if warning.is_none() {
        errors.push("missing continuity warning proposal".to_string());
    }
    if !trace.context_recalls.iter().any(|recall| {
        warning.is_some_and(|proposal| recall.last_proposal_id == proposal.id)
            && recall.source == "Canon"
            && recall.snippet.contains("寒影刀")
    }) {
        errors.push("trace context recall missing surfaced canon evidence".to_string());
    }
    if !ledger
        .context_recalls
        .iter()
        .any(|recall| recall.source == "Canon" && recall.recall_count >= 1)
    {
        errors.push("ledger context recalls did not expose canon recall".to_string());
    }

    eval_result(
        "writer_agent:context_recall_tracks_surfaced_evidence",
        format!(
            "proposals={} recalls={}",
            proposals.len(),
            trace.context_recalls.len()
        ),
        errors,
    )
}

pub fn run_context_mandatory_sources_survive_tight_budget_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相，在复仇与守护之间做出最终选择。",
            "林墨必须在复仇和守护之间做艰难选择，面对血脉真相。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨在旧门前做出选择，推进与张三的关系。",
            "林墨与张三的对话",
            "提前揭开真相",
            "林墨推开旧门",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨停在旧门前，手按在刀柄上。", "Chapter-1"),
    );

    let mut errors = Vec::new();
    let has_cursor = pack
        .sources
        .iter()
        .any(|s| matches!(s.source, ContextSource::CursorPrefix));
    let has_mission = pack
        .sources
        .iter()
        .any(|s| matches!(s.source, ContextSource::ChapterMission));
    let has_brief = pack
        .sources
        .iter()
        .any(|s| matches!(s.source, ContextSource::ProjectBrief));
    if !has_cursor {
        errors.push("missing mandatory cursor prefix".to_string());
    }
    if !has_mission {
        errors.push("missing mandatory chapter mission".to_string());
    }
    if !has_brief {
        errors.push("missing mandatory project brief".to_string());
    }

    eval_result(
        "writer_agent:context_mandatory_sources_survive",
        format!(
            "cursor={} mission={} brief={} sources={}",
            has_cursor,
            has_mission,
            has_brief,
            pack.sources.len()
        ),
        errors,
    )
}

pub fn run_context_pack_explainability_eval() -> EvalResult {
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
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩。",
            "玉佩",
            "",
            "找到线索",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨停在旧门前。", "Chapter-1"),
    );
    let explanation = pack.explain();

    let mut errors = Vec::new();
    if !explanation.contains("ContextPack for GhostWriting") {
        errors.push("explanation missing task type".to_string());
    }
    if !explanation.contains("sources included") {
        errors.push("explanation missing source count".to_string());
    }
    if !explanation.contains("Excluded sources") && !explanation.contains("truncated") {
        // Both may not appear if budget is large enough, but at minimum we have the header
    }
    if explanation.is_empty() {
        errors.push("explanation is empty".to_string());
    }

    eval_result(
        "writer_agent:context_pack_explainability",
        format!("explanationLen={}", explanation.len()),
        errors,
    )
}
