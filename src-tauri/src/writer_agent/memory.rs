//! WriterMemory - structured creative ledgers.
//! Canon, promises, style preferences, creative decisions.
//! Ported from the plan's Creative Ledgers specification.

use rusqlite::{Connection, Result as SqlResult};
use serde::Serialize;

const SCHEMA_VERSION: i64 = 2;

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

CREATE TABLE IF NOT EXISTS canon_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL,
    priority INTEGER DEFAULT 0,
    source_ref TEXT DEFAULT '',
    status TEXT DEFAULT 'active',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

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

CREATE TABLE IF NOT EXISTS memory_audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    action TEXT NOT NULL,
    title TEXT NOT NULL,
    evidence TEXT DEFAULT '',
    rationale TEXT DEFAULT '',
    reason TEXT DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS writer_observation_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observation_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    reason TEXT NOT NULL,
    chapter_title TEXT DEFAULT '',
    paragraph_snippet TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS writer_proposal_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL UNIQUE,
    observation_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    priority TEXT NOT NULL,
    state TEXT NOT NULL,
    confidence REAL DEFAULT 0.0,
    preview_snippet TEXT DEFAULT '',
    created_at INTEGER NOT NULL,
    expires_at INTEGER
);

CREATE TABLE IF NOT EXISTS writer_feedback_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL,
    action TEXT NOT NULL,
    reason TEXT DEFAULT '',
    created_at INTEGER NOT NULL
);
"#;

const INDEX_SCHEMA: &str = r#"
CREATE INDEX IF NOT EXISTS idx_canon_name ON canon_entities(name);
CREATE INDEX IF NOT EXISTS idx_canon_kind ON canon_entities(kind);
CREATE INDEX IF NOT EXISTS idx_canon_fact_entity ON canon_facts(entity_id);
CREATE INDEX IF NOT EXISTS idx_canon_fact_key ON canon_facts(key);
CREATE INDEX IF NOT EXISTS idx_canon_rule_category ON canon_rules(category);
CREATE INDEX IF NOT EXISTS idx_canon_rule_status ON canon_rules(status);
CREATE INDEX IF NOT EXISTS idx_promise_status ON plot_promises(status);
CREATE INDEX IF NOT EXISTS idx_feedback_proposal ON proposal_feedback(proposal_id);
CREATE INDEX IF NOT EXISTS idx_memory_audit_created_at ON memory_audit_events(created_at);
CREATE INDEX IF NOT EXISTS idx_observation_trace_created_at ON writer_observation_trace(created_at);
CREATE INDEX IF NOT EXISTS idx_proposal_trace_created_at ON writer_proposal_trace(created_at);
CREATE INDEX IF NOT EXISTS idx_proposal_trace_proposal_id ON writer_proposal_trace(proposal_id);
CREATE INDEX IF NOT EXISTS idx_feedback_trace_created_at ON writer_feedback_trace(created_at);
"#;

