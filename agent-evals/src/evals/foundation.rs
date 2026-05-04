use super::*;

pub fn run_foundation_write_validation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let contract_result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StoryContractUpsert {
                contract: agent_writer_lib::writer_agent::operation::StoryContractOp {
                    project_id: "eval".to_string(),
                    title: "寒影录".to_string(),
                    genre: "玄幻".to_string(),
                    target_reader: "".to_string(),
                    reader_promise: "爽文".to_string(),
                    first_30_chapter_promise: "".to_string(),
                    main_conflict: "复仇".to_string(),
                    structural_boundary: "".to_string(),
                    tone_contract: "".to_string(),
                },
            },
            "",
            Some(&eval_approval("foundation_validation")),
        )
        .unwrap();
    let mission_result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::ChapterMissionUpsert {
                mission: agent_writer_lib::writer_agent::operation::ChapterMissionOp {
                    project_id: "eval".to_string(),
                    chapter_title: "Chapter-1".to_string(),
                    mission: "打架".to_string(),
                    must_include: "".to_string(),
                    must_not: "剧透".to_string(),
                    expected_ending: "".to_string(),
                    status: "in_progress".to_string(),
                    source_ref: "eval".to_string(),
                    blocked_reason: String::new(),
                    retired_history: String::new(),
                },
            },
            "",
            Some(&eval_approval("foundation_validation")),
        )
        .unwrap();
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    if contract_result.success {
        errors.push("incomplete story contract was accepted".to_string());
    }
    if !contract_result
        .error
        .as_ref()
        .is_some_and(|error| error.code == "invalid" && error.message.contains("Story Contract"))
    {
        errors.push(format!(
            "story contract validation error was not explicit: {:?}",
            contract_result.error
        ));
    }
    if mission_result.success {
        errors.push("vague chapter mission was accepted".to_string());
    }
    if !mission_result
        .error
        .as_ref()
        .is_some_and(|error| error.code == "invalid" && error.message.contains("Chapter Mission"))
    {
        errors.push(format!(
            "chapter mission validation error was not explicit: {:?}",
            mission_result.error
        ));
    }
    if ledger.story_contract.is_some() || ledger.active_chapter_mission.is_some() {
        errors.push("invalid foundation writes polluted the ledger snapshot".to_string());
    }

    eval_result(
        "writer_agent:foundation_write_validation",
        format!(
            "contract={} mission={}",
            contract_result.success, mission_result.success
        ),
        errors,
    )
}

pub fn run_story_contract_guard_eval() -> EvalResult {
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三终于说出真相：玉佩其实来自禁地。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let guard = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::StoryContract);
    if guard.is_none() {
        errors.push("missing story contract guard proposal".to_string());
    }
    if !guard.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::StoryContract)
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::TextAnnotate { .. }))
    }) {
        errors.push(
            "story contract guard lacks contract evidence or annotation operation".to_string(),
        );
    }
    if !debt.entries.iter().any(|entry| {
        entry.category == StoryDebtCategory::StoryContract && entry.title.contains("contract")
    }) {
        errors.push("story contract guard did not enter story debt".to_string());
    }

    eval_result(
        "writer_agent:story_contract_guard_story_debt",
        format!("proposals={} debt={}", proposals.len(), debt.total),
        errors,
    )
}

pub fn run_story_contract_negated_guard_eval() -> EvalResult {
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三没有说出真相，也拒绝解释玉佩来源。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    if proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::StoryContract)
    {
        errors.push("negated reveal still created story contract proposal".to_string());
    }
    if debt.contract_count != 0 {
        errors.push(format!(
            "negated reveal created {} contract debts",
            debt.contract_count
        ));
    }

    eval_result(
        "writer_agent:story_contract_negated_reveal_no_debt",
        format!(
            "proposals={} contractDebt={}",
            proposals.len(),
            debt.contract_count
        ),
        errors,
    )
}

