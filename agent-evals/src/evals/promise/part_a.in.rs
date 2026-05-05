pub fn run_promise_opportunity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三沉默片刻，终于把那枚玉佩放回桌上。",
            "Chapter-3",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let reminder = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("伏笔回收机会")
    });
    if reminder.is_none() {
        errors.push("missing promise payoff opportunity".to_string());
    }
    if !reminder.is_some_and(|proposal| {
        proposal
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::PromiseLedger)
    }) {
        errors.push("promise opportunity lacks promise ledger evidence".to_string());
    }
    if !reminder.is_some_and(|proposal| {
        proposal
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
    }) {
        errors.push("promise opportunity lacks executable resolve operation".to_string());
    }

    eval_result(
        "writer_agent:promise_payoff_opportunity",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_promise_payoff_planner_prioritizes_nearby_debts_eval() -> EvalResult {
    use agent_writer_lib::writer_agent::promise_planner::{
        plan_promise_payoffs, render_promise_payoff_plan, PromisePlannerAction,
    };

    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let nearby_id = memory
        .add_promise(
            "mystery_clue",
            "寒玉戒指来源",
            "寒玉戒指来源必须在第五章附近开始回收。",
            "Chapter-2",
            "Chapter-5",
            4,
        )
        .unwrap();
    memory
        .add_promise(
            "plot_promise",
            "远古王座真相",
            "远古王座真相是大后期线索。",
            "Chapter-1",
            "Chapter-20",
            10,
        )
        .unwrap();
    memory
        .add_promise(
            "object_whereabouts",
            "黑铁钥匙",
            "黑铁钥匙不能在本章打扰主线。",
            "Chapter-3",
            "Chapter-9",
            6,
        )
        .unwrap();
    let resolved_id = memory
        .add_promise(
            "mystery_clue",
            "已解伏笔",
            "这条伏笔已经解决，不应进入 planner。",
            "Chapter-1",
            "Chapter-5",
            9,
        )
        .unwrap();
    memory.resolve_promise(resolved_id, "Chapter-4").unwrap();
    memory
        .ensure_chapter_mission_seed(
            "eval",
            "Chapter-5",
            "林墨围绕寒玉戒指来源推进旧门线索。",
            "寒玉戒指来源必须被推进",
            "不要打扰黑铁钥匙线",
            "戒指来源得到新证据但不完全揭穿",
            "mission:chapter-5",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-5".to_string());
    let ledger = kernel.ledger_snapshot();
    let plan = plan_promise_payoffs(
        "Chapter-5",
        ledger.active_chapter_mission.as_ref(),
        &ledger.open_promises,
        "林墨在旧门前摸到寒玉戒指的裂痕。",
    );
    let rendered = render_promise_payoff_plan(&plan);

    let mut errors = Vec::new();
    if plan.is_empty() {
        errors.push("promise payoff planner returned no items".to_string());
        return eval_result(
            "writer_agent:promise_payoff_planner_prioritizes_nearby_debts",
            "no plan".to_string(),
            errors,
        );
    }
    if plan[0].promise_id != nearby_id
        || plan[0].action != PromisePlannerAction::PayoffNow
        || !plan[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("expected payoff"))
    {
        errors.push(format!(
            "nearby payoff should rank first with PayoffNow, got {:?}",
            plan.first()
        ));
    }
    if plan.iter().any(|item| item.title == "已解伏笔") {
        errors.push("resolved promise entered payoff planner".to_string());
    }
    let avoid = plan.iter().find(|item| item.title == "黑铁钥匙");
    if !avoid.is_some_and(|item| item.action == PromisePlannerAction::AvoidDisturbing) {
        errors.push(format!(
            "must_not overlapping promise should be AvoidDisturbing, got {:?}",
            avoid
        ));
    }
    let remote = plan.iter().find(|item| item.title == "远古王座真相");
    if !remote.is_some_and(|item| item.action == PromisePlannerAction::Defer) {
        errors.push(format!("remote promise should be Defer, got {:?}", remote));
    }
    if !rendered.contains("寒玉戒指来源") || !rendered.contains("PayoffNow") {
        errors.push("rendered payoff plan does not expose top action".to_string());
    }

    eval_result(
        "writer_agent:promise_payoff_planner_prioritizes_nearby_debts",
        format!(
            "top={} action={:?} items={} rendered={}",
            plan[0].title,
            plan[0].action,
            plan.len(),
            rendered.contains("PayoffNow")
        ),
        errors,
    )
}

pub fn run_promise_opportunity_apply_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三沉默片刻，终于把那枚玉佩放回桌上。",
            "Chapter-3",
        ))
        .unwrap();
    let operation = proposals
        .iter()
        .find(|proposal| {
            proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("伏笔回收机会")
        })
        .and_then(|proposal| {
            proposal
                .operations
                .iter()
                .find(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
                .cloned()
        });

    let mut errors = Vec::new();
    let Some(operation) = operation else {
        return eval_result(
            "writer_agent:promise_opportunity_apply_closes_ledger",
            format!("proposals={}", proposals.len()),
            vec!["missing resolve operation on opportunity proposal".to_string()],
        );
    };

    let result = kernel
        .approve_editor_operation_with_approval(
            operation,
            "",
            Some(&eval_approval("promise_opportunity_apply")),
        )
        .unwrap();
    if !result.success {
        errors.push(format!(
            "resolve operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open_count = kernel.ledger_snapshot().open_promises.len();
    if open_count != 0 {
        errors.push(format!(
            "promise ledger still has {} open entries",
            open_count
        ));
    }

    eval_result(
        "writer_agent:promise_opportunity_apply_closes_ledger",
        format!("success={} open={}", result.success, open_count),
        errors,
    )
}

pub fn run_promise_stale_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .add_promise(
            "mystery",
            "破庙密道",
            "破庙里有密道，需要说明用途。",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨把窗关上，屋外的雨声立刻远了。",
            "Chapter-3",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let stale = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("仍未回收")
    });
    if stale.is_none() {
        errors.push("missing stale promise warning at expected payoff chapter".to_string());
    }
    if !stale.is_some_and(|proposal| {
        matches!(
            proposal.priority,
            ProposalPriority::Normal | ProposalPriority::Urgent
        )
    }) {
        errors.push("stale promise warning priority too low".to_string());
    }
    if !stale.is_some_and(|proposal| {
        proposal
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::PromiseResolve { .. }))
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::PromiseDefer { .. }))
            && proposal
                .operations
                .iter()
                .any(|operation| matches!(operation, WriterOperation::PromiseAbandon { .. }))
    }) {
        errors.push("stale promise warning lacks resolve/defer/abandon choices".to_string());
    }

    eval_result(
        "writer_agent:stale_promise_at_payoff_chapter",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_promise_defer_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "mystery",
            "破庙密道",
            "破庙里有密道，需要说明用途。",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseDefer {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-3".to_string(),
                expected_payoff: "Chapter-5".to_string(),
            },
            "",
            Some(&eval_approval("promise_defer")),
        )
        .unwrap();

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "defer operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open = kernel.ledger_snapshot().open_promises;
    if open.len() != 1 || open[0].expected_payoff != "Chapter-5" {
        errors.push("promise payoff chapter was not updated while staying open".to_string());
    }
    if !kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "deferred_promise")
    {
        errors.push("promise defer did not record a creative decision".to_string());
    }
    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨把窗关上，屋外的雨声立刻远了。",
            "Chapter-3",
        ))
        .unwrap();
    if proposals.iter().any(|proposal| {
        proposal.kind == ProposalKind::PlotPromise && proposal.preview.contains("仍未回收")
    }) {
        errors.push("deferred promise still warns at the old payoff chapter".to_string());
    }

    eval_result(
        "writer_agent:promise_defer_updates_expected_payoff",
        format!("success={} open={}", result.success, open.len()),
        errors,
    )
}

