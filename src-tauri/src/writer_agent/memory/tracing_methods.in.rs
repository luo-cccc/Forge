impl WriterMemory {
    pub fn record_observation_trace(
        &self,
        id: &str,
        created_at: u64,
        reason: &str,
        chapter_title: Option<&str>,
        paragraph_snippet: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_observation_trace
             (observation_id, created_at, reason, chapter_title, paragraph_snippet)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id,
                created_at as i64,
                reason,
                chapter_title.unwrap_or(""),
                paragraph_snippet,
            ],
        )?;
        Ok(())
    }

    pub fn record_proposal_trace(
        &self,
        proposal: &ProposalTraceSummary,
        created_at: u64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_proposal_trace
             (proposal_id, observation_id, kind, priority, state, confidence, preview_snippet, evidence_json, context_budget_json, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(proposal_id) DO UPDATE SET
                observation_id=excluded.observation_id,
                kind=excluded.kind,
                priority=excluded.priority,
                state=excluded.state,
                confidence=excluded.confidence,
                preview_snippet=excluded.preview_snippet,
                evidence_json=excluded.evidence_json,
                context_budget_json=excluded.context_budget_json,
                created_at=excluded.created_at,
                expires_at=excluded.expires_at",
            rusqlite::params![
                proposal.id,
                proposal.observation_id,
                proposal.kind,
                proposal.priority,
                proposal.state,
                proposal.confidence,
                proposal.preview_snippet,
                serde_json::to_string(&proposal.evidence).unwrap_or_else(|_| "[]".to_string()),
                proposal
                    .context_budget
                    .as_ref()
                    .and_then(|budget| serde_json::to_string(budget).ok())
                    .unwrap_or_default(),
                created_at as i64,
                proposal.expires_at.map(|value| value as i64),
            ],
        )?;
        Ok(())
    }

    pub fn update_proposal_trace_state(
        &self,
        proposal_id: &str,
        state: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE writer_proposal_trace SET state=?1 WHERE proposal_id=?2",
            rusqlite::params![state, proposal_id],
        )?;
        Ok(())
    }

    pub fn record_feedback_trace(&self, feedback: &FeedbackTraceSummary) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_feedback_trace (proposal_id, action, reason, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                feedback.proposal_id,
                feedback.action,
                feedback.reason.clone().unwrap_or_default(),
                feedback.created_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn record_run_event(&self, event: &RunEventSummary) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_run_events
             (seq, project_id, session_id, task_id, event_type, source_refs_json, data_json, ts_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                event.seq as i64,
                event.project_id,
                event.session_id,
                event.task_id.clone().unwrap_or_default(),
                event.event_type,
                string_vec_json(&event.source_refs),
                serde_json::to_string(&event.data).unwrap_or_else(|_| "{}".to_string()),
                event.ts_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn record_context_recalls(
        &self,
        project_id: &str,
        proposal_id: &str,
        observation_id: &str,
        evidence: &[super::proposal::EvidenceRef],
        recalled_at: u64,
    ) -> rusqlite::Result<()> {
        for evidence in evidence
            .iter()
            .filter(|entry| !entry.reference.trim().is_empty() || !entry.snippet.trim().is_empty())
        {
            let source = format!("{:?}", evidence.source);
            let reference = if evidence.reference.trim().is_empty() {
                snippet_for_storage(&evidence.snippet, 80)
            } else {
                evidence.reference.trim().to_string()
            };
            let snippet = snippet_for_storage(&evidence.snippet, 240);
            self.conn.execute(
                "INSERT INTO writer_context_recalls
                 (project_id, source, reference, snippet, recall_count, first_recalled_at, last_recalled_at, last_observation_id, last_proposal_id)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5, ?6, ?7)
                 ON CONFLICT(project_id, source, reference) DO UPDATE SET
                    snippet=excluded.snippet,
                    recall_count=writer_context_recalls.recall_count + 1,
                    last_recalled_at=excluded.last_recalled_at,
                    last_observation_id=excluded.last_observation_id,
                    last_proposal_id=excluded.last_proposal_id",
                rusqlite::params![
                    project_id,
                    source,
                    reference,
                    snippet,
                    recalled_at as i64,
                    observation_id,
                    proposal_id,
                ],
            )?;
        }
        Ok(())
    }

    pub fn list_context_recalls(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ContextRecallSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, reference, snippet, recall_count, first_recalled_at, last_recalled_at, last_observation_id, last_proposal_id
             FROM writer_context_recalls
             WHERE project_id=?1
             ORDER BY recall_count DESC, last_recalled_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            let recall_count: i64 = row.get(3)?;
            let first_recalled_at: i64 = row.get(4)?;
            let last_recalled_at: i64 = row.get(5)?;
            Ok(ContextRecallSummary {
                source: row.get(0)?,
                reference: row.get(1)?,
                snippet: row.get(2)?,
                recall_count: recall_count.max(0) as u64,
                first_recalled_at: first_recalled_at.max(0) as u64,
                last_recalled_at: last_recalled_at.max(0) as u64,
                last_observation_id: row.get(6)?,
                last_proposal_id: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_observation_traces(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<ObservationTraceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT observation_id, created_at, reason, chapter_title, paragraph_snippet
             FROM writer_observation_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let chapter_title: String = row.get(3)?;
            let created_at: i64 = row.get(1)?;
            Ok(ObservationTraceSummary {
                id: row.get(0)?,
                created_at: created_at.max(0) as u64,
                reason: row.get(2)?,
                chapter_title: if chapter_title.trim().is_empty() {
                    None
                } else {
                    Some(chapter_title)
                },
                paragraph_snippet: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_proposal_traces(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<ProposalTraceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT proposal_id, observation_id, kind, priority, state, confidence, preview_snippet, evidence_json, context_budget_json, expires_at
             FROM writer_proposal_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let evidence_json: String = row.get(7)?;
            let context_budget_json: String = row.get(8)?;
            let context_budget = if context_budget_json.trim().is_empty() {
                None
            } else {
                serde_json::from_str::<ContextBudgetTrace>(&context_budget_json).ok()
            };
            let expires_at: Option<i64> = row.get(9)?;
            Ok(ProposalTraceSummary {
                id: row.get(0)?,
                observation_id: row.get(1)?,
                kind: row.get(2)?,
                priority: row.get(3)?,
                state: row.get(4)?,
                confidence: row.get(5)?,
                preview_snippet: row.get(6)?,
                evidence: serde_json::from_str::<Vec<super::proposal::EvidenceRef>>(&evidence_json)
                    .unwrap_or_default(),
                context_budget,
                expires_at: expires_at.map(|value| value.max(0) as u64),
            })
        })?;
        rows.collect()
    }

    pub fn list_feedback_traces(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<FeedbackTraceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT proposal_id, action, reason, created_at
             FROM writer_feedback_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let reason: String = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            Ok(FeedbackTraceSummary {
                proposal_id: row.get(0)?,
                action: row.get(1)?,
                reason: if reason.trim().is_empty() {
                    None
                } else {
                    Some(reason)
                },
                created_at: created_at.max(0) as u64,
            })
        })?;
        rows.collect()
    }

    pub fn list_run_events(
        &self,
        project_id: &str,
        session_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<RunEventSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, project_id, session_id, task_id, event_type, source_refs_json, data_json, ts_ms
             FROM writer_run_events
             WHERE project_id=?1 AND session_id=?2
             ORDER BY seq DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![project_id, session_id, limit as i64],
            |row| {
                let seq: i64 = row.get(0)?;
                let task_id: String = row.get(3)?;
                let data_json: String = row.get(6)?;
                let ts_ms: i64 = row.get(7)?;
                Ok(RunEventSummary {
                    seq: seq.max(0) as u64,
                    project_id: row.get(1)?,
                    session_id: row.get(2)?,
                    task_id: if task_id.trim().is_empty() {
                        None
                    } else {
                        Some(task_id)
                    },
                    event_type: row.get(4)?,
                    source_refs: string_vec_from_json(row.get::<_, String>(5)?.as_str()),
                    data: serde_json::from_str(&data_json)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    ts_ms: ts_ms.max(0) as u64,
                })
            },
        )?;
        let mut events = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        events.reverse();
        Ok(events)
    }

    pub fn list_project_run_events(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<RunEventSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, project_id, session_id, task_id, event_type, source_refs_json, data_json, ts_ms
             FROM writer_run_events
             WHERE project_id=?1
             ORDER BY ts_ms DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            let seq: i64 = row.get(0)?;
            let task_id: String = row.get(3)?;
            let data_json: String = row.get(6)?;
            let ts_ms: i64 = row.get(7)?;
            Ok(RunEventSummary {
                seq: seq.max(0) as u64,
                project_id: row.get(1)?,
                session_id: row.get(2)?,
                task_id: if task_id.trim().is_empty() {
                    None
                } else {
                    Some(task_id)
                },
                event_type: row.get(4)?,
                source_refs: string_vec_from_json(row.get::<_, String>(5)?.as_str()),
                data: serde_json::from_str(&data_json).unwrap_or_else(|_| serde_json::json!({})),
                ts_ms: ts_ms.max(0) as u64,
            })
        })?;
        let mut events = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        events.reverse();
        Ok(events)
    }

    #[cfg(test)]
    pub fn feedback_stats(&self, proposal_id: &str) -> rusqlite::Result<(i64, i64)> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*) FILTER(WHERE action='accepted'), COUNT(*) FILTER(WHERE action='rejected')
             FROM proposal_feedback WHERE proposal_id=?1"
        )?;
        let (accepted, rejected) = stmt.query_row(rusqlite::params![proposal_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok((accepted, rejected))
    }
}

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

