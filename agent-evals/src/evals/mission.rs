use super::*;

fn approve_calibration(
    kernel: &mut WriterAgentKernel,
    proposals: &[agent_writer_lib::writer_agent::proposal::AgentProposal],
) {
    if let Some(cal) = proposals.iter().find(|p| {
        p.kind == ProposalKind::ChapterMission && p.rationale.contains("mission calibration")
    }) {
        let op = cal.operations[0].clone();
        let mut approval = eval_approval("mission_calibration");
        approval.proposal_id = Some(cal.id.clone());
        kernel
            .approve_editor_operation_with_approval(op, "", Some(&approval))
            .ok();
    }
}

pub fn run_chapter_mission_result_feedback_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩的下落线索",
            "提前揭开真相",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation("林墨发现玉佩的下落线索，但仍以新的疑问收束，张三没有说出真相。");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    approve_calibration(&mut kernel, &proposals);

    let ledger = kernel.ledger_snapshot();
    let mut errors = Vec::new();
    let mission = ledger.active_chapter_mission.as_ref();
    if !mission.is_some_and(|mission| mission.status == "completed") {
        errors.push(format!(
            "mission was not completed: {:?}",
            mission.map(|mission| mission.status.as_str())
        ));
    }
    if ledger.recent_chapter_results.is_empty() {
        errors.push("save did not record chapter result".to_string());
    }
    if !ledger
        .recent_chapter_results
        .iter()
        .any(|result| result.new_clues.iter().any(|clue| clue == "玉佩"))
    {
        errors.push("chapter result lacks carried clue".to_string());
    }

    eval_result(
        "writer_agent:chapter_mission_result_feedback",
        format!(
            "mission={} results={}",
            mission
                .map(|mission| mission.status.as_str())
                .unwrap_or("missing"),
            ledger.recent_chapter_results.len()
        ),
        errors,
    )
}

pub fn run_chapter_mission_partial_progress_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落。",
            "玉佩下落线索",
            "提前揭开真相",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter(
        "林墨找到玉佩的下落，却发现线索指向另一个疑问。",
        "Chapter-2",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    approve_calibration(&mut kernel, &proposals);
    let ledger = kernel.ledger_snapshot();

    let mut errors = Vec::new();
    let mission = ledger.active_chapter_mission.as_ref();
    if !mission.is_some_and(|mission| mission.status == "completed") {
        errors.push(format!(
            "expected mission completed from must_include + ending, got {:?}",
            mission.map(|mission| mission.status.as_str())
        ));
    }
    if proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::ChapterMission && proposal.preview.contains("必保事项")
    }) {
        errors.push("completed mission still emitted save-gap proposal".to_string());
    }

    eval_result(
        "writer_agent:chapter_mission_completed_no_save_gap",
        format!(
            "mission={} proposals={}",
            mission
                .map(|mission| mission.status.as_str())
                .unwrap_or("missing"),
            proposals.len()
        ),
        errors,
    )
}

pub fn run_chapter_mission_guard_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索推进",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨直接揭开了真相，玉佩来自禁地。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let guard = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ChapterMission);
    if guard.is_none() {
        errors.push("missing chapter mission guard proposal".to_string());
    }
    if !guard.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::TextAnnotate { .. }))
    }) {
        errors.push(
            "chapter mission guard lacks mission evidence or annotation operation".to_string(),
        );
    }
    if !debt.entries.iter().any(|entry| {
        entry.category == StoryDebtCategory::ChapterMission && entry.title.contains("mission")
    }) {
        errors.push("chapter mission guard did not enter story debt".to_string());
    }

    eval_result(
        "writer_agent:chapter_mission_guard_story_debt",
        format!("proposals={} debt={}", proposals.len(), debt.total),
        errors,
    )
}

