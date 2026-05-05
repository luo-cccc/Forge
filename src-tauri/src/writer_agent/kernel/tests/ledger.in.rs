#[test]
fn save_observation_calibrates_completed_chapter_mission() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "default",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩",
            "提前揭开真相",
            "下落",
            "test",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨发现玉佩的下落，但张三仍没有说出真相。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.observe(obs).unwrap();

    let calibration = proposals
        .iter()
        .find(|p| p.rationale.contains("mission calibration"))
        .expect("should produce a mission calibration proposal");
    assert_eq!(calibration.kind, ProposalKind::ChapterMission);
    assert!(calibration.preview.contains("completed"));
    // Mission in DB should NOT be auto-calibrated — proposals require author approval
    let mission = kernel
        .ledger_snapshot()
        .active_chapter_mission
        .expect("mission should exist");
    assert_eq!(mission.status, "draft");
}

#[test]
fn save_observation_marks_chapter_mission_drifted_on_must_not_hit() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "default",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩",
            "真相",
            "下落",
            "test",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨发现玉佩的下落，并当场揭开真相。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.observe(obs).unwrap();

    let calibration = proposals
        .iter()
        .find(|p| p.rationale.contains("mission calibration"))
        .expect("should produce a mission calibration proposal");
    assert_eq!(calibration.kind, ProposalKind::ChapterMission);
    assert_eq!(calibration.priority, ProposalPriority::Urgent);
    assert!(calibration.preview.contains("drifted"));
    assert!(calibration.risks.iter().any(|r| r.contains("drifted")));
    // Mission in DB should NOT be auto-calibrated
    let mission = kernel
        .ledger_snapshot()
        .active_chapter_mission
        .expect("mission should exist");
    assert_eq!(mission.status, "draft");
}

#[test]
fn ledger_snapshot_derives_next_beat_from_latest_result_and_promises() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩",
            "Chapter-1",
            "Chapter-3",
            4,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("林墨发现玉佩的下落，却开始怀疑张三。新的冲突就此埋下。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs.chapter_title = Some("Chapter-2".to_string());

    kernel.observe(obs).unwrap();

    let next_beat = kernel
        .ledger_snapshot()
        .next_beat
        .expect("next beat should be derived from saved result");
    assert!(next_beat.goal.contains("冲突"));
    assert!(next_beat
        .carryovers
        .iter()
        .any(|line| line.contains("玉佩")));
    assert!(next_beat
        .source_refs
        .iter()
        .any(|source| source.contains("chapter_save:Chapter-2")));
}

#[test]
fn accepted_memory_candidate_writes_ledger() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposal = kernel
        .observe(obs)
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::CanonUpdate)
        .unwrap();
    let result = kernel
        .approve_editor_operation_with_approval(
            proposal.operations[0].clone(),
            "",
            Some(&test_approval("memory_candidate")),
        )
        .unwrap();

    assert!(result.success);
    assert!(kernel
        .ledger_snapshot()
        .canon_entities
        .iter()
        .any(|entity| entity.name == "沈照"));
}

#[test]
fn promise_resolve_operation_closes_open_promise() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩",
            "Chapter-1",
            "Chapter-4",
            4,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseResolve {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-4".to_string(),
            },
            "",
            Some(&test_approval("promise_test")),
        )
        .unwrap();

    assert!(result.success);
    assert!(kernel.ledger_snapshot().open_promises.is_empty());
}

#[test]
fn story_contract_operation_updates_ledger_snapshot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StoryContractUpsert {
                contract: crate::writer_agent::operation::StoryContractOp {
                    project_id: "novel-a".to_string(),
                    title: "寒影录".to_string(),
                    genre: "玄幻".to_string(),
                    target_reader: "长篇玄幻读者".to_string(),
                    reader_promise: "刀客追查玉佩真相。".to_string(),
                    first_30_chapter_promise: "建立宗门危机与玉佩谜团。".to_string(),
                    main_conflict: "复仇与守护的冲突。".to_string(),
                    structural_boundary: "不得提前泄露玉佩来源。".to_string(),
                    tone_contract: "克制、冷峻、少解释。".to_string(),
                },
            },
            "",
            Some(&test_approval("contract_test")),
        )
        .unwrap();

    assert!(result.success);
    let ledger = kernel.ledger_snapshot();
    let contract = ledger.story_contract.unwrap();
    assert_eq!(contract.title, "寒影录");
    assert!(contract.render_for_context().contains("前30章承诺"));
}

#[test]
fn story_contract_operation_rejects_incomplete_foundation() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::StoryContractUpsert {
                contract: crate::writer_agent::operation::StoryContractOp {
                    project_id: "novel-a".to_string(),
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
            Some(&test_approval("contract_test")),
        )
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.error.unwrap().code, "invalid");
    assert!(kernel.ledger_snapshot().story_contract.is_none());
}

