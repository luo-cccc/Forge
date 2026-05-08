# Sprints B-C-D: Knowledge, Scene, Timeline — Design Spec

Date: 2026-05-07
Status: draft
Whitepaper refs: plan.md §§3.3.8-3.3.10

## Overview

Complete the four foundational state layers of Forge's long-form production kernel. Sprint A delivered entity state (characters, relationships, promise.subject). Sprints B-D deliver knowledge/identity, scene orchestration, and story timeline — in strict dependency order.

## Principle

Independent new tables with `holder_type + holder_id` for polymorphic entity references. No JSON-field overloading. Each sprint follows the established pattern: schema → memory methods → typed ops → settlement integration → diagnostics/TodayFive → evals.

## Current Baseline

```
agent-harness-core:  89 tests
agent-writer:        247 tests
agent-evals:         272/272
check:audit:         74 commands, 0 issues
```

---

## Sprint B: 知识与身份状态封顶 (§3.3.8)

**Goal:** Distinguish objective truth, character knowledge, false beliefs, and visible identity.

### Schema

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

Truth states: `objective`, `ambiguous`, `retconned`.
Knowledge modes: `aware`, `misbelief`, `suspecting`, `concealing`.
Holder types: `character`, `relationship`, `faction`, `public`.
Reveal types: `knowledge`, `identity`.

### Memory Methods

- `knowledge_methods.in.rs`: upsert/get/list knowledge items, query by truth_state
- `ownership_methods.in.rs`: upsert/get ownership, query by holder, query by knowledge_mode
- `identity_methods.in.rs`: upsert/get identity layers, get by character+chapter, close version
- `reveal_methods.in.rs`: record reveal event, list reveals by chapter, list by subject

### Typed Operations

- `KnowledgeUpsert { topic, truth_state }`
- `KnowledgeOwnershipUpsert { knowledge_id, holder_type, holder_id, knowledge_mode, valid_from_chapter }`
- `IdentityLayerUpsert { character_id, public_identity, private_identity, valid_from_chapter }`
- `RevealEventRecord { subject_id, reveal_type, revealed_to, chapter }`

### Settlement Integration

`ChapterSettlementDelta` adds:
```rust
pub knowledge_deltas: Vec<KnowledgeDeltaEntry>,
pub identity_deltas: Vec<IdentityDeltaEntry>,
```

**Extraction**: `settlement.in.rs` derives `knowledge_deltas` from chapter text reveals (e.g., "他终于知道了真相"). `identity_deltas` from identity-related state changes.

**Apply**: `settlement_apply.rs` upserts knowledge_ownership rows and identity_layers, records reveal events for mode transitions from `concealing/suspecting` → `aware`.

### Diagnostics Integration

- `bad payoff`: gates on knowledge mode — payoff requires appropriate characters to be in `aware` or `suspecting` mode
- `canon drift`: checks that `public_identity` ≠ `private_identity` scenes don't mix without `reveal_events`
- `OOC`: adds knowledge check — character actions must be consistent with what they `aware`/`misbelief` state allows

### TodayFive Extension

New slot content: "which secret can't be revealed yet, who already knows, who still misbelieves".

### Completion Gate

3 new evals:
- `knowledge_visibility_consistency`
- `identity_reveal_consistency`
- `false_belief_preservation_consistency`

### Scope Boundaries

**In:** 4 tables + migration + memory methods + typed ops + settlement delta + diagnostics + TodayFive + 3 evals
**Out:** Knowledge graph UI, secret management panel, faction UI

---

## Sprint C: 场景编排内核封顶 (§3.3.9)

**Goal:** Organize chapters by Scene objects instead of whole-chapter text blocks.

### Schema

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

Scene types: `scene`, `flashback`, `interlude`, `teaser`.

### Memory Methods

- `scene_methods.in.rs`: upsert/get/list scenes by chapter, reorder sequence
- `scene_state_methods.in.rs`: upsert/get state by scene, get entry/exit state
- `scene_obligation_methods.in.rs`: upsert obligations, list by promise/mission
- `scene_result_methods.in.rs`: upsert results, get by scene

