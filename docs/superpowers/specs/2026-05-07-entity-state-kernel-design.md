# Sprint A: Entity State Kernel — Design Spec

Date: 2026-05-07
Status: draft
Plan ref: plan.md §3.3.7

## Overview

Upgrade Forge from "chapter-level typed state" to "entity-level typed state" by adding Characters, CharacterStateVersions, CharacterRelationships, and Promise.subject as first-class authoritative entities. Canon_entities is demoted to a projection layer for non-character metadata.

## Principle

Radical migration: build the new tables, migrate `canon_entities WHERE kind='character'` into `characters`, then block new character writes to `canon_entities`. No dual-system coexistence.

## Current Baseline

```
agent-harness-core: 89 tests
agent-writer:       247 tests
agent-evals:        268/268
check:audit:        74 commands, 0 issues
```

## Schema Changes

### New Tables

**`characters`** — role master table, extracted from canon_entities:

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
```

Role types: `protagonist`, `supporting`, `functional`.

**`character_state_versions`** — chapter-interval-valid state snapshots:

```sql
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
```

`valid_to_chapter = ''` means "still active".

**`character_relationships`** — chapter-interval-valid relationship edges:

```sql
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

Relation types: `ally`, `enemy`, `hidden`, `complex`.
Visibility: `public`, `hidden`, `suspected`.

### Extended Table

**`plot_promises`** — add subject columns:

```sql
ALTER TABLE plot_promises ADD COLUMN subject_ids_json TEXT DEFAULT '[]';
ALTER TABLE plot_promises ADD COLUMN subject_type TEXT DEFAULT '';
```

Subject types: `character`, `relationship`, `object`, `mixed`.

### Migration (one-shot, idempotent)

```sql
INSERT INTO characters (name, aliases_json, role_type, current_state_summary)
SELECT name, aliases_json, 'supporting', summary
FROM canon_entities WHERE kind = 'character'
AND name NOT IN (SELECT name FROM characters);
```

After migration, `canon_entities` no longer accepts `kind='character'` writes. Existing rows remain as read-only projection.

## Memory Methods

**New files:**

`character_methods.in.rs`:
- `upsert_character(name, aliases, role_type, summary)` → `Result<CharacterId>`
- `get_character_by_name(name)` → `Result<Option<CharacterSummary>>`
- `get_character_by_id(id)` → `Result<Option<CharacterSummary>>`
- `list_characters(role_type_filter)` → `Result<Vec<CharacterSummary>>`

`character_state_methods.in.rs`:
- `upsert_character_state(character_id, valid_from, commitments, goal, identity, rel_refs, source_ref)` → `Result<i64>`
- `get_active_state(character_id, chapter_title)` → `Result<Option<CharacterStateVersion>>`
- `close_state_version(version_id, valid_to_chapter)` → `Result<()>`

`relationship_methods.in.rs`:
- `upsert_relationship(char_a, char_b, rel_type, visibility, valid_from, source_ref)` → `Result<i64>`
- `get_active_relationships(character_id, chapter_title)` → `Result<Vec<RelationshipSummary>>`
- `close_relationship(rel_id, valid_to_chapter)` → `Result<()>`

**Extended file:**

`promises_methods.in.rs`:
- `bind_promise_subject(promise_id, subject_ids, subject_type)` → `Result<()>`

## Typed Operations

New `WriterOperation` variants:

```rust
CharacterUpsert {
    name: String,
    aliases: Vec<String>,
    role_type: String,
    summary: String,
}
CharacterStateUpsert {
    character_id: i64,
    valid_from_chapter: String,
    core_commitments: serde_json::Value,
    goal_state: serde_json::Value,
    identity_state: serde_json::Value,
    source_ref: String,
}
RelationshipUpsert {
    character_a_id: i64,
    character_b_id: i64,
    relation_type: String,
    visibility: String,
    valid_from_chapter: String,
    source_ref: String,
}
PromiseBindSubject {
    promise_id: i64,
    subject_ids: Vec<i64>,
    subject_type: String,
}
```

