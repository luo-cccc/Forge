fn simulate_real_author_writing_session(kernel: &mut WriterAgentKernel, feedback_delay_ms: u64) {
    let chapters = [("第一章", "苏晚把铜镜翻过来，背面刻的不是她记忆中的名字。镜墟在黄昏时分最安静，她把沈砚的画展开——画里少了一座塔。"),
        ("第二章", "沈砚不愿意看铜镜。\"那东西会记住看它的人。\"苏晚把画摊在他面前，塔的位置是一团墨，像被刻意涂掉的。"),
        ("第三章", "父亲的遗信藏在铜镜夹层。苏晚读到一半发现笔迹变了——后半段不是父亲写的。\"第七面是假的\"——但没有说为什么。"),
        ("第四章", "沈砚终于说：\"我不是逃出镜墟的——我是被赶出来的。\"苏晚没有接话。铜镜在他说话时微微发亮。"),
        ("第五章", "云娘把交易摆上桌：\"用一段你不想要的记忆，换第四面铜镜的位置。\"苏晚把五岁生日的画面递出去，发现云娘不敢看那面铜镜。")];

    for (chapter_idx, (chapter, paragraph)) in chapters.iter().enumerate() {
        kernel.active_chapter = Some(chapter.to_string());
        let save_proposals = kernel
            .observe(save_observation(paragraph, chapter))
            .unwrap();

        for cal in save_proposals.iter().filter(|p| {
            p.kind == ProposalKind::ChapterMission && p.rationale.contains("mission calibration")
        }) {
            let op = cal.operations[0].clone();
            let mut approval = eval_approval("fixture_mission_calibration");
            approval.proposal_id = Some(cal.id.clone());
            kernel
                .approve_editor_operation_with_approval(op, "", Some(&approval))
                .ok();
        }

        let observation = observation_in_chapter(paragraph, chapter);
        let continuation = format!(
            "苏晚把第{}面铜镜放回匣中，发现它们彼此之间在轻声说话。",
            chapter_idx + 1
        );

        if *chapter == "第三章" {
            let bundle = WriterFailureEvidenceBundle::new(
                WriterFailureCategory::ContextMissing,
                "context_pack_dropped_chapter_mission",
                "Context pressure dropped ChapterMission before Planning Review at chapter 3.",
                true,
                Some("real-author-task-3".to_string()),
                vec!["context:ChapterMission".to_string()],
                serde_json::json!({"droppedSource": "ChapterMission", "chapter": chapter}),
                vec!["rebuild_context_pack: Run Planning Review before continuing.".to_string()],
                now_ms(),
            );
            kernel.record_failure_evidence_bundle(&bundle);
        }

        let proposal = kernel
            .create_llm_ghost_proposal(observation, continuation.clone(), "fixture-model")
            .unwrap();
        let operation = proposal.operations[0].clone();
        kernel
            .record_operation_durable_save(
                Some(proposal.id.clone()),
                operation,
                format!("editor_save:{}", chapter),
            )
            .unwrap();

        let is_fourth = *chapter == "第四章";
        let reason = if is_fourth {
            "沈砚的实话太长——留一半，让铜镜替他说。"
        } else {
            "这一章的感觉对了。"
        };
        kernel
            .apply_feedback(ProposalFeedback {
                proposal_id: proposal.id.clone(),
                action: if is_fourth {
                    FeedbackAction::Edited
                } else {
                    FeedbackAction::Accepted
                },
                final_text: if is_fourth {
                    Some("沈砚只说了半句实话——铜镜替他补了后半句。".to_string())
                } else {
                    Some(continuation)
                },
                reason: Some(reason.to_string()),
                created_at: now_ms() + feedback_delay_ms,
            })
            .unwrap();
    }
}

