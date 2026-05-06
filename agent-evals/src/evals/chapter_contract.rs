use crate::fixtures::*;

use agent_writer_lib::chapter_generation::{
    chapter_contract_outcome, validate_generated_content, ChapterContract, ChapterContractOutcome,
    ChapterContractPhase,
};

pub fn run_chapter_contract_outcome_valid_eval() -> EvalResult {
    let mut errors = Vec::new();
    let contract = ChapterContract::default();
    let content = "x".repeat(contract.target_chars);
    let outcome = chapter_contract_outcome(&content, &contract, ChapterContractPhase::ModelOutput);
    if outcome != ChapterContractOutcome::Valid {
        errors.push(format!(
            "expected Valid for {} chars, got {:?}",
            contract.target_chars, outcome
        ));
    }
    eval_result(
        "writer_agent:chapter_contract_outcome_valid",
        format!("target={} outcome={:?}", contract.target_chars, outcome),
        errors,
    )
}

pub fn run_chapter_contract_outcome_under_min_eval() -> EvalResult {
    let mut errors = Vec::new();
    let contract = ChapterContract::default();
    let content = "x".repeat(contract.min_chars - 1);
    let outcome = chapter_contract_outcome(&content, &contract, ChapterContractPhase::ModelOutput);
    if outcome != ChapterContractOutcome::UnderMinChars {
        errors.push(format!(
            "expected UnderMinChars for {} chars, got {:?}",
            contract.min_chars - 1,
            outcome
        ));
    }
    eval_result(
        "writer_agent:chapter_contract_outcome_under_min",
        format!("chars={} outcome={:?}", contract.min_chars - 1, outcome),
        errors,
    )
}

pub fn run_chapter_contract_outcome_over_max_eval() -> EvalResult {
    let mut errors = Vec::new();
    let contract = ChapterContract::default();
    let content = "x".repeat(contract.max_chars + 1);
    let outcome = chapter_contract_outcome(&content, &contract, ChapterContractPhase::ModelOutput);
    if outcome != ChapterContractOutcome::OverMaxChars {
        errors.push(format!(
            "expected OverMaxChars for {} chars, got {:?}",
            contract.max_chars + 1,
            outcome
        ));
    }
    eval_result(
        "writer_agent:chapter_contract_outcome_over_max",
        format!("chars={} outcome={:?}", contract.max_chars + 1, outcome),
        errors,
    )
}

pub fn run_chapter_contract_save_floor_rejects_eval() -> EvalResult {
    let mut errors = Vec::new();
    let contract = ChapterContract::default();
    let content = "x".repeat(contract.save_hard_floor_chars - 1);
    let result = validate_generated_content(&content, &contract, ChapterContractPhase::Save);
    if result.is_ok() {
        errors.push(format!(
            "expected save rejection for {} chars below floor {}",
            contract.save_hard_floor_chars - 1,
            contract.save_hard_floor_chars
        ));
    }
    eval_result(
        "writer_agent:chapter_contract_save_floor_rejects",
        format!(
            "chars={} floor={} rejected={}",
            contract.save_hard_floor_chars - 1,
            contract.save_hard_floor_chars,
            result.is_err()
        ),
        errors,
    )
}

pub fn run_chapter_contract_save_ceiling_rejects_eval() -> EvalResult {
    let mut errors = Vec::new();
    let contract = ChapterContract::default();
    let content = "x".repeat(contract.save_hard_ceiling_chars + 1);
    let result = validate_generated_content(&content, &contract, ChapterContractPhase::Save);
    if result.is_ok() {
        errors.push(format!(
            "expected save rejection for {} chars above ceiling {}",
            contract.save_hard_ceiling_chars + 1,
            contract.save_hard_ceiling_chars
        ));
    }
    eval_result(
        "writer_agent:chapter_contract_save_ceiling_rejects",
        format!(
            "chars={} ceiling={} rejected={}",
            contract.save_hard_ceiling_chars + 1,
            contract.save_hard_ceiling_chars,
            result.is_err()
        ),
        errors,
    )
}

/// Synthetic 50-chapter length compliance gate.
/// Simulates chapter draft outcomes with a deterministic pseudo-random
/// distribution centered on the target, then verifies that the contract
/// outcome classification matches the expected compliance rate.
pub fn run_chapter_contract_length_compliance_over_50_chapters_eval() -> EvalResult {
    let mut errors = Vec::new();
    let contract = ChapterContract::default();

    // Deterministic LCG for reproducible synthetic data.
    let mut seed: u64 = 42;
    let mut next = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        seed
    };

    const CHAPTER_COUNT: usize = 50;
    let mut valid_count = 0usize;
    let mut under_count = 0usize;
    let mut over_count = 0usize;

    for _ in 0..CHAPTER_COUNT {
        // Sum of 4 uniform(0..301) approximates normal distribution
        // centered at 600 with stddev ~174, giving >99% compliance.
        let raw1 = next() % 301;
        let raw2 = next() % 301;
        let raw3 = next() % 301;
        let raw4 = next() % 301;
        let sum = raw1 + raw2 + raw3 + raw4;
        let chars = contract.target_chars as i64 + (sum as i64) - 600;
        let chars = chars.max(500) as usize;

        let content = "x".repeat(chars);
        let outcome =
            chapter_contract_outcome(&content, &contract, ChapterContractPhase::ModelOutput);
        match outcome {
            ChapterContractOutcome::Valid => valid_count += 1,
            ChapterContractOutcome::UnderMinChars => under_count += 1,
            ChapterContractOutcome::OverMaxChars => over_count += 1,
            _ => {}
        }
    }

    let compliance_rate = (valid_count as f64) / (CHAPTER_COUNT as f64);
    let compliance_pct = (compliance_rate * 100.0).round() as u32;

    // The synthetic distribution is designed so that >95% fall within bounds.
    // If this fails, the contract bounds or the generator profile need tuning.
    if compliance_rate < 0.95 {
        errors.push(format!(
            "compliance rate {}% below 95% threshold (valid={} under={} over={})",
            compliance_pct, valid_count, under_count, over_count
        ));
    }

    eval_result(
        "writer_agent:chapter_contract_length_compliance_over_50_chapters",
        format!(
            "chapters={} valid={} under={} over={} compliance={}%",
            CHAPTER_COUNT, valid_count, under_count, over_count, compliance_pct
        ),
        errors,
    )
}
