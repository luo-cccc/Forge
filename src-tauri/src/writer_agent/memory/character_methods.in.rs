impl WriterMemory {
    pub fn upsert_character(
        &self,
        name: &str,
        aliases: &[String],
        role_type: &str,
        summary: &str,
    ) -> rusqlite::Result<i64> {
        let aliases_json = serde_json::to_string(aliases).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO characters (name, aliases_json, role_type, current_state_summary, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(name) DO UPDATE SET
                 aliases_json = excluded.aliases_json,
                 role_type = excluded.role_type,
                 current_state_summary = excluded.current_state_summary,
                 updated_at = datetime('now')",
            rusqlite::params![name, aliases_json, role_type, summary],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_character_by_name(&self, name: &str) -> rusqlite::Result<Option<CharacterSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, aliases_json, role_type, current_state_summary, updated_at
             FROM characters WHERE name = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![name], |row| {
            Ok(CharacterSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                aliases: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                role_type: row.get(3)?,
                current_state_summary: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        match rows.next() {
            Some(Ok(summary)) => Ok(Some(summary)),
            _ => Ok(None),
        }
    }

    pub fn get_character_by_id(&self, id: i64) -> rusqlite::Result<Option<CharacterSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, aliases_json, role_type, current_state_summary, updated_at
             FROM characters WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok(CharacterSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                aliases: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                role_type: row.get(3)?,
                current_state_summary: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        match rows.next() {
            Some(Ok(summary)) => Ok(Some(summary)),
            _ => Ok(None),
        }
    }

    pub fn list_characters(&self, role_type_filter: Option<&str>) -> rusqlite::Result<Vec<CharacterSummary>> {
        let query = if let Some(rt) = role_type_filter {
            format!("SELECT id, name, aliases_json, role_type, current_state_summary, updated_at FROM characters WHERE role_type = '{}' ORDER BY name", rt)
        } else {
            "SELECT id, name, aliases_json, role_type, current_state_summary, updated_at FROM characters ORDER BY name".to_string()
        };
        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([], |row| {
            Ok(CharacterSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                aliases: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                role_type: row.get(3)?,
                current_state_summary: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }

    pub fn character_exists(&self, name: &str) -> rusqlite::Result<bool> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM characters WHERE name = ?1",
            rusqlite::params![name],
            |row| row.get::<_, i64>(0),
        ).map(|count| count > 0)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSummary {
    pub id: i64,
    pub name: String,
    pub aliases: Vec<String>,
    pub role_type: String,
    pub current_state_summary: String,
    pub updated_at: String,
}