fn chapter_mission_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChapterMissionSummary> {
    let status: String = row.get(7)?;
    Ok(ChapterMissionSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_title: row.get(2)?,
        mission: row.get(3)?,
        must_include: row.get(4)?,
        must_not: row.get(5)?,
        expected_ending: row.get(6)?,
        status: crate::writer_agent::kernel::normalize_chapter_mission_status(&status),
        source_ref: row.get(8)?,
        updated_at: row.get(9)?,
        blocked_reason: row.get::<_, String>(10).unwrap_or_default(),
        retired_history: row.get::<_, String>(11).unwrap_or_default(),
    })
}

fn chapter_result_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChapterResultSummary> {
    let created_at: i64 = row.get(12)?;
    Ok(ChapterResultSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_title: row.get(2)?,
        chapter_revision: row.get(3)?,
        summary: row.get(4)?,
        state_changes: string_vec_from_json(row.get::<_, String>(5)?.as_str()),
        character_progress: string_vec_from_json(row.get::<_, String>(6)?.as_str()),
        new_conflicts: string_vec_from_json(row.get::<_, String>(7)?.as_str()),
        new_clues: string_vec_from_json(row.get::<_, String>(8)?.as_str()),
        promise_updates: string_vec_from_json(row.get::<_, String>(9)?.as_str()),
        canon_updates: string_vec_from_json(row.get::<_, String>(10)?.as_str()),
        source_ref: row.get(11)?,
        created_at: created_at.max(0) as u64,
    })
}

