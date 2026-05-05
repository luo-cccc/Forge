pub fn run_chapter_mission_opens_next_reader_lack_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mission = agent_writer_lib::writer_agent::memory::ChapterMissionSummary {
        project_id: "eval".to_string(),
        chapter_title: "Chapter-2".to_string(),
        next_lack_opened: "获得地位后，发现更大的阴谋".to_string(),
        ..Default::default()
    };
    memory.upsert_chapter_mission(&mission).unwrap();
    let got = memory
        .get_chapter_mission("eval", "Chapter-2")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.next_lack_opened != "获得地位后，发现更大的阴谋" {
        errors.push("next_lack_opened not preserved".to_string());
    }
    eval_result("chapter_mission_opens_next_reader_lack", String::new(), errors)
}

pub fn run_chapter_mission_tracks_relationship_soil_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mission = agent_writer_lib::writer_agent::memory::ChapterMissionSummary {
        project_id: "eval".to_string(),
        chapter_title: "Chapter-3".to_string(),
        relationship_soil_this_chapter: "师徒之间的信任危机".to_string(),
        ..Default::default()
    };
    memory.upsert_chapter_mission(&mission).unwrap();
    let got = memory
        .get_chapter_mission("eval", "Chapter-3")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.relationship_soil_this_chapter != "师徒之间的信任危机" {
        errors.push("relationship_soil not preserved".to_string());
    }
    eval_result(
        "chapter_mission_tracks_relationship_soil",
        String::new(),
        errors,
    )
}

pub fn run_emotional_debt_created_from_pressure_scene_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let id = memory
        .add_emotional_debt(
            "eval",
            "dignity_debt",
            "朝堂之辱",
            "主角被当众羞辱",
            "Chapter-1",
            "scene:ch1:council",
            "师徒",
            "主角跪在殿前，无人为其说话",
            "误解延迟",
            "主角将在殿试中证明自己",
            "Chapter-5",
            "正面逆袭",
            "high",
            &["scene:ch1:council".to_string()],
        )
        .unwrap();
    let mut errors = Vec::new();
    if id <= 0 {
        errors.push("debt id should be positive".to_string());
    }
    let open = memory.get_open_emotional_debts("eval").unwrap();
    if open.is_empty() {
        errors.push("should have open debts".to_string());
    }
    eval_result("emotional_debt_created_from_pressure_scene", String::new(), errors)
}

pub fn run_emotional_debt_payoff_closes_with_evidence_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let id = memory
        .add_emotional_debt(
            "eval", "recognition_debt", "无人认可", "努力不被看见",
            "Chapter-1", "ref:1", "同伴", "被忽视", "公开羞辱",
            "获得认可", "Chapter-3", "成长证明", "medium",
            &["ref:1".to_string()],
        )
        .unwrap();
    memory
        .record_emotional_payoff(id, "主角获颁奖章", "scene:ch3:award")
        .unwrap();
    let open = memory.get_open_emotional_debts("eval").unwrap();
    let mut errors = Vec::new();
    if open.iter().any(|d| d.id == id) {
        errors.push("paid debt should not appear in open debts".to_string());
    }
    eval_result(
        "emotional_debt_payoff_closes_with_evidence",
        String::new(),
        errors,
    )
}

pub fn run_emotional_debt_does_not_autowrite_promise_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .add_emotional_debt(
            "eval", "safety_debt", "安全威胁", "世界崩坏",
            "Chapter-1", "ref:1", "家族", "压迫", "计息",
            "兑现", "Chapter-5", "逆袭", "low",
            &[],
        )
        .unwrap();
    let debts = memory.list_emotional_debts("eval", 10).unwrap();
    let mut errors = Vec::new();
    for debt in &debts {
        if debt.description.contains("自动生成") {
            errors.push("debt should not contain auto-generated content".to_string());
        }
    }
    eval_result("emotional_debt_does_not_autowrite_promise", String::new(), errors)
}

