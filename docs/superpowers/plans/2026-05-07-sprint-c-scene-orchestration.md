# Sprint C: Scene Orchestration Kernel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Organize chapters by Scene objects instead of whole-chapter text blocks — add scene-level objectives, participants, obligations, and results.

**Architecture:** 4 new SQLite tables (v18→v19), 4 memory method files, 4 typed ops, settlement delta expansion with scene result projection, generation pipeline gets structured `ScenePlanEntry` array, TodayFive next-move defaults to scene objective.

**Tech Stack:** Rust (src-tauri, agent-evals), SQLite

---

### Task 1: Schema Migration — v19

**Files:**
- Modify: `src-tauri/src/writer_agent/memory/schema.in.rs`
- Modify: `src-tauri/src/writer_agent/memory/tracing_migrate.in.rs`

- [ ] **Step 1: Read existing files**

Read `schema.in.rs` (note SCHEMA_VERSION=18) and `tracing_migrate.in.rs` (understand migration pattern).

- [ ] **Step 2: Bump version and add tables**

Bump `SCHEMA_VERSION` from `18` to `19`. Add 4 tables to the `SCHEMA` string:

```sql
CREATE TABLE IF NOT EXISTS scenes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chapter_title TEXT NOT NULL,
    sequence INTEGER NOT NULL DEFAULT 0,
    scene_type TEXT DEFAULT 'scene',
    summary TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS scene_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scene_id INTEGER NOT NULL,
    objective TEXT DEFAULT '',
    participants_json TEXT DEFAULT '[]',
    location_ref TEXT DEFAULT '',
    entry_state_json TEXT DEFAULT '{}',
    exit_state_json TEXT DEFAULT '{}',
    FOREIGN KEY (scene_id) REFERENCES scenes(id)
);

CREATE TABLE IF NOT EXISTS scene_obligations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scene_id INTEGER NOT NULL,
    promise_ids_json TEXT DEFAULT '[]',
    mission_refs_json TEXT DEFAULT '[]',
    payoff_targets_json TEXT DEFAULT '[]',
    FOREIGN KEY (scene_id) REFERENCES scenes(id)
);

CREATE TABLE IF NOT EXISTS scene_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scene_id INTEGER NOT NULL,
    outcome TEXT DEFAULT '',
    consequence TEXT DEFAULT '',
    source_ref TEXT DEFAULT '',
    FOREIGN KEY (scene_id) REFERENCES scenes(id)
);
```

- [ ] **Step 3: Add migration block**

In `tracing_migrate.in.rs`, add v19 migration: `CREATE TABLE IF NOT EXISTS` for all 4 tables.

- [ ] **Step 4: Verify**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```
Expected: compiles, 247 tests passing.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/writer_agent/memory/ && git commit -m "feat: add scenes, scene_state, scene_obligations, scene_results tables (v19)"
```

---

