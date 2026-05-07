# Reinforcement Sprint: Closing §3.2 High-Completion Items

Date: 2026-05-07
Status: draft
Plan ref: plan.md §3.2

## Overview

Close the 4 remaining §3.2 items at 50-80% completion: authoritative state convergence, independent length governance, hook debt triage, and entity-aware repair-state.

## Principle

Surgical completions only — no new tables, no pipeline rewrites. Each item is one code change + one eval gate.

## Baseline

```
agent-harness-core:  89 tests
agent-writer:        247 tests
agent-evals:         284/284
```

---

## Item 2: Authoritative State Convergence (80→100%)

**Target:** Cross-chapter facts are deduplicated at settlement apply. `ChapterFactDelta` entries from different chapters that describe the same canonical fact converge to one authoritative row.

**What changes:**
- In `settlement_apply.rs`, before inserting `chapter_fact_delta` entries, check `canon_facts` for existing rows with the same `(entity_id, key)`. If found, update the value + bump confidence only if the new source is more recent. Don't create duplicate rows.
- This closes the gap where repeated settlement of the same fact (e.g., "林墨 weapon=寒影刀") creates N rows instead of 1.

**Files:** `src-tauri/src/writer_agent/settlement_apply.rs`

**Gate:** `cross_chapter_fact_dedup_consistency` — apply same fact delta twice, verify only 1 canon_fact row exists.

---

## Item 3: Independent Length Governance (50→100%)

**Target:** Length phases (continuation, compress, hard_compress) each record independent telemetry — count + latency — separate from the general generation telemetry.

**What changes:**
- In `pipeline/main.in.rs`, add `LengthPhaseTelemetry` struct with per-phase counters and latency_ms.
- Record `continuation_count`, `compress_count`, `hard_compress_count`, and each phase's wall clock duration.
- Persist as `length_telemetry.json` alongside existing runtime artifacts.
- Don't change any length logic — just add observability.

**Files:** `src-tauri/src/chapter_generation/pipeline/main.in.rs`, `types_and_utils.in.rs`

**Gate:** `length_phase_independent_telemetry` — run pipeline with continuation, verify telemetry has per-phase counts > 0 and latencies non-zero.

---

## Item 4: Hook Debt Triage (60→100%)

**Target:** Promise planner explicitly handles `advance` / `resolve` / `defer(with reason)` triage. Stale promises (10+ chapters unseen), volume-boundary promises, and error-fulfilled promises get priority boost.

**What changes:**
- In `promise_planner.rs`, add `hook_debt_triage_factor()`:
  - `stale` (>10 chapters unseen): ×1.5
  - `volume_boundary` (promise payoff chapter is in a different volume than current): ×1.3
  - `error_fulfilled` (resolved but with blocked_reason): ×0.2 (already closed, don't re-promote)
  - `deferred` (explicit defer with reason): preserve current priority but add reason to planner output
- Multiply into promise_subject_pressure after existing factors.

**Files:** `src-tauri/src/writer_agent/promise_planner.rs`

**Gate:** `hook_debt_triage_consistency` — create stale promise (>10 chapters old), verify it ranks above a fresh promise with same base priority.

---

## Item 5: Entity-Aware Repair-State (70→100%)

**Target:** `repair_chapter_state` rebuilds not just chapter settlement, but also character state versions, relationships, and knowledge ownership for the affected chapter.

**What changes:**
- In `repair_chapter_state` (already has chronology assertion and idempotency guard), after settlement replay, also trigger rebuild of entity state from settlement deltas:
  - `character_state_deltas` → close active → upsert new
  - `relationship_deltas` → close active → upsert new  
  - `knowledge_deltas` → upsert knowledge ownership
- These paths already exist in `settlement_apply.rs`. Repair-state reuses them by calling `apply_chapter_settlement_delta` with the replayed delta.
- Verify: the `settlement_apply` call already happens in repair. Ensure it processes entity deltas correctly.

**Files:** `src-tauri/src/commands/generation.rs` (minimal change — verify existing apply covers entity deltas)

**Gate:** `entity_aware_repair_state_consistency` — create character state + relationship, run repair, verify state versions and relationships are rebuilt.

---

## Completion Definition

- [ ] 4 surgical code changes
- [ ] 4 new eval gates (284→288)
- [ ] `npm run verify` all green
- [ ] `cargo run -p agent-evals` 288/288

## Scope Boundaries

**In:** 4 surgical completions + 4 evals
**Out:** §3.2 item 1 (input governance compiler), performance gates (§3.3.13.8), UI changes
