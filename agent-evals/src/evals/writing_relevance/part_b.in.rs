pub fn run_project_brain_long_session_candidate_recall_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-9",
            "林墨必须追查寒玉戒指下落，并揭开黑衣人把戒指带往北境宗门的来源线索。",
            "寒玉戒指下落",
            "不要被旧门传闻或无关闲谈稀释主线",
            "以戒指来源的新线索收束。",
            "eval",
        )
        .unwrap();
    memory
        .record_chapter_result(
            &agent_writer_lib::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: "Chapter-8".to_string(),
                chapter_revision: "rev-8".to_string(),
                summary: "黑衣人带着寒玉戒指越过北境界碑，留下宗门旧印。".to_string(),
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec!["寒玉戒指被带往北境宗门".to_string()],
                promise_updates: vec!["寒玉戒指下落: 北境宗门待查".to_string()],
                canon_updates: vec![],
                source_ref: "eval".to_string(),
                created_at: now_ms(),
            },
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-9".to_string());
    let focus = ProjectBrainFocus::from_kernel("旧门风声有什么含义？", &kernel);

    let mut db = VectorDB::new();
    for i in 0..8 {
        db.upsert(Chunk {
            id: format!("old-door-noise-{}", i + 1),
            chapter: format!("Chapter-{}", i + 1),
            text: format!(
                "旧门外的风声在第{}夜反复出现，旧门、风声、旧门传闻、寒玉戒指传闻、北境宗门闲谈、戒指来源闲谈、下落猜测、线索闲谈和林墨的犹疑被路人反复提起。",
                i + 1
            ),
            embedding: vec![],
            keywords: vec!["旧门".to_string(), "风声".to_string()],
            topic: Some("旧门风声传闻".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        });
    }
    db.upsert(Chunk {
        id: "ring-long-session".to_string(),
        chapter: "Chapter-8".to_string(),
        text: "黑衣人带着寒玉戒指抵达北境宗门，宗门旧印揭开戒指来源线索，林墨必须查清寒玉戒指下落，并以戒指来源线索收束本章承诺。"
            .to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec![
            "寒玉戒指".to_string(),
            "北境".to_string(),
            "宗门".to_string(),
        ],
        topic: Some("寒玉戒指下落".to_string()),
        source_ref: None,
        source_revision: None,
        source_kind: None,
        chunk_index: None,
        archived: false,
    });

    let search_text = focus.search_text();
    let embedding = vec![1.0, 0.0];
    let query_only_top_five = db.search_hybrid("旧门风声有什么含义？", &embedding, 5);
    let query_only_contains_ring = query_only_top_five
        .iter()
        .any(|(_, chunk)| chunk.id == "ring-long-session");
    let narrow_focus_top_five = db.search_hybrid(&search_text, &embedding, 5);
    let narrow_focus_contains_ring = narrow_focus_top_five
        .iter()
        .any(|(_, chunk)| chunk.id == "ring-long-session");
    let reranked = search_project_brain_results_with_focus(&db, &focus, &embedding);
    let first_id = reranked
        .first()
        .map(|(_, _, chunk)| chunk.id.as_str())
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if query_only_contains_ring {
        errors.push("fixture should prove query-only top-5 would miss mission chunk".to_string());
    }
    if first_id != "ring-long-session" {
        errors.push(format!(
            "expanded Project Brain candidate pool should recall and prioritize mission chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("寒玉戒指")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "rerank explanation missing mission and payoff signals: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_long_session_candidate_recall",
        format!(
            "queryOnlyTop5ContainsRing={} narrowFocusTop5ContainsRing={} first={} explanation={}",
            query_only_contains_ring, narrow_focus_contains_ring, first_id, first_explanation
        ),
        errors,
    )
}

pub fn run_project_brain_avoid_terms_preserve_payoff_eval() -> EvalResult {
    let chunks = vec![
        (
            36.0,
            (
                "rumor-noise",
                "旧门传闻在酒肆里反复扩散，路人只谈旧门传闻和无关闲谈，没有新的线索。",
            ),
        ),
        (
            1.0,
            (
                "old-door-payoff",
                "林墨回到旧门，发现门缝里的钥匙正是前文伏笔，旧门钥匙揭开密信来源并回收承诺。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章必须回收旧门钥匙伏笔，揭开密信来源；不要被旧门传闻或无关闲谈稀释主线。",
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
    if first_id != "old-door-payoff" {
        errors.push(format!(
            "avoid-term rerank should preserve old-door payoff while suppressing rumor noise, got {}",
            first_id
        ));
    }
    if first_explanation.contains("avoid term 旧门")
        || !first_explanation.contains("旧门钥匙")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "payoff explanation should keep old-door-key relevance without broad old-door avoid penalty: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_avoid_terms_preserve_payoff",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_project_brain_must_not_boundary_eval() -> EvalResult {
    let chunks = vec![
        (
            48.0,
            (
                "rumor-dominates",
                "旧门传闻盖过寒玉戒指下落，酒肆闲谈只把旧门传闻当成主线，林墨没有得到新线索。",
            ),
        ),
        (
            1.0,
            (
                "ring-payoff",
                "林墨追查寒玉戒指下落，发现黑衣人把戒指带往北境宗门，戒指来源线索终于收束。",
            ),
        ),
    ];
    let reranked = rerank_text_chunks(
        chunks,
        "本章必须追查寒玉戒指下落，揭开戒指来源；不得让旧门传闻盖过寒玉戒指下落。",
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
    if first_id != "ring-payoff" {
        errors.push(format!(
            "must_not boundary should suppress rumor while preserving ring target, got {}",
            first_id
        ));
    }
    if first_explanation.contains("avoid term 寒玉戒指")
        || !first_explanation.contains("寒玉戒指")
        || !first_explanation.contains("scene type setup_payoff")
    {
        errors.push(format!(
            "must_not boundary should keep ring payoff as positive target: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_must_not_boundary",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}

pub fn run_project_brain_author_fixture_rerank_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-17",
            "阿洛必须追查霜铃塔钥的下落，并揭开它和潮汐祭账之间的旧约。",
            "霜铃塔钥下落",
            "别再让盐市流言抢走霜铃塔钥下落",
            "以潮汐祭账的真实签名收束。",
            "eval",
        )
        .unwrap();
    memory
        .record_chapter_result(
            &agent_writer_lib::writer_agent::memory::ChapterResultSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: "Chapter-16".to_string(),
                chapter_revision: "rev-16".to_string(),
                summary: "阿洛在潮井边确认霜铃塔钥被镜盐会带走，祭账上留下潮汐旧约签名。"
                    .to_string(),
                state_changes: vec![],
                character_progress: vec![],
                new_conflicts: vec![],
                new_clues: vec![
                    "霜铃塔钥被镜盐会带走".to_string(),
                    "潮汐祭账留下旧约签名".to_string(),
                ],
                promise_updates: vec!["霜铃塔钥下落: 镜盐会待追查".to_string()],
                canon_updates: vec![],
                source_ref: "eval".to_string(),
                created_at: now_ms(),
            },
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-17".to_string());
    let focus = ProjectBrainFocus::from_kernel("盐市流言到底指向谁？", &kernel);

    let mut db = VectorDB::new();
    for i in 0..20 {
        db.upsert(Chunk {
            id: format!("salt-rumor-noise-{}", i + 1),
            chapter: format!("Chapter-{}", i + 1),
            text: format!(
                "第{}章盐市流言继续扩散，茶摊都在重复盐市、流言、镜盐会、霜铃塔钥传闻和潮汐祭账闲谈，但没有人真正追查塔钥下落。",
                i + 1
            ),
            embedding: vec![1.0, 0.0],
            keywords: vec!["盐市".to_string(), "流言".to_string()],
            topic: Some("盐市流言".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        });
    }
    db.upsert(Chunk {
        id: "author-project-payoff".to_string(),
        chapter: "Chapter-16".to_string(),
        text: "阿洛在潮井石阶下发现霜铃塔钥的下落：镜盐会把塔钥藏进潮汐祭账封皮，旧约签名揭开真实来源，这条伏笔终于回收。"
            .to_string(),
        embedding: vec![0.0, 1.0],
        keywords: vec!["霜铃塔钥".to_string(), "潮汐祭账".to_string()],
        topic: Some("霜铃塔钥下落".to_string()),
        source_ref: None,
        source_revision: None,
        source_kind: None,
        chunk_index: None,
        archived: false,
    });

    let embedding = vec![1.0, 0.0];
    let query_only_top_ten = db.search_hybrid("盐市流言到底指向谁？", &embedding, 10);
    let query_only_contains_payoff = query_only_top_ten
        .iter()
        .any(|(_, chunk)| chunk.id == "author-project-payoff");
    let reranked = search_project_brain_results_with_focus(&db, &focus, &embedding);
    let first_id = reranked
        .first()
        .map(|(_, _, chunk)| chunk.id.as_str())
        .unwrap_or("none");
    let first_explanation = reranked
        .first()
        .map(|(_, reasons, _)| format_text_chunk_relevance(reasons))
        .unwrap_or_default();

    let mut errors = Vec::new();
    if query_only_contains_payoff {
        errors
            .push("fixture should prove query-only top-10 misses author payoff chunk".to_string());
    }
    if first_id != "author-project-payoff" {
        errors.push(format!(
            "author-project fixture should recall and prioritize custom-term payoff chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("霜铃塔钥")
        || !first_explanation.contains("潮汐祭账")
        || first_explanation.contains("avoid term 霜铃塔钥")
    {
        errors.push(format!(
            "rerank explanation should include custom positive terms without boundary-after avoid penalty: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_author_fixture_rerank",
        format!(
            "queryOnlyTop10ContainsPayoff={} first={} explanation={}",
            query_only_contains_payoff, first_id, first_explanation
        ),
        errors,
    )
}

pub fn run_project_brain_chapter_proximity_rerank_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-12",
            "林墨必须追查寒玉戒指下落，并用北境宗门线索回收前文伏笔。",
            "寒玉戒指下落",
            "不要回到远古王座支线",
            "以北境宗门的新证据收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-12".to_string());
    let focus = ProjectBrainFocus::from_kernel("寒玉戒指下落下一步怎么写？", &kernel);
    let chunks = [Chunk {
            id: "distant-ring-archive".to_string(),
            chapter: "Chapter-2".to_string(),
            text: "林墨第一次听说寒玉戒指下落与北境宗门有关，这个旧线索需要以后回收。".to_string(),
            embedding: vec![0.0, 1.0],
            keywords: vec!["寒玉戒指".to_string(), "北境宗门".to_string()],
            topic: Some("寒玉戒指下落".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        },
        Chunk {
            id: "nearby-ring-setup".to_string(),
            chapter: "Chapter-11".to_string(),
            text: "黑衣人带着寒玉戒指逼近北境宗门，林墨发现宗门旧印能指向戒指下落。".to_string(),
            embedding: vec![0.0, 1.0],
            keywords: vec!["寒玉戒指".to_string(), "北境宗门".to_string()],
            topic: Some("寒玉戒指下落".to_string()),
            source_ref: None,
            source_revision: None,
            source_kind: None,
            chunk_index: None,
            archived: false,
        }];
    let raw_results = vec![(90.0, &chunks[0]), (90.0, &chunks[1])];
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
    if first_id != "nearby-ring-setup" {
        errors.push(format!(
            "nearby chapter chunk should outrank same-topic distant archive chunk, got {}",
            first_id
        ));
    }
    if !first_explanation.contains("chapter proximity adjacent chapter") {
        errors.push(format!(
            "rerank explanation missing chapter proximity signal: {}",
            first_explanation
        ));
    }

    eval_result(
        "writer_agent:project_brain_chapter_proximity_rerank",
        format!("first={} explanation={}", first_id, first_explanation),
        errors,
    )
}