pub fn run_chapter_mission_negated_guard_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索推进",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨没有揭开真相，只确认玉佩仍在张三袖中。",
            "Chapter-2",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    if proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::ChapterMission)
    {
        errors.push("negated mission reveal still created guard proposal".to_string());
    }
    if debt.mission_count != 0 {
        errors.push(format!(
            "negated mission reveal created {} mission debts",
            debt.mission_count
        ));
    }

    eval_result(
        "writer_agent:chapter_mission_negated_reveal_no_debt",
        format!(
            "proposals={} missionDebt={}",
            proposals.len(),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_chapter_mission_save_gap_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落。",
            "玉佩线索推进",
            "提前揭开真相",
            "以线索推进收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨站在雨里，沉默地看着远处灯火。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    approve_calibration(&mut kernel, &proposals);
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let guard = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::ChapterMission && proposal.preview.contains("必保事项")
    });
    if guard.is_none() {
        errors.push("missing chapter mission save-gap guard".to_string());
    }
    if !guard.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && proposal
                .evidence
                .iter()
                .any(|evidence| evidence.source == EvidenceSource::ChapterText)
    }) {
        errors.push("save-gap guard lacks mission and chapter-result evidence".to_string());
    }
    if debt.mission_count < 1 {
        errors.push(format!(
            "expected at least 1 mission debt after save gap, got {}",
            debt.mission_count
        ));
    }

    eval_result(
        "writer_agent:chapter_mission_save_gap_story_debt",
        format!(
            "proposals={} missionDebt={}",
            proposals.len(),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_chapter_mission_drifted_no_duplicate_save_gap_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-2",
            "林墨追查玉佩下落，但不能提前揭开真相。",
            "玉佩线索推进",
            "提前揭开真相",
            "以误导线索收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation_in_chapter("林墨直接揭开真相，玉佩来自禁地。", "Chapter-2");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    approve_calibration(&mut kernel, &proposals);
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let mission = kernel.ledger_snapshot().active_chapter_mission;
    if !mission.is_some_and(|mission| mission.status == "drifted") {
        errors.push("mission did not calibrate to drifted".to_string());
    }
    let calibration = proposals.iter().any(|p| {
        p.kind == ProposalKind::ChapterMission && p.rationale.contains("mission calibration")
    });
    if !calibration {
        errors.push("missing mission calibration proposal".to_string());
    }
    if debt.mission_count < 1 {
        errors.push(format!(
            "expected at least one mission debt after drift, got {}",
            debt.mission_count
        ));
    }

    eval_result(
        "writer_agent:chapter_mission_drift_no_duplicate_gap",
        format!(
            "proposals={} missionDebt={}",
            proposals.len(),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_mission_state_transition_requires_evidence_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩的下落线索",
            "提前揭开真相",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let initial_status = kernel
        .ledger_snapshot()
        .active_chapter_mission
        .as_ref()
        .map(|mission| mission.status.clone())
        .unwrap_or_default();

    let mut save = observation("林墨发现玉佩的下落线索，但仍以新的疑问收束，张三没有说出真相。");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    approve_calibration(&mut kernel, &proposals);

    let ledger = kernel.ledger_snapshot();
    let mission = ledger.active_chapter_mission.as_ref();
    let result_source = ledger
        .recent_chapter_results
        .first()
        .map(|result| result.source_ref.as_str())
        .unwrap_or_default();
    let evidence_linked = mission.is_some_and(|mission| {
        mission.status == "completed"
            && mission.source_ref.contains("result_feedback:chapter_save")
            && mission.source_ref.contains(result_source)
            && ledger
                .recent_decisions
                .iter()
                .any(|decision| decision.decision == "mission_status:completed")
    });

    let mut errors = Vec::new();
    if initial_status != "draft" {
        errors.push(format!(
            "seeded mission should start as draft, got {}",
            initial_status
        ));
    }
    if !evidence_linked {
        errors.push(format!(
            "mission status transition did not retain result evidence: status={:?} source={:?}",
            mission.map(|mission| mission.status.as_str()),
            mission.map(|mission| mission.source_ref.as_str())
        ));
    }

    eval_result(
        "writer_agent:mission_state_transition_requires_evidence",
        format!(
            "initial={} final={} decisions={}",
            initial_status,
            mission
                .map(|mission| mission.status.as_str())
                .unwrap_or("missing"),
            ledger.recent_decisions.len()
        ),
        errors,
    )
}

pub fn run_mission_blocked_retired_not_auto_calibrated_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩的下落线索",
            "提前揭开真相",
            "以新的疑问收束。",
            "eval",
        )
        .unwrap();
    let mut blocked = memory
        .get_chapter_mission("eval", "Chapter-1")
        .unwrap()
        .unwrap();
    blocked.status = "blocked".to_string();
    blocked.source_ref = "author:blocker".to_string();
    memory.upsert_chapter_mission(&blocked).unwrap();

    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    let mut save = observation("林墨发现玉佩的下落，但张三仍没有说出真相。");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    let proposals = kernel.observe(save).unwrap();
    let ledger = kernel.ledger_snapshot();
    let mission = ledger.active_chapter_mission.as_ref();
    let mission_gap_proposals = proposals
        .iter()
        .filter(|proposal| proposal.kind == ProposalKind::ChapterMission)
        .count();

    let mut errors = Vec::new();
    if !mission.is_some_and(|mission| mission.status == "blocked") {
        errors.push(format!(
            "blocked mission was auto-calibrated: {:?}",
            mission.map(|mission| mission.status.as_str())
        ));
    }
    if mission_gap_proposals != 0 {
        errors.push(format!(
            "blocked mission emitted {} mission proposals",
            mission_gap_proposals
        ));
    }

    eval_result(
        "writer_agent:mission_blocked_retired_not_auto_calibrated",
        format!(
            "status={} missionProposals={}",
            mission
                .map(|mission| mission.status.as_str())
                .unwrap_or("missing"),
            mission_gap_proposals
        ),
        errors,
    )
}

pub fn run_mission_drift_flag_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落，推进与张三的关系。",
            "林墨与张三的对话应揭示关系变化",
            "",
            "林墨对张三的态度从仇恨转为理解",
            "eval",
        )
        .unwrap();
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
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());

    let mut save = observation_in_chapter(
        "远山如黛，云雾缭绕。林间的溪水潺潺流淌。一座凉亭矗立在悬崖边，寂寞而安静。风吹过竹林，沙沙作响。",
        "Chapter-1",
    );
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

    let ledger = kernel.ledger_snapshot();
    let debt = kernel.story_debt_snapshot();
    let mut errors = Vec::new();

    let mission = ledger.active_chapter_mission.as_ref();
    if mission.is_none() {
        errors.push("chapter mission not found".to_string());
    } else if mission.unwrap().status == "completed" {
        errors.push(
            "mission should not be completed when text ignores mission requirements".to_string(),
        );
    }
    if debt.mission_count == 0 {
        errors.push("mission drift should produce mission story debt".to_string());
    }

    eval_result(
        "writer_agent:mission_drift_flag",
        format!(
            "missionStatus={} missionDebt={}",
            mission.map(|m| m.status.as_str()).unwrap_or("none"),
            debt.mission_count
        ),
        errors,
    )
}

