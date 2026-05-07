# Sprint A: Entity State Kernel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Forge from chapter-level to entity-level typed state by adding Characters, CharacterStateVersions, CharacterRelationships, and Promise.subject as authoritative entities.

**Architecture:** 3 new SQLite tables + 1 ALTER on plot_promises. Canon_entities character rows migrated, then blocked. New memory method files follow the existing `include!` pattern. Settlement delta expanded with entity-scoped entries. Promise planner and TodayFive consume entity signals.

**Tech Stack:** Rust (src-tauri, agent-evals), SQLite schema migration

---

### Task 1: Schema Migration — New Tables + Version Bump

**Files:**
- Modify: `src-tauri/src/writer_agent/memory/schema.in.rs`
- Modify: `src-tauri/src/writer_agent/memory/tracing_migrate.in.rs`

- [ ] **Step 1: Add 3 new tables to schema.in.rs**

In `schema.in.rs`, bump `SCHEMA_VERSION` from `16` to `17`. Add these table definitions inside the `SCHEMA` string, after the `plot_promises` table:

```sql
CREATE TABLE IF NOT EXISTS characters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    aliases_json TEXT DEFAULT '[]',
    role_type TEXT NOT NULL DEFAULT 'supporting',
    current_state_summary TEXT DEFAULT '',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS character_state_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    valid_from_chapter TEXT NOT NULL,
    valid_to_chapter TEXT DEFAULT '',
    core_commitments_json TEXT DEFAULT '[]',
    goal_state_json TEXT DEFAULT '{}',
    identity_state_json TEXT DEFAULT '{}',
    relationship_refs_json TEXT DEFAULT '[]',
    source_ref TEXT DEFAULT '',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (character_id) REFERENCES characters(id)
);

CREATE TABLE IF NOT EXISTS character_relationships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_a_id INTEGER NOT NULL,
    character_b_id INTEGER NOT NULL,
    relation_type TEXT NOT NULL DEFAULT 'neutral',
    visibility TEXT NOT NULL DEFAULT 'public',
    valid_from_chapter TEXT NOT NULL,
    valid_to_chapter TEXT DEFAULT '',
    source_ref TEXT DEFAULT '',
    FOREIGN KEY (character_a_id) REFERENCES characters(id),
    FOREIGN KEY (character_b_id) REFERENCES characters(id)
);
```

- [ ] **Step 2: Add ALTER to plot_promises in schema.in.rs**

Add after the `plot_promises` table definition:

```sql
ALTER TABLE plot_promises ADD COLUMN subject_ids_json TEXT DEFAULT '[]';
ALTER TABLE plot_promises ADD COLUMN subject_type TEXT DEFAULT '';
```

- [ ] **Step 3: Add migration in tracing_migrate.in.rs**

In the migration function for version 17, add:

```rust
if current_version < 17 {
    // Create new entity tables
    db.execute_batch("
        CREATE TABLE IF NOT EXISTS characters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            aliases_json TEXT DEFAULT '[]',
            role_type TEXT NOT NULL DEFAULT 'supporting',
            current_state_summary TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS character_state_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL,
            valid_from_chapter TEXT NOT NULL,
            valid_to_chapter TEXT DEFAULT '',
            core_commitments_json TEXT DEFAULT '[]',
            goal_state_json TEXT DEFAULT '{}',
            identity_state_json TEXT DEFAULT '{}',
            relationship_refs_json TEXT DEFAULT '[]',
            source_ref TEXT DEFAULT '',
            created_at INTEGER NOT NULL,
            FOREIGN KEY (character_id) REFERENCES characters(id)
        );
        CREATE TABLE IF NOT EXISTS character_relationships (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character_a_id INTEGER NOT NULL,
            character_b_id INTEGER NOT NULL,
            relation_type TEXT NOT NULL DEFAULT 'neutral',
            visibility TEXT NOT NULL DEFAULT 'public',
            valid_from_chapter TEXT NOT NULL,
            valid_to_chapter TEXT DEFAULT '',
            source_ref TEXT DEFAULT '',
            FOREIGN KEY (character_a_id) REFERENCES characters(id),
            FOREIGN KEY (character_b_id) REFERENCES characters(id)
        );
    ").map_err(|e| e.to_string())?;

    // Migrate canon_entities WHERE kind='character' → characters
    db.execute(
        "INSERT OR IGNORE INTO characters (name, aliases_json, role_type, current_state_summary)
         SELECT name, aliases_json, 'supporting', summary
         FROM canon_entities WHERE kind = 'character'",
        [],
    ).map_err(|e| e.to_string())?;

    // Add subject columns to plot_promises
    db.execute_batch("
        ALTER TABLE plot_promises ADD COLUMN subject_ids_json TEXT DEFAULT '[]';
        ALTER TABLE plot_promises ADD COLUMN subject_type TEXT DEFAULT '';
    ").map_err(|e| e.to_string())?;
}
```

