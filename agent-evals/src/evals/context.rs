use super::*;

pub fn run_context_budget_eval() -> EvalResult {
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
            4,
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨看向张三，想起那枚玉佩，却没有把手从寒影刀上移开。");
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &obs, 1_200);

    let mut errors = Vec::new();
    if pack.total_chars > pack.budget_limit {
        errors.push(format!(
            "context exceeded budget: used {} > {}",
            pack.total_chars, pack.budget_limit
        ));
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::CanonSlice)
    {
        errors.push("missing relevant canon slice".to_string());
    }
    if !pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::PromiseSlice)
    {
        errors.push("missing active promise slice".to_string());
    }

    eval_result(
        "writer_agent:context_budget_required_sources",
        format!("sources={} used={}", pack.sources.len(), pack.total_chars),
        errors,
    )
}

pub fn run_context_budget_trace_eval() -> EvalResult {
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation(
        "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。",
    );
    let proposals = kernel.observe(obs).unwrap();
    let ghost = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost);
    let trace = kernel.trace_snapshot(10);
    let trace_budget = ghost.and_then(|proposal| {
        trace
            .recent_proposals
            .iter()
            .find(|entry| entry.id == proposal.id)
            .and_then(|entry| entry.context_budget.as_ref())
    });

    let mut errors = Vec::new();
    let actual = if let Some(budget) = trace_budget {
        if budget.task != "GhostWriting" {
            errors.push(format!("unexpected task {}", budget.task));
        }
        if budget.used > budget.total_budget {
            errors.push(format!(
                "trace budget exceeded: used {} > {}",
                budget.used, budget.total_budget
            ));
        }
        if budget.source_reports.is_empty() {
            errors.push("trace missing source budget reports".to_string());
        }
        format!(
            "task={} used={} total={} sources={}",
            budget.task,
            budget.used,
            budget.total_budget,
            budget.source_reports.len()
        )
    } else {
        errors.push("missing context budget trace for ghost proposal".to_string());
        "missing".to_string()
    };

    eval_result("writer_agent:context_budget_trace", actual, errors)
}

pub fn run_context_source_trend_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。林墨必须在复仇和守护之间做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-3",
            "承接上一章玉佩线索，并让林墨发现张三的新隐瞒。",
            "玉佩线索",
            "提前揭开玉佩来源",
            "以张三隐瞒的新证据收束。",
            "eval",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀，正在追查玉佩线。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-3".to_string());
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨深吸一口气，说道：“这件事我本来不该告诉你，可玉佩线索已经把我们推到这里。”他停在门外，没有立刻回头。",
            "Chapter-3",
        ))
        .unwrap();
    let trace = kernel.trace_snapshot(10);
    let trends = &trace.context_source_trends;
    let cursor = trends.iter().find(|trend| trend.source == "CursorPrefix");
    let mission = trends.iter().find(|trend| trend.source == "ChapterMission");
    let any_budget_sources = trace
        .recent_proposals
        .iter()
        .filter_map(|proposal| proposal.context_budget.as_ref())
        .flat_map(|budget| budget.source_reports.iter())
        .count();

    let mut errors = Vec::new();
    if proposals.is_empty() {
        errors.push("fixture should produce at least one proposal".to_string());
    }
    if any_budget_sources == 0 {
        errors.push("fixture should expose proposal context budget reports".to_string());
    }
    if trends.is_empty() {
        errors.push("trace missing context source trends".to_string());
    }
    if !cursor.is_some_and(|trend| {
        trend.appearances >= 1
            && trend.provided_count >= 1
            && trend.total_provided > 0
            && trend.average_provided > 0.0
    }) {
        errors.push(format!("cursor trend missing or empty: {:?}", cursor));
    }
    if !mission.is_some_and(|trend| trend.appearances >= 1 && trend.provided_count >= 1) {
        errors.push(format!(
            "chapter mission trend missing or empty: {:?}",
            mission
        ));
    }

    eval_result(
        "writer_agent:context_source_trend",
        format!(
            "trends={} budgetSources={} cursorProvided={} missionProvided={}",
            trends.len(),
            any_budget_sources,
            cursor.map(|trend| trend.provided_count).unwrap_or_default(),
            mission
                .map(|trend| trend.provided_count)
                .unwrap_or_default()
        ),
        errors,
    )
}

