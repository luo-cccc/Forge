# Sprint B: Knowledge & Identity State — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add knowledge/identity state layer — distinguish objective truth, character knowledge, false beliefs, and identity reveals.

**Architecture:** 4 new SQLite tables (knowledge_items, knowledge_ownership, identity_layers, reveal_events), version v17→v18 migration, 4 memory method files, 4 typed ops, settlement delta expansion, diagnostics wiring for bad payoff / canon drift / OOC based on knowledge visibility, TodayFive extension.

**Tech Stack:** Rust (src-tauri, agent-evals), SQLite

---

### Task 1: Schema Migration — v18

**Files:**
- Modify: `src-tauri/src/writer_agent/memory/schema.in.rs`
- Modify: `src-tauri/src/writer_agent/memory/tracing_migrate.in.rs`

- [ ] **Step 1: Read existing schema files**

Read `schema.in.rs` to understand the v17 schema format and the `SCHEMA_VERSION` constant. Read `tracing_migrate.in.rs` to understand the migration pattern.

- [ ] **Step 2: Bump version and add tables**

In `schema.in.rs`, bump `SCHEMA_VERSION` from `17` to `18`. Add 4 new tables to the `SCHEMA` string:

```sql
CREATE TABLE IF NOT EXISTS knowledge_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    truth_state TEXT NOT NULL DEFAULT 'objective',
    source_ref TEXT DEFAULT '',
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS knowledge_ownership (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    knowledge_id INTEGER NOT NULL,
    holder_type TEXT NOT NULL,
    holder_id INTEGER NOT NULL,
    knowledge_mode TEXT NOT NULL DEFAULT 'aware',
    valid_from_chapter TEXT NOT NULL,
    valid_to_chapter TEXT DEFAULT '',
    source_ref TEXT DEFAULT '',
    FOREIGN KEY (knowledge_id) REFERENCES knowledge_items(id)
);

CREATE TABLE IF NOT EXISTS identity_layers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    public_identity TEXT DEFAULT '',
    private_identity TEXT DEFAULT '',
    revealed_to_json TEXT DEFAULT '[]',
    valid_from_chapter TEXT NOT NULL,
    valid_to_chapter TEXT DEFAULT '',
    FOREIGN KEY (character_id) REFERENCES characters(id)
);

CREATE TABLE IF NOT EXISTS reveal_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER NOT NULL,
    reveal_type TEXT NOT NULL,
    revealed_to TEXT NOT NULL,
    chapter TEXT NOT NULL,
    source_ref TEXT DEFAULT ''
);
```

- [ ] **Step 3: Add migration block**

In `tracing_migrate.in.rs`, add migration for v18: `CREATE TABLE IF NOT EXISTS` for all 4 tables.

- [ ] **Step 4: Verify**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```
Expected: compiles, 247 tests passing.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/writer_agent/memory/schema.in.rs src-tauri/src/writer_agent/memory/tracing_migrate.in.rs
git commit -m "feat: add knowledge_items, knowledge_ownership, identity_layers, reveal_events tables (v18)"
```

---