- [ ] **Step 5: Verify schema migration compiles**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 6: Run writer tests to confirm migration works**

Run: `cargo test -p agent-writer 2>&1 | grep "test result"`
Expected: 247 tests passing

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/writer_agent/memory/
git commit -m "feat: add characters, character_state_versions, character_relationships tables and migration"
```

---

### Task 2: Character Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/character_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create character_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_character(
        &self,
        name: &str,
        aliases: &[String],
        role_type: &str,
        summary: &str,
    ) -> rusqlite::Result<i64> {
        let aliases_json = serde_json::to_string(aliases).unwrap_or_default();
        self.db.execute(
            "INSERT INTO characters (name, aliases_json, role_type, current_state_summary, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(name) DO UPDATE SET
                 aliases_json = excluded.aliases_json,
                 role_type = excluded.role_type,
                 current_state_summary = excluded.current_state_summary,
                 updated_at = datetime('now')",
            rusqlite::params![name, aliases_json, role_type, summary],
        )?;
        self.db.last_insert_rowid()
    }

    pub fn get_character_by_name(&self, name: &str) -> rusqlite::Result<Option<CharacterSummary>> {
        let mut stmt = self.db.prepare(
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
        let mut stmt = self.db.prepare(
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
        let mut stmt = self.db.prepare(&query)?;
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
        self.db.query_row(
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
```

- [ ] **Step 2: Register in memory.rs**

After `include!("memory/canon_methods.in.rs");`:
```rust
include!("memory/character_methods.in.rs");
```

- [ ] **Step 3: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/memory/
git commit -m "feat: add character CRUD memory methods"
```

---

### Task 3: Character State Version Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/character_state_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create character_state_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_character_state(
        &self,
        character_id: i64,
        valid_from_chapter: &str,
        core_commitments: &serde_json::Value,
        goal_state: &serde_json::Value,
        identity_state: &serde_json::Value,
        relationship_refs: &[i64],
        source_ref: &str,
    ) -> rusqlite::Result<i64> {
        let commitments_json = serde_json::to_string(core_commitments).unwrap_or_default();
        let goal_json = serde_json::to_string(goal_state).unwrap_or_default();
        let identity_json = serde_json::to_string(identity_state).unwrap_or_default();
        let rel_refs_json = serde_json::to_string(relationship_refs).unwrap_or_default();
        let now_ms = crate::agent_runtime::now_ms();
        self.db.execute(
            "INSERT INTO character_state_versions
             (character_id, valid_from_chapter, core_commitments_json, goal_state_json,
              identity_state_json, relationship_refs_json, source_ref, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                character_id, valid_from_chapter, commitments_json, goal_json,
                identity_json, rel_refs_json, source_ref, now_ms
            ],
        )?;
        self.db.last_insert_rowid()
    }

    pub fn get_active_state(
        &self,
        character_id: i64,
        chapter_title: &str,
    ) -> rusqlite::Result<Option<CharacterStateVersion>> {
        self.db.query_row(
            "SELECT id, character_id, valid_from_chapter, valid_to_chapter,
                    core_commitments_json, goal_state_json, identity_state_json,
                    relationship_refs_json, source_ref, created_at
             FROM character_state_versions
             WHERE character_id = ?1
               AND valid_from_chapter <= ?2
               AND (valid_to_chapter = '' OR valid_to_chapter >= ?2)
             ORDER BY valid_from_chapter DESC
             LIMIT 1",
            rusqlite::params![character_id, chapter_title],
            |row| {
                Ok(CharacterStateVersion {
                    id: row.get(0)?,
                    character_id: row.get(1)?,
                    valid_from_chapter: row.get(2)?,
                    valid_to_chapter: row.get(3)?,
                    core_commitments: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                    goal_state: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                    identity_state: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    relationship_refs: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                    source_ref: row.get(8)?,
                    created_at: row.get(9)?,
                })
            },
        ).optional().map(|opt| opt.flatten())
    }

    pub fn close_state_version(&self, version_id: i64, valid_to_chapter: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE character_state_versions SET valid_to_chapter = ?1 WHERE id = ?2",
            rusqlite::params![valid_to_chapter, version_id],
        )?;
        Ok(())
    }

    pub fn close_active_states_for_character(
        &self,
        character_id: i64,
        valid_to_chapter: &str,
    ) -> rusqlite::Result<usize> {
        let count = self.db.execute(
            "UPDATE character_state_versions
             SET valid_to_chapter = ?1
             WHERE character_id = ?2 AND valid_to_chapter = ''",
            rusqlite::params![valid_to_chapter, character_id],
        )?;
        Ok(count)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterStateVersion {
    pub id: i64,
    pub character_id: i64,
    pub valid_from_chapter: String,
    pub valid_to_chapter: String,
    pub core_commitments: serde_json::Value,
    pub goal_state: serde_json::Value,
    pub identity_state: serde_json::Value,
    pub relationship_refs: Vec<i64>,
    pub source_ref: String,
    pub created_at: i64,
}
```