### Typed Operations

- `SceneUpsert { chapter_title, sequence, scene_type, summary }`
- `SceneStateUpsert { scene_id, objective, participants, location_ref, ... }`
- `SceneObligationUpsert { scene_id, promise_ids, mission_refs, payoff_targets }`
- `SceneResultRecord { scene_id, outcome, consequence }`

### Settlement Integration

`ChapterSettlementDelta` adds `scene_deltas: Vec<SceneResultProjection>`. Generation pipeline's `scene_plan` phase becomes a structured artifact (list of scene objects with objectives) instead of a phase name. Settlement extraction populates scene results from chapter content.

### TodayFive Extension

`Next Move` defaults to scene-level objective, not chapter-level summary.

### Completion Gate

3 new evals:
- `scene_sequence_consistency`
- `scene_obligation_binding_consistency`
- `scene_result_projection_consistency`

### Scope Boundaries

**In:** 4 tables + migration + methods + typed ops + scene plan artifact + settlement + TodayFive + 3 evals
**Out:** Scene drag-and-drop UI, storyboard frontend, film-style storyboard

---

## Sprint D: 时间轴内核封顶 (§3.3.10)

**Goal:** Separate chapter order from story time order. Enable flashback/flashforward/parallel narrative handling.

### Schema

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

Narrative modes: `present`, `flashback`, `flashforward`, `parallel`.
Event types: `death`, `reveal`, `confrontation`, `departure`, `arrival`, `decision`, `betrayal`.

### Memory Methods

- `time_slice_methods.in.rs`: upsert/get/list slices, query by order range
- `chapter_time_methods.in.rs`: upsert mapping, get by chapter/scene, get narrative_mode
- `timeline_event_methods.in.rs`: upsert events, list by time_slice, list by subject

### Typed Operations

- `TimeSliceUpsert { label, relative_order }`
- `ChapterTimeMappingUpsert { chapter_title, scene_id, time_slice_id, narrative_mode }`
- `TimelineEventRecord { subject_ids, event_type, time_slice_id }`

### Entity Binding Extension

`character_state_versions`, `character_relationships`, `identity_layers` add optional `time_slice_id` foreign key for binding to story time instead of chapter.

### Diagnostics Integration

`TimelineIssue` detection no longer relies primarily on `chapter_number_from_title`. Uses `story_time_slices.relative_order` for proper temporal reasoning. Flashback identity consistency checked via `narrative_mode = flashback` + `identity_layers` cross-reference.

### TodayFive Extension

During flashback/interlude writing: "Current section is story time: [label], narrative mode: [flashback/parallel]".

### Completion Gate

3 new evals:
- `story_time_mapping_consistency`
- `flashback_identity_consistency`
- `timeline_event_order_consistency`

### Scope Boundaries

**In:** 3 tables + migration + methods + typed ops + entity binding + diagnostics + TodayFive + 3 evals
**Out:** Visual timeline UI, calendar/world-chronology frontend, editor timeline plugin

---

## Global Constraints

### Performance Redlines (plan.md §3.3.13.6)

- No full-state scan before each generation
- No full entity/knowledge/timeline index rebuild after each save
- No full embedding/VectorDB refresh in settlement
- Write = delta apply only
- Query = active snapshot + recent window
- Recall = coarse filter → typed filter → small-set rerank
- Backfill/repair/reindex = offline path only

### Completion Definition

All three sprints are complete when:
- [ ] Sprite B-C-D schema + migration in v18/v19/v20
- [ ] All memory methods + typed ops wired
- [ ] Settlement extraction produces knowledge/identity/scene/timeline deltas
- [ ] Settlement apply materializes all entity-level deltas
- [ ] Diagnostics consume new layers (knowledge visibility, scene obligation, timeline order)
- [ ] TodayFive includes knowledge/scene/timeline signals
- [ ] `npm run verify` all green
- [ ] `cargo run -p agent-evals` all green (+9 new gates)
- [ ] Performance redlines not violated (no full-scan save, no global rebuild)
