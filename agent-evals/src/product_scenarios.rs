use crate::fixtures::*;
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::memory::{ChapterResultSummary, WriterMemory};
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::proposal::{EvidenceSource, ProposalKind};
use agent_writer_lib::writer_agent::WriterAgentKernel;
use std::path::Path;

pub fn run_product_scenario_evals() -> Vec<EvalResult> {
    vec![
        run_multi_chapter_scenario_eval(),
        run_scenario_chapter_save_feedback_handoff_eval(),
        run_scenario_promise_payoff_nearby_eval(),
        run_scenario_resolved_promise_stays_quiet_eval(),
        run_scenario_object_whereabouts_context_priority_eval(),
        run_scenario_mission_drift_save_eval(),
        run_scenario_canon_conflict_no_autowrite_eval(),
        run_scenario_style_feedback_affects_ghost_context_eval(),
        run_scenario_manual_ask_records_decision_eval(),
        run_scenario_context_explainability_for_longform_eval(),
    ]
}

fn seeded_memory() -> WriterMemory {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻悬疑",
            "刀客追查玉佩真相，在复仇与守护之间做出最终选择。",
            "林墨必须在复仇和守护之间做艰难选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
    memory
}

fn save_observation(
    paragraph: &str,
    chapter_title: &str,
) -> agent_writer_lib::writer_agent::observation::WriterObservation {
    let mut obs = observation_in_chapter(paragraph, chapter_title);
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;
    obs
}

fn switch_observation(
    paragraph: &str,
    chapter_title: &str,
) -> agent_writer_lib::writer_agent::observation::WriterObservation {
    let mut obs = observation_in_chapter(paragraph, chapter_title);
    obs.reason = ObservationReason::ChapterSwitch;
    obs
}

fn record_result(
    memory: &WriterMemory,
    chapter_title: &str,
    revision: &str,
    summary: &str,
    state_changes: &[&str],
    character_progress: &[&str],
    new_conflicts: &[&str],
    new_clues: &[&str],
    promise_updates: &[&str],
    canon_updates: &[&str],
) {
    memory
        .record_chapter_result(&ChapterResultSummary {
            id: 0,
            project_id: "eval".to_string(),
            chapter_title: chapter_title.to_string(),
            chapter_revision: revision.to_string(),
            summary: summary.to_string(),
            state_changes: state_changes.iter().map(|s| s.to_string()).collect(),
            character_progress: character_progress.iter().map(|s| s.to_string()).collect(),
            new_conflicts: new_conflicts.iter().map(|s| s.to_string()).collect(),
            new_clues: new_clues.iter().map(|s| s.to_string()).collect(),
            promise_updates: promise_updates.iter().map(|s| s.to_string()).collect(),
            canon_updates: canon_updates.iter().map(|s| s.to_string()).collect(),
            source_ref: format!("scenario:{}:{}", chapter_title, revision),
            created_at: now_ms(),
        })
        .unwrap();
}

fn has_source(
    kernel: &WriterAgentKernel,
    task: AgentTask,
    obs_text: &str,
    source: ContextSource,
) -> bool {
    let pack =
        kernel.context_pack_for_default(task, &observation_in_chapter(obs_text, "Chapter-5"));
    pack.sources.iter().any(|entry| entry.source == source)
}