- [ ] **Step 2: Register in memory.rs**

After `include!("memory/character_methods.in.rs");`:
```rust
include!("memory/character_state_methods.in.rs");
```

- [ ] **Step 3: Add optional helper to rusqlite imports**

At the top of `memory.rs`, ensure `use rusqlite::OptionalExtension;` is imported.

Run: `grep "OptionalExtension" src-tauri/src/writer_agent/memory.rs`
If not found, add: `use rusqlite::OptionalExtension;`

- [ ] **Step 4: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/writer_agent/memory/
git commit -m "feat: add character state version memory methods"
```

---

### Task 4: Character Relationship Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/relationship_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create relationship_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_relationship(
        &self,
        character_a_id: i64,
        character_b_id: i64,
        relation_type: &str,
        visibility: &str,
        valid_from_chapter: &str,
        source_ref: &str,
    ) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO character_relationships
             (character_a_id, character_b_id, relation_type, visibility,
              valid_from_chapter, source_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                character_a_id, character_b_id, relation_type, visibility,
                valid_from_chapter, source_ref
            ],
        )?;
        self.db.last_insert_rowid()
    }

    pub fn get_active_relationships(
        &self,
        character_id: i64,
        chapter_title: &str,
    ) -> rusqlite::Result<Vec<RelationshipSummary>> {
        let mut stmt = self.db.prepare(
            "SELECT id, character_a_id, character_b_id, relation_type, visibility,
                    valid_from_chapter, valid_to_chapter, source_ref
             FROM character_relationships
             WHERE (character_a_id = ?1 OR character_b_id = ?1)
               AND valid_from_chapter <= ?2
               AND (valid_to_chapter = '' OR valid_to_chapter >= ?2)
             ORDER BY valid_from_chapter DESC"
        )?;
        let rows = stmt.query_map(rusqlite::params![character_id, chapter_title], |row| {
            Ok(RelationshipSummary {
                id: row.get(0)?,
                character_a_id: row.get(1)?,
                character_b_id: row.get(2)?,
                relation_type: row.get(3)?,
                visibility: row.get(4)?,
                valid_from_chapter: row.get(5)?,
                valid_to_chapter: row.get(6)?,
                source_ref: row.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }

    pub fn close_relationship(&self, rel_id: i64, valid_to_chapter: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE character_relationships SET valid_to_chapter = ?1 WHERE id = ?2",
            rusqlite::params![valid_to_chapter, rel_id],
        )?;
        Ok(())
    }

    pub fn close_active_relationships_for_character(
        &self,
        character_id: i64,
        valid_to_chapter: &str,
    ) -> rusqlite::Result<usize> {
        let count = self.db.execute(
            "UPDATE character_relationships
             SET valid_to_chapter = ?1
             WHERE (character_a_id = ?2 OR character_b_id = ?2)
               AND valid_to_chapter = ''",
            rusqlite::params![valid_to_chapter, character_id],
        )?;
        Ok(count)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipSummary {
    pub id: i64,
    pub character_a_id: i64,
    pub character_b_id: i64,
    pub relation_type: String,
    pub visibility: String,
    pub valid_from_chapter: String,
    pub valid_to_chapter: String,
    pub source_ref: String,
}
```

- [ ] **Step 2: Register in memory.rs**

After `include!("memory/character_state_methods.in.rs");`:
```rust
include!("memory/relationship_methods.in.rs");
```

