fn run_scenario_context_explainability_for_longform_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-5",
            "让林墨决定是否相信张三。",
            "张三交出关键证据",
            "提前揭露幕后主谋",
            "林墨同意与张三同行",
            "scenario",
        )
        .unwrap();
    memory
        .add_promise(
            "emotional_debt",
            "张三背叛后的道歉",
            "张三欠林墨一次真正解释",
            "Chapter-2",
            "Chapter-5",
            4,
        )
        .unwrap();
    memory
        .upsert_canon_rule(
            "林墨不能主动杀害无辜者",
            "character_boundary",
            5,
            "scenario",
        )
        .unwrap();
    record_result(
        &memory,
        "Chapter-4",
        "rev-4",
        "张三救下林墨，但仍没有解释背叛。",
        &["林墨暂时欠张三一命"],
        &["林墨开始动摇"],
        &["追兵逼近旧井"],
        &["井底有青铜钥匙"],
        &["张三的道歉仍未完成"],
        &[],
    );
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(
        AgentTask::ChapterGeneration,
        &observation_in_chapter("林墨看着张三递来的证据。", "Chapter-5"),
    );
    let source_kinds = pack
        .sources
        .iter()
        .map(|source| format!("{:?}", source.source))
        .collect::<Vec<_>>();
    let required = [
        ContextSource::ProjectBrief,
        ContextSource::ChapterMission,
        ContextSource::ResultFeedback,
        ContextSource::PromiseSlice,
        ContextSource::CanonSlice,
    ];
    let mut errors = Vec::new();
    for expected in required {
        if !pack.sources.iter().any(|source| source.source == expected) {
            errors.push(format!("missing required longform source {:?}", expected));
        }
    }
    if pack.budget_report.source_reports.is_empty() {
        errors.push("context pack did not expose source budget report".to_string());
    }

    eval_result(
        "writer_agent:scenario_longform_context_explainability",
        format!(
            "expected=context explains why agent knows book actual=sources:{:?} budgetReports:{} evidence=ContextPack",
            source_kinds,
            pack.budget_report.source_reports.len()
        ),
        errors,
    )
}