fn run_multi_chapter_scenario_eval() -> EvalResult {
    let memory = seeded_memory();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut errors = Vec::new();

    kernel.active_chapter = Some("Chapter-1".to_string());
    kernel
        .memory
        .add_promise(
            "mystery_clue",
            "玉佩线索",
            "张三拿走了刻有龙纹的玉佩",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let p1 = kernel
        .observe(observation_in_chapter(
            "林墨发现张三留下的玉佩盒子里是空的。",
            "Chapter-1",
        ))
        .unwrap();

    kernel.active_chapter = Some("Chapter-2".to_string());
    let p2 = kernel
        .observe(observation_in_chapter(
            "林墨握紧刀柄，终于决定不再逃避。",
            "Chapter-2",
        ))
        .unwrap();

    kernel.active_chapter = Some("Chapter-3".to_string());
    let p3 = kernel
        .observe(observation_in_chapter(
            "一个戴斗笠的神秘人递给林墨另一块完全相同的玉佩。",
            "Chapter-3",
        ))
        .unwrap();

    kernel.active_chapter = Some("Chapter-4".to_string());
    kernel
        .memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-4",
            "林墨找到玉佩的真正主人。",
            "玉佩主人现身",
            "提前揭开玉佩来源",
            "林墨将玉佩归还主人",
            "eval",
        )
        .unwrap();
    kernel
        .observe(save_observation(
            "林墨终于见到了玉佩的真正主人——他的父亲。",
            "Chapter-4",
        ))
        .unwrap();

    kernel.active_chapter = Some("Chapter-5".to_string());
    let p5 = kernel
        .observe(observation_in_chapter(
            "林墨将玉佩挂回父亲的颈上，转身走入风雪。",
            "Chapter-5",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();
    let ledger = kernel.ledger_snapshot();

    let promise_in_context = ledger
        .open_promises
        .iter()
        .any(|p| p.title.contains("玉佩"));
    if !promise_in_context {
        errors.push("promise not tracked in ledger across chapters".to_string());
    }
    if debt.total == 0 {
        errors.push("5-chapter scenario should produce story debt".to_string());
    }
    if p5.is_empty() {
        errors.push("chapter-5 observe produced zero proposals".to_string());
    }

    eval_result(
        "writer_agent:scenario_multi_chapter_promise",
        format!(
            "expected=track promise across 5 chapters actual=p1:{} p2:{} p3:{} p5:{} debt:{} promiseInLedger:{} evidence=PromiseLedger/StoryDebt",
            p1.len(),
            p2.len(),
            p3.len(),
            p5.len(),
            debt.total,
            promise_in_context
        ),
        errors,
    )
}

fn run_scenario_chapter_save_feedback_handoff_eval() -> EvalResult {
    let memory = seeded_memory();
    record_result(
        &memory,
        "Chapter-2",
        "rev-2",
        "林墨承认自己仍想保护张三。",
        &["林墨从追杀转向保护"],
        &["林墨对张三的信任上升"],
        &["追兵已发现客栈"],
        &["玉佩背面有旧族徽"],
        &["玉佩线索需要在第五章回收"],
        &[],
    );
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-3".to_string());

    let ledger = kernel.ledger_snapshot();
    let pack = kernel.context_pack_for_default(
        AgentTask::ChapterGeneration,
        &observation_in_chapter("林墨在客栈门前停住。", "Chapter-3"),
    );
    let has_result = pack
        .sources
        .iter()
        .any(|source| source.source == ContextSource::ResultFeedback);
    let has_next = ledger
        .next_beat
        .as_ref()
        .is_some_and(|beat| beat.carryovers.iter().any(|c| c.contains("追兵")));

    let mut errors = Vec::new();
    if !has_result {
        errors.push("chapter generation context missing result feedback".to_string());
    }
    if !has_next {
        errors.push("ledger next beat did not carry previous conflict".to_string());
    }

    eval_result(
        "writer_agent:scenario_result_feedback_handoff",
        format!(
            "expected=previous save feeds next chapter actual=resultSource:{} nextBeatConflict:{} evidence=ResultFeedback/NextBeat",
            has_result, has_next
        ),
        errors,
    )
}

fn run_scenario_promise_payoff_nearby_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .add_promise(
            "mystery_clue",
            "黑匣暗号",
            "黑匣里藏着能指向旧案主谋的暗号",
            "Chapter-1",
            "Chapter-5",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-5".to_string());
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨打开黑匣，却只看见一张空白纸。",
            "Chapter-5",
        ))
        .unwrap();
    let has_promise_proposal = proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::PlotPromise);

    let mut errors = Vec::new();
    if !has_promise_proposal {
        errors.push("payoff chapter did not surface promise proposal".to_string());
    }

    eval_result(
        "writer_agent:scenario_promise_payoff_nearby",
        format!(
            "expected=payoff warning near target chapter actual=proposals:{} promiseProposal:{} evidence=PromiseLedger",
            proposals.len(), has_promise_proposal
        ),
        errors,
    )
}

fn run_scenario_resolved_promise_stays_quiet_eval() -> EvalResult {
    let memory = seeded_memory();
    let promise_id = memory
        .add_promise(
            "object_whereabouts",
            "寒玉戒指",
            "寒玉戒指被张三带走",
            "Chapter-2",
            "Chapter-6",
            5,
        )
        .unwrap();
    memory.resolve_promise(promise_id, "Chapter-6").unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-6".to_string());
    let proposals = kernel
        .observe(switch_observation("林墨把寒玉戒指放回木匣。", "Chapter-6"))
        .unwrap();
    let resurfaces = proposals
        .iter()
        .any(|proposal| proposal.preview.contains("寒玉戒指"));

    let mut errors = Vec::new();
    if resurfaces {
        errors.push("resolved promise resurfaced in chapter switch proposals".to_string());
    }
    if kernel
        .ledger_snapshot()
        .open_promises
        .iter()
        .any(|p| p.title == "寒玉戒指")
    {
        errors.push("resolved promise still present in open ledger".to_string());
    }

    eval_result(
        "writer_agent:scenario_resolved_promise_quiet",
        format!(
            "expected=resolved promise stays quiet actual=proposals:{} resurfaces:{} evidence=PromiseLedger",
            proposals.len(), resurfaces
        ),
        errors,
    )
}

