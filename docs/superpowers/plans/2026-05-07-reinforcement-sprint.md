# Reinforcement Sprint — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans.

**Goal:** Close 4 remaining §3.2 high-completion items: fact dedup, length telemetry, hook triage, entity repair-state.

**Architecture:** 4 surgical changes + 4 evals. No new tables.

**Tech Stack:** Rust (src-tauri, agent-evals)

---

### Task 1: Cross-Chapter Fact Dedup

**Files:** `src-tauri/src/writer_agent/settlement_apply.rs`

In `apply_chapter_settlement_delta`, where chapter_fact_delta entries are processed, add dedup logic before inserting into `canon_facts`: check for existing `(entity_id, key)` pair. If exists, update value only if source_ref is newer (higher chapter number via `extract_chapter_number`). Skip duplicate insert.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: deduplicate cross-chapter facts in settlement apply`

---

### Task 2: Length Phase Independent Telemetry

**Files:** `types_and_utils.in.rs`, `pipeline/main.in.rs`

Add `LengthPhaseTelemetry` struct to `types_and_utils.in.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LengthPhaseTelemetry {
    pub continuation_count: usize,
    pub compress_count: usize,
    pub hard_compress_count: usize,
    pub continuation_latency_ms: u64,
    pub compress_latency_ms: u64,
    pub hard_compress_latency_ms: u64,
}
```

Add to `ChapterLengthTelemetry` or persist separately. In pipeline/main.in.rs, record counts and measure wall-clock per phase. Persist as `length_telemetry.json`.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add independent length phase telemetry`

---

### Task 3: Hook Debt Triage

**Files:** `src-tauri/src/writer_agent/promise_planner.rs`

Add `hook_debt_triage_factor()`:
```rust
fn hook_debt_triage_factor(promise: &PlotPromiseSummary, current_chapter: &str) -> f64 {
    let mut factor = 1.0;
    let current_num = extract_chapter_number(current_chapter);
    let last_num = extract_chapter_number(&promise.last_seen_chapter);
    if current_num.saturating_sub(last_num) > 10 { factor *= 1.5; }
    if promise.blocked_reason.is_empty() && promise.status == "resolved" { factor *= 0.2; }
    factor
}
```

Multiply into `promise_subject_pressure` alongside existing factors.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add hook debt triage factor to promise planner`

---

### Task 4: Entity-Aware Repair-State

**Files:** `src-tauri/src/commands/generation.rs`

In `repair_chapter_state`, verify that the existing `apply_chapter_settlement_delta` call already processes entity deltas (character_state, relationship, knowledge). If not, add explicit entity rebuild calls using the existing delta fields. The apply function from settlement_apply.rs already handles entity deltas — confirm the call is present and wired.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: verify entity-aware repair-state coverage`

---

### Tasks 5-8: 4 New Evals

**Files:** Create 4 eval files, register in `evals.rs` + `main.rs`

- `agent-evals/src/evals/fact_dedup.rs` — apply same fact twice, verify 1 row
- `agent-evals/src/evals/length_telemetry.rs` — verify telemetry has per-phase counts
- `agent-evals/src/evals/hook_triage.rs` — stale > fresh rank
- `agent-evals/src/evals/entity_repair_state.rs` — repair rebuilds entity state

Register all 4. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 288
Commit: `feat: add 4 reinforcement evals`

---

### Task 9: Baseline Update

Change `284/284` to `288/288` in baseline. Run `npm run baseline`.
Commit: `chore: update baseline after reinforcement sprint`

Work from: c:\Users\Msi\Desktop\Forge