pub fn run_context_source_trend_pressure_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let premise = "刀客追查玉佩真相，必须保留复仇与守护的双重压力。".repeat(8);
    let promise = "林墨必须在复仇和守护之间做选择。".repeat(10);
    let boundary = "不得提前泄露玉佩来源。".repeat(10);
    let mission_goal =
        "林墨要在审讯张三时逼近玉佩真相，但不能让读者提前知道玉佩来自北境密库。".repeat(8);
    let ending = "以张三交出一枚伪造令牌收束。".repeat(5);
    memory
        .ensure_story_contract_seed("eval", "寒影录", "玄幻", &premise, &promise, &boundary)
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-8",
            &mission_goal,
            "审讯张三、保留玉佩来源悬念、暴露新的行动线索",
            "直接揭开玉佩真实来源",
            &ending,
            "eval",
        )
        .unwrap();
    for idx in 0..6 {
        memory
            .upsert_canon_entity(
                "character",
                &format!("线索人物{}", idx),
                &[],
                &format!(
                    "线索人物{}知道玉佩线的一部分，但每个人都只提供含混证词。{}",
                    idx,
                    "寒玉戒指、北境密库、伪造令牌、张三隐瞒。".repeat(8)
                ),
                &serde_json::json!({ "clue": "玉佩", "chapter": idx }),
                0.9,
            )
            .unwrap();
    }
    for idx in 0..5 {
        memory
            .add_promise(
                "clue",
                &format!("玉佩线索{}", idx),
                &format!(
                    "第{}条线索必须在审讯后续回收，不能提前解释来源。{}",
                    idx,
                    "延迟揭示。".repeat(20)
                ),
                "Chapter-3",
                "Chapter-12",
                5,
            )
            .unwrap();
    }
    memory
        .upsert_style_preference(
            "dialogue_subtext",
            &"对白保持克制，用动作和停顿暗示压力，不用直白解释。".repeat(12),
            true,
        )
        .unwrap();

    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-8".to_string());
    let observation = observation_in_chapter(
        &format!(
            "{}林墨看着张三，问他那枚玉佩到底从谁手里来。",
            "审讯室里只剩烛火和铁链轻响。".repeat(60)
        ),
        "Chapter-8",
    );
    let pack = kernel.context_pack_for(AgentTask::GhostWriting, &observation, 720);
    let dropped = pack
        .budget_report
        .source_reports
        .iter()
        .filter(|report| report.provided == 0)
        .count();
    let truncated = pack
        .budget_report
        .source_reports
        .iter()
        .filter(|report| report.truncated)
        .count();
    let dropped_reason = pack.budget_report.source_reports.iter().any(|report| {
        report.provided == 0
            && report.reason.contains("dropped")
            && report
                .truncation_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("ContextPack total budget"))
    });
    let cursor_report = pack
        .budget_report
        .source_reports
        .iter()
        .find(|report| report.source == "CursorPrefix");

    let proposals = kernel.observe(observation).unwrap();
    let trace = kernel.trace_snapshot(10);
    let has_trace_pressure = trace.context_source_trends.iter().any(|trend| {
        trend.truncated_count > 0
            && trend.total_requested > trend.total_provided
            && trend.last_truncation_reason.is_some()
    });

    let mut errors = Vec::new();
    if pack.total_chars > pack.budget_limit {
        errors.push(format!(
            "pressure fixture exceeded budget: used {} > {}",
            pack.total_chars, pack.budget_limit
        ));
    }
    if dropped == 0 {
        errors.push("tight context pack did not expose any dropped sources".to_string());
    }
    if truncated == 0 {
        errors.push("tight context pack did not expose any truncated sources".to_string());
    }
    if !dropped_reason {
        errors.push("dropped source did not carry budget-exhaustion reason".to_string());
    }
    if !cursor_report.is_some_and(|report| report.provided > 0 && report.truncated) {
        errors.push(format!(
            "required cursor source should be included but pressure-marked: {:?}",
            cursor_report
        ));
    }
    if proposals.is_empty() {
        errors.push("pressure fixture should still produce proposals".to_string());
    }
    if !has_trace_pressure {
        errors.push("trace trends did not expose truncated source pressure".to_string());
    }

    eval_result(
        "writer_agent:context_source_trend_pressure",
        format!(
            "tightSources={} dropped={} truncated={} traceTrends={}",
            pack.budget_report.source_reports.len(),
            dropped,
            truncated,
            trace.context_source_trends.len()
        ),
        errors,
    )
}

pub fn run_context_window_guard_eval() -> EvalResult {
    let messages = vec![agent_harness_core::provider::LlmMessage {
        role: "user".to_string(),
        content: Some("风".repeat(12_000)),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];
    let guard = agent_harness_core::evaluate_context_window(
        agent_harness_core::ContextWindowInfo {
            tokens: 4_096,
            reference_tokens: None,
            source: agent_harness_core::ContextWindowSource::Env,
        },
        agent_harness_core::context_window_guard::estimate_request_tokens(&messages, None),
        512,
    );

    let mut errors = Vec::new();
    if !guard.should_block {
        errors.push("oversized prompt did not block against small context window".to_string());
    }
    if !guard
        .message
        .as_deref()
        .is_some_and(|message| message.contains("too small"))
    {
        errors.push("guard message does not explain context window failure".to_string());
    }

    eval_result(
        "writer_agent:context_window_guard_blocks_small_model",
        format!(
            "ctx={} estimated={} output={} block={}",
            guard.tokens,
            guard.estimated_input_tokens,
            guard.requested_output_tokens,
            guard.should_block
        ),
        errors,
    )
}

pub fn run_compaction_latest_user_anchor_eval() -> EvalResult {
    let messages = vec![
        eval_llm_message("user", "旧请求：分析第一章"),
        eval_llm_message("assistant", "旧回答：第一章节奏偏慢"),
        eval_llm_message("user", "ACTIVE TASK: 继续写第七章的审讯场景"),
        eval_llm_message("assistant", "我正在查看上下文"),
        eval_llm_message("assistant", "准备续写"),
    ];
    let anchored = agent_harness_core::anchor_latest_user_message(&messages, 4);
    let safe = agent_harness_core::find_safe_boundary(&messages, anchored);
    let preserved = &messages[safe..];

    let mut errors = Vec::new();
    if anchored != 2 {
        errors.push(format!(
            "latest user anchor should move cut to 2, got {}",
            anchored
        ));
    }
    if !preserved.iter().any(|message| {
        message.role == "user"
            && message
                .content
                .as_deref()
                .is_some_and(|content| content.contains("ACTIVE TASK"))
    }) {
        errors.push("latest user task was not preserved in compaction tail".to_string());
    }

    eval_result(
        "agent_harness:compaction_preserves_latest_user_task",
        format!(
            "anchored={} safe={} preserved={}",
            anchored,
            safe,
            preserved.len()
        ),
        errors,
    )
}

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