fn run_scenario_object_whereabouts_context_priority_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .add_promise_with_entities(
            "object_whereabouts",
            "青铜钥匙",
            "青铜钥匙由张三藏进井底，关系到密室入口",
            "Chapter-2",
            "Chapter-7",
            5,
            &["张三".to_string(), "青铜钥匙".to_string()],
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-5".to_string());
    let pack = kernel.context_pack_for_default(
        AgentTask::GhostWriting,
        &observation_in_chapter("林墨经过旧井，忽然停下脚步。", "Chapter-5"),
    );
    let promise_source = pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice);
    let includes_key = promise_source.is_some_and(|source| source.content.contains("青铜钥匙"));

    let mut errors = Vec::new();
    if !includes_key {
        errors.push("object whereabouts promise missing from ghost context".to_string());
    }

    eval_result(
        "writer_agent:scenario_object_whereabouts_context",
        format!(
            "expected=object whereabouts enters context actual=promiseSource:{} evidence=PromiseSlice",
            includes_key
        ),
        errors,
    )
}

fn run_scenario_mission_drift_save_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-3",
            "推进林墨与张三的互信关系。",
            "张三交出玉佩",
            "纯风景描写",
            "林墨决定暂时相信张三",
            "scenario",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-3".to_string());
    let proposals = kernel
        .observe(save_observation(
            "山色空蒙，雨落青瓦，整章都是纯风景描写，林墨始终没有见到张三。",
            "Chapter-3",
        ))
        .unwrap();
    let mission = kernel
        .ledger_snapshot()
        .chapter_missions
        .into_iter()
        .find(|mission| mission.chapter_title == "Chapter-3");
    let status = mission.map(|mission| mission.status).unwrap_or_default();
    let mission_debt = kernel.story_debt_snapshot().mission_count;

    let mut errors = Vec::new();
    if status != "drifted" {
        errors.push(format!("mission drift not calibrated, status={}", status));
    }
    if mission_debt == 0 && proposals.is_empty() {
        errors.push("mission drift did not produce debt or proposal".to_string());
    }

    eval_result(
        "writer_agent:scenario_mission_drift_save",
        format!(
            "expected=save detects mission drift actual=status:{} missionDebt:{} proposals:{} evidence=ChapterMission",
            status,
            mission_debt,
            proposals.len()
        ),
        errors,
    )
}

fn run_scenario_canon_conflict_no_autowrite_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.95,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨拔出长剑，挡在张三身前。",
            "Chapter-4",
        ))
        .unwrap();
    let warning = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning);
    let warning_has_operation = warning
        .as_ref()
        .is_some_and(|proposal| !proposal.operations.is_empty());
    let pending = kernel.pending_proposals().len();
    let status = kernel.status();

    let mut errors = Vec::new();
    if warning.is_none() {
        errors.push("canon conflict did not surface continuity warning".to_string());
    }
    if status.total_feedback_events != 0 {
        errors.push("canon conflict warning should not record feedback automatically".to_string());
    }
    if pending == 0 {
        errors.push("canon conflict should remain pending for author review".to_string());
    }

    eval_result(
        "writer_agent:scenario_canon_conflict_no_autowrite",
        format!(
            "expected=warn without automatic acceptance actual=warning:{} repairOps:{} pending:{} feedback:{} evidence=Canon",
            warning.is_some(),
            warning_has_operation,
            pending,
            status.total_feedback_events
        ),
        errors,
    )
}

fn run_scenario_style_feedback_affects_ghost_context_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .upsert_style_preference("accepted_Ghost", "短句、克制、少解释", true)
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let has_style = has_source(
        &kernel,
        AgentTask::GhostWriting,
        "林墨站在门口，听见张三在屋内低声咳嗽。",
        ContextSource::AuthorStyle,
    );
    let proposal = kernel
        .observe(observation_in_chapter(
            "林墨站在门口，听见张三在屋内低声咳嗽。他没有立刻推门，也没有喊人，只把手指按在刀柄上。",
            "Chapter-5",
        ))
        .unwrap()
        .into_iter()
        .find(|proposal| proposal.kind == ProposalKind::Ghost);
    let ghost_has_style_evidence = proposal.as_ref().is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::StyleLedger)
    });

    let mut errors = Vec::new();
    if !has_style {
        errors.push("author style preference missing from ghost context".to_string());
    }
    if !ghost_has_style_evidence {
        errors.push("ghost proposal missing style ledger evidence".to_string());
    }

    eval_result(
        "writer_agent:scenario_style_feedback_context",
        format!(
            "expected=accepted style informs ghost actual=contextStyle:{} ghostStyleEvidence:{} evidence=AuthorStyle",
            has_style, ghost_has_style_evidence
        ),
        errors,
    )
}

