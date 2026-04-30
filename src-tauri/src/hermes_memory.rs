use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftProfile {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub confidence: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub id: i64,
    pub skill: String,
    pub category: String,
    pub active: bool,
    pub created_at: String,
}

pub struct HermesDB {
    conn: Connection,
}

impl HermesDB {
    pub fn open(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    fn initialize(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_session_role ON session_history(role);
            CREATE INDEX IF NOT EXISTS idx_session_created ON session_history(created_at);

            CREATE TABLE IF NOT EXISTS user_drift_profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                value TEXT NOT NULL,
                confidence REAL DEFAULT 0.0,
                source TEXT DEFAULT 'extracted'
            );

            CREATE TABLE IF NOT EXISTS agent_skills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                skill TEXT NOT NULL,
                category TEXT DEFAULT 'general',
                active INTEGER DEFAULT 1,
                created_at TEXT DEFAULT (datetime('now'))
            );",
        )
    }

    // ---- session_history ----

    pub fn log_interaction(&self, role: &str, content: &str) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO session_history (role, content) VALUES (?1, ?2)",
            params![role, content],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn recent_interactions(&self, limit: usize) -> SqlResult<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, role, content, created_at FROM session_history
             ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        let mut records = Vec::new();
        for r in rows {
            records.push(r?);
        }
        records.reverse();
        Ok(records)
    }

    // ---- user_drift_profile ----

    pub fn upsert_drift(&self, key: &str, value: &str, confidence: f64) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO user_drift_profile (key, value, confidence)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value=?2, confidence=?3",
            params![key, value, confidence],
        )?;
        Ok(())
    }

    pub fn get_drift_profiles(&self) -> SqlResult<Vec<DriftProfile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, key, value, confidence, source FROM user_drift_profile")?;
        let rows = stmt.query_map([], |row| {
            Ok(DriftProfile {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                confidence: row.get(3)?,
                source: row.get(4)?,
            })
        })?;
        let mut profiles = Vec::new();
        for r in rows {
            profiles.push(r?);
        }
        Ok(profiles)
    }

    // ---- agent_skills ----

    pub fn insert_skill(&self, skill: &str, category: &str) -> SqlResult<i64> {
        self.conn.execute(
            "INSERT INTO agent_skills (skill, category) VALUES (?1, ?2)",
            params![skill, category],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_active_skills(&self) -> SqlResult<Vec<AgentSkill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, skill, category, active, created_at FROM agent_skills WHERE active = 1",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AgentSkill {
                id: row.get(0)?,
                skill: row.get(1)?,
                category: row.get(2)?,
                active: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut skills = Vec::new();
        for r in rows {
            skills.push(r?);
        }
        Ok(skills)
    }

    pub fn search_skills(&self, keyword: &str) -> SqlResult<Vec<AgentSkill>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT id, skill, category, active, created_at FROM agent_skills
             WHERE active = 1 AND skill LIKE ?1",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(AgentSkill {
                id: row.get(0)?,
                skill: row.get(1)?,
                category: row.get(2)?,
                active: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut skills = Vec::new();
        for r in rows {
            skills.push(r?);
        }
        Ok(skills)
    }

    pub fn deactivate_skill(&self, id: i64) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE agent_skills SET active = 0 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}