#[test]
fn chapter_mission_operation_updates_ledger_snapshot() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    kernel.active_chapter = Some("第一章".to_string());
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::ChapterMissionUpsert {
                mission: crate::writer_agent::operation::ChapterMissionOp {
                    project_id: "novel-a".to_string(),
                    chapter_title: "第一章".to_string(),
                    mission: "林墨发现玉佩线索。".to_string(),
                    must_include: "推进玉佩线索".to_string(),
                    must_not: "不要提前揭开真相".to_string(),
                    expected_ending: "以新的疑问收束。".to_string(),
                    status: "active".to_string(),
                    source_ref: "test".to_string(),
                    blocked_reason: String::new(),
                    retired_history: String::new(),
                },
            },
            "",
            Some(&test_approval("mission_test")),
        )
        .unwrap();

    assert!(result.success);
    let ledger = kernel.ledger_snapshot();
    assert_eq!(ledger.chapter_missions.len(), 1);
    assert_eq!(
        ledger.active_chapter_mission.unwrap().mission,
        "林墨发现玉佩线索。"
    );
    assert_eq!(
        kernel
            .ledger_snapshot()
            .active_chapter_mission
            .unwrap()
            .status,
        "active"
    );
}

#[test]
fn chapter_mission_status_machine_accepts_new_statuses_and_legacy_alias() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    kernel.active_chapter = Some("第一章".to_string());

    for (raw, normalized) in [
        ("in_progress", "active"),
        ("active", "active"),
        ("draft", "draft"),
        ("blocked", "blocked"),
        ("retired", "retired"),
        ("needs_review", "needs_review"),
    ] {
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::ChapterMissionUpsert {
                    mission: crate::writer_agent::operation::ChapterMissionOp {
                        project_id: "novel-a".to_string(),
                        chapter_title: "第一章".to_string(),
                        mission: "林墨发现玉佩线索。".to_string(),
                        must_include: "推进玉佩线索".to_string(),
                        must_not: "不要提前揭开真相".to_string(),
                        expected_ending: "以新的疑问收束。".to_string(),
                        status: raw.to_string(),
                        source_ref: "test".to_string(),
                        blocked_reason: if raw == "blocked" {
                            "等待作者确认张三是否已经离场。".to_string()
                        } else {
                            String::new()
                        },
                        retired_history: if raw == "retired" {
                            "作者已改用第二章任务承接这条线。".to_string()
                        } else {
                            String::new()
                        },
                    },
                },
                "",
                Some(&test_approval("mission_status_machine")),
            )
            .unwrap();
        assert!(result.success, "{raw} should be accepted");
        assert_eq!(
            kernel
                .ledger_snapshot()
                .active_chapter_mission
                .unwrap()
                .status,
            normalized
        );
    }
}

#[test]
fn chapter_mission_blocked_and_retired_require_explanations() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    kernel.active_chapter = Some("第一章".to_string());

    for (status, expected) in [
        ("blocked", "blocked_reason"),
        ("retired", "retired_history"),
    ] {
        let result = kernel
            .approve_editor_operation_with_approval(
                WriterOperation::ChapterMissionUpsert {
                    mission: crate::writer_agent::operation::ChapterMissionOp {
                        project_id: "novel-a".to_string(),
                        chapter_title: "第一章".to_string(),
                        mission: "林墨发现玉佩线索。".to_string(),
                        must_include: "推进玉佩线索".to_string(),
                        must_not: "不要提前揭开真相".to_string(),
                        expected_ending: "以新的疑问收束。".to_string(),
                        status: status.to_string(),
                        source_ref: "test".to_string(),
                        blocked_reason: String::new(),
                        retired_history: String::new(),
                    },
                },
                "",
                Some(&test_approval("mission_status_explanation")),
            )
            .unwrap();

        assert!(!result.success, "{status} should require explanation");
        let error = result.error.expect("invalid mission should return error");
        assert_eq!(error.code, "invalid");
        assert!(
            error.message.contains(expected),
            "{status} error should mention {expected}: {}",
            error.message
        );
    }
}

#[test]
fn chapter_mission_operation_rejects_vague_foundation() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("novel-a", memory);
    kernel.active_chapter = Some("第一章".to_string());
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::ChapterMissionUpsert {
                mission: crate::writer_agent::operation::ChapterMissionOp {
                    project_id: "novel-a".to_string(),
                    chapter_title: "第一章".to_string(),
                    mission: "打架".to_string(),
                    must_include: "".to_string(),
                    must_not: "剧透".to_string(),
                    expected_ending: "".to_string(),
                    status: "in_progress".to_string(),
                    source_ref: "test".to_string(),
                    blocked_reason: String::new(),
                    retired_history: String::new(),
                },
            },
            "",
            Some(&test_approval("mission_test")),
        )
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.error.unwrap().code, "invalid");
    assert!(kernel.ledger_snapshot().active_chapter_mission.is_none());
}

