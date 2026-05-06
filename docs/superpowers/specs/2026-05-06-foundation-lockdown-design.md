# Foundation Lockdown Sprint — §3.3 Design Spec

Date: 2026-05-06
Status: draft

## Overview

Execute plan.md §3.3 (底层封顶冲刺): close the semantic gaps in Forge's long-form production kernel. This is NOT a feature phase — it is an audit-and-gate phase that locks down authoritative state, settlement extraction, recovery, provider budget, and default user surface before any UX polish begins.

## Principle

**Audit → Fix → Gate.** Find every bypass path first, fix it, then prove closure with 3 new eval gates. Do not rewrite working extraction or generation pipelines.

## Current Baseline (Pre-Sprint)

```
agent-harness-core: 89 tests passing
agent-writer:       247 tests passing
agent-evals:        265/265 passing
check:audit:        74 commands, 0 issues
check:p2:           18/18 passing
check:p2-render:    write-mode DOM guard passing
check:architecture: 14/14 files within budget
```

## Work Items

### 1. Authoritative State Lockdown

**Target:** All save paths produce isomorphic `observe_chapter_save_with_result` observations. No backend inference bypasses. Companion write mode consumes only `TodayFiveSummary`.

**Actions:**
- Write `scripts/check-save-path-consistency.cjs` — scans all Tauri command handlers and internal call chains for `save_chapter` invocations, verifies each path ends in `observe_chapter_save_with_result`
- Audit `generate_chapter_autonomous`, `batch_generate_chapter`, `repair_chapter_state`, manual `save_chapter` for isomorphic observation emission
- Verify `CompanionPanel.tsx` write-mode render path calls only `getWriterAgentTodayFive`, no direct ledger/status inference
- Fix any bypass found

**Verification:**
- `npm run check:save-path-consistency` passes
- New eval: `writer_agent:save_path_consistency_all_paths_emit_observation`

### 2. Settlement Extraction Lockdown

**Target:** Settlement extraction is explicitly replayable and comparable. Same input produces identical delta.

**Actions:**
- Add `SettlementReplay` struct: `{ input_content_hash, memory_snapshot_id, output_delta_hash, created_at }`
- Add `replay_settlement_extraction()` — given a persisted settlement JSON, recompute extraction from raw chapter content + memory snapshot, assert delta fields match exactly
- Integrate replay assertion into `repair_chapter_state`: rebuilt settlement must match original
- Add `settlement_replay.json` artifact to persisted runtime artifacts

**Verification:**
- New eval: `writer_agent:settlement_replay_produces_identical_delta`

### 3. Recovery & Chronology Lockdown

**Target:** `repair_chapter_state` is strictly idempotent and does not alter chapter chronology, recent result ordering, or next-beat semantics.

**Actions:**
- Add idempotency guard: if `repair_chapter_state` detects existing valid settlement + runtime artifacts at current revision, return `already_repaired: true` with no mutation
- Add chronology assertion: after repair, `recent_chapter_results` order, `next_beat`, and `active_chapter_mission` must be unchanged
- Record repair action as idempotent audit event

**Verification:**
- New eval: `writer_agent:repair_state_is_idempotent_and_preserves_chronology`

### 4. Provider Budget / Audit Lockdown

**Target:** Every provider call in the system goes through `WriterProviderBudgetRequest` with a recorded `writer.provider_budget` run event. No silent, unbudgeted, or unlogged provider paths.

**Actions:**
- Audit all provider call sites: `ghost_proposal`, `semantic_lint`, `editor_prediction`, `chapter_draft`, `continuation`, `compress`, `analysis`, `project_brain_query`, `manual_request`, `metacognitive_recovery`, `research_subtask`
- Verify each call site:
  - Creates `WriterProviderBudgetRequest` before the real call
  - Records `WriterProviderBudgetReport` after decision
  - Emits `writer.provider_budget` run event
  - Records `writer.model_started` after budget gate passes
- Upgrade `check:audit` to include provider budget coverage: every llm_runtime/agent_loop call site must reference a budget task category
- Fix any unbudgeted paths found

**Verification:**
- `npm run check:audit` reports provider budget coverage (new check section)
- Audit summary artifact: `reports/provider-budget-coverage.json`

### 5. Default User Surface Lockdown

**Target:** Companion write mode renders exactly and only the 5 items from `TodayFiveSummary`. No frontend display-helper inference, no extra ledger queries in write mode.

**Actions:**
- Audit `CompanionPanel.tsx` and `CompanionPanel.*.ts` — verify no direct `getWriterAgentLedger` / `getWriterAgentStatus` / `getStoryDebtSnapshot` calls in write-mode rendering
- Upgrade `check:p2` to verify TodayFive is the exclusive data source for Companion write mode
- Upgrade `check:p2-render` to assert Companion DOM contains exactly 5 TodayFive items

**Verification:**
- `npm run check:p2` includes TodayFive exclusivity check
- `npm run check:p2-render` includes TodayFive item count assertion

## New Gates

Three new eval gates define completion of this sprint:

| Gate | Eval Name | What It Proves |
|------|-----------|----------------|
| Save path consistency | `writer_agent:save_path_consistency_all_paths_emit_observation` | All chapter save paths emit isomorphic observations |
| Settlement replay consistency | `writer_agent:settlement_replay_produces_identical_delta` | Same input → same delta, replayable and comparable |
| Chronology preservation | `writer_agent:repair_state_is_idempotent_and_preserves_chronology` | repair_state is idempotent and preserves chapter ordering |

## Completion Definition

All of the following must be true:

- [ ] `npm run check:save-path-consistency` passes (new script)
- [ ] `npm run check:audit` includes provider budget coverage section
- [ ] `npm run check:p2` includes TodayFive exclusivity
- [ ] `npm run check:p2-render` includes TodayFive item count assertion
- [ ] `cargo run -p agent-evals` includes 3 new gates: `save_path_consistency_all_paths_emit_observation`, `settlement_replay_produces_identical_delta`, `repair_state_is_idempotent_and_preserves_chronology` — all passing
- [ ] `npm run verify` all green (includes all of the above)
- [ ] `cargo test -p agent-harness-core` 89+ passing
- [ ] `cargo test -p agent-writer` 247+ passing

## Scope Boundaries

**In scope:**
- Audit scripts and verification tooling
- Settlement replay struct and replay assertion
- repair_state idempotency guard
- Provider budget coverage audit and fixes
- TodayFive exclusivity enforcement
- 3 new evals

**Explicitly NOT in scope:**
- Rewriting or restructuring the extraction pipeline
- New UI panels or components
- New agent types or command handlers
- Changing ChapterContract semantics
- Story OS query path changes
- Visual/design changes
