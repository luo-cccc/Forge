pub fn run_story_impact_radius_enters_task_packet_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "寒玉戒指线推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "",
        )
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指",
            "寒玉戒指被黑衣人夺走，必须回收。",
            "Chapter-2",
            "Chapter-5",
            8,
        )
        .unwrap();

    let mut kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation_in_chapter("林墨摸了摸空荡荡的手指。", "Chapter-3");
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "这段接下来怎么推进？".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨摸了摸空荡荡的手指。".to_string(),
            paragraph: "林墨摸了摸空荡荡的手指。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o-mini",
        ),
    );
    let prepared =
        kernel.prepare_task_run(request, provider, StoryImpactEvalToolHandler, "gpt-4o-mini");
    match prepared {
        Ok(prepared) => {
            let has_required_context = prepared
                .task_packet()
                .required_context
                .iter()
                .any(|context| context.source_type == "StoryImpactRadius" && context.required);
            let has_belief = prepared.task_packet().beliefs.iter().any(|belief| {
                belief.subject == "Story Impact Radius"
                    && belief
                        .source
                        .as_deref()
                        .is_some_and(|source| source == "writer.story_impact_radius_built")
                    && belief.statement.contains("risk=")
            });
            if !has_required_context {
                errors.push("task packet missing required StoryImpactRadius context".to_string());
            }
            if !has_belief {
                errors.push("task packet missing Story Impact Radius belief summary".to_string());
            }
            eval_result(
                "writer_agent:story_impact_radius_enters_task_packet",
                format!(
                    "requiredContext={} belief={}",
                    has_required_context, has_belief
                ),
                errors,
            )
        }
        Err(error) => eval_result(
            "writer_agent:story_impact_radius_enters_task_packet",
            format!("prepare=false error={}", error),
            vec![format!("prepare_task_run failed: {}", error)],
        ),
    }
}

pub fn run_story_impact_radius_enters_prompt_context_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "寒玉戒指线推动林墨做选择。",
            "林墨必须在复仇和守护之间做选择。",
            "",
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-3",
            "林墨必须追查寒玉戒指下落，但不能直接揭开黑衣人身份。",
            "寒玉戒指下落",
            "直接揭开黑衣人身份",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指",
            "寒玉戒指被黑衣人夺走，必须回收。",
            "Chapter-2",
            "Chapter-5",
            8,
        )
        .unwrap();

    let mut kernel = WriterAgentKernel::new("eval", memory);
    let obs = observation_in_chapter("林墨摸了摸空荡荡的手指。", "Chapter-3");
    let request = WriterAgentRunRequest {
        task: WriterAgentTask::ManualRequest,
        observation: obs,
        user_instruction: "这段接下来怎么推进？".to_string(),
        frontend_state: WriterAgentFrontendState {
            truncated_context: "林墨摸了摸空荡荡的手指。".to_string(),
            paragraph: "林墨摸了摸空荡荡的手指。".to_string(),
            selected_text: String::new(),
            memory_context: String::new(),
            has_lore: true,
            has_outline: true,
        },
        approval_mode: WriterAgentApprovalMode::SurfaceProposals,
        stream_mode: WriterAgentStreamMode::Text,
        manual_history: Vec::new(),
    };
    let provider = std::sync::Arc::new(
        agent_harness_core::provider::openai_compat::OpenAiCompatProvider::new(
            "https://api.invalid/v1",
            "sk-eval",
            "gpt-4o-mini",
        ),
    );
    let prepared =
        kernel.prepare_task_run(request, provider, StoryImpactEvalToolHandler, "gpt-4o-mini");

    match prepared {
        Ok(prepared) => {
            let prompt = prepared.system_prompt();
            let prompt_has_source = prompt.contains("## StoryImpactRadius")
                && prompt.contains("Story Impact Radius")
                && prompt.contains("risk:");
            let source_refs_have_impact = prepared
                .source_refs()
                .iter()
                .any(|source| source == "StoryImpactRadius");
            let trace = kernel.trace_snapshot(20);
            let context_event_has_impact = trace.run_events.iter().any(|event| {
                event.event_type == "writer.context_pack_built"
                    && event
                        .source_refs
                        .iter()
                        .any(|source| source == "context_source:StoryImpactRadius")
                    && event
                        .data
                        .get("sourceReports")
                        .and_then(|value| value.as_array())
                        .is_some_and(|reports| {
                            reports.iter().any(|report| {
                                report.get("source").and_then(|value| value.as_str())
                                    == Some("StoryImpactRadius")
                                    && report
                                        .get("provided")
                                        .and_then(|value| value.as_u64())
                                        .unwrap_or(0)
                                        > 0
                            })
                        })
            });
            if !prompt_has_source {
                errors.push("system prompt missing StoryImpactRadius context source".to_string());
            }
            if !source_refs_have_impact {
                errors.push("prepared source refs missing StoryImpactRadius".to_string());
            }
            if !context_event_has_impact {
                errors.push(
                    "context_pack_built run event missing StoryImpactRadius source report"
                        .to_string(),
                );
            }
            eval_result(
                "writer_agent:story_impact_radius_enters_prompt_context",
                format!(
                    "prompt={} sourceRefs={} contextEvent={}",
                    prompt_has_source, source_refs_have_impact, context_event_has_impact
                ),
                errors,
            )
        }
        Err(error) => eval_result(
            "writer_agent:story_impact_radius_enters_prompt_context",
            format!("prepare=false error={}", error),
            vec![format!("prepare_task_run failed: {}", error)],
        ),
    }
}

