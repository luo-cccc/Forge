impl WriterMemory {
    pub fn add_promise(
        &self,
        kind: &str,
        title: &str,
        description: &str,
        chapter: &str,
        payoff: &str,
        priority: i32,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO plot_promises
             (kind, title, description, introduced_chapter, introduced_ref, last_seen_chapter,
              last_seen_ref, expected_payoff, priority, related_entities_json)
             VALUES (?1, ?2, ?3, ?4, '', ?4, '', ?5, ?6, '[]')",
            rusqlite::params![kind, title, description, chapter, payoff, priority],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_promise_with_status_flags(
        &self,
        kind: &str,
        title: &str,
        description: &str,
        chapter: &str,
        source_ref: &str,
        payoff: &str,
        priority: i32,
        related_entities: &[String],
        blocked_reason: &str,
        promoted: bool,
        core: bool,
    ) -> rusqlite::Result<i64> {
        let entities_json = serde_json::to_string(related_entities).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO plot_promises
             (kind, title, description, introduced_chapter, introduced_ref, last_seen_chapter,
              last_seen_ref, expected_payoff, priority, blocked_reason, promoted, core, related_entities_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                kind,
                title,
                description,
                chapter,
                source_ref,
                payoff,
                priority,
                blocked_reason,
                if promoted { 1 } else { 0 },
                if core { 1 } else { 0 },
                entities_json,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_promise_with_entities(
        &self,
        kind: &str,
        title: &str,
        description: &str,
        chapter: &str,
        payoff: &str,
        priority: i32,
        related_entities: &[String],
    ) -> rusqlite::Result<i64> {
        let entities_json = serde_json::to_string(related_entities).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO plot_promises
             (kind, title, description, introduced_chapter, introduced_ref, last_seen_chapter,
              last_seen_ref, expected_payoff, priority, related_entities_json)
             VALUES (?1, ?2, ?3, ?4, '', ?4, '', ?5, ?6, ?7)",
            rusqlite::params![
                kind,
                title,
                description,
                chapter,
                payoff,
                priority,
                entities_json
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_open_promises(&self) -> rusqlite::Result<Vec<(String, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, title, description, introduced_chapter FROM plot_promises
             WHERE status = 'open' ORDER BY priority DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect()
    }

    pub fn get_open_promise_summaries(&self) -> rusqlite::Result<Vec<PlotPromiseSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, title, description, introduced_chapter,
                    last_seen_chapter, last_seen_ref, expected_payoff, priority,
                    blocked_reason, promoted, core
             FROM plot_promises WHERE status = 'open' ORDER BY priority DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let kind: String = row.get(1)?;
            let risk = PromiseKind::from_kind_str(&kind).default_risk().to_string();
            Ok(PlotPromiseSummary {
                id: row.get(0)?,
                kind,
                title: row.get(2)?,
                description: row.get(3)?,
                introduced_chapter: row.get(4)?,
                last_seen_chapter: row.get(5)?,
                last_seen_ref: row.get(6)?,
                expected_payoff: row.get(7)?,
                priority: row.get(8)?,
                risk,
                blocked_reason: row.get::<_, String>(9).unwrap_or_default(),
                promoted: row.get::<_, i64>(10).unwrap_or_default() != 0,
                core: row.get::<_, i64>(11).unwrap_or_default() != 0,
            })
        })?;
        rows.collect()
    }

    pub fn find_open_promise_by_title(
        &self,
        title: &str,
    ) -> rusqlite::Result<Option<PlotPromiseSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, title, description, introduced_chapter,
                    last_seen_chapter, last_seen_ref, expected_payoff, priority,
                    blocked_reason, promoted, core
             FROM plot_promises
             WHERE status = 'open' AND title = ?1
             ORDER BY priority DESC, created_at DESC
             LIMIT 1",
        )?;
        stmt.query_row(rusqlite::params![title], |row| {
            let kind: String = row.get(1)?;
            let risk = PromiseKind::from_kind_str(&kind).default_risk().to_string();
            Ok(PlotPromiseSummary {
                id: row.get(0)?,
                kind,
                title: row.get(2)?,
                description: row.get(3)?,
                introduced_chapter: row.get(4)?,
                last_seen_chapter: row.get(5)?,
                last_seen_ref: row.get(6)?,
                expected_payoff: row.get(7)?,
                priority: row.get(8)?,
                risk,
                blocked_reason: row.get::<_, String>(9).unwrap_or_default(),
                promoted: row.get::<_, i64>(10).unwrap_or_default() != 0,
                core: row.get::<_, i64>(11).unwrap_or_default() != 0,
            })
        })
        .optional()
    }

    pub fn touch_promise_last_seen(
        &self,
        promise_id: i64,
        chapter: &str,
        source_ref: &str,
    ) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises
             SET last_seen_chapter=?1, last_seen_ref=?2
             WHERE id=?3 AND status='open'",
            rusqlite::params![chapter, source_ref, promise_id],
        )?;
        Ok(changed > 0)
    }

    pub fn update_promise_status_flags(
        &self,
        promise_id: i64,
        blocked_reason: &str,
        promoted: bool,
        core: bool,
    ) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises
             SET blocked_reason=?1, promoted=?2, core=?3
             WHERE id=?4 AND status='open'",
            rusqlite::params![
                blocked_reason,
                if promoted { 1 } else { 0 },
                if core { 1 } else { 0 },
                promise_id
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn resolve_promise(&self, promise_id: i64, _chapter: &str) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises SET status='resolved' WHERE id=?1 AND status='open'",
            rusqlite::params![promise_id],
        )?;
        Ok(changed > 0)
    }

    pub fn defer_promise(&self, promise_id: i64, expected_payoff: &str) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises
             SET expected_payoff=?1, blocked_reason=''
             WHERE id=?2 AND status='open'",
            rusqlite::params![expected_payoff, promise_id],
        )?;
        Ok(changed > 0)
    }

    pub fn abandon_promise(&self, promise_id: i64) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises SET status='abandoned' WHERE id=?1 AND status='open'",
            rusqlite::params![promise_id],
        )?;
        Ok(changed > 0)
    }

    // -- Style Preferences --


}
