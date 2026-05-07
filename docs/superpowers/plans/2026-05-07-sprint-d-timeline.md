# Sprint D: Timeline Kernel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate chapter order from story time order — enable flashback, flashforward, and parallel narrative handling.

**Architecture:** 3 new SQLite tables (v19→v20), 3 ALTER on existing entity tables, 3 memory method files, 3 typed ops, diagnostics `TimelineIssue` uses `relative_order` instead of `chapter_number_from_title`, TodayFive shows current story time + narrative mode.

**Tech Stack:** Rust (src-tauri, agent-evals), SQLite

---

### Task 1: Schema Migration — v20

**Files:**
- Modify: `src-tauri/src/writer_agent/memory/schema.in.rs`
- Modify: `src-tauri/src/writer_agent/memory/tracing_migrate.in.rs`

- [ ] **Step 1: Bump version and add 3 new tables + 3 ALTERs**

Bump `SCHEMA_VERSION` from `19` to `20`. Add to SCHEMA string:

```sql
CREATE TABLE IF NOT EXISTS story_time_slices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    relative_order INTEGER DEFAULT 0,
    start_ref TEXT DEFAULT '',
    end_ref TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS chapter_time_mapping (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chapter_title TEXT NOT NULL,
    scene_id INTEGER,
    time_slice_id INTEGER NOT NULL,
    narrative_mode TEXT DEFAULT 'present',
    FOREIGN KEY (time_slice_id) REFERENCES story_time_slices(id)
);

CREATE TABLE IF NOT EXISTS timeline_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_ids_json TEXT DEFAULT '[]',
    event_type TEXT NOT NULL,
    time_slice_id INTEGER NOT NULL,
    source_ref TEXT DEFAULT '',
    FOREIGN KEY (time_slice_id) REFERENCES story_time_slices(id)
);
```

Add ALTERs for entity binding:
```sql
ALTER TABLE character_state_versions ADD COLUMN time_slice_id INTEGER;
ALTER TABLE character_relationships ADD COLUMN time_slice_id INTEGER;
ALTER TABLE identity_layers ADD COLUMN time_slice_id INTEGER;
```

- [ ] **Step 2: Migration block**

In `tracing_migrate.in.rs`, add v20 migration: `CREATE TABLE IF NOT EXISTS` for 3 tables + `ensure_column` (or ALTER TABLE) for 3 entity columns.

- [ ] **Step 3: Verify**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/writer_agent/memory/ && git commit -m "feat: add story_time_slices, chapter_time_mapping, timeline_events (v20)"
```

---

### Task 2: Time Slice Memory Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/time_slice_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create time_slice_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_time_slice(&self, label: &str, relative_order: i32, start_ref: &str, end_ref: &str) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO story_time_slices (label, relative_order, start_ref, end_ref) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![label, relative_order, start_ref, end_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn list_time_slices(&self) -> rusqlite::Result<Vec<TimeSliceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, label, relative_order, start_ref, end_ref FROM story_time_slices ORDER BY relative_order"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TimeSliceSummary { id: row.get(0)?, label: row.get(1)?, relative_order: row.get(2)?, start_ref: row.get(3)?, end_ref: row.get(4)? })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }
    pub fn get_time_slice_by_order(&self, relative_order: i32) -> rusqlite::Result<Option<TimeSliceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, label, relative_order, start_ref, end_ref FROM story_time_slices WHERE relative_order = ?1"
        )?;
        let mut rows = stmt.query_map(rusqlite::params![relative_order], |row| {
            Ok(TimeSliceSummary { id: row.get(0)?, label: row.get(1)?, relative_order: row.get(2)?, start_ref: row.get(3)?, end_ref: row.get(4)? })
        })?;
        Ok(rows.next().and_then(|r| r.ok()))
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSliceSummary { pub id: i64, pub label: String, pub relative_order: i32, pub start_ref: String, pub end_ref: String }
```

