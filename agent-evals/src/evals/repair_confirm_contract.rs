use crate::fixtures::*;
use agent_writer_lib::chapter_generation::{
    chapter_contract_outcome, ChapterContract, ChapterContractOutcome, ChapterContractPhase,
};

pub fn run_repair_confirm_contract_eval() -> EvalResult {
    let contract = ChapterContract::default();
    let under_content = "x".repeat(2500);
    let over_content = "x".repeat(4500);
    let under = chapter_contract_outcome(&under_content, &contract, ChapterContractPhase::ModelOutput);
    let over = chapter_contract_outcome(&over_content, &contract, ChapterContractPhase::ModelOutput);
    let bounds_ok = contract.target_chars == 3500
        && contract.min_chars == 3000
        && contract.max_chars == 4000
        && contract.save_hard_floor_chars == 2800
        && contract.save_hard_ceiling_chars == 4300;
    let outcome_ok = matches!(under, ChapterContractOutcome::UnderMinChars)
        && matches!(over, ChapterContractOutcome::OverMaxChars);
    let ok = bounds_ok && outcome_ok;
    EvalResult::pass_if(
        "writer_agent:auto_repair_vs_author_confirm",
        ok,
        format!(
            "bounds={} under={:?} over={:?} t={} min={} max={} floor={} ceil={}",
            bounds_ok,
            under,
            over,
            contract.target_chars,
            contract.min_chars,
            contract.max_chars,
            contract.save_hard_floor_chars,
            contract.save_hard_ceiling_chars,
        ),
    )
}
