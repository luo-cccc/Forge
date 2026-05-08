# Algorithm Adaptation Sprint: Making the Four Pillars Consumable

Date: 2026-05-07
Status: draft
Whitepaper refs: plan.md §§3.3.13.4-3.3.13.8

## Overview

The four foundational state layers (entities, knowledge, scenes, timeline) have their schema and settlements in place. This sprint closes the consumption gap: retrieval, planner, diagnostics, and performance gates start consuming the new typed state instead of relying on text patterns and chapter numbers.

## Principle

No new tables. No pipeline rewrites. Each adaptation point is a surgical insertion into an existing code path, protected by a new eval gate.

## Current Baseline

```
agent-harness-core:  89 tests
agent-writer:        247 tests
agent-evals:         281/281
```

---

## Work Item 1: Typed Filter for Context Retrieval

**Target:** Context assembly uses cheap typed filters (character/relationship/knowledge/scene) before text rerank, instead of "word hit + chapter proximity" alone.

**What already exists:**
- `context/assembly.in.rs` — `assemble_observation_context_with_default_budget`
- `context_relevance/scoring.in.rs` — writing-relevance rerank
- All entity tables are queryable by ID and chapter

**What changes:**
1. In `context_relevance/scoring.in.rs`, add a pre-filter step before the existing relevance scoring loop:
   - If a context source references a character (by name), check `characters` table. Boost if protagonist or if character state version has pending commitments.
   - If a source references a relationship, check `character_relationships` for active/visibility status. Boost hidden relationships that appear in proximity.
   - If a source references a knowledge topic, check `knowledge_ownership` for concealment status. Boost if topic is in `concealing` or `suspecting` mode.
   - If a source references a scene, check `scenes` + `scene_obligations` for binding. Boost if unpaid obligations exist.

2. The pre-filter produces a `TypedFilterResult` — a small struct with boost multipliers — that feeds into the existing relevance scorer.

**Files:**
- Modify: `src-tauri/src/writer_agent/context_relevance/scoring.in.rs`
- Create: `src-tauri/src/writer_agent/context_relevance/typed_filter.in.rs`

**New gate:** `typed_context_filter_under_large_state_fixture` — creates 50 characters, 30 relationships, 20 knowledge items, 10 scenes across 5 chapters; verifies context assembly < 1s and returns correct boosted sources.

---

## Work Item 2: Planner Full-Factor Scoring

**Target:** Promise planner consumes knowledge readiness and timeline due, not just subject pressure.

**What already exists:**
- `promise_planner.rs` — `promise_subject_pressure()` with protagonist ×2, core ×1.5, stale +0.1/ch

**What changes:**
1. Add `knowledge_readiness_factor()` — checks `knowledge_ownership` for the promise's subject:
   - If all related characters are `aware` of the payoff topic → ×1.2 (ripe for payoff)
   - If any character is still `concealing` → ×0.8 (not ready to reveal)
   - If any character is `misbelief` → ×0.5 (payoff would contradict current knowledge)

2. Add `timeline_due_factor()` — checks `chapter_time_mapping`:
   - If a promise's expected payoff chapter maps to a story time slice, check its `relative_order`
   - If current chapter's time slice is past the expected payoff time slice → ×1.3 (overdue in story time)
   - If narrative mode is `flashback` and promise was introduced later in story time → ×0.3 (not yet relevant)

3. Multiply existing pressure score by both factors. Fallback: if no knowledge/timeline data exists, factors default to 1.0.

**Files:**
- Modify: `src-tauri/src/writer_agent/promise_planner.rs`

**New gate:** `planner_subject_scoring_fallback_consistency` — verifies that promises without subject/knowledge/timeline data still score the same as before (fallback behavior preserved).

---

## Work Item 3: Diagnostics Full-Factor Checks

**Target:** Diagnostics engine checks scene obligations and flashback identity consistency, not just text contradictions and knowledge visibility.

**What already exists:**
- `diagnostics/core.in.rs` — knowledge visibility, identity conflict, hidden relationship exposure
- `diagnostics/helpers_extract.in.rs` — `detect_timeline_issue`

**What changes:**
1. **Scene obligation check:** After existing checks, verify that the current chapter's scenes have their obligations addressed:
   - For each scene with `scene_obligations`, check if the promised payoff or mission goal appears in the chapter text.
   - If a scene has obligations but no text evidence of fulfillment, flag `SceneDebt` diagnostic.

2. **Flashback identity consistency:** In `detect_timeline_issue`, when a `chapter_time_mapping` has `narrative_mode = "flashback"`:
   - Query the character's `identity_layers` at the flashback's `time_slice_id` (not the current chapter).
   - If the character's identity in the flashback contradicts their identity at that story time, flag `FlashbackIdentityConflict`.

**Files:**
- Modify: `src-tauri/src/writer_agent/diagnostics/core.in.rs`
- Modify: `src-tauri/src/writer_agent/diagnostics/helpers_extract.in.rs`

**New gate:** `scene_obligation_fulfillment_diagnostic` — creates scene with obligation, runs diagnostics on chapter text missing the obligation, verifies SceneDebt diagnostic produced.

---

## Work Item 4: Performance Redline Gates

**Target:** Prove that the new semantic layers don't cause full-state scans on the hot path.

**Gates (2 of 4, minimal viable):**

1. **`save_path_without_full_state_rescan`** — Creates 100 characters + 50 relationships + 30 knowledge items + 20 scenes. Calls `build_basic_chapter_settlement_delta` and measures that `apply_chapter_settlement_delta` touches only the entities referenced in the delta (not all 200 rows). Verified by counting SQL queries or checking execution time < 100ms.

2. **`entity_scoped_settlement_apply_without_global_rebuild`** — Creates a settlement delta with 1 character state change. Applies it. Verifies that the total number of entity rows before and after has the expected delta (+1 new state version, all other tables unchanged).

**Files:**
- Create: `agent-evals/src/evals/save_performance.rs`
- Create: `agent-evals/src/evals/entity_apply_performance.rs`

---

## Completion Definition

- [ ] Typed filter pre-step in context assembly, protected by fallback
- [ ] Planner knowledge_readiness + timeline_due factors, protected by fallback consistency gate
- [ ] Diagnostics scene obligation check + flashback identity check
- [ ] 2 performance redline gates
- [ ] 4 new eval gates total:
  - `typed_context_filter_under_large_state_fixture`
  - `planner_subject_scoring_fallback_consistency`
  - `scene_obligation_fulfillment_diagnostic`
  - `save_path_without_full_state_rescan` + `entity_scoped_settlement_apply_without_global_rebuild`
- [ ] `npm run verify` all green
- [ ] `cargo run -p agent-evals` 285/285

## Scope Boundaries

**In scope:** 4 surgical algorithm insertions + 4 eval gates + 2 performance gates
**Out of scope:** Full retrieval rewrite, planner redesign, story impact node expansion, all 4 performance gates