pub fn run_story_contract_quality_nominal_eval() -> EvalResult {
    let empty = StoryContractSummary::default();
    let mut vague = StoryContractSummary::default();
    vague.project_id = "eval".to_string();
    vague.title = "测试".to_string();
    vague.genre = "玄幻".to_string();
    vague.reader_promise = "一个故事".to_string();
    vague.main_conflict = "冲突".to_string();
    let mut usable = vague.clone();
    usable.reader_promise = "刀客追查玉佩真相，在复仇与守护之间做出最终选择。".to_string();
    usable.main_conflict = "林墨必须在复仇和守护之间做艰难选择。".to_string();
    usable.tone_contract = "冷峻克制的武侠叙述".to_string();
    let mut strong = usable.clone();
    strong.reader_promise =
        "刀客追查玉佩真相，在复仇与守护之间做出最终选择，揭示隐藏身份。".to_string();
    strong.main_conflict = "林墨必须在复仇和守护之间做艰难选择，同时面对血脉真相。".to_string();
    strong.first_30_chapter_promise = "前30章完成玉佩线第一次大转折".to_string();
    strong.structural_boundary = "不得提前泄露玉佩来源".to_string();
    strong.tone_contract = "冷峻克制的武侠叙述，对话精准，心理描写内敛".to_string();

    let mut errors = Vec::new();
    if empty.quality() != StoryContractQuality::Missing {
        errors.push("empty contract should be Missing".to_string());
    }
    if vague.quality() != StoryContractQuality::Vague {
        errors.push(format!("vague contract was {:?}", vague.quality()));
    }
    if usable.quality() != StoryContractQuality::Usable {
        errors.push(format!("usable contract was {:?}", usable.quality()));
    }
    if strong.quality() != StoryContractQuality::Strong {
        errors.push(format!("strong contract was {:?}", strong.quality()));
    }
    if empty.quality_gaps().len() < 4 {
        errors.push("empty contract should report several gaps".to_string());
    }
    if vague.quality_gaps().len() < 3 {
        errors.push("vague contract should report specific gaps".to_string());
    }
    if strong.quality_gaps().len() != 0 {
        errors.push("strong contract should have zero gaps".to_string());
    }

    eval_result(
        "writer_agent:story_contract_quality_nominal",
        format!(
            "qualities={:?}/{:?}/{:?}/{:?} strongGaps={}",
            empty.quality(),
            vague.quality(),
            usable.quality(),
            strong.quality(),
            strong.quality_gaps().len()
        ),
        errors,
    )
}

pub fn run_story_contract_vague_excluded_from_context_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed("eval", "寒影录", "玄幻", "一个故事", "选择", "")
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation("林墨停在旧门前。");
    let pack = kernel.context_pack_for_default(
        agent_writer_lib::writer_agent::context::AgentTask::GhostWriting,
        &obs,
    );

    let mut errors = Vec::new();
    let contract_source = pack.sources.iter().find(|source| {
        matches!(
            source.source,
            agent_writer_lib::writer_agent::context::ContextSource::ProjectBrief
        )
    });
    if contract_source.is_some() {
        errors.push("vague StoryContract leaked into context pack".to_string());
    }
    if pack.sources.is_empty() {
        errors.push("context pack has zero sources after vague contract exclusion".to_string());
    }

    eval_result(
        "writer_agent:story_contract_vague_excluded_from_context",
        format!(
            "sources={} contractIncluded={}",
            pack.sources.len(),
            contract_source.is_some()
        ),
        errors,
    )
}

pub fn run_story_contract_quality_chapter_gen_eval() -> EvalResult {
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-2".to_string());
    let obs = observation_in_chapter("林墨拔出长刀，迎着风雪走向旧门。", "Chapter-2");
    let pack = kernel.context_pack_for_default(
        agent_writer_lib::writer_agent::context::AgentTask::ChapterGeneration,
        &obs,
    );

    let mut errors = Vec::new();
    let contract_source = pack.sources.iter().find(|source| {
        matches!(
            source.source,
            agent_writer_lib::writer_agent::context::ContextSource::ProjectBrief
        )
    });
    if contract_source.is_none() {
        errors.push("chapter generation pack must include StoryContract source".to_string());
    }
    if let Some(source) = contract_source {
        if !source.content.contains("合同质量") {
            errors.push("chapter generation contract source lacks quality annotation".to_string());
        }
        if !source.content.contains("可用") && !source.content.contains("完整") {
            errors.push("chapter generation contract source quality not visible".to_string());
        }
    }

    eval_result(
        "writer_agent:story_contract_quality_chapter_gen",
        format!(
            "sources={} contractIncluded={}",
            pack.sources.len(),
            contract_source.is_some()
        ),
        errors,
    )
}