pub fn run_story_impact_radius_small_change_stays_minimal_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "task:1",
        StoryNodeKind::SeedTask,
        "",
        1.0,
        "fix typo",
    )];
    let nodes = vec![
        seeds[0].clone(),
        make_si_node("canon:1", StoryNodeKind::CanonEntity, "Lin", 0.9, "char"),
    ];
    let edges = vec![];
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, 500);
    if radius.impacted_nodes.len() != 1 {
        errors.push(format!(
            "expected 1 node, got {}",
            radius.impacted_nodes.len()
        ));
    }
    if radius.truncated {
        errors.push("should not be truncated".to_string());
    }
    if !matches!(radius.risk, StoryImpactRisk::Low) {
        errors.push("should be low risk".to_string());
    }
    eval_result(
        "writer_agent:story_impact_radius_small_change_stays_minimal",
        format!(
            "impacted={} risk={:?}",
            radius.impacted_nodes.len(),
            radius.risk
        ),
        errors,
    )
}

pub fn run_story_impact_radius_prioritizes_high_value_nodes_eval() -> EvalResult {
    let mut errors = Vec::new();
    let seeds = vec![make_si_node(
        "task:1",
        StoryNodeKind::SeedTask,
        "",
        1.0,
        "draft scene",
    )];
    let noisy_chunk_summary = "background archive note ".repeat(8);
    let promise_summary = "promise: ring payoff";
    let nodes = vec![
        seeds[0].clone(),
        make_si_node(
            "chunk:noisy",
            StoryNodeKind::ProjectBrainChunk,
            "old archive chunk",
            0.95,
            &noisy_chunk_summary,
        ),
        make_si_node(
            "promise:ring",
            StoryNodeKind::PlotPromise,
            "jade ring payoff",
            0.82,
            promise_summary,
        ),
    ];
    let edges = vec![
        make_si_edge("task:1", "chunk:noisy", StoryEdgeKind::SharedKeyword),
        make_si_edge("task:1", "promise:ring", StoryEdgeKind::UpdatesPromise),
    ];
    let budget = seeds[0].summary.chars().count() + promise_summary.chars().count() + 2;
    let radius = compute_story_impact_radius(&seeds, &nodes, &edges, budget);
    let has_promise = radius
        .impacted_nodes
        .iter()
        .any(|node| node.id == "promise:ring");
    let has_noisy_chunk = radius
        .impacted_nodes
        .iter()
        .any(|node| node.id == "chunk:noisy");

    if !has_promise {
        errors.push("tight budget should keep high-value promise node".to_string());
    }
    if has_noisy_chunk {
        errors.push("low-value noisy chunk should not consume budget before promise".to_string());
    }
    if !radius.truncated {
        errors.push("tight budget should report truncation".to_string());
    }

    eval_result(
        "writer_agent:story_impact_radius_prioritizes_high_value_nodes",
        format!(
            "promise={} noisyChunk={} impacted={} truncated={}",
            has_promise,
            has_noisy_chunk,
            radius.impacted_nodes.len(),
            radius.truncated
        ),
        errors,
    )
}

pub fn run_story_impact_budget_counts_only_reached_drops_eval() -> EvalResult {
    let mut errors = Vec::new();
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "寒玉戒指",
            &[],
            &"寒玉戒指的真实功能、限制和来源必须保留。".repeat(20),
            &serde_json::json!({}),
            0.9,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "place",
            "无关旧井",
            &[],
            &"旧井传闻暂时与当前章节任务无关。".repeat(20),
            &serde_json::json!({}),
            0.9,
        )
        .unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-3",
            "林墨必须追查寒玉戒指，但不要转入旧井传闻。",
            "寒玉戒指线索",
            "旧井传闻",
            "以戒指的新线索收束。",
            "eval",
        )
        .unwrap();

    let kernel = WriterAgentKernel::new("eval", memory);
    let observation = observation_in_chapter("林墨摸了摸空荡荡的手指。", "Chapter-3");
    let pack = kernel.context_pack_for_default(AgentTask::GhostWriting, &observation);
    let (radius, budget) = compute_story_impact(&observation, &pack, &kernel.memory, Some(64));
    let dropped_ids = radius
        .dropped_nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();

    if budget.truncated_node_count != 1 {
        errors.push(format!(
            "budget should count only reached dropped nodes, got {}",
            budget.truncated_node_count
        ));
    }
    if !dropped_ids.contains(&"canon:寒玉戒指") {
        errors.push(format!(
            "reachable canon node should be the only budget drop: {:?}",
            dropped_ids
        ));
    }
    if dropped_ids.contains(&"canon:无关旧井") {
        errors.push(format!(
            "disconnected canon node should not be counted as a budget drop: {:?}",
            dropped_ids
        ));
    }
    if budget
        .dropped_high_risk_sources
        .iter()
        .any(|source| source.contains("无关旧井"))
    {
        errors.push(format!(
            "disconnected promise should not be reported as dropped high-risk source: {:?}",
            budget.dropped_high_risk_sources
        ));
    }

    eval_result(
        "writer_agent:story_impact_budget_counts_only_reached_drops",
        format!(
            "truncatedNodes={} droppedHighRisk={}",
            budget.truncated_node_count,
            budget.dropped_high_risk_sources.len()
        ),
        errors,
    )
}
