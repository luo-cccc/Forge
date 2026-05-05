fn merge_style_preference_value(existing: &str, candidate: &str) -> String {
    let existing = existing.trim();
    let candidate = candidate.trim();
    if existing.is_empty() || existing == candidate {
        return candidate.to_string();
    }
    if candidate.is_empty() {
        return existing.to_string();
    }
    if existing.contains(candidate) {
        return existing.to_string();
    }
    if candidate.contains(existing) {
        return candidate.to_string();
    }
    format!("{}；{}", existing, candidate)
}

fn initialize_schema(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(SCHEMA)?;
    migrate_writer_memory_schema(conn)?;
    conn.execute_batch(INDEX_SCHEMA)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

fn migrate_writer_memory_schema(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS writer_run_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            seq INTEGER NOT NULL,
            project_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            task_id TEXT DEFAULT '',
            event_type TEXT NOT NULL,
            source_refs_json TEXT DEFAULT '[]',
            data_json TEXT DEFAULT '{}',
            ts_ms INTEGER NOT NULL
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_feedback_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slot TEXT NOT NULL,
            category TEXT NOT NULL,
            action TEXT NOT NULL,
            confidence_delta REAL DEFAULT 0.0,
            source_error TEXT DEFAULT '',
            proposal_id TEXT DEFAULT '',
            reason TEXT DEFAULT '',
            created_at INTEGER NOT NULL
        );",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "aliases_json",
        "aliases_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(conn, "canon_entities", "summary", "summary TEXT DEFAULT ''")?;
    ensure_column(
        conn,
        "canon_entities",
        "attributes_json",
        "attributes_json TEXT DEFAULT '{}'",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "confidence",
        "confidence REAL DEFAULT 0.5",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "canon_entities", "created_at")?;
    backfill_empty_timestamp(conn, "canon_entities", "updated_at")?;

    ensure_column(
        conn,
        "canon_facts",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "confidence",
        "confidence REAL DEFAULT 0.5",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "status",
        "status TEXT DEFAULT 'active'",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "canon_facts", "created_at")?;
    backfill_empty_timestamp(conn, "canon_facts", "updated_at")?;

    ensure_column(
        conn,
        "canon_rules",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_rules",
        "status",
        "status TEXT DEFAULT 'active'",
    )?;
    ensure_column(
        conn,
        "canon_rules",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_rules",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "canon_rules", "created_at")?;
    backfill_empty_timestamp(conn, "canon_rules", "updated_at")?;

    ensure_column(
        conn,
        "plot_promises",
        "description",
        "description TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "introduced_chapter",
        "introduced_chapter TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "introduced_ref",
        "introduced_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "last_seen_chapter",
        "last_seen_chapter TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "last_seen_ref",
        "last_seen_ref TEXT DEFAULT ''",
    )?;
    conn.execute_batch(
        "UPDATE plot_promises
         SET last_seen_chapter=introduced_chapter
         WHERE last_seen_chapter IS NULL OR last_seen_chapter='';
         UPDATE plot_promises
         SET last_seen_ref=introduced_ref
         WHERE last_seen_ref IS NULL OR last_seen_ref='';",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "expected_payoff",
        "expected_payoff TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "status",
        "status TEXT DEFAULT 'open'",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "priority",
        "priority INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "plot_promises", "created_at")?;

    ensure_column(
        conn,
        "style_preferences",
        "evidence_ref",
        "evidence_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "confidence",
        "confidence REAL DEFAULT 0.5",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "accepted_count",
        "accepted_count INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "rejected_count",
        "rejected_count INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "style_preferences", "updated_at")?;

    ensure_column(conn, "creative_decisions", "scope", "scope TEXT DEFAULT ''")?;
    ensure_column(
        conn,
        "creative_decisions",
        "alternatives_json",
        "alternatives_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "creative_decisions",
        "rationale",
        "rationale TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "creative_decisions",
        "source_refs_json",
        "source_refs_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "creative_decisions",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "creative_decisions", "created_at")?;

    ensure_column(
        conn,
        "proposal_feedback",
        "final_text",
        "final_text TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "proposal_feedback",
        "reason",
        "reason TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "proposal_feedback",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "proposal_feedback", "created_at")?;

    ensure_column(
        conn,
        "writer_proposal_trace",
        "evidence_json",
        "evidence_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "writer_proposal_trace",
        "context_budget_json",
        "context_budget_json TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "writer_proposal_trace",
        "expires_at",
        "expires_at INTEGER",
    )?;
    ensure_column(
        conn,
        "writer_context_recalls",
        "snippet",
        "snippet TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "writer_context_recalls",
        "last_observation_id",
        "last_observation_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "writer_context_recalls",
        "last_proposal_id",
        "last_proposal_id TEXT DEFAULT ''",
    )?;

    ensure_column(
        conn,
        "manual_agent_turns",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "observation_id",
        "observation_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "chapter_title",
        "chapter_title TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "user_text",
        "user_text TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "assistant_text",
        "assistant_text TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "source_refs_json",
        "source_refs_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "created_at",
        "created_at INTEGER DEFAULT 0",
    )?;

    ensure_column(
        conn,
        "story_contracts",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(conn, "story_contracts", "title", "title TEXT DEFAULT ''")?;
    ensure_column(conn, "story_contracts", "genre", "genre TEXT DEFAULT ''")?;
    ensure_column(
        conn,
        "story_contracts",
        "target_reader",
        "target_reader TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "reader_promise",
        "reader_promise TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "first_30_chapter_promise",
        "first_30_chapter_promise TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "main_conflict",
        "main_conflict TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "structural_boundary",
        "structural_boundary TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "tone_contract",
        "tone_contract TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "story_contracts", "updated_at")?;

    ensure_column(
        conn,
        "chapter_missions",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "chapter_title",
        "chapter_title TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "mission",
        "mission TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "must_include",
        "must_include TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "must_not",
        "must_not TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "expected_ending",
        "expected_ending TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "status",
        "status TEXT DEFAULT 'draft'",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "chapter_missions", "created_at")?;
    backfill_empty_timestamp(conn, "chapter_missions", "updated_at")?;
    ensure_column(
        conn,
        "chapter_missions",
        "blocked_reason",
        "blocked_reason TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "retired_history",
        "retired_history TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "reader_lack_this_chapter",
        "reader_lack_this_chapter TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "relationship_soil_this_chapter",
        "relationship_soil_this_chapter TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "pressure_scene",
        "pressure_scene TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "interest_mechanism",
        "interest_mechanism TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "payoff_target",
        "payoff_target TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "payoff_path",
        "payoff_path TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "next_lack_opened",
        "next_lack_opened TEXT DEFAULT ''",
    )?;

    ensure_column(
        conn,
        "chapter_result_snapshots",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "chapter_title",
        "chapter_title TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "chapter_revision",
        "chapter_revision TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "summary",
        "summary TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "state_changes_json",
        "state_changes_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "character_progress_json",
        "character_progress_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "new_conflicts_json",
        "new_conflicts_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "new_clues_json",
        "new_clues_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "promise_updates_json",
        "promise_updates_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "canon_updates_json",
        "canon_updates_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "created_at",
        "created_at INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "risk_level",
        "risk_level TEXT DEFAULT 'medium'",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "related_entities_json",
        "related_entities_json TEXT DEFAULT '[]'",
    )?;

    ensure_column(
        conn,
        "memory_feedback_events",
        "slot",
        "slot TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "category",
        "category TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "action",
        "action TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "confidence_delta",
        "confidence_delta REAL DEFAULT 0.0",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "source_error",
        "source_error TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "proposal_id",
        "proposal_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "reason",
        "reason TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "memory_feedback_events",
        "created_at",
        "created_at INTEGER DEFAULT 0",
    )?;

    Ok(())
}
