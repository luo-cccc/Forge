use super::*;

pub fn run_chapter_mission_result_feedback_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-1",
            "林墨追查玉佩下落。",
            "玉佩",
            "提前揭开真相",
            "下落",
            "eval",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut save = observation("林墨发现玉佩的下落，但张三仍没有说出真相。");
    save.reason = ObservationReason::Save;
    save.source = ObservationSource::ChapterSave;
    kernel.observe(save).unwrap();

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
            "玉佩下落",
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
            "玉佩线索",
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
            "玉佩线索",
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
            "玉佩线索",
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
    if debt.mission_count != 1 {
        errors.push(format!(
            "expected 1 mission debt after save gap, got {}",
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
            "玉佩线索",
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
    let debt = kernel.story_debt_snapshot();

    let mut errors = Vec::new();
    let mission = kernel.ledger_snapshot().active_chapter_mission;
    if !mission.is_some_and(|mission| mission.status == "drifted") {
        errors.push("mission did not calibrate to drifted".to_string());
    }
    let mission_proposals = proposals
        .iter()
        .filter(|proposal| proposal.kind == ProposalKind::ChapterMission)
        .count();
    if mission_proposals != 1 {
        errors.push(format!(
            "expected one mission violation proposal, got {}",
            mission_proposals
        ));
    }
    if debt.mission_count != 1 {
        errors.push(format!(
            "expected one mission debt after drift, got {}",
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