pub fn run_real_author_long_session_calibration_eval() -> EvalResult {
    let db_path = std::env::temp_dir().join(format!(
        "forge-real-author-calibration-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let mut errors = Vec::new();

    let session_metrics;
    let story_debt;
    let context_sources;
    let mission_statuses;
    let trace_has_metacognition;
    let trace_has_product_trend;
    let trace_has_context_recalls;

    {
        let memory = WriterMemory::open(&db_path).unwrap();
        seed_real_author_project_memory(&memory);
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "real-author-calibration".to_string();
        simulate_real_author_writing_session(&mut kernel, 80);

        session_metrics = kernel.trace_snapshot(120).product_metrics.clone();
        story_debt = kernel.story_debt_snapshot();
        let snapshot = kernel.trace_snapshot(200);

        let obs = observation_in_chapter(
            "苏晚发现铜镜背面刻的不是名字——是沈砚的画中塔的位置。",
            "第三章",
        );
        let pack = kernel.context_pack_for_default(AgentTask::PlanningReview, &obs);
        context_sources = pack
            .sources
            .iter()
            .map(|source| format!("{:?}", source.source))
            .collect::<Vec<_>>();

        let ledger = kernel.ledger_snapshot();
        mission_statuses = ledger
            .chapter_missions
            .iter()
            .map(|m| (m.chapter_title.clone(), m.status.clone()))
            .collect::<Vec<_>>();

        let meta = snapshot.metacognitive_snapshot;
        trace_has_metacognition =
            !meta.reasons.is_empty() && !meta.remediation.is_empty() && meta.confidence > 0.0;
        trace_has_product_trend = snapshot.product_metrics_trend.session_count >= 1;
        trace_has_context_recalls = !snapshot.context_recalls.is_empty();
    }

    let _ = std::fs::remove_file(&db_path);

    if session_metrics.proposal_count < 5 {
        errors.push(format!(
            "real author session under-produced proposals: {}",
            session_metrics.proposal_count
        ));
    }
    if session_metrics.feedback_count < 5 {
        errors.push(format!(
            "real author session under-recorded feedback: {}",
            session_metrics.feedback_count
        ));
    }
    if session_metrics.durable_save_success_rate <= 0.0 {
        errors.push("real author session had zero durable save success rate".to_string());
    }
    if !context_sources
        .iter()
        .any(|source| source.contains("CanonSlice"))
    {
        errors.push("context pack missing canon sources for named entities".to_string());
    }
    if !context_sources
        .iter()
        .any(|source| source.contains("PromiseSlice"))
    {
        errors.push("context pack missing promise sources for multi-chapter arcs".to_string());
    }
    let has_completed = mission_statuses.iter().any(|(_, s)| s == "completed");
    let has_drifted = mission_statuses.iter().any(|(_, s)| s == "drifted");
    let has_review = mission_statuses.iter().any(|(_, s)| s == "needs_review");
    if !has_completed && !has_drifted && !has_review {
        errors.push(format!(
            "real author session produced no mission state changes: {:?}",
            mission_statuses
        ));
    }
    if !trace_has_metacognition {
        errors.push("metacognitive snapshot did not register real author session risk".to_string());
    }
    if !trace_has_product_trend {
        errors.push("product metrics trend not replayable in real author session".to_string());
    }
    if !trace_has_context_recalls {
        errors.push("context recalls not tracked in real author session".to_string());
    }
    if story_debt.total == 0 {
        errors.push("story debt remained zero across real author chapters".to_string());
    }

    eval_result(
        "writer_agent:real_author_long_session_calibration",
        format!(
            "proposals={} feedback={} saveRate={:.2} debtTotal={} promiseDebt={} missionDebt={} sources:{:?} missions:{:?}",
            session_metrics.proposal_count,
            session_metrics.feedback_count,
            session_metrics.durable_save_success_rate,
            story_debt.total,
            story_debt.promise_count,
            story_debt.mission_count,
            context_sources,
            mission_statuses,
        ),
        errors,
    )
}

fn simulate_continuous_writing_session(
    kernel: &mut WriterAgentKernel,
    start: usize,
    end: usize,
    feedback_delay_ms: u64,
) {
    for chapter in start..=end {
        kernel.active_chapter = Some(format!("Chapter-{}", chapter));
        let paragraph = continuous_chapter_text(chapter);
        let save_proposals = kernel
            .observe(save_observation(
                &paragraph,
                &format!("Chapter-{}", chapter),
            ))
            .unwrap();
        for cal in save_proposals.iter().filter(|p| {
            p.kind == ProposalKind::ChapterMission && p.rationale.contains("mission calibration")
        }) {
            let op = cal.operations[0].clone();
            let mut approval = eval_approval("fixture_mission_calibration");
            approval.proposal_id = Some(cal.id.clone());
            kernel
                .approve_editor_operation_with_approval(op, "", Some(&approval))
                .ok();
        }

        if matches!(chapter, 4 | 9 | 13 | 17 | 20) {
            let observation = observation_in_chapter(&paragraph, &format!("Chapter-{}", chapter));
            let continuation = format!(
                "林墨把第{}章的选择压低成一句话：先保住人，再追问债。",
                chapter
            );
            let proposal = kernel
                .create_llm_ghost_proposal(observation, continuation.clone(), "fixture-model")
                .unwrap();
            let operation = proposal.operations[0].clone();
            kernel
                .record_operation_durable_save(
                    Some(proposal.id.clone()),
                    operation,
                    format!("editor_save:chapter-{}", chapter),
                )
                .unwrap();
            kernel
                .apply_feedback(ProposalFeedback {
                    proposal_id: proposal.id.clone(),
                    action: if matches!(chapter, 13) {
                        FeedbackAction::Edited
                    } else {
                        FeedbackAction::Accepted
                    },
                    final_text: Some(continuation),
                    reason: Some(format!("continuous fixture chapter {}", chapter)),
                    created_at: now_ms() + feedback_delay_ms + chapter as u64,
                })
                .unwrap();
        } else if matches!(chapter, 6 | 15) {
            let observation = observation_in_chapter(&paragraph, &format!("Chapter-{}", chapter));
            let proposal = kernel
                .create_llm_ghost_proposal(
                    observation,
                    "张三忽然解释了一切，白昼王座来源也被说破。".to_string(),
                    "fixture-model",
                )
                .unwrap();
            kernel
                .apply_feedback(ProposalFeedback {
                    proposal_id: proposal.id.clone(),
                    action: FeedbackAction::Rejected,
                    final_text: None,
                    reason: Some("作者拒绝过早揭示核心谜底".to_string()),
                    created_at: now_ms() + feedback_delay_ms + chapter as u64,
                })
                .unwrap();
        }
    }
}

fn continuous_chapter_text(chapter: usize) -> String {
    match chapter {
        1 => "林墨发现霜铃塔钥在旧井边发冷，决定把它藏进袖中。线索没有解释来源，只留下新的疑问。".to_string(),
        2 => "林墨握着霜铃塔钥听见塔内铃声，发现钥齿和旧门铜痕相合，选择暂不告诉张三。".to_string(),
        3 => "张三交出潮汐祭账，林墨发现账册缺页，怀疑有人删去了白昼王座的旧债。".to_string(),
        4 => "潮汐祭账被雨水打湿，林墨确认缺页边缘有霜铃塔印，新的敌人开始追查账册。".to_string(),
        5 => "张三挡下追兵，林墨决定暂时相信他，却仍把霜铃塔钥握在掌心。".to_string(),
        6 => "张三拒绝说明背叛原因，林墨发现他的伤口来自旧门机关，信任仍被怀疑撕扯。".to_string(),
        7 => "林墨带张三返回旧门，霜铃塔钥只转动半圈，门后传来账册缺页被焚的气味。".to_string(),
        8 => "旧门忽然合拢，林墨选择救张三而不是追门后黑影，新的危险压住了钥匙线索。".to_string(),
        9 => "林墨拔出寒影刀，发现刀身映出潮汐祭账缺页的编号，敌人杀意逼近。".to_string(),
        10 => "寒影刀斩断锁链，林墨确认旧门机关会吞掉持钥者的记忆，新的选择变得更重。".to_string(),
        11 => "林墨在废庙发现缺页拓印，潮汐祭账的空白处只留下张三的旧名。".to_string(),
        12 => "缺页拓印被敌人夺走，林墨发现张三曾试图保护孩子，危机转向城南码头。".to_string(),
        13 => "林墨没有直接追问张三，而是把信任押在他递来的半页祭账上。".to_string(),
        14 => "张三承认自己曾背叛林墨，林墨选择继续同行，信任变成有条件的交换。".to_string(),
        15 => "白昼王座来源忽然被说破，整章只写山色、雨声和远处灯火，林墨没有见到张三，也没有处理道歉。".to_string(),
        16 => "张三终于低头道歉，林墨没有原谅，却决定让他活到潮汐祭账真相出现。".to_string(),
        17 => "祭账缺页在塔底重现，林墨发现霜铃塔钥能换来一次延后揭示的机会。".to_string(),
        18 => "林墨用祭账缺页逼退敌人，仍没有说破白昼王座来源，只把债推到更深处。".to_string(),
        19 => "林墨把霜铃塔钥交还张三保管，决定先保护活人，再追问白昼王座。".to_string(),
        20 => "霜铃塔钥插进暗格，潮汐祭账展开新页，林墨发现这不是结局，只是更大的债。".to_string(),
        _ => "林墨继续推进线索。".to_string(),
    }
}