pub struct WriterMemory {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonEntitySummary {
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub attributes: serde_json::Value,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonRuleSummary {
    pub rule: String,
    pub category: String,
    pub priority: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotPromiseSummary {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub introduced_chapter: String,
    pub expected_payoff: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreativeDecisionSummary {
    pub scope: String,
    pub title: String,
    pub decision: String,
    pub rationale: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StylePreferenceSummary {
    pub key: String,
    pub value: String,
    pub accepted_count: i64,
    pub rejected_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAuditSummary {
    pub proposal_id: String,
    pub kind: String,
    pub action: String,
    pub title: String,
    pub evidence: String,
    pub rationale: String,
    pub reason: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone)]
pub struct ObservationTraceSummary {
    pub id: String,
    pub created_at: u64,
    pub reason: String,
    pub chapter_title: Option<String>,
    pub paragraph_snippet: String,
}

#[derive(Debug, Clone)]
pub struct ProposalTraceSummary {
    pub id: String,
    pub observation_id: String,
    pub kind: String,
    pub priority: String,
    pub state: String,
    pub confidence: f64,
    pub preview_snippet: String,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FeedbackTraceSummary {
    pub proposal_id: String,
    pub action: String,
    pub reason: Option<String>,
    pub created_at: u64,
}

impl WriterMemory {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        initialize_schema(&conn)?;
        Ok(Self { conn })
    }

    // -- Canon Entities --

    pub fn upsert_canon_entity(
        &self,
        kind: &str,
        name: &str,
        aliases: &[String],
        summary: &str,
        attributes: &serde_json::Value,
        confidence: f64,
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

        let entity_id: i64 = self.conn.query_row(
            "SELECT id FROM canon_entities WHERE name=?1",
            rusqlite::params![name],
            |row| row.get(0),
        )?;

        if let Some(map) = attributes.as_object() {
            for (key, value) in map {
                let fact_value = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => continue,
                    other => other.to_string(),
                };
                self.conn.execute(
                    "DELETE FROM canon_facts WHERE entity_id=?1 AND key=?2",
                    rusqlite::params![entity_id, key],
                )?;
                self.conn.execute(
                    "INSERT INTO canon_facts (entity_id, key, value, source_ref, confidence, status, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'active', datetime('now'))",
                    rusqlite::params![entity_id, key, fact_value, "canon_entity.attributes", confidence],
                )?;
            }
        }

        Ok(entity_id)
    }

    pub fn get_canon_facts_for_entity(
        &self,
        entity_name: &str,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let resolved_name = self
            .resolve_canon_entity_name(entity_name)?
            .unwrap_or_else(|| entity_name.to_string());
        let mut stmt = self.conn.prepare(
            "SELECT f.key, f.value FROM canon_facts f
             JOIN canon_entities e ON f.entity_id = e.id
             WHERE e.name = ?1 AND f.status = 'active'",
        )?;
        let rows = stmt.query_map(rusqlite::params![resolved_name], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.collect()
    }

    pub fn update_canon_attribute(
        &self,
        entity_name: &str,
        attribute: &str,
        value: &str,
        confidence: f64,
    ) -> rusqlite::Result<()> {
        let Some(resolved_name) = self.resolve_canon_entity_name(entity_name)? else {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        };

        let (entity_id, attributes_json): (i64, String) = self.conn.query_row(
            "SELECT id, attributes_json FROM canon_entities WHERE name=?1",
            rusqlite::params![resolved_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut attributes =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&attributes_json)
                .unwrap_or_default();
        attributes.insert(
            attribute.to_string(),
            serde_json::Value::String(value.to_string()),
        );
        self.conn.execute(
            "UPDATE canon_entities SET attributes_json=?1, confidence=?2, updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![serde_json::Value::Object(attributes).to_string(), confidence, entity_id],
        )?;
        self.conn.execute(
            "DELETE FROM canon_facts WHERE entity_id=?1 AND key=?2",
            rusqlite::params![entity_id, attribute],
        )?;
        self.conn.execute(
            "INSERT INTO canon_facts (entity_id, key, value, source_ref, confidence, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', datetime('now'))",
            rusqlite::params![entity_id, attribute, value, "canon.update_attribute", confidence],
        )?;
        Ok(())
    }

    pub fn resolve_canon_entity_name(&self, entity_name: &str) -> rusqlite::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, aliases_json FROM canon_entities ORDER BY length(name) DESC")?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let aliases_json: String = row.get(1)?;
            Ok((name, aliases_json))
        })?;

        for row in rows {
            let (name, aliases_json) = row?;
            if name == entity_name {
                return Ok(Some(name));
            }
            if let Ok(aliases) = serde_json::from_str::<Vec<String>>(&aliases_json) {
                if aliases.iter().any(|alias| alias == entity_name) {
                    return Ok(Some(name));
                }
            }
        }
        Ok(None)
    }

    pub fn get_canon_entity_names(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, aliases_json FROM canon_entities ORDER BY length(name) DESC")?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let aliases_json: String = row.get(1)?;
            Ok((name, aliases_json))
        })?;

        let mut names = Vec::new();
        for row in rows {
            let (name, aliases_json) = row?;
            if !name.trim().is_empty() {
                names.push(name);
            }
            if let Ok(aliases) = serde_json::from_str::<Vec<String>>(&aliases_json) {
                for alias in aliases {
                    if !alias.trim().is_empty() && !names.contains(&alias) {
                        names.push(alias);
                    }
                }
            }
        }
        Ok(names)
    }

    pub fn list_canon_entities(&self) -> rusqlite::Result<Vec<CanonEntitySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, name, summary, attributes_json, confidence
             FROM canon_entities ORDER BY updated_at DESC, name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let attributes_json: String = row.get(3)?;
            let attributes = serde_json::from_str(&attributes_json)
                .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
            Ok(CanonEntitySummary {
                kind: row.get(0)?,
                name: row.get(1)?,
                summary: row.get(2)?,
                attributes,
                confidence: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_canon_rule(
        &self,
        rule: &str,
        category: &str,
        priority: i32,
        source_ref: &str,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO canon_rules (rule, category, priority, source_ref, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'active', datetime('now'))
             ON CONFLICT(rule) DO UPDATE SET
                category=excluded.category,
                priority=excluded.priority,
                source_ref=excluded.source_ref,
                status='active',
                updated_at=datetime('now')",
            rusqlite::params![rule, category, priority, source_ref],
        )?;
        self.conn.query_row(
            "SELECT id FROM canon_rules WHERE rule=?1",
            rusqlite::params![rule],
            |row| row.get(0),
        )
    }

    pub fn list_canon_rules(&self, limit: usize) -> rusqlite::Result<Vec<CanonRuleSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT rule, category, priority, status
             FROM canon_rules
             WHERE status = 'active'
             ORDER BY priority DESC, updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(CanonRuleSummary {
                rule: row.get(0)?,
                category: row.get(1)?,
                priority: row.get(2)?,
                status: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // -- Plot Promises --

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
            "INSERT INTO plot_promises (kind, title, description, introduced_chapter, expected_payoff, priority)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![kind, title, description, chapter, payoff, priority],
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
            "SELECT id, kind, title, description, introduced_chapter, expected_payoff, priority
             FROM plot_promises WHERE status = 'open' ORDER BY priority DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PlotPromiseSummary {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                introduced_chapter: row.get(4)?,
                expected_payoff: row.get(5)?,
                priority: row.get(6)?,
            })
        })?;
        rows.collect()
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
            "UPDATE plot_promises SET expected_payoff=?1 WHERE id=?2 AND status='open'",
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