### Task 2: Scene Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/scene_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create scene_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_scene(&self, chapter_title: &str, sequence: i32, scene_type: &str, summary: &str) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO scenes (chapter_title, sequence, scene_type, summary)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![chapter_title, sequence, scene_type, summary],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_scenes_by_chapter(&self, chapter_title: &str) -> rusqlite::Result<Vec<SceneSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, chapter_title, sequence, scene_type, summary FROM scenes WHERE chapter_title = ?1 ORDER BY sequence"
        )?;
        let rows = stmt.query_map(rusqlite::params![chapter_title], |row| {
            Ok(SceneSummary { id: row.get(0)?, chapter_title: row.get(1)?, sequence: row.get(2)?, scene_type: row.get(3)?, summary: row.get(4)? })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }

    pub fn reorder_scenes(&self, chapter_title: &str, ordered_ids: &[i64]) -> rusqlite::Result<()> {
        for (i, id) in ordered_ids.iter().enumerate() {
            self.conn.execute(
                "UPDATE scenes SET sequence = ?1 WHERE id = ?2 AND chapter_title = ?3",
                rusqlite::params![i as i32, id, chapter_title],
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSummary {
    pub id: i64, pub chapter_title: String, pub sequence: i32, pub scene_type: String, pub summary: String,
}
```

- [ ] **Step 2: Register in memory.rs**

Add `include!("memory/scene_methods.in.rs");` after reveal methods.

- [ ] **Step 3: Commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add scene CRUD memory methods"
```

---

### Task 3: Scene State Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/scene_state_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create scene_state_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_scene_state(
        &self, scene_id: i64, objective: &str, participants: &[String],
        location_ref: &str, entry_state: &serde_json::Value, exit_state: &serde_json::Value,
    ) -> rusqlite::Result<i64> {
        let participants_json = serde_json::to_string(participants).unwrap_or_default();
        let entry_json = serde_json::to_string(entry_state).unwrap_or_default();
        let exit_json = serde_json::to_string(exit_state).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO scene_state (scene_id, objective, participants_json, location_ref, entry_state_json, exit_state_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![scene_id, objective, participants_json, location_ref, entry_json, exit_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_scene_state(&self, scene_id: i64) -> rusqlite::Result<Option<SceneStateSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scene_id, objective, participants_json, location_ref, entry_state_json, exit_state_json FROM scene_state WHERE scene_id = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![scene_id], |row| {
            Ok(SceneStateSummary {
                id: row.get(0)?, scene_id: row.get(1)?, objective: row.get(2)?,
                participants: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                location_ref: row.get(4)?,
                entry_state: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                exit_state: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
            })
        })?;
        Ok(rows.next().and_then(|r| r.ok()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneStateSummary {
    pub id: i64, pub scene_id: i64, pub objective: String,
    pub participants: Vec<String>, pub location_ref: String,
    pub entry_state: serde_json::Value, pub exit_state: serde_json::Value,
}
```

- [ ] **Step 2: Register and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add scene state memory methods"
```

---

### Task 4: Scene Obligations + Results Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/scene_obligation_methods.in.rs`
- Create: `src-tauri/src/writer_agent/memory/scene_result_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create scene_obligation_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_scene_obligations(&self, scene_id: i64, promise_ids: &[i64], mission_refs: &[String], payoff_targets: &[String]) -> rusqlite::Result<i64> {
        let p_json = serde_json::to_string(promise_ids).unwrap_or_default();
        let m_json = serde_json::to_string(mission_refs).unwrap_or_default();
        let t_json = serde_json::to_string(payoff_targets).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO scene_obligations (scene_id, promise_ids_json, mission_refs_json, payoff_targets_json)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![scene_id, p_json, m_json, t_json],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_scene_obligations(&self, scene_id: i64) -> rusqlite::Result<Option<SceneObligationSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scene_id, promise_ids_json, mission_refs_json, payoff_targets_json FROM scene_obligations WHERE scene_id = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![scene_id], |row| {
            Ok(SceneObligationSummary {
                id: row.get(0)?, scene_id: row.get(1)?,
                promise_ids: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                mission_refs: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                payoff_targets: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
            })
        })?;
        Ok(rows.next().and_then(|r| r.ok()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneObligationSummary {
    pub id: i64, pub scene_id: i64, pub promise_ids: Vec<i64>,
    pub mission_refs: Vec<String>, pub payoff_targets: Vec<String>,
}
```

- [ ] **Step 2: Create scene_result_methods.in.rs**

```rust
impl WriterMemory {
    pub fn record_scene_result(&self, scene_id: i64, outcome: &str, consequence: &str, source_ref: &str) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO scene_results (scene_id, outcome, consequence, source_ref) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![scene_id, outcome, consequence, source_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_scene_results(&self, scene_id: i64) -> rusqlite::Result<Vec<SceneResultSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scene_id, outcome, consequence, source_ref FROM scene_results WHERE scene_id = ?1 ORDER BY id"
        )?;
        let rows = stmt.query_map(rusqlite::params![scene_id], |row| {
            Ok(SceneResultSummary { id: row.get(0)?, scene_id: row.get(1)?, outcome: row.get(2)?, consequence: row.get(3)?, source_ref: row.get(4)? })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneResultSummary {
    pub id: i64, pub scene_id: i64, pub outcome: String, pub consequence: String, pub source_ref: String,
}
```

- [ ] **Step 3: Register both in memory.rs and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add scene obligation and result memory methods"
```

---

### Task 5: Typed Operations — 4 Scene Ops

**Files:**
- Modify: `src-tauri/src/writer_agent/operation.rs`
- Modify: `src-tauri/src/writer_agent/kernel/helpers.rs`
- Modify: `src-tauri/src/writer_agent/kernel/ops.rs`

- [ ] **Step 1: Add 4 new WriterOperation variants**

```rust
SceneUpsert { chapter_title: String, sequence: i32, scene_type: String, summary: String },
SceneStateUpsert { scene_id: i64, objective: String, participants: Vec<String>, location_ref: String, entry_state: serde_json::Value, exit_state: serde_json::Value },
SceneObligationUpsert { scene_id: i64, promise_ids: Vec<i64>, mission_refs: Vec<String>, payoff_targets: Vec<String> },
SceneResultRecord { scene_id: i64, outcome: String, consequence: String },
```

- [ ] **Step 2: Add labels/scope/write-capable**

In `helpers.rs`:
```rust
WriterOperation::SceneUpsert { .. } => "scene.upsert",
WriterOperation::SceneStateUpsert { .. } => "scene_state.upsert",
WriterOperation::SceneObligationUpsert { .. } => "scene_obligation.upsert",
WriterOperation::SceneResultRecord { .. } => "scene_result.record",
```

All 4 → `vec!["scene"]` scope, all write-capable.

- [ ] **Step 3: Add execution handlers in ops.rs**

Each calls the corresponding memory method and returns `Ok(OperationResult { success: true, ... })`.

- [ ] **Commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add Scene typed ops"
```

---

### Task 6: Scene Delta Types + Settlement Extraction

**Files:**
- Modify: `src-tauri/src/chapter_generation/types_and_utils.in.rs`
- Modify: `src-tauri/src/chapter_generation/settlement.in.rs`

- [ ] **Step 1: Add SceneResultProjection type and delta field**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SceneResultProjection {
    pub scene_id: i64,
    pub outcome: String,
    pub consequence: String,
    pub source_ref: String,
}
```

Add `pub scene_deltas: Vec<SceneResultProjection>` to `ChapterSettlementDelta`.

- [ ] **Step 2: Extract scene deltas in settlement**

In `build_settlement_extraction`, scan `chapter_result.summary` for scene-like segments. Minimal extraction: create 1 scene per chapter with summary as outcome. Add scene_deltas to `ChapterSettlementExtraction`.

- [ ] **Commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: add scene delta types and extraction"
```

---

### Task 7: Settlement Apply — Scene Deltas

**Files:**
- Modify: `src-tauri/src/writer_agent/settlement_apply.rs`

- [ ] **Step 1: Apply scene deltas**

After identity apply, for each `SceneResultProjection` in `delta.scene_deltas`, call `memory.record_scene_result(scene_id, outcome, consequence, source_ref)`. Add `scene_applied: usize` to apply result.

- [ ] **Commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: apply scene deltas in settlement"
```

---

### Task 8: Generation Pipeline — Scene Plan

**Files:**
- Modify: `src-tauri/src/chapter_generation/types_and_utils.in.rs`
- Modify: `src-tauri/src/chapter_generation/context.in.rs`
- Modify: `src-tauri/src/chapter_generation/pipeline/main.in.rs`

- [ ] **Step 1: Add ScenePlanEntry type**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScenePlanEntry {
    pub name: String,
    pub objective: String,
    pub participants: Vec<String>,
}
```

- [ ] **Step 2: Add scene_plan to BuiltChapterContext**

Add `pub scene_plan: Vec<ScenePlanEntry>` field. In pipeline, after `scene_plan` phase, populate from context (default: one entry per chapter with chapter mission as objective).

- [ ] **Step 3: Persist scene_plan as runtime artifact**

Add `scene_plan.json` to persisted artifacts alongside existing settlement/intent files.

- [ ] **Commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: add scene plan to generation pipeline"
```

---

### Task 9: TodayFive — Scene Objective in Next Move

**Files:**
- Modify: `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

- [ ] **Step 1: Prioritize scene objective over chapter summary**

In the `next` slot, check if current chapter has scenes. If `list_scenes_by_chapter` returns scenes with objectives, use the first scene's objective as the Next Move value instead of the chapter-level result summary.

- [ ] **Commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: prioritize scene objective in TodayFive next move"
```

---

### Tasks 10-12: 3 New Evals

**Files:**
- Create: `agent-evals/src/evals/scene_sequence.rs`
- Create: `agent-evals/src/evals/scene_obligation.rs`
- Create: `agent-evals/src/evals/scene_result.rs`
- Modify: `agent-evals/src/evals.rs`, `agent-evals/src/main.rs`

- [ ] **Step 1: scene_sequence_consistency — verify scenes ordered by sequence**

Create 3 scenes for a chapter, reorder via `reorder_scenes`, verify `list_scenes_by_chapter` returns them in new order.

- [ ] **Step 2: scene_obligation_binding_consistency — verify obligations bind correctly**

Create scene + obligation with promise_ids, query back, verify promise_ids match.

- [ ] **Step 3: scene_result_projection_consistency — verify results project correctly**

Create scene + record_result, apply settlement delta with scene_deltas, verify result stored.

- [ ] **Step 4: Register all 3 and verify**

```bash
cargo run -p agent-evals 2>&1 | grep -E "scene_sequence|scene_obligation|scene_result|Total:"
```
Expected: 3 [PASS], Total: 278.

- [ ] **Commit**

```bash
git add agent-evals/ && git commit -m "feat: add 3 scene orchestration evals"
```

---

### Task 13: Baseline Update

**Files:**
- Modify: `scripts/verification-baseline.cjs`

- [ ] **Step 1: Update eval count**

Change `275/275 evals passing` to `278/278 evals passing`. Run `npm run baseline`.

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "chore: update baseline after Sprint C"
```
