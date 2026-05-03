use super::*;

use agent_writer_lib::writer_agent::provider_budget::{
    estimate_provider_cost_micros, evaluate_provider_budget, WriterProviderBudgetDecision,
    WriterProviderBudgetRequest, WriterProviderBudgetTask,
};

pub fn run_provider_budget_requires_approval_eval() -> EvalResult {
    let mut request = WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        "gpt-4o",
        70_000,
        18_000,
    );
    request.max_total_tokens_without_approval = 60_000;
    request.max_estimated_cost_micros_without_approval = 120_000;
    let report = evaluate_provider_budget(request);

    let mut approved_request = WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ExternalResearch,
        "deepseek/deepseek-v4-flash",
        42_000,
        8_000,
    );
    approved_request.max_total_tokens_without_approval = 45_000;
    approved_request.max_estimated_cost_micros_without_approval = 20_000;
    approved_request.already_approved = true;
    let approved_report = evaluate_provider_budget(approved_request);

    let mut errors = Vec::new();
    if report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "over-budget chapter generation should require approval, got {:?}",
            report.decision
        ));
    }
    if !report.approval_required {
        errors.push("over-budget report did not set approval_required".to_string());
    }
    if !report
        .reasons
        .iter()
        .any(|reason| reason.contains("tokens"))
        || !report.reasons.iter().any(|reason| reason.contains("cost"))
    {
        errors.push(format!(
            "provider budget reasons should include token and cost evidence: {:?}",
            report.reasons
        ));
    }
    if report.remediation.is_empty() {
        errors.push("provider budget approval report lacks remediation".to_string());
    }
    if approved_report.decision != WriterProviderBudgetDecision::Warn {
        errors.push(format!(
            "approved over-budget request should continue as warning, got {:?}",
            approved_report.decision
        ));
    }
    if approved_report.approval_required {
        errors.push("approved over-budget request still requires approval".to_string());
    }
    if estimate_provider_cost_micros("gpt-4o", 1_000_000, 1_000_000) <= 0 {
        errors.push("provider cost estimator returned zero for known model".to_string());
    }

    eval_result(
        "writer_agent:provider_budget_requires_approval",
        format!(
            "decision={:?} cost={} approvedDecision={:?}",
            report.decision, report.estimated_cost_micros, approved_report.decision
        ),
        errors,
    )
}

pub fn run_chapter_generation_provider_budget_preflight_eval() -> EvalResult {
    let target = ChapterTarget {
        title: "Chapter-9".to_string(),
        filename: "chapter-9.md".to_string(),
        number: Some(9),
        summary: "林墨追查寒玉戒指下落。".to_string(),
        status: "empty".to_string(),
    };
    let receipt = agent_writer_lib::chapter_generation::build_chapter_generation_receipt(
        "budget-preflight-1",
        &target,
        "rev-9",
        "写第九章。",
        &[ChapterContextSource {
            source_type: "instruction".to_string(),
            id: "user-instruction".to_string(),
            label: "User instruction".to_string(),
            original_chars: 5,
            included_chars: 5,
            truncated: false,
            score: None,
        }],
        now_ms(),
    );
    let over_budget_report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
        WriterProviderBudgetTask::ChapterGeneration,
        "gpt-4o",
        90_000,
        24_000,
    ));
    let error = agent_writer_lib::chapter_generation::provider_budget_error(
        "budget-preflight-1",
        &receipt,
        over_budget_report.clone(),
    );
    let bundle = error.evidence.clone();

    let mut errors = Vec::new();
    if over_budget_report.decision != WriterProviderBudgetDecision::ApprovalRequired {
        errors.push(format!(
            "over-budget chapter preflight should require approval, got {:?}",
            over_budget_report.decision
        ));
    }
    if error.code != "PROVIDER_BUDGET_APPROVAL_REQUIRED" {
        errors.push(format!(
            "unexpected provider budget error code {}",
            error.code
        ));
    }
    let Some(bundle) = bundle else {
        errors.push("provider budget error lacks failure evidence bundle".to_string());
        return eval_result(
            "writer_agent:chapter_generation_provider_budget_preflight",
            "missing bundle".to_string(),
            errors,
        );
    };
    if bundle.category
        != agent_writer_lib::writer_agent::task_receipt::WriterFailureCategory::ProviderFailed
    {
        errors.push("provider budget failure does not map to provider_failed".to_string());
    }
    if bundle.remediation.is_empty() {
        errors.push("provider budget failure lacks remediation".to_string());
    }
    if bundle
        .details
        .get("providerBudget")
        .and_then(|value| value.get("approvalRequired"))
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        errors.push("failure bundle does not preserve approval-required budget report".to_string());
    }

    eval_result(
        "writer_agent:chapter_generation_provider_budget_preflight",
        format!(
            "decision={:?} evidenceRefs={}",
            over_budget_report.decision,
            bundle.evidence_refs.len()
        ),
        errors,
    )
}