    pub fn upsert_style_preference(
        &self,
        key: &str,
        value: &str,
        accepted: bool,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO style_preferences (key, value, accepted_count, rejected_count, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
             accepted_count = accepted_count + ?3,
             rejected_count = rejected_count + ?4,
             updated_at = datetime('now')",
            rusqlite::params![
                key,
                value,
                if accepted { 1 } else { 0 },
                if accepted { 0 } else { 1 }
            ],
        )?;
        Ok(())
    }

    pub fn list_style_preferences(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<StylePreferenceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, value, accepted_count, rejected_count
             FROM style_preferences
             ORDER BY accepted_count DESC, updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(StylePreferenceSummary {
                key: row.get(0)?,
                value: row.get(1)?,
                accepted_count: row.get(2)?,
                rejected_count: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // -- Creative Decisions --

    pub fn record_decision(
        &self,
        scope: &str,
        title: &str,
        decision: &str,
        alternatives: &[String],
        rationale: &str,
        sources: &[String],
    ) -> rusqlite::Result<i64> {
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

    pub fn list_recent_decisions(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<CreativeDecisionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT scope, title, decision, rationale, created_at
             FROM creative_decisions ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(CreativeDecisionSummary {
                scope: row.get(0)?,
                title: row.get(1)?,
                decision: row.get(2)?,
                rationale: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    // -- Feedback --

    pub fn record_feedback(
        &self,
        proposal_id: &str,
        action: &str,
        reason: &str,
        final_text: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO proposal_feedback (proposal_id, action, reason, final_text) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![proposal_id, action, reason, final_text],
        )?;
        Ok(())
    }

    pub fn record_memory_audit(&self, entry: &MemoryAuditSummary) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO memory_audit_events
             (proposal_id, kind, action, title, evidence, rationale, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                entry.proposal_id,
                entry.kind,
                entry.action,
                entry.title,
                entry.evidence,
                entry.rationale,
                entry.reason.clone().unwrap_or_default(),
                entry.created_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_memory_audit(&self, limit: usize) -> rusqlite::Result<Vec<MemoryAuditSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT proposal_id, kind, action, title, evidence, rationale, reason, created_at
             FROM memory_audit_events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let reason: String = row.get(6)?;
            let created_at: i64 = row.get(7)?;
            Ok(MemoryAuditSummary {
                proposal_id: row.get(0)?,
                kind: row.get(1)?,
                action: row.get(2)?,
                title: row.get(3)?,
                evidence: row.get(4)?,
                rationale: row.get(5)?,
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

    // -- Writer Agent Trace --

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
             (proposal_id, observation_id, kind, priority, state, confidence, preview_snippet, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(proposal_id) DO UPDATE SET
                observation_id=excluded.observation_id,
                kind=excluded.kind,
                priority=excluded.priority,
                state=excluded.state,
                confidence=excluded.confidence,
                preview_snippet=excluded.preview_snippet,
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
            "SELECT proposal_id, observation_id, kind, priority, state, confidence, preview_snippet, expires_at
             FROM writer_proposal_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let expires_at: Option<i64> = row.get(7)?;
            Ok(ProposalTraceSummary {
                id: row.get(0)?,
                observation_id: row.get(1)?,
                kind: row.get(2)?,
                priority: row.get(3)?,
                state: row.get(4)?,
                confidence: row.get(5)?,
                preview_snippet: row.get(6)?,
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

fn initialize_schema(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(SCHEMA)?;
    migrate_writer_memory_schema(conn)?;
    conn.execute_batch(INDEX_SCHEMA)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

fn migrate_writer_memory_schema(conn: &Connection) -> SqlResult<()> {
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
        "expires_at",
        "expires_at INTEGER",
    )?;

    Ok(())
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
        assert_eq!(
            m.list_feedback_traces(5).unwrap()[0].reason.as_deref(),
            Some("fits")
        );
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
        assert!(table_has_column(&conn, "writer_proposal_trace", "expires_at").unwrap());
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let m = WriterMemory { conn };
        let entities = m.list_canon_entities().unwrap();
        assert_eq!(entities[0].name, "林墨");
        let promises = m.get_open_promise_summaries().unwrap();
        assert_eq!(promises[0].title, "玉佩");
    }
}
