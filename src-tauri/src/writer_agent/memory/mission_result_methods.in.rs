impl WriterMemory {
    pub fn upsert_chapter_mission(&self, mission: &ChapterMissionSummary) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO chapter_missions
             (project_id, chapter_title, mission, must_include, must_not, expected_ending,
              status, source_ref, updated_at, blocked_reason, retired_history)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), ?9, ?10)
             ON CONFLICT(project_id, chapter_title) DO UPDATE SET
                mission=excluded.mission,
                must_include=excluded.must_include,
                must_not=excluded.must_not,
                expected_ending=excluded.expected_ending,
                status=excluded.status,
                source_ref=excluded.source_ref,
                updated_at=datetime('now'),
                blocked_reason=excluded.blocked_reason,
                retired_history=excluded.retired_history",
            rusqlite::params![
                mission.project_id,
                mission.chapter_title,
                mission.mission,
                mission.must_include,
                mission.must_not,
                mission.expected_ending,
                mission.status,
                mission.source_ref,
                mission.blocked_reason,
                mission.retired_history,
            ],
        )?;
        self.conn.query_row(
            "SELECT id FROM chapter_missions WHERE project_id=?1 AND chapter_title=?2",
            rusqlite::params![mission.project_id, mission.chapter_title],
            |row| row.get(0),
        )
    }

    pub fn get_chapter_mission(
        &self,
        project_id: &str,
        chapter_title: &str,
    ) -> rusqlite::Result<Option<ChapterMissionSummary>> {
        self.conn
            .query_row(
                "SELECT id, project_id, chapter_title, mission, must_include, must_not,
                        expected_ending, status, source_ref, updated_at,
                        blocked_reason, retired_history
                 FROM chapter_missions WHERE project_id=?1 AND chapter_title=?2",
                rusqlite::params![project_id, chapter_title],
                chapter_mission_from_row,
            )
            .optional()
    }

    pub fn list_chapter_missions(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ChapterMissionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, chapter_title, mission, must_include, must_not,
                    expected_ending, status, source_ref, updated_at,
                    blocked_reason, retired_history
             FROM chapter_missions
             WHERE project_id=?1
             ORDER BY id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            chapter_mission_from_row(row)
        })?;
        rows.collect()
    }

    pub fn ensure_chapter_mission_seed(
        &self,
        project_id: &str,
        chapter_title: &str,
        mission: &str,
        must_include: &str,
        must_not: &str,
        expected_ending: &str,
        source_ref: &str,
    ) -> rusqlite::Result<bool> {
        if self
            .get_chapter_mission(project_id, chapter_title)?
            .is_some()
        {
            return Ok(false);
        }
        let summary = ChapterMissionSummary {
            id: 0,
            project_id: project_id.to_string(),
            chapter_title: chapter_title.to_string(),
            mission: mission.to_string(),
            must_include: must_include.to_string(),
            must_not: must_not.to_string(),
            expected_ending: expected_ending.to_string(),
            status: "draft".to_string(),
            source_ref: source_ref.to_string(),
            updated_at: String::new(),
            blocked_reason: String::new(),
            retired_history: String::new(),
        };
        self.upsert_chapter_mission(&summary)?;
        Ok(true)
    }

    // -- Chapter Result Snapshots --

    pub fn record_chapter_result(&self, result: &ChapterResultSummary) -> rusqlite::Result<i64> {
        if !result.chapter_revision.trim().is_empty() {
            if let Some(existing_id) = self
                .conn
                .query_row(
                    "SELECT id FROM chapter_result_snapshots
                     WHERE project_id=?1 AND chapter_title=?2 AND chapter_revision=?3
                     ORDER BY created_at DESC, id DESC
                     LIMIT 1",
                    rusqlite::params![
                        result.project_id,
                        result.chapter_title,
                        result.chapter_revision
                    ],
                    |row| row.get(0),
                )
                .optional()?
            {
                return Ok(existing_id);
            }
        }

        self.conn.execute(
            "INSERT INTO chapter_result_snapshots
             (project_id, chapter_title, chapter_revision, summary, state_changes_json,
              character_progress_json, new_conflicts_json, new_clues_json, promise_updates_json,
              canon_updates_json, source_ref, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                result.project_id,
                result.chapter_title,
                result.chapter_revision,
                result.summary,
                string_vec_json(&result.state_changes),
                string_vec_json(&result.character_progress),
                string_vec_json(&result.new_conflicts),
                string_vec_json(&result.new_clues),
                string_vec_json(&result.promise_updates),
                string_vec_json(&result.canon_updates),
                result.source_ref,
                result.created_at as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_recent_chapter_results(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ChapterResultSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, chapter_title, chapter_revision, summary,
                    state_changes_json, character_progress_json, new_conflicts_json,
                    new_clues_json, promise_updates_json, canon_updates_json,
                    source_ref, created_at
             FROM chapter_result_snapshots
             WHERE project_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            chapter_result_from_row(row)
        })?;
        rows.collect()
    }

    pub fn latest_chapter_result(
        &self,
        project_id: &str,
        chapter_title: &str,
    ) -> rusqlite::Result<Option<ChapterResultSummary>> {
        self.conn
            .query_row(
                "SELECT id, project_id, chapter_title, chapter_revision, summary,
                        state_changes_json, character_progress_json, new_conflicts_json,
                        new_clues_json, promise_updates_json, canon_updates_json,
                        source_ref, created_at
                 FROM chapter_result_snapshots
                 WHERE project_id = ?1 AND chapter_title = ?2
                 ORDER BY created_at DESC, id DESC
                 LIMIT 1",
                rusqlite::params![project_id, chapter_title],
                chapter_result_from_row,
            )
            .optional()
    }

    // -- Creative Decisions --


}