- [ ] **Step 3: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/memory/
git commit -m "feat: add character relationship memory methods"
```

---

### Task 5: Promise Subject Binding

**Files:**
- Modify: `src-tauri/src/writer_agent/memory/promises_methods.in.rs`

- [ ] **Step 1: Add bind_promise_subject method**

Find the end of the `impl WriterMemory` block in `promises_methods.in.rs` and add:

```rust
pub fn bind_promise_subject(
    &self,
    promise_id: i64,
    subject_ids: &[i64],
    subject_type: &str,
) -> rusqlite::Result<()> {
    let ids_json = serde_json::to_string(subject_ids).unwrap_or_default();
    self.db.execute(
        "UPDATE plot_promises SET subject_ids_json = ?1, subject_type = ?2 WHERE id = ?3",
        rusqlite::params![ids_json, subject_type, promise_id],
    )?;
    // Also sync to related_entities_json for backward compatibility
    let existing_json: String = self.db.query_row(
        "SELECT related_entities_json FROM plot_promises WHERE id = ?1",
        rusqlite::params![promise_id],
        |row| row.get(0),
    )?;
    let mut related: Vec<String> = serde_json::from_str(&existing_json).unwrap_or_default();
    // Append subject ids as entity references if not already present
    for id in subject_ids {
        let ref_str = format!("character:{}", id);
        if !related.contains(&ref_str) {
            related.push(ref_str);
        }
    }
    let updated = serde_json::to_string(&related).unwrap_or_default();
    self.db.execute(
        "UPDATE plot_promises SET related_entities_json = ?1 WHERE id = ?2",
        rusqlite::params![updated, promise_id],
    )?;
    Ok(())
}

