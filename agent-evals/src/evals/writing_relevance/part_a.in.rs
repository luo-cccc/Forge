pub fn run_current_plot_relevance_prioritizes_same_name_entity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-7",
            "北境林墨在雪线外追查寒玉戒指下落。",
            "北境林墨与寒玉戒指",
            "不要切到南境支线",
            "以寒玉戒指出现新线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &["北境林墨".to_string()],
            "北境线主角，追查寒玉戒指，被黑衣人追杀。",
            &serde_json::json!({"arc": "北境", "object": "寒玉戒指"}),
            0.9,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨影",
            &["南境林墨".to_string()],
            "南境支线人物，负责朝堂密信。",
            &serde_json::json!({"arc": "南境", "object": "密信"}),
            0.9,
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter(
            "林墨摸到雪地里的戒指痕迹，黑衣人的脚印还很新。",
            "Chapter-7",
        ),
    );

    let canon = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::CanonSlice);
    let canon_text = canon.map(|source| source.content.as_str()).unwrap_or("");
    let north_pos = canon_text.find("北境线主角").unwrap_or(usize::MAX);
    let south_pos = canon_text.find("南境支线人物").unwrap_or(usize::MAX);

    let mut errors = Vec::new();
    if canon.is_none() {
        errors.push("missing canon slice".to_string());
    }
    if north_pos == usize::MAX {
        errors.push("current plot entity missing from canon slice".to_string());
    }
    if south_pos != usize::MAX && north_pos > south_pos {
        errors.push("less relevant same-name entity ranked before current plot entity".to_string());
    }
    if !canon_text.contains("WHY writing_relevance") || !canon_text.contains("mission/result") {
        errors.push("canon slice lacks writing relevance explanation".to_string());
    }

    eval_result(
        "writer_agent:current_plot_relevance_prioritizes_same_name_entity",
        format!("northPos={} southPos={}", north_pos, south_pos),
        errors,
    )
}

pub fn run_promise_relevance_beats_plain_similarity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-4",
            "林墨必须决定是否公开寒玉戒指的下落。",
            "寒玉戒指下落",
            "不要用无关传闻稀释主线",
            "以戒指下落产生新代价收束。",
            "eval",
        )
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指下落",
            "黑衣人夺走寒玉戒指，林墨必须查清它被带往何处。",
            "Chapter-2",
            "Chapter-4",
            3,
        )
        .unwrap();
    memory
        .add_promise(
            "mystery_clue",
            "旧门传闻",
            "旧门外的风声像传闻中的哭声，需要后续解释。",
            "Chapter-1",
            "Chapter-9",
            9,
        )
        .unwrap();
    memory
        .record_decision(
            "Chapter-3",
            "寒玉戒指暂不公开",
            "accepted",
            &[],
            "先让林墨独自承担戒指下落的风险。",
            &[],
        )
        .unwrap();
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨合上掌心，戒指的冷意还没有散。", "Chapter-4"),
    );

    let promise = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice);
    let promise_text = promise.map(|source| source.content.as_str()).unwrap_or("");
    let ring_pos = promise_text.find("寒玉戒指下落").unwrap_or(usize::MAX);
    let rumor_pos = promise_text.find("旧门传闻").unwrap_or(usize::MAX);

    let mut errors = Vec::new();
    if promise.is_none() {
        errors.push("missing promise slice".to_string());
    }
    if ring_pos == usize::MAX {
        errors.push("mission-relevant promise missing from promise slice".to_string());
    }
    if rumor_pos != usize::MAX && ring_pos > rumor_pos {
        errors
            .push("plain high-priority promise ranked before mission-relevant promise".to_string());
    }
    if !promise_text.contains("WHY writing_relevance")
        || !promise_text.contains("current chapter is expected payoff")
    {
        errors.push("promise slice lacks relevance explanation for payoff timing".to_string());
    }

    eval_result(
        "writer_agent:promise_relevance_beats_plain_similarity",
        format!("ringPos={} rumorPos={}", ring_pos, rumor_pos),
        errors,
    )
}

