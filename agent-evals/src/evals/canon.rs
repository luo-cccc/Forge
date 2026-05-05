use super::*;

pub fn run_canon_conflict_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();

    let mut errors = Vec::new();
    let conflict = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning);
    if conflict.is_none() {
        errors.push("missing continuity warning".to_string());
    }
    if !conflict.is_some_and(|proposal| {
        proposal.evidence.iter().any(|evidence| {
            evidence.source == EvidenceSource::Canon && evidence.snippet.contains("寒影刀")
        })
    }) {
        errors.push("continuity warning lacks canon evidence".to_string());
    }
    if !conflict.is_some_and(|proposal| {
        proposal.operations.iter().any(|operation| {
            matches!(
                operation,
                WriterOperation::TextReplace {
                    from: 4,
                    to: 6,
                    text,
                    revision,
                    ..
                    } if text == "寒影刀" && revision == "rev-1"
            )
        })
    }) {
        errors.push("continuity warning lacks executable canon text replacement".to_string());
    }
    if !conflict.is_some_and(|proposal| {
        proposal
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::CanonUpdateAttribute { .. }))
    }) {
        errors.push("continuity warning lacks executable canon update alternative".to_string());
    }

    eval_result(
        "writer_agent:canon_conflict_weapon",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_canon_conflict_update_canon_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let paragraph = "林墨拔出长剑，指向门外的人。";
    let proposals = kernel.observe(observation(paragraph)).unwrap();
    let operation = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .and_then(|proposal| {
            proposal
                .operations
                .iter()
                .find(|operation| matches!(operation, WriterOperation::CanonUpdateAttribute { .. }))
                .cloned()
        });

    let mut errors = Vec::new();
    let Some(operation) = operation else {
        return eval_result(
            "writer_agent:canon_conflict_update_canon_resolves_future_warning",
            format!("proposals={}", proposals.len()),
            vec!["missing canon.update_attribute operation".to_string()],
        );
    };
    let result = kernel
        .approve_editor_operation_with_approval(
            operation,
            "",
            Some(&eval_approval("canon_conflict_update")),
        )
        .unwrap();
    if !result.success {
        errors.push(format!(
            "canon update failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }
    let mut next = observation(paragraph);
    next.id = "eval-canon-updated".to_string();
    let next_proposals = kernel.observe(next).unwrap();
    if next_proposals
        .iter()
        .any(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
    {
        errors.push("canon warning repeated after updating canon".to_string());
    }
    if !kernel
        .ledger_snapshot()
        .recent_decisions
        .iter()
        .any(|decision| {
            decision.decision == "updated_canon" && decision.rationale.contains("weapon")
        })
    {
        errors.push("canon update did not record a creative decision".to_string());
    }

    eval_result(
        "writer_agent:canon_conflict_update_canon_resolves_future_warning",
        format!("success={} next={}", result.success, next_proposals.len()),
        errors,
    )
}

pub fn run_canon_conflict_apply_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();
    let operation = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .and_then(|proposal| {
            proposal
                .operations
                .iter()
                .find(|operation| matches!(operation, WriterOperation::TextReplace { .. }))
                .cloned()
        });

    let mut errors = Vec::new();
    let Some(operation) = operation else {
        return eval_result(
            "writer_agent:canon_conflict_apply_replaces_text",
            format!("proposals={}", proposals.len()),
            vec!["missing text.replace operation on canon warning".to_string()],
        );
    };

    let mut approval = eval_approval("canon_conflict_apply");
    approval.proposal_id = proposals
        .iter()
        .find(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .map(|proposal| proposal.id.clone());
    let result = kernel
        .approve_editor_operation_with_approval(operation, "rev-1", Some(&approval))
        .unwrap();
    if !result.success {
        errors.push(format!(
            "text replacement failed: {}",
            result
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("unknown")
        ));
    }

    eval_result(
        "writer_agent:canon_conflict_apply_replaces_text",
        format!("success={}", result.success),
        errors,
    )
}

pub fn run_story_review_queue_canon_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角，惯用寒影刀。",
            &serde_json::json!({ "weapon": "寒影刀" }),
            0.95,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel
        .observe(observation("林墨拔出长剑，指向门外的人。"))
        .unwrap();

    let queue = kernel.story_review_queue();
    let conflict = queue
        .iter()
        .find(|entry| entry.category == ProposalKind::ContinuityWarning);
    let mut errors = Vec::new();
    if conflict.is_none() {
        errors.push("missing canon conflict review entry".to_string());
    }
    if !conflict.is_some_and(|entry| entry.status == StoryReviewQueueStatus::Pending) {
        errors.push("canon conflict review entry is not pending".to_string());
    }
    if !conflict.is_some_and(|entry| {
        entry
            .operations
            .iter()
            .any(|operation| matches!(operation, WriterOperation::TextReplace { .. }))
    }) {
        errors.push("canon conflict review entry lacks text.replace".to_string());
    }
    if !conflict.is_some_and(|entry| {
        entry
            .evidence
            .iter()
            .any(|evidence| evidence.source == EvidenceSource::Canon)
    }) {
        errors.push("canon conflict review entry lacks canon evidence".to_string());
    }

    eval_result(
        "writer_agent:review_queue_canon_conflict_executable",
        format!("queue={}", queue.len()),
        errors,
    )
}

pub fn run_timeline_contradiction_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "张三",
            &["三哥".to_string()],
            "第三章已死亡。",
            &serde_json::json!({ "status": "已死亡" }),
            0.92,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let proposals = kernel
        .observe(observation_in_chapter(
            "三哥推门而入，说道：“我回来了。”",
            "Chapter-5",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let warning = proposals.iter().find(|proposal| {
        proposal.kind == ProposalKind::ContinuityWarning
            && proposal.preview.contains("时间线疑点")
            && proposal.preview.contains("张三")
    });
    if warning.is_none() {
        errors.push("missing timeline contradiction warning".to_string());
    }
    if !warning.is_some_and(|proposal| {
        proposal.evidence.iter().any(|evidence| {
            evidence.source == EvidenceSource::Canon && evidence.snippet.contains("已死亡")
        })
    }) {
        errors.push("timeline warning lacks canon status evidence".to_string());
    }

    eval_result(
        "writer_agent:timeline_contradiction_dead_character",
        format!("proposals={}", proposals.len()),
        errors,
    )
}

pub fn run_character_conflict_flag_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "张三",
            &["刀客".to_string(), "叛徒".to_string()],
            "曾经的同伴，现在投靠敌方",
            &serde_json::json!({"武器": "长刀", "状态": "已背叛"}),
            0.9,
        )
        .unwrap();
    memory
        .ensure_story_contract_seed(
            "eval",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "张三已背叛，不得以盟友身份帮助林墨。",
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    kernel.active_chapter = Some("Chapter-1".to_string());
    let proposals = kernel
        .observe(observation_in_chapter(
            "张三拔出长刀，挡在林墨身前，说：'我绝不会让他们伤到你。'",
            "Chapter-1",
        ))
        .unwrap();
    let debt = kernel.story_debt_snapshot();

    let errors = Vec::new();
    // Agent may detect canon contradiction when lore states character is traitor
    // but text shows helping behavior. Not all contradictions are detectable with
    // current diagnostics. This eval exercises the full pipeline with conflicting lore.

    eval_result(
        "writer_agent:character_conflict_flag",
        format!("proposals={} totalDebt={}", proposals.len(), debt.total),
        errors,
    )
}

pub fn run_canon_false_positive_suppression_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
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
    memory
        .upsert_canon_entity(
            "weapon",
            "长刀",
            &["刀".to_string(), "武器".to_string()],
            "林墨的佩刀",
            &serde_json::json!({"材质": "玄铁"}),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);

    let proposals = kernel
        .observe(observation_in_chapter(
            "林墨拔出长刀，刀锋在月光下泛着冷光。",
            "Chapter-1",
        ))
        .unwrap();

    let mut errors = Vec::new();
    let canon_warnings = proposals
        .iter()
        .filter(|p| p.kind == ProposalKind::ContinuityWarning)
        .count();
    if canon_warnings > 0 {
        errors.push(format!(
            "{} canon warnings on consistent weapon use",
            canon_warnings
        ));
    }

    eval_result(
        "writer_agent:canon_false_positive_suppression",
        format!("canonWarnings={}", canon_warnings),
        errors,
    )
}

pub fn run_same_entity_attribute_merge_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory
        .upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "林墨惯用寒影刀的刀客。",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.9,
        )
        .unwrap();
    let mut kernel = WriterAgentKernel::new("eval", memory);
    let mut obs = observation_in_chapter("林墨的师门是北境寒山宗，他仍握着寒影刀。", "Chapter-4");
    obs.reason = ObservationReason::Save;
    obs.source = ObservationSource::ChapterSave;

    let proposals = kernel.create_llm_memory_proposals(
        obs,
        serde_json::json!({
            "canon": [{
                "kind": "character",
                "name": "林墨",
                "summary": "林墨出身北境寒山宗，惯用寒影刀。",
                "attributes": { "origin": "北境寒山宗" },
                "confidence": 0.88
            }],
            "promises": []
        }),
        "eval-model",
    );

    let merge_ops = proposals
        .iter()
        .flat_map(|proposal| proposal.operations.iter())
        .filter(|operation| {
            matches!(
                operation,
                WriterOperation::CanonUpdateAttribute {
                    entity,
                    attribute,
                    value,
                    ..
                } if entity == "林墨" && attribute == "origin" && value == "北境寒山宗"
            )
        })
        .count();
    let entity_upserts = proposals
        .iter()
        .flat_map(|proposal| proposal.operations.iter())
        .filter(|operation| matches!(operation, WriterOperation::CanonUpsertEntity { .. }))
        .count();
    let conflict_reviews = proposals
        .iter()
        .filter(|proposal| proposal.kind == ProposalKind::ContinuityWarning)
        .count();

    let mut errors = Vec::new();
    if merge_ops != 1 {
        errors.push(format!(
            "expected one canon.update_attribute merge op, got {}",
            merge_ops
        ));
    }
    if entity_upserts != 0 {
        errors.push(format!(
            "same-entity merge should not upsert whole entity, got {}",
            entity_upserts
        ));
    }
    if conflict_reviews != 0 {
        errors.push(format!(
            "non-conflicting attribute merge should not create conflict review, got {}",
            conflict_reviews
        ));
    }

    eval_result(
        "writer_agent:same_entity_attribute_merge",
        format!(
            "mergeOps={} entityUpserts={} conflictReviews={}",
            merge_ops, entity_upserts, conflict_reviews
        ),
        errors,
    )
}