pub fn run_promise_abandon_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "mystery",
            "破庙密道",
            "破庙里有密道，需要说明用途。",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseAbandon {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-3".to_string(),
                reason: "Author cut this thread during restructuring.".to_string(),
            },
            "",
            Some(&eval_approval("promise_abandon")),
        )
        .unwrap();

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "abandon operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open = kernel.ledger_snapshot().open_promises;
    if !open.is_empty() {
        errors.push(format!("abandoned promise still open: {}", open.len()));
    }
    if !kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| decision.decision == "abandoned_promise")
    {
        errors.push("promise abandon did not record a creative decision".to_string());
    }

    eval_result(
        "writer_agent:promise_abandon_closes_with_decision",
        format!("success={} open={}", result.success, open.len()),
        errors,
    )
}

pub fn run_promise_resolve_operation_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let promise_id = memory
        .add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落。",
            "Chapter-1",
            "Chapter-4",
            5,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let result = kernel
        .approve_editor_operation_with_approval(
            WriterOperation::PromiseResolve {
                promise_id: promise_id.to_string(),
                chapter: "Chapter-4".to_string(),
            },
            "",
            Some(&eval_approval("promise_resolve")),
        )
        .unwrap();

    let mut errors = Vec::new();
    if !result.success {
        errors.push(format!(
            "resolve operation failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let open = kernel.ledger_snapshot().open_promises;
    if !open.is_empty() {
        errors.push(format!("promise still open after resolve: {}", open.len()));
    }

    eval_result(
        "writer_agent:promise_resolve_operation_closes_ledger",
        format!("success={} open={}", result.success, open.len()),
        errors,
    )
}