pub fn run_emotional_debt_tracks_interest_mechanism_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .add_emotional_debt(
            "eval", "fate_debt", "命运之债", "无法改变的宿命",
            "Chapter-1", "ref:1", "自我", "压迫证据", "误解延迟",
            "兑现合同", "Chapter-10", "改变规则", "high",
            &[],
        )
        .unwrap();
    let debts = memory.list_emotional_debts("eval", 10).unwrap();
    let mut errors = Vec::new();
    let found = debts.iter().any(|d| d.interest_mechanism == "误解延迟");
    if !found {
        errors.push("interest_mechanism should be preserved".to_string());
    }
    eval_result(
        "emotional_debt_tracks_interest_mechanism",
        String::new(),
        errors,
    )
}

pub fn run_payoff_diagnostic_flags_pressure_without_payoff_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .add_emotional_debt(
            "eval", "dignity_debt", "朝堂之辱", "被羞辱",
            "Chapter-1", "ref:1", "师徒", "压迫证据",
            "计息", "兑现", "Chapter-3", "逆袭", "high",
            &[],
        )
        .unwrap();
    let engine = agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine::new();
    let results = engine.diagnose_payoff("Chapter-3", "eval", &memory);
    let mut errors = Vec::new();
    if results.is_empty() {
        errors.push("should flag overdue emotional debt".to_string());
    }
    eval_result(
        "payoff_diagnostic_flags_pressure_without_payoff",
        String::new(),
        errors,
    )
}

pub fn run_payoff_diagnostic_flags_unearned_payoff_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mission = agent_writer_lib::writer_agent::memory::ChapterMissionSummary {
        project_id: "eval".to_string(),
        chapter_title: "Chapter-1".to_string(),
        payoff_target: "主角大获全胜".to_string(),
        pressure_scene: String::new(),
        ..Default::default()
    };
    memory.upsert_chapter_mission(&mission).unwrap();
    let engine = agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine::new();
    let results = engine.diagnose_payoff("Chapter-1", "eval", &memory);
    let mut errors = Vec::new();
    let has_unearned = results.iter().any(|r| r.message.contains("缺少前置压迫场景"));
    if !has_unearned {
        errors.push("should flag payoff without pressure".to_string());
    }
    eval_result(
        "payoff_diagnostic_flags_unearned_payoff",
        String::new(),
        errors,
    )
}

pub fn run_payoff_diagnostic_flags_overfilled_lack_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mission = agent_writer_lib::writer_agent::memory::ChapterMissionSummary {
        project_id: "eval".to_string(),
        chapter_title: "Chapter-1".to_string(),
        payoff_target: "所有问题都解决了".to_string(),
        next_lack_opened: String::new(),
        ..Default::default()
    };
    memory.upsert_chapter_mission(&mission).unwrap();
    let engine = agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine::new();
    let results = engine.diagnose_payoff("Chapter-1", "eval", &memory);
    let mut errors = Vec::new();
    let has_overfilled = results.iter().any(|r| r.message.contains("追读可能在此处断裂"));
    if !has_overfilled {
        errors.push("should flag overfilled lack".to_string());
    }
    eval_result(
        "payoff_diagnostic_flags_overfilled_lack",
        String::new(),
        errors,
    )
}

pub fn run_payoff_diagnostic_flags_repetitive_interest_mechanism_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    memory
        .add_emotional_debt(
            "eval", "dignity_debt", "测试债务", "test",
            "Chapter-1", "ref:1", "", "", "误解延迟",
            "兑现", "Chapter-5", "逆袭", "medium",
            &[],
        )
        .unwrap();
    let engine = agent_writer_lib::writer_agent::diagnostics::DiagnosticsEngine::new();
    let results = engine.diagnose_payoff("Chapter-2", "eval", &memory);
    let mut errors = Vec::new();
    let has_soil_gap = results.iter().any(|r| r.message.contains("缺少关系土壤"));
    if !has_soil_gap {
        errors.push("should flag missing relationship soil".to_string());
    }
    eval_result(
        "payoff_diagnostic_flags_repetitive_interest_mechanism",
        String::new(),
        errors,
    )
}