pub fn get_promises_by_subject(
    &self,
    subject_id: i64,
    subject_type: &str,
) -> rusqlite::Result<Vec<PlotPromiseSummary>> {
    let pattern = format!("\"{}\"", subject_id);
    let mut stmt = self.db.prepare(
        "SELECT id, kind, title, description, introduced_chapter, last_seen_chapter,
                expected_payoff, status, priority, blocked_reason, promoted, core,
                related_entities_json, subject_ids_json, subject_type, created_at
         FROM plot_promises
         WHERE (subject_type = ?1 AND subject_ids_json LIKE ?2)
            OR related_entities_json LIKE ?2
         ORDER BY priority DESC"
    )?;
    let rows = stmt.query_map(rusqlite::params![subject_type, pattern], |row| {
        Ok(PlotPromiseSummary {
            id: row.get(0)?,
            kind: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            introduced_chapter: row.get(4)?,
            last_seen_chapter: row.get(5)?,
            expected_payoff: row.get(6)?,
            status: row.get(7)?,
            priority: row.get(8)?,
            blocked_reason: row.get(9)?,
            promoted: row.get::<_, i32>(10)? != 0,
            core: row.get::<_, i32>(11)? != 0,
            related_entities: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
            subject_ids: serde_json::from_str(&row.get::<_, Option<String>>(13)?.unwrap_or_default()).unwrap_or_default(),
            subject_type: row.get(14)?,
            created_at: row.get(15)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
}
```

Note: If `PlotPromiseSummary` doesn't have `subject_ids` and `subject_type` fields yet, add them:
```rust
pub subject_ids: Vec<i64>,
pub subject_type: String,
```

- [ ] **Step 2: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/writer_agent/memory/
git commit -m "feat: add promise subject binding methods"
```

---

### Task 6: Canon Entities Demotion

**Files:**
- Modify: `src-tauri/src/writer_agent/memory/canon_methods.in.rs`

- [ ] **Step 1: Block character writes to canon_entities**

In `upsert_canon_entity`, add a guard at the top:

```rust
pub fn upsert_canon_entity(
    &self,
    kind: &str,
    name: &str,
    aliases: &[String],
    summary: &str,
    attributes: &serde_json::Value,
    confidence: f64,
) -> rusqlite::Result<()> {
    // Block character writes — characters table is now authoritative
    if kind == "character" {
        return Err(rusqlite::Error::InvalidParameterName(
            "use upsert_character for character entities".to_string()
        ));
    }
    // ... existing upsert logic ...
}
```

- [ ] **Step 2: Run tests to verify no breakage**

Run: `cargo test -p agent-writer 2>&1 | grep "test result"`
Expected: 247 tests still passing (adjust if any tests directly insert character-type canon_entities)

- [ ] **Step 3: Fix any broken tests**

If existing tests call `upsert_canon_entity("character", ...)`, update them to use `upsert_character` instead. Run grep:
```bash
grep -rn 'upsert_canon_entity.*"character"' src-tauri/src/
```
For each match, replace with `upsert_character` and remove the `kind` parameter.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/memory/
git commit -m "feat: demote canon_entities — block character writes, route to characters table"
```

---

### Task 7: Typed Operations — 4 New Variants

**Files:**
- Modify: `src-tauri/src/writer_agent/operation.rs`

- [ ] **Step 1: Add 4 new WriterOperation variants**

After the existing `PromiseAbandon` variant, add:

```rust
CharacterUpsert {
    name: String,
    aliases: Vec<String>,
    role_type: String,
    summary: String,
},
CharacterStateUpsert {
    character_id: i64,
    valid_from_chapter: String,
    core_commitments: serde_json::Value,
    goal_state: serde_json::Value,
    identity_state: serde_json::Value,
    source_ref: String,
},
RelationshipUpsert {
    character_a_id: i64,
    character_b_id: i64,
    relation_type: String,
    visibility: String,
    valid_from_chapter: String,
    source_ref: String,
},
PromiseBindSubject {
    promise_id: i64,
    subject_ids: Vec<i64>,
    subject_type: String,
},
```

- [ ] **Step 2: Add operation_kind_label entries**

Find `operation_kind_label` function and add:

```rust
WriterOperation::CharacterUpsert { .. } => "character.upsert",
WriterOperation::CharacterStateUpsert { .. } => "character_state.upsert",
WriterOperation::RelationshipUpsert { .. } => "relationship.upsert",
WriterOperation::PromiseBindSubject { .. } => "promise.bind_subject",
```

- [ ] **Step 3: Add to `operation_affected_scope` and `operation_is_write_capable`**

Find these functions and add the new variants. Character ops affect "canon", relationship ops affect "canon", promise.bind_subject affects "promise". All are write-capable.

```rust
WriterOperation::CharacterUpsert { .. }
| WriterOperation::CharacterStateUpsert { .. }
| WriterOperation::RelationshipUpsert { .. } => vec!["canon"],
WriterOperation::PromiseBindSubject { .. } => vec!["promise"],
```

- [ ] **Step 4: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/writer_agent/operation.rs
git commit -m "feat: add Character, CharacterState, Relationship, PromiseBindSubject typed ops"
```

---

### Task 8: Settlement Delta Types Extension

**Files:**
- Modify: `src-tauri/src/chapter_generation/types_and_utils.in.rs`

- [ ] **Step 1: Add entity delta types**

After `ChapterArcDeltaEntry`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CharacterStateDeltaEntry {
    pub character_name: String,
    pub chapter_title: String,
    pub action: String,
    #[serde(default)]
    pub core_commitments: Vec<String>,
    #[serde(default)]
    pub goal_state: serde_json::Value,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipDeltaEntry {
    pub character_a_name: String,
    pub character_b_name: String,
    pub action: String,
    pub relation_type: String,
    pub visibility: String,
    pub chapter_title: String,
    pub source_ref: String,
}
```

- [ ] **Step 2: Add fields to ChapterSettlementDelta**

In `ChapterSettlementDelta`, add after `book_state_delta`:

```rust
#[serde(default)]
pub character_state_deltas: Vec<CharacterStateDeltaEntry>,
#[serde(default)]
pub relationship_deltas: Vec<RelationshipDeltaEntry>,
```

- [ ] **Step 3: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/chapter_generation/
git commit -m "feat: add entity-scoped delta types to ChapterSettlementDelta"
```

---

### Task 9: Settlement Extraction — Entity Deltas

**Files:**
- Modify: `src-tauri/src/chapter_generation/settlement.in.rs`

- [ ] **Step 1: Extract character state deltas**

In `build_settlement_extraction`, after building `book_state_candidates`, add:

```rust
let character_state_deltas: Vec<CharacterStateDeltaEntry> = chapter_result
    .character_progress
    .iter()
    .filter_map(|prog| {
        let parts: Vec<&str> = prog.splitn(2, ':').collect();
        if parts.len() < 2 { return None; }
        let name = parts[0].trim();
        let detail = parts[1].trim();
        Some(CharacterStateDeltaEntry {
            character_name: name.to_string(),
            chapter_title: chapter_result.chapter_title.clone(),
            action: "upserted".to_string(),
            core_commitments: vec![detail.to_string()],
            goal_state: serde_json::json!({}),
            source_ref: chapter_result.source_ref.clone(),
        })
    })
    .collect();

let relationship_deltas: Vec<RelationshipDeltaEntry> = chapter_result
    .new_conflicts
    .iter()
    .filter(|c| {
        let lower = c.to_lowercase();
        lower.contains("关系") || lower.contains("盟友") || lower.contains("敌对")
            || lower.contains("决裂") || lower.contains("结盟")
    })
    .filter_map(|conflict| {
        Some(RelationshipDeltaEntry {
            character_a_name: String::new(),
            character_b_name: String::new(),
            action: "changed".to_string(),
            relation_type: "complex".to_string(),
            visibility: "public".to_string(),
            chapter_title: chapter_result.chapter_title.clone(),
            source_ref: chapter_result.source_ref.clone(),
        })
    })
    .collect();
```

- [ ] **Step 2: Include entity deltas in settlement delta**

In `build_basic_chapter_settlement_delta`, add the new fields to the `ChapterSettlementDelta` constructor:

```rust
character_state_deltas: character_state_deltas.clone(),
relationship_deltas: relationship_deltas.clone(),
```

- [ ] **Step 3: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/chapter_generation/
git commit -m "feat: extract entity-scoped deltas in settlement"
```

---

### Task 10: Settlement Apply — Entity Deltas

**Files:**
- Modify: `src-tauri/src/writer_agent/settlement_apply.rs`

- [ ] **Step 1: Apply character_state_deltas**

In `apply_chapter_settlement_delta`, after the promise update loop, add:

```rust
// Apply character state deltas
let mut character_state_applied = 0usize;
for delta in &delta.character_state_deltas {
    if let Ok(Some(character)) = memory.get_character_by_name(&delta.character_name) {
        memory.close_active_states_for_character(character.id, &delta.chapter_title)
            .map_err(|e| e.to_string())?;
        memory.upsert_character_state(
            character.id,
            &delta.chapter_title,
            &serde_json::json!(delta.core_commitments),
            &delta.goal_state,
            &serde_json::json!({}),
            &[],
            &delta.source_ref,
        ).map_err(|e| e.to_string())?;
        character_state_applied += 1;
    }
}

// Apply relationship deltas
let mut relationship_applied = 0usize;
for delta in &delta.relationship_deltas {
    if !delta.character_a_name.is_empty() && !delta.character_b_name.is_empty() {
        if let (Ok(Some(a)), Ok(Some(b))) = (
            memory.get_character_by_name(&delta.character_a_name),
            memory.get_character_by_name(&delta.character_b_name),
        ) {
            memory.upsert_relationship(
                a.id, b.id,
                &delta.relation_type,
                &delta.visibility,
                &delta.chapter_title,
                &delta.source_ref,
            ).map_err(|e| e.to_string())?;
            relationship_applied += 1;
        }
    }
}
```

- [ ] **Step 2: Add entity counts to apply result**

In the `ChapterSettlementApplyResult`, add:

```rust
pub character_state_applied: usize,
pub relationship_applied: usize,
```

And set them in the return value.

- [ ] **Step 3: Update callers**

Run: `cargo check -p agent-writer` and fix any compilation errors in callers that construct `ChapterSettlementApplyResult`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/
git commit -m "feat: apply entity-scoped deltas in settlement apply"
```

---

### Task 11: Promise Planner — Subject Pressure Scoring

**Files:**
- Modify: `src-tauri/src/writer_agent/promise_planner.rs`

- [ ] **Step 1: Add subject_pressure helper**

```rust
fn promise_subject_pressure(
    promise: &PlotPromiseSummary,
    memory: &WriterMemory,
    current_chapter: &str,
) -> f64 {
    let mut pressure = promise.priority as f64;

    // Protagonist-subject promises: ×2 weight
    if promise.subject_type == "character" {
        for subject_id in &promise.subject_ids {
            if let Ok(Some(character)) = memory.get_character_by_id(*subject_id) {
                if character.role_type == "protagonist" {
                    pressure *= 2.0;
                }
            }
        }
    }

    // Core-relationship-subject promises: ×1.5 weight
    if promise.subject_type == "relationship" && promise.core {
        pressure *= 1.5;
    }

    // Stale debt: +0.1 per chapter since last_seen
    if !promise.last_seen_chapter.is_empty() {
        let last_chapter_num = extract_chapter_number(&promise.last_seen_chapter);
        let current_num = extract_chapter_number(current_chapter);
        let gap = current_num.saturating_sub(last_chapter_num);
        if gap > 5 {
            pressure += (gap - 5) as f64 * 0.1;
        }
    }

    pressure
}

fn extract_chapter_number(chapter: &str) -> i64 {
    chapter.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or(0)
}
```

- [ ] **Step 2: Use subject_pressure in promise ranking**

Find the promise prioritization logic and multiply existing score by `promise_subject_pressure()`. The fallback for promises without subject binding uses the existing scoring unchanged.

- [ ] **Step 3: Compile**

Run: `cargo check -p agent-writer`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/promise_planner.rs
git commit -m "feat: add subject-pressure scoring to promise planner"
```

---

### Task 12: TodayFiveSummary — Entity-Driven Slots

**Files:**
- Modify: `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

- [ ] **Step 1: Add entity stats to guard slot**

In `today_five_summary()`, add character/relationship counts to the guard item:

```rust
let character_count = memory
    .list_characters(None)
    .unwrap_or_default()
    .len();
let active_relationship_count = memory
    .get_active_relationships(0, &current_chapter.as_deref().unwrap_or("Chapter-1"))
    .unwrap_or_default()
    .len();
let guard_detail = format!(
    "{} characters, {} active relationships. {}",
    character_count,
    active_relationship_count,
    guard_detail
);
```

- [ ] **Step 2: Sort promise slot by subject pressure**

In the promise item, rank by `promise_subject_pressure` descending:

```rust
let ranked_by_pressure = {
    let mut sorted = ledger.open_promises.clone();
    sorted.sort_by(|a, b| {
        let pa = promise_subject_pressure(a, &self.memory, current_chapter.as_deref().unwrap_or("Chapter-1"));
        let pb = promise_subject_pressure(b, &self.memory, current_chapter.as_deref().unwrap_or("Chapter-1"));
        pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted
};
let top_promise = ranked_by_pressure.first();
```

Annotate the promise detail with character name if subject is bound:
```rust
detail: top_promise.and_then(|p| {
    if p.subject_type == "character" && !p.subject_ids.is_empty() {
        memory.get_character_by_id(p.subject_ids[0]).ok().flatten()
            .map(|c| format!("{} → {} ({} 的承诺)", p.description, p.expected_payoff, c.name))
    } else {
        Some(format!("{} → {}", p.description, p.expected_payoff))
    }
}).unwrap_or_else(|| "No open promise".to_string()),
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-writer 2>&1 | grep "test result"`
Expected: all passing

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/kernel/snapshots/
git commit -m "feat: add entity-driven slots to TodayFiveSummary"
```

---

### Task 13-16: 4 New Evals

**Files:**
- Create: `agent-evals/src/evals/character_state_versioning.rs`
- Create: `agent-evals/src/evals/relationship_validity.rs`
- Create: `agent-evals/src/evals/promise_subject.rs`
- Create: `agent-evals/src/evals/entity_settlement.rs`
- Modify: `agent-evals/src/evals.rs`
- Modify: `agent-evals/src/main.rs`

- [ ] **Step 1: Character state versioning eval**

Create `agent-evals/src/evals/character_state_versioning.rs`:

```rust
#![allow(unused_imports)]
use crate::fixtures::*;
use std::path::Path;
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn run_character_state_versioning_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let char_id = memory.upsert_character("林墨", &[], "protagonist", "主角").unwrap();
    
    // Create two state versions
    memory.upsert_character_state(char_id, "Chapter-1",
        &serde_json::json!(["protect the ring"]),
        &serde_json::json!({"goal": "find the truth"}),
        &serde_json::json!({"identity": "wanderer"}),
        &[], "settlement:Chapter-1").unwrap();
    
    memory.close_active_states_for_character(char_id, "Chapter-3").unwrap();
    memory.upsert_character_state(char_id, "Chapter-3",
        &serde_json::json!(["reveal the secret"]),
        &serde_json::json!({"goal": "confront the enemy"}),
        &serde_json::json!({"identity": "avenger"}),
        &[], "settlement:Chapter-3").unwrap();
    
    // Query by chapter — should return correct version
    let state_ch1 = memory.get_active_state(char_id, "Chapter-1").unwrap();
    let state_ch3 = memory.get_active_state(char_id, "Chapter-3").unwrap();
    
    let versioning_works = state_ch1.is_some() && state_ch3.is_some()
        && state_ch1.unwrap().valid_from_chapter == "Chapter-1"
        && state_ch3.unwrap().valid_from_chapter == "Chapter-3";
    
    EvalResult::pass_if(versioning_works,
        format!("characterStateVersioning={}", versioning_works))
}
```

- [ ] **Step 2: Relationship validity window eval**

Create `agent-evals/src/evals/relationship_validity.rs`:

```rust
pub fn run_relationship_validity_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let a = memory.upsert_character("张三", &[], "protagonist", "hero").unwrap();
    let b = memory.upsert_character("李四", &[], "supporting", "rival").unwrap();
    
    // Create relationship at Chapter-1
    let rel1 = memory.upsert_relationship(a, b, "enemy", "public", "Chapter-1", "src1").unwrap();
    let active = memory.get_active_relationships(a, "Chapter-3").unwrap();
    let found = active.iter().any(|r| r.id == rel1);
    
    // Close at Chapter-5
    memory.close_relationship(rel1, "Chapter-5").unwrap();
    let active_after = memory.get_active_relationships(a, "Chapter-6").unwrap();
    let not_found = !active_after.iter().any(|r| r.id == rel1);
    
    // Reopen at Chapter-7 (new row)
    let rel2 = memory.upsert_relationship(a, b, "ally", "public", "Chapter-7", "src2").unwrap();
    let reopened = rel2 != rel1;
    
    EvalResult::pass_if(found && not_found && reopened,
        format!("validityWindow={} closed={} reopened={}", found, not_found, reopened))
}
```

- [ ] **Step 3: Promise subject binding eval**

Create `agent-evals/src/evals/promise_subject.rs`:

```rust
pub fn run_promise_subject_binding_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory.ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "").unwrap();
    let char_id = memory.upsert_character("林墨", &[], "protagonist", "lead").unwrap();
    let promise_id = memory.add_promise("plot_promise", "test ring", "find the ring",
        "Chapter-1", "Chapter-5", 4).unwrap();
    
    memory.bind_promise_subject(promise_id, &[char_id], "character").unwrap();
    let promises = memory.get_promises_by_subject(char_id, "character").unwrap();
    let bound = promises.iter().any(|p| p.id == promise_id);
    
    EvalResult::pass_if(bound,
        format!("subjectBound={} promiseCount={}", bound, promises.len()))
}
```

- [ ] **Step 4: Entity-scoped settlement apply eval**

Create `agent-evals/src/evals/entity_settlement.rs`:

```rust
pub fn run_entity_settlement_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory.ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "").unwrap();
    let char_id = memory.upsert_character("林墨", &[], "protagonist", "hero").unwrap();
    
    let delta = ChapterSettlementDelta {
        character_state_deltas: vec![CharacterStateDeltaEntry {
            character_name: "林墨".to_string(),
            chapter_title: "Chapter-2".to_string(),
            action: "upserted".to_string(),
            core_commitments: vec!["sworn to protect".to_string()],
            goal_state: serde_json::json!({"goal": "revenge"}),
            source_ref: "test".to_string(),
        }],
        ..Default::default()
    };
    
    let result = apply_chapter_settlement_delta(&memory, "eval", &delta).unwrap();
    let state = memory.get_active_state(char_id, "Chapter-2").unwrap();
    
    EvalResult::pass_if(result.applied && state.is_some(),
        format!("entitySettlementApplied={} stateExists={}", result.applied, state.is_some()))
}
```

- [ ] **Step 5: Register all 4 evals**

In `agent-evals/src/evals.rs`, add:
```rust
mod character_state_versioning;
mod relationship_validity;
mod promise_subject;
mod entity_settlement;
pub use character_state_versioning::*;
pub use relationship_validity::*;
pub use promise_subject::*;
pub use entity_settlement::*;
```

In `agent-evals/src/main.rs`, add:
```rust
results.push(run_character_state_versioning_eval());
results.push(run_relationship_validity_eval());
results.push(run_promise_subject_binding_eval());
results.push(run_entity_settlement_eval());
```

- [ ] **Step 6: Verify all evals pass**

Run: `cargo run -p agent-evals 2>&1 | grep -E "character_state|relationship_validity|promise_subject|entity_settlement"`
Expected: 4 [PASS] lines

- [ ] **Step 7: Commit**

```bash
git add agent-evals/
git commit -m "feat: add 4 entity state kernel evals"
```

---

### Task 17: Verification Baseline Update

**Files:**
- Modify: `scripts/verification-baseline.cjs`
- Modify: `README.md`

- [ ] **Step 1: Update eval count in baseline**

In `verification-baseline.cjs`, change:
```
["cargo run -p agent-evals", "268/268 evals passing"],
```
to:
```
["cargo run -p agent-evals", "272/272 evals passing"],
```

- [ ] **Step 2: Run full verify**

```bash
npm run verify
```

- [ ] **Step 3: Update baseline**

```bash
npm run baseline
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: update verification baseline after Sprint A"
```
