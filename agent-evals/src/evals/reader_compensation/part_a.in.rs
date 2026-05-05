use agent_writer_lib::writer_agent::memory::{
    EmotionalDebtLifecycle, ReaderCompensationProfile, WriterMemory,
};

pub fn run_reader_compensation_profile_extracts_lack_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let profile = ReaderCompensationProfile {
        target_reader: "渴望尊严的年轻读者".to_string(),
        primary_lack: "dignity".to_string(),
        secondary_lacks: vec!["recognition".to_string()],
        dominant_relationship_soil: "师徒误解".to_string(),
        confidence: 0.8,
        pending_approval: true,
        ..Default::default()
    };
    memory
        .upsert_reader_compensation_profile("eval", &profile)
        .unwrap();
    let got = memory
        .get_reader_compensation_profile("eval")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.primary_lack != "dignity" {
        errors.push(format!("expected dignity, got {}", got.primary_lack));
    }
    if !got.pending_approval {
        errors.push("profile should require approval".to_string());
    }
    eval_result("reader_compensation_profile_extracts_lack", String::new(), errors)
}

pub fn run_reader_compensation_profile_requires_author_approval_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let profile = ReaderCompensationProfile {
        primary_lack: "safety".to_string(),
        pending_approval: true,
        ..Default::default()
    };
    memory
        .upsert_reader_compensation_profile("eval", &profile)
        .unwrap();
    let got = memory
        .get_reader_compensation_profile("eval")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if !got.pending_approval {
        errors.push("new profile must require author approval".to_string());
    }
    memory
        .approve_reader_compensation_profile("eval", "author")
        .unwrap();
    let approved = memory
        .get_reader_compensation_profile("eval")
        .unwrap()
        .unwrap();
    if approved.pending_approval {
        errors.push("profile should be approved after explicit approval".to_string());
    }
    eval_result(
        "reader_compensation_profile_requires_author_approval",
        String::new(),
        errors,
    )
}

pub fn run_reader_compensation_profile_preserves_project_tone_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let profile = ReaderCompensationProfile {
        primary_lack: "dignity".to_string(),
        payoff_mode: "gradual_growth".to_string(),
        forbidden_shortcuts: vec!["instant_revenge".to_string(), "deus_ex_machina".to_string()],
        ..Default::default()
    };
    memory
        .upsert_reader_compensation_profile("eval", &profile)
        .unwrap();
    let got = memory
        .get_reader_compensation_profile("eval")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.forbidden_shortcuts.len() != 2 {
        errors.push("should preserve forbidden_shortcuts".to_string());
    }
    if got.payoff_mode != "gradual_growth" {
        errors.push("payoff_mode should be preserved".to_string());
    }
    eval_result(
        "reader_compensation_profile_preserves_project_tone",
        String::new(),
        errors,
    )
}

pub fn run_emotional_debt_lifecycle_tracks_partial_payoff_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let lc = EmotionalDebtLifecycle {
        debt_id: "eval-debt-1".to_string(),
        debt_kind: "dignity_debt".to_string(),
        current_state: "introduced".to_string(),
        ..Default::default()
    };
    memory.upsert_emotional_debt_lifecycle(&lc).unwrap();
    memory
        .advance_emotional_debt_state("eval-debt-1", "partially_paid", "chapter-5")
        .unwrap();
    let got = memory
        .get_emotional_debt_lifecycle("eval-debt-1")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.current_state != "partially_paid" {
        errors.push(format!(
            "expected partially_paid, got {}",
            got.current_state
        ));
    }
    eval_result(
        "emotional_debt_lifecycle_tracks_partial_payoff",
        String::new(),
        errors,
    )
}

pub fn run_emotional_debt_lifecycle_rolls_over_after_payoff_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let lc = EmotionalDebtLifecycle {
        debt_id: "eval-rollover".to_string(),
        debt_kind: "recognition_debt".to_string(),
        current_state: "paid".to_string(),
        rollover_target: "eval-next-debt".to_string(),
        ..Default::default()
    };
    memory.upsert_emotional_debt_lifecycle(&lc).unwrap();
    let got = memory
        .get_emotional_debt_lifecycle("eval-rollover")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.rollover_target != "eval-next-debt" {
        errors.push("rollover_target should be preserved".to_string());
    }
    eval_result(
        "emotional_debt_lifecycle_rolls_over_after_payoff",
        String::new(),
        errors,
    )
}

pub fn run_emotional_debt_lifecycle_flags_overdue_without_autowrite_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let lc = EmotionalDebtLifecycle {
        debt_id: "eval-overdue".to_string(),
        debt_kind: "fate_debt".to_string(),
        current_state: "escalating".to_string(),
        overdue_risk: "high".to_string(),
        ..Default::default()
    };
    memory.upsert_emotional_debt_lifecycle(&lc).unwrap();
    let got = memory
        .get_emotional_debt_lifecycle("eval-overdue")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.overdue_risk != "high" {
        errors.push("overdue risk should be flagged".to_string());
    }
    eval_result(
        "emotional_debt_lifecycle_flags_overdue_without_autowrite",
        String::new(),
        errors,
    )
}

pub fn run_chapter_mission_tracks_pressure_and_payoff_eval() -> EvalResult {
    let memory = WriterMemory::open(std::path::Path::new(":memory:")).unwrap();
    let mission = agent_writer_lib::writer_agent::memory::ChapterMissionSummary {
        project_id: "eval".to_string(),
        chapter_title: "Chapter-1".to_string(),
        mission: "test".to_string(),
        pressure_scene: "主角在朝堂被当众羞辱".to_string(),
        payoff_target: "主角以实力证明自己".to_string(),
        ..Default::default()
    };
    memory.upsert_chapter_mission(&mission).unwrap();
    let got = memory
        .get_chapter_mission("eval", "Chapter-1")
        .unwrap()
        .unwrap();
    let mut errors = Vec::new();
    if got.pressure_scene != "主角在朝堂被当众羞辱" {
        errors.push("pressure_scene not preserved".to_string());
    }
    if got.payoff_target != "主角以实力证明自己" {
        errors.push("payoff_target not preserved".to_string());
    }
    eval_result(
        "chapter_mission_tracks_pressure_and_payoff",
        String::new(),
        errors,
    )
}
