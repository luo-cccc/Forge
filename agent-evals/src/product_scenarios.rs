use crate::fixtures::*;
use agent_writer_lib::writer_agent::context::{AgentTask, ContextSource};
use agent_writer_lib::writer_agent::feedback::{FeedbackAction, ProposalFeedback};
use agent_writer_lib::writer_agent::memory::{
    ChapterMissionSummary, ChapterResultSummary, WriterMemory,
};
use agent_writer_lib::writer_agent::observation::{ObservationReason, ObservationSource};
use agent_writer_lib::writer_agent::proposal::{EvidenceSource, ProposalKind};
use agent_writer_lib::writer_agent::trajectory::export_trace_snapshot;
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
        run_continuous_writing_fixture_20_chapters_eval(),
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

fn run_continuous_writing_fixture_20_chapters_eval() -> EvalResult {
    let db_path = std::env::temp_dir().join(format!(
        "forge-continuous-writing-20-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let mut errors = Vec::new();
    let session_a_metrics;
    let session_b_metrics;
    let session_b_debt;
    let session_b_context_sources;
    let latest_result_count;
    let mission_completed;
    let mission_drifted;
    let promise_context_hit;
    let promise_recall_hit;
    let trace_has_product_trend;
    let trace_has_context_recall;
    let trajectory_has_save;

    {
        let memory = WriterMemory::open(&db_path).unwrap();
        seed_continuous_writing_memory(&memory);
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "continuous-20-session-a".to_string();
        simulate_continuous_writing_session(&mut kernel, 1, 10, 30);
        session_a_metrics = kernel.trace_snapshot(120).product_metrics;
    }

    {
        let memory = WriterMemory::open(&db_path).unwrap();
        let mut kernel = WriterAgentKernel::new("eval", memory);
        kernel.session_id = "continuous-20-session-b".to_string();
        simulate_continuous_writing_session(&mut kernel, 11, 20, 140);

        let obs = observation_in_chapter(
            "林墨把霜铃塔钥按进潮汐祭账的暗格，决定先保住张三，而不是揭开白昼王座的来源。",
            "Chapter-20",
        );
        let pack = kernel.context_pack_for_default(AgentTask::PlanningReview, &obs);
        session_b_context_sources = pack
            .sources
            .iter()
            .map(|source| format!("{:?}", source.source))
            .collect::<Vec<_>>();
        promise_context_hit = pack.sources.iter().any(|source| {
            source.source == ContextSource::PromiseSlice && source.content.contains("霜铃塔钥")
        });

        let ledger = kernel.ledger_snapshot();
        latest_result_count = ledger.recent_chapter_results.len();
        mission_completed = ledger
            .chapter_missions
            .iter()
            .filter(|mission| mission.status == "completed")
            .count();
        mission_drifted = ledger
            .chapter_missions
            .iter()
            .filter(|mission| mission.status == "drifted")
            .count();

        let snapshot = kernel.trace_snapshot(200);
        session_b_metrics = snapshot.product_metrics.clone();
        session_b_debt = kernel.story_debt_snapshot();
        promise_recall_hit = snapshot
            .context_recalls
            .iter()
            .any(|recall| recall.source == "PromiseLedger" && recall.snippet.contains("霜铃塔钥"));
        trace_has_product_trend = snapshot.product_metrics_trend.session_count >= 2
            && snapshot.product_metrics_trend.recent_sessions.len() >= 2
            && snapshot
                .product_metrics_trend
                .overall_average_save_to_feedback_ms
                .is_some()
            && snapshot
                .product_metrics_trend
                .save_to_feedback_delta_ms
                .is_some();
        trace_has_context_recall = !snapshot.context_recalls.is_empty();
        let export = export_trace_snapshot("eval", &kernel.session_id, &snapshot);
        trajectory_has_save = export
            .jsonl
            .contains("\"eventType\":\"writer.save_completed\"")
            && export
                .jsonl
                .contains("\"eventType\":\"writer.product_metrics_trend\"")
            && export
                .jsonl
                .contains("\"eventType\":\"writer.context_recall\"");
    }

    let _ = std::fs::remove_file(&db_path);

    if latest_result_count < 20 {
        errors.push(format!(
            "continuous fixture recorded only {} chapter results",
            latest_result_count
        ));
    }
    if mission_completed < 6 {
        errors.push(format!(
            "continuous fixture completed too few missions: {}",
            mission_completed
        ));
    }
    if mission_drifted == 0 {
        errors.push("continuous fixture did not preserve a mission drift case".to_string());
    }
    if session_b_debt.promise_count == 0 || session_b_debt.mission_count == 0 {
        errors.push(format!(
            "story debt did not include both promise and mission debt: promise={} mission={}",
            session_b_debt.promise_count, session_b_debt.mission_count
        ));
    }
    if !promise_context_hit {
        errors.push("planning context did not recall the long-running key promise".to_string());
    }
    if !promise_recall_hit {
        errors.push("context recall ledger did not record PromiseLedger evidence".to_string());
    }
    if session_a_metrics.proposal_count < 2 || session_b_metrics.proposal_count < 2 {
        errors.push(format!(
            "sessions produced too few proposals: a={} b={}",
            session_a_metrics.proposal_count, session_b_metrics.proposal_count
        ));
    }
    if session_b_metrics.feedback_count < 2
        || session_b_metrics.average_save_to_feedback_ms.is_none()
    {
        errors.push(format!(
            "session-b feedback/save metrics incomplete: feedback={} latency={:?}",
            session_b_metrics.feedback_count, session_b_metrics.average_save_to_feedback_ms
        ));
    }
    if session_b_metrics.promise_recall_hit_rate <= 0.0 {
        errors.push("promise recall hit rate did not move above zero".to_string());
    }
    if !trace_has_product_trend {
        errors.push("product metrics trend did not prove multi-session replay".to_string());
    }
    if !trace_has_context_recall {
        errors.push("trace snapshot lacked context recalls".to_string());
    }
    if !trajectory_has_save {
        errors.push("trajectory did not export save/metrics/context events".to_string());
    }

    eval_result(
        "writer_agent:continuous_writing_fixture_20_chapters",
        format!(
            "expected=20-chapter continuous product fixture actual=results:{} completed:{} drifted:{} debt:{} promiseDebt:{} missionDebt:{} sources:{:?} aFeedback:{} bFeedback:{} bLatency:{:?}",
            latest_result_count,
            mission_completed,
            mission_drifted,
            session_b_debt.total,
            session_b_debt.promise_count,
            session_b_debt.mission_count,
            session_b_context_sources,
            session_a_metrics.feedback_count,
            session_b_metrics.feedback_count,
            session_b_metrics.average_save_to_feedback_ms
        ),
        errors,
    )
}

fn seed_continuous_writing_memory(memory: &WriterMemory) {
    memory
        .ensure_story_contract_seed(
            "eval",
            "霜塔旧账",
            "长篇玄幻悬疑",
            "林墨在二十章内追查霜铃塔钥与潮汐祭账的因果，同时保护张三的灰色忠诚。",
            "林墨必须在守护张三与揭开白昼王座之间持续付出代价。",
            "不得提前揭示白昼王座真正来源；不得把霜铃塔钥当作普通钥匙处理。",
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &["寒影刀客".to_string()],
            "主角，惯用寒影刀，追查霜铃塔钥与潮汐祭账。",
            &serde_json::json!({"weapon": "寒影刀", "loyalty": "protects Zhang San"}),
            0.92,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "张三",
            &["账房".to_string()],
            "张三保管过潮汐祭账，动机可疑但多次保护林墨。",
            &serde_json::json!({"holdsLedger": true, "trust": "unstable"}),
            0.88,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "霜铃塔钥",
            &["塔钥".to_string()],
            "开启霜铃塔旧门的钥匙，与潮汐祭账缺页互相印证。",
            &serde_json::json!({"location": "with Lin Mo", "risk": "reveals old debt"}),
            0.9,
        )
        .unwrap();
    memory
        .upsert_canon_entity(
            "object",
            "潮汐祭账",
            &["祭账".to_string()],
            "记载白昼王座旧债的账册，缺页仍未找回。",
            &serde_json::json!({"missingPage": true}),
            0.87,
        )
        .unwrap();
    memory
        .upsert_canon_rule(
            "白昼王座的真正来源必须延后到二十章后再揭示。",
            "mystery_boundary",
            8,
            "fixture",
        )
        .unwrap();
    memory
        .upsert_style_preference("accepted_Ghost", "短句、克制、少解释", true)
        .unwrap();
    memory
        .add_promise_with_entities(
            "object_whereabouts",
            "霜铃塔钥",
            "霜铃塔钥能打开旧门，但不能提前解释白昼王座来源",
            "Chapter-1",
            "Chapter-20",
            8,
            &["霜铃塔钥".to_string(), "白昼王座".to_string()],
        )
        .unwrap();
    memory
        .add_promise_with_entities(
            "mystery_clue",
            "潮汐祭账缺页",
            "潮汐祭账缺页记录张三背叛的真实原因",
            "Chapter-3",
            "Chapter-18",
            7,
            &["潮汐祭账".to_string(), "张三".to_string()],
        )
        .unwrap();
    memory
        .add_promise(
            "emotional_debt",
            "张三的真正道歉",
            "张三欠林墨一次不带借口的道歉",
            "Chapter-5",
            "Chapter-16",
            6,
        )
        .unwrap();

    for chapter in 1..=20 {
        let mission = continuous_chapter_mission(chapter);
        memory.upsert_chapter_mission(&mission).unwrap();
    }
}

fn continuous_chapter_mission(chapter: usize) -> ChapterMissionSummary {
    ChapterMissionSummary {
        id: 0,
        project_id: "eval".to_string(),
        chapter_title: format!("Chapter-{}", chapter),
        mission: format!(
            "推进林墨与张三围绕霜铃塔钥、潮汐祭账和第{}章压力的选择。",
            chapter
        ),
        must_include: continuous_must_include(chapter),
        must_not: if chapter >= 15 {
            "提前揭示白昼王座来源".to_string()
        } else {
            "跳过霜铃塔钥因果".to_string()
        },
        expected_ending: continuous_expected_ending(chapter),
        status: "active".to_string(),
        source_ref: format!("fixture:chapter-mission:{}", chapter),
        updated_at: String::new(),
    }
}

fn continuous_must_include(chapter: usize) -> String {
    match chapter {
        1 | 2 => "霜铃塔钥".to_string(),
        3 | 4 => "潮汐祭账".to_string(),
        5 | 6 => "张三".to_string(),
        7 | 8 => "旧门".to_string(),
        9 | 10 => "寒影刀".to_string(),
        11 | 12 => "缺页".to_string(),
        13 | 14 => "信任".to_string(),
        15 | 16 => "道歉".to_string(),
        17 | 18 => "祭账".to_string(),
        _ => "霜铃塔钥".to_string(),
    }
}

fn continuous_expected_ending(chapter: usize) -> String {
    continuous_must_include(chapter)
}

fn simulate_continuous_writing_session(
    kernel: &mut WriterAgentKernel,
    start: usize,
    end: usize,
    feedback_delay_ms: u64,
) {
    for chapter in start..=end {
        kernel.active_chapter = Some(format!("Chapter-{}", chapter));
        let paragraph = continuous_chapter_text(chapter);
        kernel
            .observe(save_observation(
                &paragraph,
                &format!("Chapter-{}", chapter),
            ))
            .unwrap();

        if matches!(chapter, 4 | 9 | 13 | 17 | 20) {
            let observation = observation_in_chapter(&paragraph, &format!("Chapter-{}", chapter));
            let continuation = format!(
                "林墨把第{}章的选择压低成一句话：先保住人，再追问债。",
                chapter
            );
            let proposal = kernel
                .create_llm_ghost_proposal(observation, continuation.clone(), "fixture-model")
                .unwrap();
            let operation = proposal.operations[0].clone();
            kernel
                .record_operation_durable_save(
                    Some(proposal.id.clone()),
                    operation,
                    format!("editor_save:chapter-{}", chapter),
                )
                .unwrap();
            kernel
                .apply_feedback(ProposalFeedback {
                    proposal_id: proposal.id.clone(),
                    action: if matches!(chapter, 13) {
                        FeedbackAction::Edited
                    } else {
                        FeedbackAction::Accepted
                    },
                    final_text: Some(continuation),
                    reason: Some(format!("continuous fixture chapter {}", chapter)),
                    created_at: now_ms() + feedback_delay_ms + chapter as u64,
                })
                .unwrap();
        } else if matches!(chapter, 6 | 15) {
            let observation = observation_in_chapter(&paragraph, &format!("Chapter-{}", chapter));
            let proposal = kernel
                .create_llm_ghost_proposal(
                    observation,
                    "张三忽然解释了一切，白昼王座来源也被说破。".to_string(),
                    "fixture-model",
                )
                .unwrap();
            kernel
                .apply_feedback(ProposalFeedback {
                    proposal_id: proposal.id.clone(),
                    action: FeedbackAction::Rejected,
                    final_text: None,
                    reason: Some("作者拒绝过早揭示核心谜底".to_string()),
                    created_at: now_ms() + feedback_delay_ms + chapter as u64,
                })
                .unwrap();
        }
    }
}

fn continuous_chapter_text(chapter: usize) -> String {
    match chapter {
        1 => "林墨发现霜铃塔钥在旧井边发冷，决定把它藏进袖中。线索没有解释来源，只留下新的疑问。".to_string(),
        2 => "林墨握着霜铃塔钥听见塔内铃声，发现钥齿和旧门铜痕相合，选择暂不告诉张三。".to_string(),
        3 => "张三交出潮汐祭账，林墨发现账册缺页，怀疑有人删去了白昼王座的旧债。".to_string(),
        4 => "潮汐祭账被雨水打湿，林墨确认缺页边缘有霜铃塔印，新的敌人开始追查账册。".to_string(),
        5 => "张三挡下追兵，林墨决定暂时相信他，却仍把霜铃塔钥握在掌心。".to_string(),
        6 => "张三拒绝说明背叛原因，林墨发现他的伤口来自旧门机关，信任仍被怀疑撕扯。".to_string(),
        7 => "林墨带张三返回旧门，霜铃塔钥只转动半圈，门后传来账册缺页被焚的气味。".to_string(),
        8 => "旧门忽然合拢，林墨选择救张三而不是追门后黑影，新的危险压住了钥匙线索。".to_string(),
        9 => "林墨拔出寒影刀，发现刀身映出潮汐祭账缺页的编号，敌人杀意逼近。".to_string(),
        10 => "寒影刀斩断锁链，林墨确认旧门机关会吞掉持钥者的记忆，新的选择变得更重。".to_string(),
        11 => "林墨在废庙发现缺页拓印，潮汐祭账的空白处只留下张三的旧名。".to_string(),
        12 => "缺页拓印被敌人夺走，林墨发现张三曾试图保护孩子，危机转向城南码头。".to_string(),
        13 => "林墨没有直接追问张三，而是把信任押在他递来的半页祭账上。".to_string(),
        14 => "张三承认自己曾背叛林墨，林墨选择继续同行，信任变成有条件的交换。".to_string(),
        15 => "白昼王座来源忽然被说破，整章只写山色、雨声和远处灯火，林墨没有见到张三，也没有处理道歉。".to_string(),
        16 => "张三终于低头道歉，林墨没有原谅，却决定让他活到潮汐祭账真相出现。".to_string(),
        17 => "祭账缺页在塔底重现，林墨发现霜铃塔钥能换来一次延后揭示的机会。".to_string(),
        18 => "林墨用祭账缺页逼退敌人，仍没有说破白昼王座来源，只把债推到更深处。".to_string(),
        19 => "林墨把霜铃塔钥交还张三保管，决定先保护活人，再追问白昼王座。".to_string(),
        20 => "霜铃塔钥插进暗格，潮汐祭账展开新页，林墨发现这不是结局，只是更大的债。".to_string(),
        _ => "林墨继续推进线索。".to_string(),
    }
}