fn memory_feedback_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryFeedbackSummary> {
    let source_error: String = row.get(4)?;
    let reason: String = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    Ok(MemoryFeedbackSummary {
        slot: row.get(0)?,
        category: row.get(1)?,
        action: row.get(2)?,
        confidence_delta: row.get(3)?,
        source_error: if source_error.trim().is_empty() {
            None
        } else {
            Some(source_error)
        },
        proposal_id: row.get(5)?,
        reason: if reason.trim().is_empty() {
            None
        } else {
            Some(reason)
        },
        created_at: created_at.max(0) as u64,
    })
}

fn string_vec_json(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn string_vec_from_json(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn snippet_for_storage(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    normalized.chars().take(max_chars).collect()
}

fn backfill_empty_timestamp(conn: &Connection, table: &str, column: &str) -> SqlResult<()> {
    if !table_exists(conn, table)? || !table_has_column(conn, table, column)? {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "UPDATE {table} SET {column}=datetime('now') WHERE {column} IS NULL OR {column}=''"
    ))?;
    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    column_definition: &str,
) -> SqlResult<()> {
    if !table_exists(conn, table)? || table_has_column(conn, table, column)? {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "ALTER TABLE {table} ADD COLUMN {column_definition}"
    ))?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> SqlResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        rusqlite::params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> SqlResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> WriterMemory {
        WriterMemory::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_canon_entity_upsert() {
        let m = memory();
        let id = m
            .upsert_canon_entity(
                "character",
                "主角",
                &["林墨".into()],
                "主角",
                &serde_json::json!({"weapon": "剑"}),
                0.9,
            )
            .unwrap();
        assert!(id > 0);
        let facts = m.get_canon_facts_for_entity("主角").unwrap();
        assert_eq!(facts, vec![("weapon".to_string(), "剑".to_string())]);
        let entities = m.list_canon_entities().unwrap();
        assert_eq!(entities[0].name, "主角");
    }

    #[test]
    fn test_canon_rule_upsert() {
        let m = memory();
        let id = m
            .upsert_canon_rule("林墨绝不主动弃刀。", "character_rule", 7, "test")
            .unwrap();
        assert!(id > 0);
        m.upsert_canon_rule("林墨绝不主动弃刀。", "combat_rule", 9, "test2")
            .unwrap();
        let rules = m.list_canon_rules(10).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].category, "combat_rule");
        assert_eq!(rules[0].priority, 9);
    }

    #[test]
    fn test_promise_lifecycle() {
        let m = memory();
        let id = m
            .add_promise("clue", "密道", "第2章破庙有密道", "ch2", "ch8", 5)
            .unwrap();
        assert!(id > 0);
        let open = m.get_open_promises().unwrap();
        assert_eq!(open.len(), 1);
        let summaries = m.get_open_promise_summaries().unwrap();
        assert_eq!(summaries[0].title, "密道");
        assert!(m.resolve_promise(id, "ch8").unwrap());
        let open2 = m.get_open_promises().unwrap();
        assert_eq!(open2.len(), 0);
    }

    #[test]
    fn test_promise_defer_and_abandon() {
        let m = memory();
        let deferred_id = m
            .add_promise("clue", "密道", "第2章破庙有密道", "ch2", "ch8", 5)
            .unwrap();
        assert!(m.defer_promise(deferred_id, "ch10").unwrap());
        let summaries = m.get_open_promise_summaries().unwrap();
        assert_eq!(summaries[0].expected_payoff, "ch10");

        let abandoned_id = m
            .add_promise("clue", "铜铃", "铜铃声需要解释", "ch2", "ch6", 5)
            .unwrap();
        assert!(m.abandon_promise(abandoned_id).unwrap());
        let open_titles = m
            .get_open_promise_summaries()
            .unwrap()
            .into_iter()
            .map(|promise| promise.title)
            .collect::<Vec<_>>();
        assert!(!open_titles.contains(&"铜铃".to_string()));
    }

    #[test]
    fn test_style_preference_update() {
        let m = memory();
        m.upsert_style_preference("dialog_style", "prefers_subtext", true)
            .unwrap();
        m.upsert_style_preference("exposition", "rejects_info_dump", false)
            .unwrap();
        let prefs = m.list_style_preferences(5).unwrap();
        assert_eq!(prefs.len(), 2);
        assert!(prefs.iter().any(|p| p.key == "dialog_style"));
    }

    #[test]
    fn test_feedback_record() {
        let m = memory();
        m.record_feedback("prop_1", "accepted", "", "").unwrap();
        m.record_feedback("prop_2", "rejected", "not my style", "")
            .unwrap();
        let (accepted, rejected) = m.feedback_stats("prop_1").unwrap();
        assert_eq!(accepted, 1);
        assert_eq!(rejected, 0);
    }

    #[test]
    fn test_memory_audit_record() {
        let m = memory();
        m.record_memory_audit(&MemoryAuditSummary {
            proposal_id: "prop_1".to_string(),
            kind: "CanonUpdate".to_string(),
            action: "Accepted".to_string(),
            title: "沈照 [character]".to_string(),
            evidence: "那个少年名叫沈照".to_string(),
            rationale: "durable character".to_string(),
            reason: Some("approved".to_string()),
            created_at: 42,
        })
        .unwrap();
        let audit = m.list_memory_audit(5).unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].proposal_id, "prop_1");
        assert_eq!(audit[0].reason.as_deref(), Some("approved"));
    }

    #[test]
    fn test_memory_feedback_record_and_filter_by_slot() {
        let m = memory();
        m.record_memory_feedback(&MemoryFeedbackSummary {
            slot: "memory|canon|character|沈照".to_string(),
            category: "canon".to_string(),
            action: "reinforcement".to_string(),
            confidence_delta: 0.08,
            source_error: None,
            proposal_id: "prop_1".to_string(),
            reason: Some("accepted after save".to_string()),
            created_at: 42,
        })
        .unwrap();
        m.record_memory_feedback(&MemoryFeedbackSummary {
            slot: "memory|promise|mystery_clue|玉佩".to_string(),
            category: "promise".to_string(),
            action: "correction".to_string(),
            confidence_delta: -0.2,
            source_error: Some("作者指出不是伏笔".to_string()),
            proposal_id: "prop_2".to_string(),
            reason: Some("not durable".to_string()),
            created_at: 43,
        })
        .unwrap();

        let feedback = m.list_memory_feedback(10).unwrap();
        assert_eq!(feedback.len(), 2);
        assert_eq!(feedback[0].slot, "memory|promise|mystery_clue|玉佩");
        assert_eq!(feedback[0].action, "correction");
        assert_eq!(
            feedback[0].source_error.as_deref(),
            Some("作者指出不是伏笔")
        );

        let slot_feedback = m
            .list_memory_feedback_for_slot("memory|canon|character|沈照", 10)
            .unwrap();
        assert_eq!(slot_feedback.len(), 1);
        assert_eq!(slot_feedback[0].category, "canon");
        assert_eq!(slot_feedback[0].confidence_delta, 0.08);
    }

    #[test]
    fn test_manual_agent_turns_persist_and_filter_by_project() {
        let m = memory();
        m.record_manual_agent_turn(&ManualAgentTurnSummary {
            project_id: "novel-a".to_string(),
            observation_id: "obs-a-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            user: "上一轮怎么处理玉佩？".to_string(),
            assistant: "让张三暂时隐瞒玉佩。".to_string(),
            source_refs: vec!["PromiseLedger".to_string()],
            created_at: 10,
        })
        .unwrap();
        m.record_manual_agent_turn(&ManualAgentTurnSummary {
            project_id: "novel-b".to_string(),
            observation_id: "obs-b-1".to_string(),
            chapter_title: None,
            user: "另一个项目".to_string(),
            assistant: "不应混入".to_string(),
            source_refs: Vec::new(),
            created_at: 11,
        })
        .unwrap();
        m.record_manual_agent_turn(&ManualAgentTurnSummary {
            project_id: "novel-a".to_string(),
            observation_id: "obs-a-2".to_string(),
            chapter_title: Some("第二章".to_string()),
            user: "继续上一轮".to_string(),
            assistant: "把玉佩变成下一章冲突。".to_string(),
            source_refs: vec!["CreativeDecision".to_string()],
            created_at: 12,
        })
        .unwrap();

        let turns = m.list_manual_agent_turns("novel-a", 10).unwrap();

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].observation_id, "obs-a-1");
        assert_eq!(turns[0].chapter_title.as_deref(), Some("第一章"));
        assert_eq!(turns[0].source_refs, vec!["PromiseLedger".to_string()]);
        assert_eq!(turns[1].user, "继续上一轮");
    }

    #[test]
    fn test_writer_trace_record() {
        let m = memory();
        m.record_observation_trace("obs_1", 10, "Idle", Some("Chapter-1"), "林墨停下脚步")
            .unwrap();
        m.record_proposal_trace(
            &ProposalTraceSummary {
                id: "prop_1".to_string(),
                observation_id: "obs_1".to_string(),
                kind: "Ghost".to_string(),
                priority: "Ambient".to_string(),
                state: "pending".to_string(),
                confidence: 0.7,
                preview_snippet: "他没有立刻回答".to_string(),
                evidence: vec![crate::writer_agent::proposal::EvidenceRef {
                    source: crate::writer_agent::proposal::EvidenceSource::ChapterMission,
                    reference: "Chapter-1:mission".to_string(),
                    snippet: "本章任务".to_string(),
                }],
                context_budget: Some(ContextBudgetTrace {
                    task: "GhostWriting".to_string(),
                    used: 40,
                    total_budget: 100,
                    wasted: 60,
                    source_reports: vec![ContextSourceBudgetTrace {
                        source: "CursorPrefix".to_string(),
                        requested: 80,
                        provided: 40,
                        truncated: true,
                        reason:
                            "GhostWriting required source reserved 240 chars before priority fill."
                                .to_string(),
                        truncation_reason: Some(
                            "Source content was limited by its per-source budget of 80 chars."
                                .to_string(),
                        ),
                    }],
                }),
                expires_at: Some(1000),
            },
            11,
        )
        .unwrap();
        m.record_feedback_trace(&FeedbackTraceSummary {
            proposal_id: "prop_1".to_string(),
            action: "Accepted".to_string(),
            reason: Some("fits".to_string()),
            created_at: 12,
        })
        .unwrap();
        m.update_proposal_trace_state("prop_1", "feedback:Accepted")
            .unwrap();

        assert_eq!(m.list_observation_traces(5).unwrap()[0].id, "obs_1");
        let proposal = m.list_proposal_traces(5).unwrap().remove(0);
        assert_eq!(proposal.state, "feedback:Accepted");
        assert_eq!(proposal.evidence.len(), 1);
        assert_eq!(
            proposal.evidence[0].source,
            crate::writer_agent::proposal::EvidenceSource::ChapterMission
        );
        let budget = proposal.context_budget.unwrap();
        assert_eq!(budget.task, "GhostWriting");
        assert_eq!(budget.used, 40);
        assert!(budget.source_reports[0].truncated);
        assert_eq!(
            m.list_feedback_traces(5).unwrap()[0].reason.as_deref(),
            Some("fits")
        );
    }

    #[test]
    fn test_run_events_persist_and_replay_in_order() {
        let m = memory();
        m.record_run_event(&RunEventSummary {
            seq: 1,
            project_id: "novel-a".to_string(),
            session_id: "session-a".to_string(),
            task_id: Some("obs-1".to_string()),
            event_type: "writer.observation".to_string(),
            source_refs: vec!["chapter:Chapter-1".to_string()],
            data: serde_json::json!({"reason": "Idle"}),
            ts_ms: 10,
        })
        .unwrap();
        m.record_run_event(&RunEventSummary {
            seq: 2,
            project_id: "novel-a".to_string(),
            session_id: "session-a".to_string(),
            task_id: Some("prop-1".to_string()),
            event_type: "writer.proposal_created".to_string(),
            source_refs: vec!["obs-1".to_string()],
            data: serde_json::json!({"kind": "Ghost"}),
            ts_ms: 11,
        })
        .unwrap();
        m.record_run_event(&RunEventSummary {
            seq: 1,
            project_id: "novel-b".to_string(),
            session_id: "session-b".to_string(),
            task_id: None,
            event_type: "writer.observation".to_string(),
            source_refs: Vec::new(),
            data: serde_json::json!({"reason": "Other"}),
            ts_ms: 12,
        })
        .unwrap();

        let events = m.list_run_events("novel-a", "session-a", 10).unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(
            events.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(events[0].event_type, "writer.observation");
        assert_eq!(events[1].task_id.as_deref(), Some("prop-1"));
        assert_eq!(
            events[1].data.get("kind").and_then(|value| value.as_str()),
            Some("Ghost")
        );
    }

    #[test]
    fn test_context_recall_records_only_surfaced_evidence() {
        let m = memory();
        let evidence = vec![
            crate::writer_agent::proposal::EvidenceRef {
                source: crate::writer_agent::proposal::EvidenceSource::ChapterMission,
                reference: "Chapter-2:mission".to_string(),
                snippet: "本章必须推进玉佩线索。".to_string(),
            },
            crate::writer_agent::proposal::EvidenceRef {
                source: crate::writer_agent::proposal::EvidenceSource::Canon,
                reference: "林墨.weapon".to_string(),
                snippet: "林墨惯用寒影刀。".to_string(),
            },
        ];
        m.record_context_recalls("novel-a", "prop_1", "obs_1", &evidence, 10)
            .unwrap();
        m.record_context_recalls("novel-a", "prop_2", "obs_2", &evidence[..1], 20)
            .unwrap();

        let recalls = m.list_context_recalls("novel-a", 10).unwrap();

        assert_eq!(recalls.len(), 2);
        assert_eq!(recalls[0].source, "ChapterMission");
        assert_eq!(recalls[0].reference, "Chapter-2:mission");
        assert_eq!(recalls[0].recall_count, 2);
        assert_eq!(recalls[0].first_recalled_at, 10);
        assert_eq!(recalls[0].last_recalled_at, 20);
        assert_eq!(recalls[0].last_proposal_id, "prop_2");
        assert_eq!(recalls[1].reference, "林墨.weapon");
        assert_eq!(recalls[1].recall_count, 1);
    }

    #[test]
    fn test_decision_summary() {
        let m = memory();
        m.record_decision(
            "Chapter-1",
            "续写建议",
            "accepted",
            &[],
            "符合角色声音",
            &[],
        )
        .unwrap();
        let decisions = m.list_recent_decisions(5).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision, "accepted");
    }

    #[test]
    fn test_story_contract_upsert_and_seed() {
        let m = memory();
        assert!(m
            .ensure_story_contract_seed(
                "novel-a",
                "寒影录",
                "玄幻",
                "刀客追查玉佩真相。",
                "复仇与守护的冲突。",
                "不得提前泄露玉佩来源。",
            )
            .unwrap());
        assert!(!m
            .ensure_story_contract_seed("novel-a", "不覆盖", "悬疑", "", "", "",)
            .unwrap());

        let mut contract = m.get_story_contract("novel-a").unwrap().unwrap();
        assert_eq!(contract.title, "寒影录");
        assert!(contract.render_for_context().contains("读者承诺"));

        contract.reader_promise = "新的读者承诺".to_string();
        m.upsert_story_contract(&contract).unwrap();
        let updated = m.get_story_contract("novel-a").unwrap().unwrap();
        assert_eq!(updated.reader_promise, "新的读者承诺");
        assert!(!updated.is_empty());
    }

    #[test]
    fn test_chapter_mission_upsert_list_and_seed() {
        let m = memory();
        assert!(m
            .ensure_chapter_mission_seed(
                "novel-a",
                "第一章",
                "林墨发现玉佩线索。",
                "推进玉佩线索",
                "不要提前揭开真相",
                "以新的疑问收束。",
                "test",
            )
            .unwrap());
        assert!(!m
            .ensure_chapter_mission_seed("novel-a", "第一章", "不覆盖", "", "", "", "test")
            .unwrap());

        let mut mission = m.get_chapter_mission("novel-a", "第一章").unwrap().unwrap();
        assert_eq!(mission.mission, "林墨发现玉佩线索。");
        assert!(mission.render_for_context().contains("本章任务"));

        mission.expected_ending = "以冲突升级收束。".to_string();
        m.upsert_chapter_mission(&mission).unwrap();

        let missions = m.list_chapter_missions("novel-a", 10).unwrap();
        assert_eq!(missions.len(), 1);
        assert_eq!(missions[0].expected_ending, "以冲突升级收束。");
        assert!(!missions[0].is_empty());
    }

    #[test]
    fn test_chapter_result_record_and_render() {
        let m = memory();
        let id = m
            .record_chapter_result(&ChapterResultSummary {
                id: 0,
                project_id: "novel-a".to_string(),
                chapter_title: "第一章".to_string(),
                chapter_revision: "rev-1".to_string(),
                summary: "林墨发现玉佩线索，张三隐瞒下落。".to_string(),
                state_changes: vec!["林墨得知玉佩存在风险".to_string()],
                character_progress: vec!["张三选择隐瞒".to_string()],
                new_conflicts: vec!["林墨与张三信任受损".to_string()],
                new_clues: vec!["玉佩".to_string()],
                promise_updates: vec!["玉佩仍需后续解释".to_string()],
                canon_updates: vec!["林墨惯用寒影刀".to_string()],
                source_ref: "chapter_save:第一章:rev-1".to_string(),
                created_at: 42,
            })
            .unwrap();
        assert!(id > 0);

        let recent = m.list_recent_chapter_results("novel-a", 5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].chapter_title, "第一章");
        assert_eq!(recent[0].new_clues, vec!["玉佩".to_string()]);
        assert!(recent[0].render_for_context().contains("章节结果"));

        let latest = m
            .latest_chapter_result("novel-a", "第一章")
            .unwrap()
            .unwrap();
        assert_eq!(latest.summary, "林墨发现玉佩线索，张三隐瞒下落。");
        assert!(!latest.is_empty());
    }

    #[test]
    fn open_migrates_legacy_writer_memory_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE canon_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL UNIQUE
            );
            INSERT INTO canon_entities (kind, name) VALUES ('character', '林墨');
            CREATE TABLE plot_promises (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                title TEXT NOT NULL
            );
            INSERT INTO plot_promises (kind, title) VALUES ('clue', '玉佩');
            CREATE TABLE writer_proposal_trace (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                proposal_id TEXT NOT NULL UNIQUE,
                observation_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                priority TEXT NOT NULL,
                state TEXT NOT NULL,
                confidence REAL DEFAULT 0.0,
                preview_snippet TEXT DEFAULT '',
                created_at INTEGER NOT NULL
            );",
        )
        .unwrap();

        initialize_schema(&conn).unwrap();

        assert!(table_has_column(&conn, "canon_entities", "attributes_json").unwrap());
        assert!(table_has_column(&conn, "plot_promises", "expected_payoff").unwrap());
        assert!(table_has_column(&conn, "plot_promises", "last_seen_chapter").unwrap());
        assert!(table_has_column(&conn, "writer_proposal_trace", "evidence_json").unwrap());
        assert!(table_has_column(&conn, "writer_proposal_trace", "expires_at").unwrap());
        assert!(table_exists(&conn, "manual_agent_turns").unwrap());
        assert!(table_has_column(&conn, "manual_agent_turns", "source_refs_json").unwrap());
        assert!(table_exists(&conn, "story_contracts").unwrap());
        assert!(table_has_column(&conn, "story_contracts", "reader_promise").unwrap());
        assert!(table_exists(&conn, "chapter_missions").unwrap());
        assert!(table_has_column(&conn, "chapter_missions", "expected_ending").unwrap());
        assert!(table_exists(&conn, "chapter_result_snapshots").unwrap());
        assert!(table_has_column(&conn, "chapter_result_snapshots", "new_clues_json").unwrap());
        assert!(table_exists(&conn, "memory_feedback_events").unwrap());
        assert!(table_has_column(&conn, "memory_feedback_events", "source_error").unwrap());
        assert!(table_has_column(&conn, "memory_feedback_events", "confidence_delta").unwrap());
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let m = WriterMemory { conn };
        let entities = m.list_canon_entities().unwrap();
        assert_eq!(entities[0].name, "林墨");
        let promises = m.get_open_promise_summaries().unwrap();
        assert_eq!(promises[0].title, "玉佩");
        assert_eq!(promises[0].last_seen_chapter, "");
    }
}