- [ ] **Step 2: Register and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add time slice memory methods"
```

---

### Task 3: Chapter Time Mapping + Timeline Event Methods

**Files:**
- Create: `src-tauri/src/writer_agent/memory/chapter_time_methods.in.rs`
- Create: `src-tauri/src/writer_agent/memory/timeline_event_methods.in.rs`
- Modify: `src-tauri/src/writer_agent/memory.rs`

- [ ] **Step 1: Create chapter_time_methods.in.rs**

```rust
impl WriterMemory {
    pub fn upsert_chapter_time_mapping(&self, chapter_title: &str, scene_id: Option<i64>, time_slice_id: i64, narrative_mode: &str) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO chapter_time_mapping (chapter_title, scene_id, time_slice_id, narrative_mode) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![chapter_title, scene_id, time_slice_id, narrative_mode],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn get_time_mapping_for_chapter(&self, chapter_title: &str) -> rusqlite::Result<Vec<ChapterTimeMappingSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, chapter_title, scene_id, time_slice_id, narrative_mode FROM chapter_time_mapping WHERE chapter_title = ?1 ORDER BY id"
        )?;
        let rows = stmt.query_map(rusqlite::params![chapter_title], |row| {
            Ok(ChapterTimeMappingSummary { id: row.get(0)?, chapter_title: row.get(1)?, scene_id: row.get(2)?, time_slice_id: row.get(3)?, narrative_mode: row.get(4)? })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterTimeMappingSummary { pub id: i64, pub chapter_title: String, pub scene_id: Option<i64>, pub time_slice_id: i64, pub narrative_mode: String }
```

- [ ] **Step 2: Create timeline_event_methods.in.rs**

```rust
impl WriterMemory {
    pub fn record_timeline_event(&self, subject_ids: &[i64], event_type: &str, time_slice_id: i64, source_ref: &str) -> rusqlite::Result<i64> {
        let ids_json = serde_json::to_string(subject_ids).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO timeline_events (subject_ids_json, event_type, time_slice_id, source_ref) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![ids_json, event_type, time_slice_id, source_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn list_timeline_events(&self, time_slice_id: Option<i64>) -> rusqlite::Result<Vec<TimelineEventSummary>> {
        let (query, param) = if let Some(tsid) = time_slice_id {
            ("SELECT id, subject_ids_json, event_type, time_slice_id, source_ref FROM timeline_events WHERE time_slice_id = ?1 ORDER BY id".to_string(), Some(tsid))
        } else {
            ("SELECT id, subject_ids_json, event_type, time_slice_id, source_ref FROM timeline_events ORDER BY id".to_string(), None)
        };
        let mut stmt = self.conn.prepare(&query)?;
        let rows = if let Some(p) = param {
            stmt.query_map(rusqlite::params![p], |row| {
                Ok(TimelineEventSummary { id: row.get(0)?, subject_ids: serde_json::from_str(&row.get::<_,String>(1)?).unwrap_or_default(), event_type: row.get(2)?, time_slice_id: row.get(3)?, source_ref: row.get(4)? })
            })?
        } else {
            stmt.query_map([], |row| {
                Ok(TimelineEventSummary { id: row.get(0)?, subject_ids: serde_json::from_str(&row.get::<_,String>(1)?).unwrap_or_default(), event_type: row.get(2)?, time_slice_id: row.get(3)?, source_ref: row.get(4)? })
            })?
        };
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEventSummary { pub id: i64, pub subject_ids: Vec<i64>, pub event_type: String, pub time_slice_id: i64, pub source_ref: String }
```

- [ ] **Step 3: Register and commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add chapter time mapping and timeline event memory methods"
```

---

### Task 4: Typed Operations — 3 Timeline Ops

**Files:**
- Modify: `src-tauri/src/writer_agent/operation.rs`
- Modify: `src-tauri/src/writer_agent/kernel/helpers.rs`
- Modify: `src-tauri/src/writer_agent/kernel/ops.rs`

- [ ] **Step 1: Add 3 new variants**

```rust
TimeSliceUpsert { label: String, relative_order: i32 },
ChapterTimeMappingUpsert { chapter_title: String, scene_id: Option<i64>, time_slice_id: i64, narrative_mode: String },
TimelineEventRecord { subject_ids: Vec<i64>, event_type: String, time_slice_id: i64 },
```

Labels: `"time_slice.upsert"`, `"chapter_time_mapping.upsert"`, `"timeline_event.record"`. Scope: `vec!["timeline"]`. All write-capable. Add execution handlers in ops.rs.

- [ ] **Step 2: Commit**

```bash
cargo check -p agent-writer && git add -A && git commit -m "feat: add Timeline typed ops"
```

---

### Task 5: Diagnostics — Timeline Uses Relative Order

**Files:**
- Modify: `src-tauri/src/writer_agent/diagnostics/helpers_extract.in.rs`

- [ ] **Step 1: Update TimelineIssue detection**

In `detect_timeline_issue`, replace `chapter_number_from_title` with `story_time_slices.relative_order` comparison. When a timeline issue is detected, query the `chapter_time_mapping` to get the `time_slice_id`, then compare `relative_order` values instead of extracting chapter numbers from titles.

- [ ] **Step 2: Add flashback identity consistency check**

In `core.in.rs`, after identity check, add: if `narrative_mode` is `flashback` and character has `identity_layers` where `public_identity` differs from `private_identity`, verify the flashback is consistent with the identity state at that story time.

- [ ] **Step 3: Commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: use story time relative_order in timeline diagnostics"
```

---

### Task 6: TodayFive — Story Time + Narrative Mode

**Files:**
- Modify: `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

- [ ] **Step 1: Add story time context**

In the guard or next slot, if current chapter has a `chapter_time_mapping` with `narrative_mode != "present"`, add the story time info:

```rust
let time_context = current_chapter.as_deref().and_then(|ch| {
    self.memory.get_time_mapping_for_chapter(ch).ok().and_then(|mappings| {
        mappings.first().and_then(|m| {
            self.memory.get_time_slice_by_order(0).ok().flatten().map(|ts| {
                format!("故事时间: {} ({})", ts.label, m.narrative_mode)
            })
        })
    })
});
```

Append to guard detail if present.

- [ ] **Step 2: Commit**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
git add -A && git commit -m "feat: show story time context in TodayFive"
```

---

### Tasks 7-9: 3 New Evals

**Files:**
- Create: `agent-evals/src/evals/story_time_mapping.rs`
- Create: `agent-evals/src/evals/flashback_identity.rs`
- Create: `agent-evals/src/evals/timeline_event_order.rs`
- Modify: `agent-evals/src/evals.rs`, `agent-evals/src/main.rs`

- [ ] **Step 1: story_time_mapping_consistency**

Create 2 time slices. Map Chapter-1 to slice 1 (present) and Chapter-5 to slice 1 too (parallel). Verify `get_time_mapping_for_chapter` returns correct mappings for both.

- [ ] **Step 2: flashback_identity_consistency**

Create character with identity at Chapter-3. Create flashback mapping for Chapter-2. Verify `get_active_identity` at Chapter-2 + flashback time slice returns the identity from the correct story time.

- [ ] **Step 3: timeline_event_order_consistency**

Create 2 time slices with different relative_orders. Record timeline events in each. Verify `list_timeline_events` returns them in order.

- [ ] **Step 4: Register and verify**

```bash
cargo run -p agent-evals 2>&1 | grep -E "story_time|flashback|timeline_event|Total:"
```
Expected: 3 [PASS], Total: 281.

- [ ] **Step 5: Commit**

```bash
git add agent-evals/ && git commit -m "feat: add 3 timeline evals"
```

---

### Task 10: Baseline Update

**Files:**
- Modify: `scripts/verification-baseline.cjs`

- [ ] **Step 1:** Change `278/278 evals passing` to `281/281 evals passing`. Run `npm run baseline`.

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "chore: update baseline after Sprint D"
```