pub fn run_project_brain_writing_relevance_rerank_eval() -> EvalResult {
    let chunks = vec![
        (
            50.0,
            (
                "semantic-distractor",
                "旧门外的风声像传闻中的哭声，林墨反复听见旧门、风声、旧门和风声。",
            ),
        ),
        (
            1.0,
            (
                "mission-relevant",
                "黑衣人夺走寒玉戒指后留下北境雪线脚印，林墨必须查清寒玉戒指下落。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章必须追查寒玉戒指下落，不要被旧门传闻稀释主线。",
        |(_, text)| text.to_string(),
    );
    let first_id = reranked
        .first()
        .map(|(_, _, (id, _))| *id)
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if first_id != "mission-relevant" {
        errors.push(format!(
            "mission-relevant project brain chunk should outrank semantic distractor, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("WHY writing_relevance")
        || !first_explanation.contains("寒玉戒指")
    {
        errors.push(format!(
            "missing writing relevance explanation for reranked chunk: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_writing_relevance_rerank",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_scene_type_relevance_signal_eval() -> EvalResult {
    let focus_scene_types = writing_scene_types("本章要揭开寒玉戒指来源的真相，并回收前文伏笔。");
    let chunks = vec![
        (
            42.0,
            (
                "surface-similar",
                "林墨摩挲寒玉戒指，旧门外的风声反复敲打窗棂，气味潮湿。",
            ),
        ),
        (
            1.0,
            (
                "reveal-scene",
                "张三终于说出真相：寒玉戒指来源于北境宗门旧案，这条线索回收了母亲遗物的伏笔。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章要揭开寒玉戒指来源的真相，并回收前文伏笔。",
        |(_, text)| text.to_string(),
    );
    let first_id = reranked
        .first()
        .map(|(_, _, (id, _))| *id)
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if !focus_scene_types.iter().any(|scene| scene == "reveal")
        || !focus_scene_types
            .iter()
            .any(|scene| scene == "setup_payoff")
    {
        errors.push(format!(
            "focus scene types should include reveal and setup_payoff, got {:?}",
            focus_scene_types
        ));
    }
    if first_id != "reveal-scene" {
        errors.push(format!(
            "reveal scene should outrank surface-similar description, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("scene type reveal")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "rerank explanation missing scene type signals: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:scene_type_relevance_signal",
        format!(
            "first={} scenes={:?} explanation={}",
            first_id, focus_scene_types, first_explanation
        ),
        errors,
    )
}

pub fn run_project_brain_uses_writer_memory_focus_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-4",
            "林墨必须追查寒玉戒指下落，并在本章揭开戒指来源线索。",
            "寒玉戒指下落",
            "不要被旧门传闻稀释主线",
            "以戒指来源的新线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .record_chapter_result(
            &agent_writer_lib::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: "Chapter-3".to_string(),
                chapter_revision: "rev-3".to_string(),
                summary: "黑衣人夺走寒玉戒指后留下北境雪线脚印。".to_string(),
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec!["寒玉戒指被带往北境".to_string()],
                promise_updates: vec!["寒玉戒指下落: 待查清".to_string()],
                canon_updates: vec![],
                source_ref: "eval".to_string(),
                created_at: now_ms(),
            },
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-4".to_string());
    let focus = ProjectBrainFocus::from_kernel("旧门风声有什么含义？", &kernel);
    let chunks = [agent_harness_core::Chunk {
            id: "old-door".to_string(),
            chapter: "Chapter-1".to_string(),
            text: "旧门外的风声像传闻中的哭声，林墨反复听见旧门、风声和旧门。".to_string(),
            embedding: vec![1.0, 0.0],
            keywords: vec!["旧门".to_string(), "风声".to_string()],
            topic: Some("旧门传闻".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        },
        agent_harness_core::Chunk {
            id: "ring-focus".to_string(),
            chapter: "Chapter-3".to_string(),
            text: "黑衣人夺走寒玉戒指后留下北境雪线脚印，林墨必须查清寒玉戒指下落。".to_string(),
            embedding: vec![],
            keywords: vec!["寒玉戒指".to_string(), "下落".to_string()],
            topic: Some("寒玉戒指下落".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        }];
    let raw_results = vec![(50.0, &chunks[0]), (1.0, &chunks[1])];
    let reranked = rerank_project_brain_results_with_focus(raw_results, &focus);
    let first_id = reranked
        .first()
        .map(|(_, _, chunk)| chunk.id.as_str())
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if !focus.as_str().contains("寒玉戒指下落") {
        errors.push("writer memory focus missing active chapter mission".to_string());
    }
    if first_id != "ring-focus" {
        errors.push(format!(
            "writer memory focus should lift active mission chunk above query-similar chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("寒玉戒指") {
        errors.push(format!(
            "rerank explanation missing memory focus term: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_writer_memory_focus",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

