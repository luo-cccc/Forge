//! WriterMemory — structured creative ledgers.
//! Canon, promises, style preferences, creative decisions.
//! Ported from the plan's Creative Ledgers specification.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS canon_entities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL UNIQUE,
    aliases_json TEXT DEFAULT '[]',
    summary TEXT DEFAULT '',
    attributes_json TEXT DEFAULT '{}',
    confidence REAL DEFAULT 0.5,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_canon_name ON canon_entities(name);
CREATE INDEX IF NOT EXISTS idx_canon_kind ON canon_entities(kind);

CREATE TABLE IF NOT EXISTS canon_facts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    source_ref TEXT DEFAULT '',
    confidence REAL DEFAULT 0.5,
    status TEXT DEFAULT 'active',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_canon_fact_entity ON canon_facts(entity_id);

CREATE TABLE IF NOT EXISTS plot_promises (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT DEFAULT '',
    introduced_chapter TEXT DEFAULT '',
    introduced_ref TEXT DEFAULT '',
    expected_payoff TEXT DEFAULT '',
    status TEXT DEFAULT 'open',
    priority INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_promise_status ON plot_promises(status);

CREATE TABLE IF NOT EXISTS style_preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL,
    evidence_ref TEXT DEFAULT '',
    confidence REAL DEFAULT 0.5,
    accepted_count INTEGER DEFAULT 0,
    rejected_count INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS creative_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT DEFAULT '',
    title TEXT NOT NULL,
    decision TEXT NOT NULL,
    alternatives_json TEXT DEFAULT '[]',
    rationale TEXT DEFAULT '',
    source_refs_json TEXT DEFAULT '[]',
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proposal_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL,
    action TEXT NOT NULL,
    final_text TEXT DEFAULT '',
    reason TEXT DEFAULT '',
    created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_feedback_proposal ON proposal_feedback(proposal_id);
"#;

pub struct WriterMemory {
    conn: Connection,
}

impl WriterMemory {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    // -- Canon Entities --

    pub fn upsert_canon_entity(
        &self, kind: &str, name: &str, aliases: &[String],
        summary: &str, attributes: &serde_json::Value, confidence: f64,
    ) -> rusqlite::Result<i64> {
        let aliases_json = serde_json::to_string(aliases).unwrap();
        let attrs_json = attributes.to_string();
        self.conn.execute(
            "INSERT INTO canon_entities (kind, name, aliases_json, summary, attributes_json, confidence, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(name) DO UPDATE SET
                kind=excluded.kind,
                aliases_json=excluded.aliases_json,
                summary=excluded.summary,
                attributes_json=excluded.attributes_json,
                confidence=excluded.confidence,
                updated_at=datetime('now')",
            rusqlite::params![kind, name, aliases_json, summary, attrs_json, confidence],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_canon_facts_for_entity(&self, entity_name: &str) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.key, f.value FROM canon_facts f
             JOIN canon_entities e ON f.entity_id = e.id
             WHERE e.name = ?1 AND f.status = 'active'"
        )?;
        let rows = stmt.query_map(rusqlite::params![entity_name], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.collect()
    }

    // -- Plot Promises --

    pub fn add_promise(&self, kind: &str, title: &str, description: &str,
                       chapter: &str, payoff: &str, priority: i32) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO plot_promises (kind, title, description, introduced_chapter, expected_payoff, priority)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![kind, title, description, chapter, payoff, priority],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_open_promises(&self) -> rusqlite::Result<Vec<(String, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, title, description, introduced_chapter FROM plot_promises
             WHERE status = 'open' ORDER BY priority DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect()
    }

    pub fn resolve_promise(&self, promise_id: i64, chapter: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE plot_promises SET status='resolved' WHERE id=?1",
            rusqlite::params![promise_id],
        )?;
        Ok(())
    }

    // -- Style Preferences --

    pub fn upsert_style_preference(&self, key: &str, value: &str, accepted: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO style_preferences (key, value, accepted_count, rejected_count, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
             accepted_count = accepted_count + ?3,
             rejected_count = rejected_count + ?4,
             updated_at = datetime('now')",
            rusqlite::params![key, value, if accepted { 1 } else { 0 }, if accepted { 0 } else { 1 }],
        )?;
        Ok(())
    }

    // -- Creative Decisions --

    pub fn record_decision(&self, scope: &str, title: &str, decision: &str,
                           alternatives: &[String], rationale: &str, sources: &[String]) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO creative_decisions (scope, title, decision, alternatives_json, rationale, source_refs_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![scope, title, decision,
                serde_json::to_string(alternatives).unwrap(),
                rationale,
                serde_json::to_string(sources).unwrap()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // -- Feedback --

    pub fn record_feedback(&self, proposal_id: &str, action: &str, reason: &str, final_text: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO proposal_feedback (proposal_id, action, reason, final_text) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![proposal_id, action, reason, final_text],
        )?;
        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> WriterMemory {
        WriterMemory::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_canon_entity_upsert() {
        let m = memory();
        let id = m.upsert_canon_entity("character", "主角", &["林墨".into()], "主角", &serde_json::json!({"weapon": "剑"}), 0.9).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_promise_lifecycle() {
        let m = memory();
        let id = m.add_promise("clue", "密道", "第2章破庙有密道", "ch2", "ch8", 5).unwrap();
        assert!(id > 0);
        let open = m.get_open_promises().unwrap();
        assert_eq!(open.len(), 1);
        m.resolve_promise(id, "ch8").unwrap();
        let open2 = m.get_open_promises().unwrap();
        assert_eq!(open2.len(), 0);
    }

    #[test]
    fn test_style_preference_update() {
        let m = memory();
        m.upsert_style_preference("dialog_style", "prefers_subtext", true).unwrap();
        m.upsert_style_preference("exposition", "rejects_info_dump", false).unwrap();
    }

    #[test]
    fn test_feedback_record() {
        let m = memory();
        m.record_feedback("prop_1", "accepted", "", "").unwrap();
        m.record_feedback("prop_2", "rejected", "not my style", "").unwrap();
        let (accepted, rejected) = m.feedback_stats("prop_1").unwrap();
        assert_eq!(accepted, 1);
        assert_eq!(rejected, 0);
    }
}