fn run_continuous_writing_fixture_20_chapters_eval() -> EvalResult {
    let db_path = std::env::temp_dir().join(format!(
        "forge-continuous-writing-20-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let mut errors = Vec::new();
    let session_a_metrics;
    let session_b_metrics;
    let session_b_debt;
    let session_b_context_sources;
    let latest_result_count;
    let mission_completed;
    let mission_drifted;
    let promise_context_hit;
    let promise_recall_hit;
    let trace_has_product_trend;
    let trace_has_context_recall;
    let trajectory_has_save;

    {
        let memory = WriterMemory::open(&db_path).unwrap();
        seed_continuous_writing_memory(&memory);
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "continuous-20-session-a".to_string();
        simulate_continuous_writing_session(&mut kernel, 1, 10, 30);
        session_a_metrics = kernel.trace_snapshot(120).product_metrics;
    }

    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "continuous-20-session-b".to_string();
        simulate_continuous_writing_session(&mut kernel, 11, 20, 140);

        let obs = observation_in_chapter(
            "林墨把霜铃塔钥按进潮汐祭账的暗格，决定先保住张三，而不是揭开白昼王座的来源。",
            "Chapter-20",
        );
        let pack = kernel.context_pack_for_default(AgentTask::PlanningReview, &obs);
        session_b_context_sources = pack
            .sources
            .iter()
            .map(|source| format!("{:?}", source.source))
            .collect::<Vec<_>>();
        promise_context_hit = pack.sources.iter().any(|source| {
            source.source == ContextSource::PromiseSlice && source.content.contains("霜铃塔钥")
        });

        let ledger = kernel.ledger_snapshot();
        latest_result_count = ledger.recent_chapter_results.len();
        mission_completed = ledger
            .chapter_missions
            .iter()
            .filter(|mission| mission.status == "completed")
            .count();
        mission_drifted = ledger
            .chapter_missions
            .iter()
            .filter(|mission| mission.status == "drifted")
            .count();

        let snapshot = kernel.trace_snapshot(200);
        session_b_metrics = snapshot.product_metrics.clone();
        session_b_debt = kernel.story_debt_snapshot();
        promise_recall_hit = snapshot
            .context_recalls
            .iter()
            .any(|recall| recall.source == "PromiseLedger" && recall.snippet.contains("霜铃塔钥"));
        trace_has_product_trend = snapshot.product_metrics_trend.session_count >= 2
            && snapshot.product_metrics_trend.recent_sessions.len() >= 2
            && snapshot
                .product_metrics_trend
                .overall_average_save_to_feedback_ms
                .is_some()
            && snapshot
                .product_metrics_trend
                .save_to_feedback_delta_ms
                .is_some();
        trace_has_context_recall = !snapshot.context_recalls.is_empty();
        let export = export_trace_snapshot("eval", &kernel.session_id, &snapshot);
        trajectory_has_save = export
            .jsonl
            .contains("\"eventType\":\"writer.save_completed\"")
            && export
                .jsonl
                .contains("\"eventType\":\"writer.product_metrics_trend\"")
            && export
                .jsonl
                .contains("\"eventType\":\"writer.context_recall\"");
    }

    let _ = std::fs::remove_file(&db_path);

    if latest_result_count < 20 {
        errors.push(format!(
            "continuous fixture recorded only {} chapter results",
            latest_result_count
        ));
    }
    if mission_completed < 1 && mission_drifted < 1 {
        errors.push(format!(
            "continuous fixture produced no mission calibration: completed={} drifted={}",
            mission_completed, mission_drifted
        ));
    }
    if mission_drifted == 0 {
        errors.push("continuous fixture did not preserve a mission drift case".to_string());
    }
    if session_b_debt.promise_count == 0 || session_b_debt.mission_count == 0 {
        errors.push(format!(
            "story debt did not include both promise and mission debt: promise={} mission={}",
            session_b_debt.promise_count, session_b_debt.mission_count
        ));
    }
    if !promise_context_hit {
        errors.push("planning context did not recall the long-running key promise".to_string());
    }
    if !promise_recall_hit {
        errors.push("context recall ledger did not record PromiseLedger evidence".to_string());
    }
    if session_a_metrics.proposal_count < 2 || session_b_metrics.proposal_count < 2 {
        errors.push(format!(
            "sessions produced too few proposals: a={} b={}",
            session_a_metrics.proposal_count, session_b_metrics.proposal_count
        ));
    }
    if session_b_metrics.feedback_count < 2
        || session_b_metrics.average_save_to_feedback_ms.is_none()
    {
        errors.push(format!(
            "session-b feedback/save metrics incomplete: feedback={} latency={:?}",
            session_b_metrics.feedback_count, session_b_metrics.average_save_to_feedback_ms
        ));
    }
    if session_b_metrics.promise_recall_hit_rate <= 0.0 {
        errors.push("promise recall hit rate did not move above zero".to_string());
    }
    if !trace_has_product_trend {
        errors.push("product metrics trend did not prove multi-session replay".to_string());
    }
    if !trace_has_context_recall {
        errors.push("trace snapshot lacked context recalls".to_string());
    }
    if !trajectory_has_save {
        errors.push("trajectory did not export save/metrics/context events".to_string());
    }

    eval_result(
        "writer_agent:continuous_writing_fixture_20_chapters",
        format!(
            "expected=20-chapter continuous product fixture actual=results:{} completed:{} drifted:{} debt:{} promiseDebt:{} missionDebt:{} sources:{:?} aFeedback:{} bFeedback:{} bLatency:{:?}",
            latest_result_count,
            mission_completed,
            mission_drifted,
            session_b_debt.total,
            session_b_debt.promise_count,
            session_b_debt.mission_count,
            session_b_context_sources,
            session_a_metrics.feedback_count,
            session_b_metrics.feedback_count,
            session_b_metrics.average_save_to_feedback_ms
        ),
        errors,
    )
}

fn seed_continuous_writing_memory(memory: &WriterMemory) {
    memory
        .ensure_story_contract_seed(
            "eval",
            "霜塔旧账",
            "长篇玄幻悬疑",
            "林墨在二十章内追查霜铃塔钥与潮汐祭账的因果，同时保护张三的灰色忠诚。",
            "林墨必须在守护张三与揭开白昼王座之间持续付出代价。",
            "不得提前揭示白昼王座真正来源；不得把霜铃塔钥当作普通钥匙处理。",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &["寒影刀客".to_string()],
            "主角，惯用寒影刀，追查霜铃塔钥与潮汐祭账。",
            &serde_json::json!({"weapon": "寒影刀", "loyalty": "protects Zhang San"}),
            0.92,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "张三",
            &["账房".to_string()],
            "张三保管过潮汐祭账，动机可疑但多次保护林墨。",
            &serde_json::json!({"holdsLedger": true, "trust": "unstable"}),
            0.88,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "霜铃塔钥",
            &["塔钥".to_string()],
            "开启霜铃塔旧门的钥匙，与潮汐祭账缺页互相印证。",
            &serde_json::json!({"location": "with Lin Mo", "risk": "reveals old debt"}),
            0.9,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "潮汐祭账",
            &["祭账".to_string()],
            "记载白昼王座旧债的账册，缺页仍未找回。",
            &serde_json::json!({"missingPage": true}),
            0.87,
        )
        .unwrap();
    memory
        .upsert_canon_rule(
            "白昼王座的真正来源必须延后到二十章后再揭示。",
            "mystery_boundary",
            8,
            "fixture",
        )
        .unwrap();
    memory
        .upsert_style_preference("accepted_Ghost", "短句、克制、少解释", true)
        .unwrap();
    memory
        .add_promise_with_entities(
            "object_whereabouts",
            "霜铃塔钥",
            "霜铃塔钥能打开旧门，但不能提前解释白昼王座来源",
            "Chapter-1",
            "Chapter-20",
            8,
            &["霜铃塔钥".to_string(), "白昼王座".to_string()],
        )
        .unwrap();
    memory
        .add_promise_with_entities(
            "mystery_clue",
            "潮汐祭账缺页",
            "潮汐祭账缺页记录张三背叛的真实原因",
            "Chapter-3",
            "Chapter-18",
            7,
            &["潮汐祭账".to_string(), "张三".to_string()],
        )
        .unwrap();
    memory
        .add_promise(
            "emotional_debt",
            "张三的真正道歉",
            "张三欠林墨一次不带借口的道歉",
            "Chapter-5",
            "Chapter-16",
            6,
        )
        .unwrap();

    for chapter in 1..=20 {
        let mission = continuous_chapter_mission(chapter);
        memory.upsert_chapter_mission(&mission).unwrap();
    }
}

fn continuous_chapter_mission(chapter: usize) -> ChapterMissionSummary {
    ChapterMissionSummary {
        id: 0,
        project_id: "eval".to_string(),
        chapter_title: format!("Chapter-{}", chapter),
        mission: format!(
            "推进林墨与张三围绕霜铃塔钥、潮汐祭账和第{}章压力的选择。",
            chapter
        ),
        must_include: continuous_must_include(chapter),
        must_not: if chapter >= 15 {
            "提前揭示白昼王座来源".to_string()
        } else {
            "跳过霜铃塔钥因果".to_string()
        },
        expected_ending: continuous_expected_ending(chapter),
        status: "active".to_string(),
        source_ref: format!("fixture:chapter-mission:{}", chapter),
        updated_at: String::new(),
        blocked_reason: String::new(),
        retired_history: String::new(),
    }
}

fn continuous_must_include(chapter: usize) -> String {
    match chapter {
        1 | 2 => "霜铃塔钥的线索".to_string(),
        3 | 4 => "潮汐祭账的记录".to_string(),
        5 | 6 => "张三的重要选择".to_string(),
        7 | 8 => "旧门的机关秘密".to_string(),
        9 | 10 => "寒影刀的来历".to_string(),
        11 | 12 => "缺页的隐藏内容".to_string(),
        13 | 14 => "信任的确立过程".to_string(),
        15 | 16 => "道歉的真诚表达".to_string(),
        17 | 18 => "祭账的完整真相".to_string(),
        _ => "霜铃塔钥的线索".to_string(),
    }
}

fn continuous_expected_ending(chapter: usize) -> String {
    match chapter {
        1 | 2 => "线索留下新的疑问。".to_string(),
        3 | 4 => "记录推动下一步行动。".to_string(),
        5 | 6 => "选择带来新的风险。".to_string(),
        7 | 8 => "机关揭示更大秘密。".to_string(),
        9 | 10 => "来历确认新的方向。".to_string(),
        11 | 12 => "内容改变任务走向。".to_string(),
        13 | 14 => "关系进入新的阶段。".to_string(),
        15 | 16 => "表达修复旧的裂痕。".to_string(),
        17 | 18 => "真相揭示完整债图。".to_string(),
        _ => "留下一个重大疑问。".to_string(),
    }
}

fn seed_real_author_project_memory(memory: &WriterMemory) {
    memory
        .ensure_story_contract_seed(
            "eval",
            "镜中墟",
            "志怪悬疑",
            "苏晚在镜墟幻界中追查亡父遗留的七面铜镜，每面镜后封印一段被修改的记忆。",
            "苏晚必须在还原记忆真相与保护镜墟秩序之间反复权衡。",
            "不得提前揭示镜墟之主真实身份；不得把铜镜记忆当作绝对事实。",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "苏晚",
            &["镜使".to_string()],
            "女主，镜墟守护者的女儿，能进入铜镜记忆片段。",
            &serde_json::json!({"ability": "镜中行走", "bond": "父亲遗命"}),
            0.95,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "沈砚",
            &["画师".to_string()],
            "曾在镜墟迷途三年，画下铜镜线索，不愿再进幻界。",
            &serde_json::json!({"role": "线索提供者", "trauma": "铜镜吞噬记忆"}),
            0.91,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "云娘",
            &["镜灵".to_string()],
            "镜墟中最古老的灵体，声称知道真相但只以交易形式透露。",
            &serde_json::json!({"alignment": "ambiguous", "knowledge": "complete"}),
            0.88,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "七面铜镜",
            &["法器".to_string()],
            "苏父留下的七面古铜镜，分别封印七段被修改的记忆。已找回三面。",
            &serde_json::json!({"found": 3, "total": 7, "danger": "镜子会反向观察持镜者"}),
            0.93,
        )
        .unwrap();
    memory
        .upsert_canon_rule(
            "镜墟之主真实身份至少延后到第七面镜出现后再揭示。",
            "mystery_boundary",
            9,
            "real_author_fixture",
        )
        .unwrap();
    memory
        .upsert_canon_rule(
            "每面铜镜揭示的记忆都可能被修改过——没有一面镜子的记忆是完全真实的。",
            "unreliable_narrator",
            9,
            "real_author_fixture",
        )
        .unwrap();
    memory
        .upsert_style_preference("accepted_Ghost", "细腻克制、留白、不解释情感动机", true)
        .unwrap();
    memory
        .add_promise_with_entities(
            "object_whereabouts",
            "第四面铜镜",
            "第四面铜镜藏在沈砚画中的旧巷，但进入巷子需要云娘的交易条件",
            "第一章",
            "第十二章",
            9,
            &[
                "七面铜镜".to_string(),
                "沈砚".to_string(),
                "云娘".to_string(),
            ],
        )
        .unwrap();
    memory
        .add_promise_with_entities(
            "mystery_clue",
            "父亲最后一封信",
            "苏父的信提到第七面镜是假的——但谁伪造了它不应提前揭晓",
            "第三章",
            "第十五章",
            8,
            &["苏晚".to_string(), "七面铜镜".to_string()],
        )
        .unwrap();
    memory
        .add_promise(
            "emotional_debt",
            "沈砚的道歉",
            "沈砚欠苏晚一句关于当年离开镜墟的实话",
            "第四章",
            "第十章",
            7,
        )
        .unwrap();

    let chapter_missions: Vec<(&str, &str, &str, &str, &str)> = vec![
        (
            "第一章",
            "引入苏晚与镜墟关系，建立铜镜的世界观规则",
            "找到第一面铜镜的线索",
            "不得揭示镜墟之主身份",
            "线索指向旧巷但铜镜被移动过",
        ),
        (
            "第二章",
            "展开沈砚与苏晚的旧识关系",
            "沈砚交出第一幅镜墟画作",
            "不得让沈砚进入镜墟",
            "沈砚画中的镜墟比现实少了一座塔",
        ),
        (
            "第三章",
            "揭示父亲最后一封信的存在",
            "找到父亲信件的隐藏段落",
            "不得让云娘直接说出真相",
            "信中提到\"第七面是假的\"但未说明原因",
        ),
        (
            "第四章",
            "沈砚道歉的第一次铺垫",
            "沈砚开口但不完整",
            "不得让苏晚直接原谅",
            "沈砚只说了一半实话",
        ),
        (
            "第五章",
            "云娘提出第一个交易条件",
            "苏晚用一段记忆换取铜镜位置",
            "不得揭示云娘真实身份",
            "苏晚失去的记忆与父亲有关",
        ),
    ];

    for (chapter, mission, must_include, must_not, ending) in &chapter_missions {
        memory
            .upsert_chapter_mission(&ChapterMissionSummary {
                id: 0,
                project_id: "eval".to_string(),
                chapter_title: chapter.to_string(),
                mission: mission.to_string(),
                must_include: must_include.to_string(),
                must_not: must_not.to_string(),
                expected_ending: ending.to_string(),
                status: "active".to_string(),
                source_ref: format!("real_author_fixture:{}", chapter),
                updated_at: String::new(),
                blocked_reason: String::new(),
                retired_history: String::new(),
            })
            .unwrap();
    }
}