fn run_scenario_manual_ask_records_decision_eval() -> EvalResult {
    let memory = seeded_memory();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-4".to_string());
    let proposal = kernel
        .create_llm_ghost_proposal(
            observation_in_chapter(
                "林墨终于问出口：“张三，你到底把玉佩藏到了哪里？”",
                "Chapter-4",
            ),
            "张三没有回答，只把目光移向井口。".to_string(),
            "scenario-model",
        )
        .unwrap();
    kernel
        .record_operation_durable_save(
            Some(proposal.id.clone()),
            proposal.operations[0].clone(),
            "saved".to_string(),
        )
        .unwrap();
    kernel
        .apply_feedback(ProposalFeedback {
            proposal_id: proposal.id,
            action: FeedbackAction::Accepted,
            final_text: Some("张三没有回答，只把目光移向井口。".to_string()),
            reason: Some("保留悬念".to_string()),
            created_at: now_ms(),
        })
        .unwrap();
    let ledger = kernel.ledger_snapshot();
    let accepted_decision = ledger
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "accepted" && decision.scope == "Chapter-4");
    let metrics = kernel.trace_snapshot(20).product_metrics;

    let mut errors = Vec::new();
    if !accepted_decision {
        errors.push("accepted ghost did not record creative decision".to_string());
    }
    if metrics.proposal_acceptance_rate <= 0.0 {
        errors.push("product metrics did not reflect accepted proposal".to_string());
    }

    eval_result(
        "writer_agent:scenario_feedback_decision_metrics",
        format!(
            "expected=accepted feedback becomes decision+metric actual=decision:{} acceptance:{:.2} evidence=FeedbackTrace/ProductMetrics",
            accepted_decision, metrics.proposal_acceptance_rate
        ),
        errors,
    )
}

fn run_scenario_context_explainability_for_longform_eval() -> EvalResult {
    let memory = seeded_memory();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-5",
            "让林墨决定是否相信张三。",
            "张三交出关键证据",
            "提前揭露幕后主谋",
            "林墨同意与张三同行",
            "scenario",
        )
        .unwrap();
    memory
        .add_promise(
            "emotional_debt",
            "张三背叛后的道歉",
            "张三欠林墨一次真正解释",
            "Chapter-2",
            "Chapter-5",
            4,
        )
        .unwrap();
    memory
        .upsert_canon_rule(
            "林墨不能主动杀害无辜者",
            "character_boundary",
            5,
            "scenario",
        )
        .unwrap();
    record_result(
        &memory,
        "Chapter-4",
        "rev-4",
        "张三救下林墨，但仍没有解释背叛。",
        &["林墨暂时欠张三一命"],
        &["林墨开始动摇"],
        &["追兵逼近旧井"],
        &["井底有青铜钥匙"],
        &["张三的道歉仍未完成"],
        &[],
    );
    let kernel = WriterAgentKernel::new("eval", memory);
    let pack = kernel.context_pack_for_default(
        AgentTask::ChapterGeneration,
        &observation_in_chapter("林墨看着张三递来的证据。", "Chapter-5"),
    );
    let source_kinds = pack
        .sources
        .iter()
        .map(|source| format!("{:?}", source.source))
        .collect::<Vec<_>>();
    let required = [
        ContextSource::ProjectBrief,
        ContextSource::ChapterMission,
        ContextSource::ResultFeedback,
        ContextSource::PromiseSlice,
        ContextSource::CanonSlice,
    ];
    let mut errors = Vec::new();
    for expected in required {
        if !pack.sources.iter().any(|source| source.source == expected) {
            errors.push(format!("missing required longform source {:?}", expected));
        }
    }
    if pack.budget_report.source_reports.is_empty() {
        errors.push("context pack did not expose source budget report".to_string());
    }

    eval_result(
        "writer_agent:scenario_longform_context_explainability",
        format!(
            "expected=context explains why agent knows book actual=sources:{:?} budgetReports:{} evidence=ContextPack",
            source_kinds,
            pack.budget_report.source_reports.len()
        ),
        errors,
    )
}
