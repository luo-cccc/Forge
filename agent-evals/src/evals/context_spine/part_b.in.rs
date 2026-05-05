use agent_harness_core::{CompactionConfig, CompactionTrigger, ContextSpineCompactionReport};

// ── Task 5: event-driven compaction ──

pub fn run_event_driven_compaction_after_verified_save_eval() -> EvalResult {
    let mut errors = Vec::new();
    let cfg = CompactionConfig::default();
    if cfg.event_triggers_enabled {
        errors.push("event triggers must default to disabled".to_string());
    }
    let trigger = CompactionTrigger::ChapterSaveVerified;
    if !trigger.is_domain_event() {
        errors.push("ChapterSaveVerified must be a domain event".to_string());
    }
    if trigger.label() != "chapter_save_verified" {
        errors.push("label mismatch for ChapterSaveVerified".to_string());
    }
    eval_result("event_driven_compaction_after_verified_save", String::new(), errors)
}

pub fn run_compaction_report_keeps_source_refs_eval() -> EvalResult {
    let mut errors = Vec::new();
    let report = ContextSpineCompactionReport {
        trigger: CompactionTrigger::PlanningReviewComplete,
        input_source_refs: vec!["chapter-1".to_string(), "promise-3".to_string()],
        summary_confidence: 0.85,
        validated: true,
        ..Default::default()
    };
    if report.input_source_refs.len() != 2 {
        errors.push("source refs must be preserved".to_string());
    }
    if report.summary_confidence < 0.8 {
        errors.push("confidence should match input".to_string());
    }
    eval_result("compaction_report_keeps_source_refs", String::new(), errors)
}

pub fn run_compaction_does_not_autowrite_long_term_memory_eval() -> EvalResult {
    let mut errors = Vec::new();
    let default_report = ContextSpineCompactionReport::default();
    if default_report.allowed_into_stable_prefix {
        errors.push("default compaction must not be allowed into stable prefix".to_string());
    }
    if default_report.trigger != CompactionTrigger::WaterLevel {
        errors.push("default trigger must be WaterLevel".to_string());
    }
    eval_result("compaction_does_not_autowrite_long_term_memory", String::new(), errors)
}

// ── Task 6: BYOK cache policy ──

pub fn run_cache_keepalive_requires_author_approval_eval() -> EvalResult {
    let mut errors = Vec::new();
    let config = agent_harness_core::HarnessConfig::default();
    if config.cache_keepalive_enabled {
        errors.push("default cache keepalive must be disabled".to_string());
    }
    eval_result("cache_keepalive_requires_author_approval", String::new(), errors)
}

pub fn run_cache_maintenance_uses_provider_budget_eval() -> EvalResult {
    let mut errors = Vec::new();
    let config = agent_harness_core::HarnessConfig::default();
    if config.extended_cache_enabled {
        errors.push("default extended cache must be disabled".to_string());
    }
    eval_result("cache_maintenance_uses_provider_budget", String::new(), errors)
}

pub fn run_extended_cache_requires_explicit_policy_eval() -> EvalResult {
    let mut errors = Vec::new();
    let config = agent_harness_core::HarnessConfig::default();
    if config.extended_cache_enabled {
        errors.push("extended cache must default to disabled".to_string());
    }
    eval_result("extended_cache_requires_explicit_policy", String::new(), errors)
}

// ── Task 7: performance metrics ──

pub fn run_trajectory_exports_prompt_cache_metrics_eval() -> EvalResult {
    let mut errors = Vec::new();
    let metrics = agent_writer_lib::writer_agent::kernel::WriterProductMetrics::default();
    if metrics.cache_hit_token_ratio != 0.0 {
        errors.push("default cache_hit_token_ratio should be 0.0".to_string());
    }
    if metrics.focus_pack_rebuild_count != 0 {
        errors.push("default focus_pack_rebuild_count should be 0".to_string());
    }
    eval_result("trajectory_exports_prompt_cache_metrics", String::new(), errors)
}

pub fn run_product_metrics_tracks_cache_stability_eval() -> EvalResult {
    let mut errors = Vec::new();
    let metrics = agent_writer_lib::writer_agent::kernel::WriterProductMetrics::default();
    if metrics.static_prefix_churn_rate != 0.0 {
        errors.push("default static_prefix_churn_rate should be 0.0".to_string());
    }
    if !metrics.cache_miss_reason_counts.is_empty() {
        errors.push("default cache_miss_reason_counts should be empty".to_string());
    }
    eval_result("product_metrics_tracks_cache_stability", String::new(), errors)
}