pub fn run_goal_drift_creates_story_debt_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-6",
            "林墨在禁地外追查玉佩线索，但本章不能提前揭开玉佩来源。",
            "玉佩线索推进",
            "提前揭开玉佩来源",
            "以新的旁证和误导收束。",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let base_text = "林墨站在禁地外，确认张三仍隐瞒着关键线索。";
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation_in_chapter(base_text, "Chapter-6"),
            "张三说出真相：玉佩来自禁地。".to_string(),
            "eval-model",
        )
        .unwrap();
    let operation = proposal.operations[0].clone();
    let mut approval = eval_approval("goal_drift_story_debt");
    approval.proposal_id = Some(proposal.id.clone());
    let approved = kernel
        .approve_editor_operation_with_approval(operation.clone(), "rev-1", Some(&approval))
        .unwrap();
    let saved_text = format!("{}{}", base_text, "张三说出真相：玉佩来自禁地。");
    kernel
        .record_operation_durable_save_with_post_write(
            Some(proposal.id.clone()),
            operation,
            "editor_save:rev-2".to_string(),
            Some(saved_text.clone()),
            Some("Chapter-6".to_string()),
            Some("rev-2".to_string()),
        )
        .unwrap();

    let snapshot = kernel.trace_snapshot(50);
    let debt = kernel.story_debt_snapshot();
    let pending = kernel.pending_proposals();
    let mission_proposal = pending
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ChapterMission);

    let mut errors = Vec::new();
    if !approved.success {
        errors.push("accepted operation approval failed".to_string());
    }
    if !snapshot.post_write_diagnostics.iter().any(|report| {
        report
            .diagnostics
            .iter()
            .any(|diagnostic| format!("{:?}", diagnostic.category) == "ChapterMissionViolation")
    }) {
        errors.push("post-write diagnostics missed chapter mission violation".to_string());
    }
    if debt.mission_count == 0 {
        errors.push("accepted operation mission drift did not enter story debt".to_string());
    }
    if mission_proposal.is_none() {
        errors.push("accepted operation mission drift did not create pending proposal".to_string());
    }
    if !mission_proposal.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::ChapterMission)
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::TextAnnotate { .. }))
    }) {
        errors.push(
            "mission drift proposal lacks mission evidence or annotation operation".to_string(),
        );
    }
    if !saved_text.contains("张三说出真相：玉佩来自禁地。") {
        errors.push("eval fixture lost accepted text before diagnostics".to_string());
    }

    eval_result(
        "writer_agent:goal_drift_creates_story_debt",
        format!(
            "reports={} pending={} missionDebt={}",
            snapshot.post_write_diagnostics.len(),
            pending.len(),
            debt.mission_count
        ),
        errors,
    )
}