## Settlement Extension

`ChapterSettlementDelta` adds:

```rust
pub character_state_deltas: Vec<CharacterStateDeltaEntry>,
pub relationship_deltas: Vec<RelationshipDeltaEntry>,
```

`CharacterStateDeltaEntry`:
- `character_name: String`
- `chapter_title: String`
- `action: String` (upserted/updated)
- `core_commitments: Vec<String>`
- `goal_state: serde_json::Value`
- `source_ref: String`

`RelationshipDeltaEntry`:
- `character_a_name: String`
- `character_b_name: String`
- `action: String` (established/changed/ended)
- `relation_type: String`
- `visibility: String`
- `chapter_title: String`
- `source_ref: String`

**Settlement extraction** (`settlement.in.rs`): derive `character_state_deltas` from `chapter_result.character_progress`, derive `relationship_deltas` from `chapter_result.new_conflicts` matching relation patterns.

**Settlement apply** (`settlement_apply.rs`): apply `character_state_deltas` → close prior active version → upsert new version; apply `relationship_deltas` → close prior active relationship → upsert new.

## Promise Planner Extension

`promise_planner.rs` adds subject-pressure scoring:
- Protagonist-subject promises: ×2 weight
- Core-relationship-subject promises: ×1.5 weight
- Stale promises (5+ chapters since last_seen): +0.1 per chapter
- Fallback preserves existing title/description/payoff scoring

## TodayFiveSummary Extension

`today_five.in.rs` — minimum 2 of 5 slots become entity-driven:

| Slot | Change |
|------|--------|
| `guard` | Adds `active_character_count` and `unresolved_relationship_count` |
| `promise` | Sorted by subject_pressure, annotated with owning character name |
| `next` | Outputs `highest_pressure_subject` — "Character X has unresolved promise Y" |

## Canon Entities Demotion

After migration:
- `canon_entities` INSERT blocked for `kind='character'`
- Existing `canon_entities` character rows remain as read-only projection
- `CanonUpsertEntity` operation rejects `entity_type='character'`
- All character writes route through `CharacterUpsert`

## New Gates (Evals)

| Gate | What It Verifies |
|------|-----------------|
| `character_state_versioning_consistency` | Multi-version per character, correct version returned by chapter query |
| `relationship_validity_window_consistency` | Non-overlapping valid_from/to intervals, close→reopen creates new row |
| `promise_subject_binding_consistency` | bind subject syncs related_entities, unbind clears |
| `entity_scoped_settlement_apply_consistency` | Settlement delta with character_state_deltas + relationship_deltas applies correctly |

## Completion Definition

- [ ] `characters` table created and populated from canon_entities migration
- [ ] `character_state_versions` CRUD working with chapter-interval lookup
- [ ] `character_relationships` CRUD working with visibility and interval
- [ ] `plot_promises.subject_ids_json` and `subject_type` columns added
- [ ] `ChapterSettlementDelta` includes entity-scoped deltas
- [ ] Settlement extraction produces entity deltas
- [ ] Settlement apply materializes entity deltas
- [ ] Promise planner uses subject-pressure scoring
- [ ] TodayFive has 2+ entity-driven slots
- [ ] Canon_entities blocks new character writes
- [ ] `npm run verify` all green
- [ ] `cargo run -p agent-evals` all green, +4 new gates
- [ ] All new schema changes in migration, no manual SQL

## Scope Boundaries

**In scope:**
- 3 new tables + 1 extension
- Migration from canon_entities
- 3 method files + 1 extension
- 4 typed ops
- Settlement delta expansion
- Entity-level apply
- Subject-pressure promise scoring
- TodayFive entity slots
- 4 new evals

**Explicitly NOT in scope:**
- Character relationship graph UI
- Character encyclopedia frontend
- Relationship visualization
- Character card panel
- Complex knowledge/identity layers (Sprint B)
- Scene objects (Sprint C)
- Timeline (Sprint D)