### Task 2: Knowledge Items Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/knowledge_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create knowledge_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_knowledge_item(&self, topic: &str, truth_state: &str, source_ref: &str) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT OR REPLACE INTO knowledge_items (topic, truth_state, source_ref, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![topic, truth_state, source_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_knowledge_item(&self, topic: &str) -> rusqlite::Result<Option<KnowledgeItemSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, topic, truth_state, source_ref FROM knowledge_items WHERE topic = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![topic], |row| {
            Ok(KnowledgeItemSummary { id: row.get(0)?, topic: row.get(1)?, truth_state: row.get(2)?, source_ref: row.get(3)? })
        })?;
        Ok(rows.next().and_then(|r| r.ok()))
    }

    pub fn list_knowledge_items(&self, truth_state_filter: Option<&str>) -> rusqlite::Result<Vec<KnowledgeItemSummary>> {
        let (query, params) = if let Some(ts) = truth_state_filter {
            ("SELECT id, topic, truth_state, source_ref FROM knowledge_items WHERE truth_state = ?1 ORDER BY topic", vec![ts.to_string()])
        } else {
            ("SELECT id, topic, truth_state, source_ref FROM knowledge_items ORDER BY topic", vec![])
        };
        let mut stmt = self.conn.prepare(query)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(KnowledgeItemSummary { id: row.get(0)?, topic: row.get(1)?, truth_state: row.get(2)?, source_ref: row.get(3)? })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeItemSummary {
    pub id: i64,
    pub topic: String,
    pub truth_state: String,
    pub source_ref: String,
}
```

- [ ] **Step 2: Register in memory.rs**

Add `include!("memory/knowledge_methods.in.rs");` after the relationship methods include.

- [ ] **Step 3: Compile and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add knowledge item memory methods"
```

---

### Task 3: Knowledge Ownership Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/ownership_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create ownership_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_knowledge_ownership(
        &self, knowledge_id: i64, holder_type: &str, holder_id: i64,
        knowledge_mode: &str, valid_from_chapter: &str, source_ref: &str,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO knowledge_ownership (knowledge_id, holder_type, holder_id, knowledge_mode, valid_from_chapter, source_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![knowledge_id, holder_type, holder_id, knowledge_mode, valid_from_chapter, source_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_knowledge_by_holder(&self, holder_type: &str, holder_id: i64, chapter_title: &str) -> rusqlite::Result<Vec<KnowledgeOwnershipSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT ko.id, ki.topic, ko.knowledge_mode, ko.valid_from_chapter, ko.valid_to_chapter
             FROM knowledge_ownership ko JOIN knowledge_items ki ON ko.knowledge_id = ki.id
             WHERE ko.holder_type = ?1 AND ko.holder_id = ?2
               AND ko.valid_from_chapter <= ?3 AND (ko.valid_to_chapter = '' OR ko.valid_to_chapter >= ?3)
             ORDER BY ko.valid_from_chapter DESC"
        )?;
        let rows = stmt.query_map(rusqlite::params![holder_type, holder_id, chapter_title], |row| {
            Ok(KnowledgeOwnershipSummary {
                id: row.get(0)?, topic: row.get(1)?, knowledge_mode: row.get(2)?,
                valid_from_chapter: row.get(3)?, valid_to_chapter: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }

    pub fn close_ownership(&self, ownership_id: i64, valid_to_chapter: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE knowledge_ownership SET valid_to_chapter = ?1 WHERE id = ?2",
            rusqlite::params![valid_to_chapter, ownership_id],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeOwnershipSummary {
    pub id: i64, pub topic: String, pub knowledge_mode: String,
    pub valid_from_chapter: String, pub valid_to_chapter: String,
}
```

- [ ] **Step 2: Register and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add knowledge ownership memory methods"
```

---

### Task 4: Identity Layer Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/identity_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create identity_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_identity_layer(
        &self, character_id: i64, public_identity: &str, private_identity: &str,
        revealed_to: &[String], valid_from_chapter: &str,
    ) -> rusqlite::Result<i64> {
        let revealed_json = serde_json::to_string(revealed_to).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO identity_layers (character_id, public_identity, private_identity, revealed_to_json, valid_from_chapter)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![character_id, public_identity, private_identity, revealed_json, valid_from_chapter],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_active_identity(&self, character_id: i64, chapter_title: &str) -> rusqlite::Result<Option<IdentityLayerSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character_id, public_identity, private_identity, revealed_to_json, valid_from_chapter, valid_to_chapter
             FROM identity_layers WHERE character_id = ?1
               AND valid_from_chapter <= ?2 AND (valid_to_chapter = '' OR valid_to_chapter >= ?2)
             ORDER BY valid_from_chapter DESC LIMIT 1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![character_id, chapter_title], |row| {
            Ok(IdentityLayerSummary {
                id: row.get(0)?, character_id: row.get(1)?, public_identity: row.get(2)?,
                private_identity: row.get(3)?,
                revealed_to: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                valid_from_chapter: row.get(5)?, valid_to_chapter: row.get(6)?,
            })
        })?;
        Ok(rows.next().and_then(|r| r.ok()))
    }

    pub fn close_identity_layer(&self, layer_id: i64, valid_to_chapter: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE identity_layers SET valid_to_chapter = ?1 WHERE id = ?2",
            rusqlite::params![valid_to_chapter, layer_id],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityLayerSummary {
    pub id: i64, pub character_id: i64, pub public_identity: String,
    pub private_identity: String, pub revealed_to: Vec<String>,
    pub valid_from_chapter: String, pub valid_to_chapter: String,
}
```

- [ ] **Step 2: Register and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add identity layer memory methods"
```

---

### Task 5: Reveal Event Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/reveal_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create reveal_methods.in.rs**

```rust
impl WriterMemory {
    pub fn record_reveal_event(
        &self, subject_id: i64, reveal_type: &str, revealed_to: &str, chapter: &str, source_ref: &str,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO reveal_events (subject_id, reveal_type, revealed_to, chapter, source_ref)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![subject_id, reveal_type, revealed_to, chapter, source_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_reveals_by_chapter(&self, chapter_title: &str) -> rusqlite::Result<Vec<RevealEventSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, subject_id, reveal_type, revealed_to, chapter, source_ref
             FROM reveal_events WHERE chapter = ?1 ORDER BY id"
        )?;
        let rows = stmt.query_map(rusqlite::params![chapter_title], |row| {
            Ok(RevealEventSummary {
                id: row.get(0)?, subject_id: row.get(1)?, reveal_type: row.get(2)?,
                revealed_to: row.get(3)?, chapter: row.get(4)?, source_ref: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }

    pub fn list_reveals_by_subject(&self, subject_id: i64) -> rusqlite::Result<Vec<RevealEventSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, subject_id, reveal_type, revealed_to, chapter, source_ref
             FROM reveal_events WHERE subject_id = ?1 ORDER BY id"
        )?;
        let rows = stmt.query_map(rusqlite::params![subject_id], |row| {
            Ok(RevealEventSummary {
                id: row.get(0)?, subject_id: row.get(1)?, reveal_type: row.get(2)?,
                revealed_to: row.get(3)?, chapter: row.get(4)?, source_ref: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealEventSummary {
    pub id: i64, pub subject_id: i64, pub reveal_type: String,
    pub revealed_to: String, pub chapter: String, pub source_ref: String,
}
```

- [ ] **Step 2: Register and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add reveal event memory methods"
```

---

### Task 6: Typed Operations — 4 Knowledge/Identity Ops

**Files:**
- Modify: `src-tauri/src/writer_agent/operation.rs`
- Modify: `src-tauri/src/writer_agent/kernel/helpers.rs`
- Modify: `src-tauri/src/writer_agent/kernel/ops.rs`

- [ ] **Step 1: Add 4 new WriterOperation variants**

```rust
KnowledgeUpsert { topic: String, truth_state: String },
KnowledgeOwnershipUpsert { knowledge_id: i64, holder_type: String, holder_id: i64, knowledge_mode: String, valid_from_chapter: String },
IdentityLayerUpsert { character_id: i64, public_identity: String, private_identity: String, valid_from_chapter: String },
RevealEventRecord { subject_id: i64, reveal_type: String, revealed_to: String, chapter: String },
```

- [ ] **Step 2: Add operation_kind_label, affected_scope, write_capable entries**

In `helpers.rs`:
```rust
WriterOperation::KnowledgeUpsert { .. } => "knowledge.upsert",
WriterOperation::KnowledgeOwnershipUpsert { .. } => "knowledge_ownership.upsert",
WriterOperation::IdentityLayerUpsert { .. } => "identity_layer.upsert",
WriterOperation::RevealEventRecord { .. } => "reveal_event.record",
```

All 4 are write-capable. `Knowledge*` → `canon` scope, `Identity*` → `canon` scope, `RevealEvent*` → `canon` scope.

- [ ] **Step 3: Add execution handlers in ops.rs**

```rust
WriterOperation::KnowledgeUpsert { topic, truth_state } => {
    memory.upsert_knowledge_item(topic, truth_state, "writer_operation")?;
    // ...
}
WriterOperation::KnowledgeOwnershipUpsert { knowledge_id, holder_type, holder_id, knowledge_mode, valid_from_chapter } => {
    memory.upsert_knowledge_ownership(*knowledge_id, holder_type, *holder_id, knowledge_mode, valid_from_chapter, "writer_operation")?;
    // ...
}
WriterOperation::IdentityLayerUpsert { character_id, public_identity, private_identity, valid_from_chapter } => {
    memory.upsert_identity_layer(*character_id, public_identity, private_identity, &[], valid_from_chapter)?;
    // ...
}
WriterOperation::RevealEventRecord { subject_id, reveal_type, revealed_to, chapter } => {
    memory.record_reveal_event(*subject_id, reveal_type, revealed_to, chapter, "writer_operation")?;
    // ...
}
```

- [ ] **Step 4: Compile and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add Knowledge, Identity, Reveal typed ops"
```

---

### Task 7: Settlement Delta — Knowledge & Identity Types

**Files:**
- Modify: `src-tauri/src/chapter_generation/types_and_utils.in.rs`

- [ ] **Step 1: Add delta types**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDeltaEntry {
    pub topic: String,
    pub truth_state: String,
    pub holder_type: String,
    pub holder_id: i64,
    pub knowledge_mode: String,
    pub chapter_title: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDeltaEntry {
    pub character_name: String,
    pub public_identity: String,
    pub private_identity: String,
    pub revealed_to: Vec<String>,
    pub chapter_title: String,
    pub source_ref: String,
}
```

- [ ] **Step 2: Add fields to ChapterSettlementDelta**

```rust
#[serde(default)]
pub knowledge_deltas: Vec<KnowledgeDeltaEntry>,
#[serde(default)]
pub identity_deltas: Vec<IdentityDeltaEntry>,
```

- [ ] **Step 3: Compile and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add knowledge and identity delta types"
```

---

### Task 8: Settlement Extraction — Knowledge & Identity

**Files:**
- Modify: `src-tauri/src/chapter_generation/settlement.in.rs`

- [ ] **Step 1: Extract knowledge deltas**

In `build_settlement_extraction`, derive `knowledge_deltas` from chapter_result content. Simple heuristic: if text contains reveal cues ("知道", "发现", "揭开"), create a knowledge delta entry. Match against existing knowledge_items topics if present.

- [ ] **Step 2: Extract identity deltas**

Derive `identity_deltas` from character state changes that reference identity ("身份", "真名", "伪装").

- [ ] **Step 3: Pass deltas to settlement**

Include `knowledge_deltas` and `identity_deltas` in `build_basic_chapter_settlement_delta`.

- [ ] **Step 4: Compile and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: extract knowledge and identity deltas in settlement"
```

---

### Task 9: Settlement Apply — Knowledge & Identity

**Files:**
- Modify: `src-tauri/src/writer_agent/settlement_apply.rs`

- [ ] **Step 1: Apply knowledge deltas**

After the relationship apply loop, for each `KnowledgeDeltaEntry`: upsert knowledge_item, then upsert ownership for the holder, record reveal if mode is 'aware' (transition from concealed).

- [ ] **Step 2: Apply identity deltas**

For each `IdentityDeltaEntry`: resolve character by name, close old identity layer, upsert new one.

- [ ] **Step 3: Add counts to apply result**

```rust
pub knowledge_applied: usize,
pub identity_applied: usize,
```

- [ ] **Step 4: Compile and commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: apply knowledge and identity deltas in settlement"
```

---

### Task 10: Diagnostics — Knowledge Visibility Integration

**Files:**
- Modify: `src-tauri/src/writer_agent/diagnostics/core.in.rs`

- [ ] **Step 1: Add knowledge-based payoff check**

In the diagnostics engine, add a check: if a payoff event is detected, verify that the affected characters are in `aware` or `suspecting` knowledge mode. Flag `bad payoff` if characters are still in `misbelief` or `concealing` mode.

- [ ] **Step 2: Add identity-based canon drift check**

If a character has `public_identity ≠ private_identity` in their identity layer, and the chapter text mixes both without a reveal event, flag canon drift.

- [ ] **Step 3: Add knowledge-based OOC check**

Cross-reference character actions with their `knowledge_mode`: a character in `misbelief` mode should not act as if they know the truth.

- [ ] **Step 4: Compile and commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: integrate knowledge visibility into diagnostics"
```

---

### Task 11: TodayFive — Knowledge & Identity Signals

**Files:**
- Modify: `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

- [ ] **Step 1: Add concealment status to guard**

In the guard slot detail, add: "N secrets still concealed, M characters in misbelief state".

- [ ] **Step 2: Add reveal readiness to promise/next slots**

If the top-ranked promise relates to a topic where `knowledge_mode = concealing`, add "⚠️ 还不能揭" to the detail.

- [ ] **Step 3: Compile and commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: add knowledge and identity signals to TodayFive"
```

---

### Tasks 12-14: 3 New Evals

**Files:**
- Create: `agent-evals/src/evals/knowledge_visibility.rs`
- Create: `agent-evals/src/evals/identity_reveal.rs`
- Create: `agent-evals/src/evals/false_belief_preservation.rs`
- Modify: `agent-evals/src/evals.rs`
- Modify: `agent-evals/src/main.rs`

- [ ] **Step 1: Eval — knowledge_visibility_consistency**

Create knowledge item + ownership for a character in 'misbelief' mode, verify query returns correct mode for that chapter.

- [ ] **Step 2: Eval — identity_reveal_consistency**

Create identity layer with public ≠ private identity, record reveal event, verify reveal list includes the event.

- [ ] **Step 3: Eval — false_belief_preservation_consistency**

Create a knowledge item where character A is in `misbelief` mode while character B is `aware`. Verify that character A's misbelief persists until a reveal event changes their mode.

- [ ] **Step 4: Register and verify**

```bash
cargo run -p agent-evals 2>&1 | grep -E "knowledge_visibility|identity_reveal|false_belief"
```
Expected: 3 [PASS] lines.

- [ ] **Step 5: Commit**

```bash
git add agent-evals/ && git commit -m "feat: add 3 knowledge/identity state evals"
```

---

### Task 15: Baseline Update

**Files:**
- Modify: `scripts/verification-baseline.cjs`

- [ ] **Step 1: Update eval count**

Change `272/272 evals passing` to `275/275 evals passing`.

- [ ] **Step 2: Run full verify and baseline**

```bash
npm run verify
npm run baseline
```

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "chore: update baseline after Sprint B"
```
