#[test]
fn observe_emits_canon_conflict_from_memory_facts() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();

    assert!(proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::ContinuityWarning));
}

#[test]
fn observe_emits_and_dedupes_diagnostic_pacing_proposal() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let paragraph = "风".repeat(2001);
    let mut obs = observation(&paragraph);
    obs.cursor = Some(TextRange {
        from: paragraph.chars().count(),
        to: paragraph.chars().count(),
    });

    let first = kernel.observe(obs.clone()).unwrap();
    let second = kernel.observe(obs).unwrap();

    assert!(first
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::StyleNote
            && proposal.preview.contains("段落较长")));
    assert!(!second
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::StyleNote
            && proposal.preview.contains("段落较长")));
}

#[test]
fn observe_ghost_uses_context_pack_evidence() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
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
    memory
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
    let proposals = kernel
        .observe(observation(
            "林墨停在旧门前，风声压低。他想起张三离开时攥紧的玉佩，却没有立刻追问。",
        ))
        .unwrap();
    let ghost = proposals
        .iter()
        .find(|p| p.kind == ProposalKind::Ghost)
        .unwrap();

    assert!(ghost
        .evidence
        .iter()
        .any(|e| e.source == EvidenceSource::Canon));
    assert!(ghost
        .evidence
        .iter()
        .any(|e| e.source == EvidenceSource::PromiseLedger));
    assert!(ghost.preview.contains("旧事") || ghost.preview.contains("兵器"));
}

#[test]
fn observe_records_context_recalls_from_surfaced_evidence() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
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
    let mut kernel = WriterAgentKernel::new("default", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let warning = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .expect("continuity warning should exist");

    let trace = kernel.trace_snapshot(10);
    let ledger = kernel.ledger_snapshot();

    assert!(trace.context_recalls.iter().any(|recall| {
        recall.source == "Canon"
            && recall.last_proposal_id == warning.id
            && recall.snippet.contains("寒影刀")
    }));
    assert!(ledger
        .context_recalls
        .iter()
        .any(|recall| recall.source == "Canon"));
}

#[test]
fn observe_ghost_contains_three_parallel_branches() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let text = "林墨深吸一口气，说道：“这件事我本来不该告诉你，可你已经走到这里，就没有回头路了。";
    let mut obs = observation(text);
    let cursor = text.chars().count();
    obs.cursor = Some(TextRange {
        from: cursor,
        to: cursor,
    });
    let proposals = kernel.observe(obs).unwrap();
    let ghost = proposals
        .iter()
        .find(|p| p.kind == ProposalKind::Ghost)
        .unwrap();

    assert_eq!(ghost.alternatives.len(), 3);
    assert_eq!(ghost.alternatives[0].label, "A 直接表态");
    assert!(ghost.alternatives.iter().all(|alternative| matches!(
        alternative.operation,
        Some(WriterOperation::TextInsert { .. })
    )));
}

#[test]
fn save_observation_suggests_memory_candidates_without_writing_ledgers() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs = observation("那个少年名叫沈照，袖中藏着一枚玉佩，却始终没有告诉任何人它的下落。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.observe(obs).unwrap();

    assert!(proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::CanonUpdate
            && matches!(
                proposal.operations.first(),
                Some(WriterOperation::CanonUpsertEntity { .. })
            )
    }));
    assert!(proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::PlotPromise
            && matches!(
                proposal.operations.first(),
                Some(WriterOperation::PromiseAdd { .. })
            )
    }));
    let ledger = kernel.ledger_snapshot();
    assert!(ledger.canon_entities.is_empty());
    assert!(ledger.open_promises.is_empty());
    assert_eq!(ledger.recent_chapter_results.len(), 1);
    assert!(ledger.recent_chapter_results[0].summary.contains("沈照"));
    assert!(ledger.recent_chapter_results[0]
        .new_clues
        .contains(&"玉佩".to_string()));
}

#[test]
fn save_observation_records_chapter_result_feedback() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs =
        observation("林墨发现玉佩的下落，却开始怀疑张三。张三选择隐瞒真相，新的冲突就此埋下。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs.chapter_title = Some("第一章".to_string());
    obs.chapter_revision = Some("rev-1".to_string());
    obs.prefix = obs.paragraph.clone();

    kernel.observe(obs).unwrap();

    let ledger = kernel.ledger_snapshot();
    let result = ledger.recent_chapter_results.first().unwrap();
    assert_eq!(result.chapter_title, "第一章");
    assert_eq!(result.chapter_revision, "rev-1");
    assert!(result.summary.contains("玉佩"));
    assert!(result
        .new_conflicts
        .iter()
        .any(|line| line.contains("冲突")));
    assert!(result.new_clues.contains(&"玉佩".to_string()));
    assert!(result.source_ref.contains("chapter_save:第一章:rev-1"));
}

#[test]
fn invalid_task_packet_is_rejected_before_trace() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let packet = TaskPacket::new(
        "bad-packet",
        "missing foundation fields",
        TaskScope::Chapter,
        1,
    );

    let error = kernel
        .record_task_packet("obs-1", "ChapterGeneration", packet)
        .unwrap_err();

    assert!(error.contains("scopeRef"));
    assert!(kernel.trace_snapshot(10).task_packets.is_empty());
}

#[test]
fn save_observation_result_feedback_feeds_next_task_packet() {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mut kernel = WriterAgentKernel::new("default", memory);
    let mut obs =
        observation("林墨发现玉佩的下落，却开始怀疑张三。张三选择隐瞒真相，新的冲突就此埋下。");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs.chapter_title = Some("第一章".to_string());
    obs.chapter_revision = Some("rev-1".to_string());
    obs.prefix = obs.paragraph.clone();

    kernel.observe(obs).unwrap();
    let next = observation("林墨深吸一口气，说道：“");
    kernel
        .create_llm_ghost_proposal(next, "我已经知道玉佩在哪了。".to_string(), "eval-model")
        .unwrap();

    let trace = kernel.trace_snapshot(10);
    let packet_trace = trace
        .task_packets
        .iter()
        .find(|packet| packet.task == "GhostWriting")
        .expect("next ghost task should record a task packet");
    assert!(packet_trace.foundation_complete);
    assert_eq!(packet_trace.packet.scope, TaskScope::CursorWindow);
    assert!(packet_trace
        .packet
        .required_context
        .iter()
        .any(|context| context.source_type == "ResultFeedback" && context.required));
    assert!(packet_trace
        .packet
        .beliefs
        .iter()
        .any(|belief| belief.subject == "ResultFeedback" && belief.statement.contains("玉佩")));
}

