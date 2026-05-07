# Feedback Learning Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Make the agent learn from author feedback — rejection patterns, acceptance patterns, and ignored warnings — adjusting planner/diagnostics/ghost behavior per author.

**Architecture:** 3 surgical insertions into existing code paths. No new tables. 3 evals.

**Tech Stack:** Rust (src-tauri, agent-evals)

---

### Task 1: Planner Learns Rejected Promise Kinds

**Files:** `src-tauri/src/writer_agent/promise_planner.rs`

Add `rejection_penalty` to promise ranking:
- Read `feedback_methods.in.rs` — `get_recent_feedback(limit)` returns accepted/rejected proposal feedback.
- For each promise kind (`plot_promise`, `object_in_motion`, `emotional_debt`, etc.), count rejections in the last N feedback entries.
- If rejection rate > 50% for a kind, apply ×0.7 penalty to new promises of that kind.
- Penalty decays over time: if no rejections in last 10 chapters, penalty resets to ×1.0.

Add function `promise_kind_rejection_penalty(kind: &str, memory: &WriterMemory) -> f64`.

Multiply into `promise_subject_pressure` at the end.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add rejection-based promise kind penalty to planner`

### Task 2: Ghost Quality Learns Style Preferences

**Files:** `src-tauri/src/writer_agent/kernel/ghost.rs`

When generating ghost proposals, boost style preferences that match the author's accepted style patterns:
- Read `style_contract_methods.in.rs` — `get_style_preferences()` returns stored preferences.
- Author-accepted style keys get +0.2 confidence boost when generating ghost alternatives.
- Already-accepted style dimensions (e.g., `dialogue.subtext`) get priority in ghost branch ranking.

Minimal implementation: in ghost proposal creation, read `style_contract_methods` and add a `style_boost` to the confidence score.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: apply author style preferences to ghost quality scoring`

### Task 3: Diagnostics Learns Ignored Warnings

**Files:** `src-tauri/src/writer_agent/diagnostics/core.in.rs`

Track which diagnostic categories the author consistently ignores:
- `continuity_warning` ignored 5+ times → demote from Error to Warning severity
- `canon_conflict` ignored 5+ times → demote from Warning to Info
- Only apply when author has explicitly accepted/rejected at least 10 proposals total.

Add `diagnostic_ignore_penalty(category: &DiagnosticCategory, memory: &WriterMemory) -> DiagnosticSeverity`.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: adjust diagnostic severity based on author ignore patterns`

---

### Tasks 4-6: 3 New Evals

**Files:** Create 3 eval files, register in evals.rs + main.rs

- `agent-evals/src/evals/feedback_planner.rs` — simulate 5 rejections of plot_promise kind, verify penalty < 1.0
- `agent-evals/src/evals/feedback_ghost.rs` — simulate accepted style preference, verify ghost boost > 1.0
- `agent-evals/src/evals/feedback_diagnostics.rs` — simulate ignored warnings, verify severity demotion

Register all 3. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 300

Commit: `feat: add 3 feedback learning evals`

### Task 7: Baseline

Change `297/297` to `300/300`. Run `npm run baseline`.
Commit: `chore: update baseline after feedback learning sprint`
