# Emotional Debt Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Make Emotional Debt consumable — extraction in settlement, payoff boost in planner, visibility in TodayFive, overdue detection in diagnostics.

**Architecture:** 4 surgical insertions into existing code paths. No new tables (table + methods already exist). 4 evals.

---

### Task 1: Settlement — Extract Emotional Debt

**Files:** `settlement.in.rs`

In `build_basic_chapter_settlement_delta`, scan `chapter_result.new_conflicts` and `chapter_result.state_changes` for emotional pressure cues (contains "愤怒", "悲伤", "背叛", "恐惧", "失去", "悔恨" etc.). When found, create an emotional debt tag in the settlement extraction. Add a simple field to `ChapterSettlementExtraction` if needed: `pub emotional_debt_cues: Vec<String>`.

This doesn't create debt rows (methods already exist for that) — it just tags what the chapter contains.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: extract emotional debt cues in settlement`

### Task 2: Promise Planner — Emotional Debt Boost

**Files:** `promise_planner.rs`

Add `emotional_debt_proximity_boost()`:
- Read emotional debt ledger entries via `memory` methods (check `emotional_debt_ledger_methods.in.rs` for available methods).
- If a debt is unpaid and the current chapter is within 2 chapters of its `trigger_chapter`, boost ×1.3.
- If debt has been unpaid for >10 chapters, boost ×1.5 (overdue).
- Multiply into `promise_subject_pressure`.

Commit: `feat: add emotional debt proximity boost to planner`

### Task 3: TodayFive — Emotional Debt Visibility

**Files:** `today_five.in.rs`

In guard item detail, add a count of unpaid emotional debts:
```rust
let unpaid_debt_count = memory.count_unpaid_emotional_debts().unwrap_or(0);
if unpaid_debt_count > 0 {
    guard_detail = format!("{}\n{} 条未偿还的情绪债", guard_detail, unpaid_debt_count);
}
```

If `count_unpaid_emotional_debts` doesn't exist, iterate the existing ledger methods to count.

Commit: `feat: show unpaid emotional debt count in TodayFive`

### Task 4: Diagnostics — Overdue Emotional Debt

**Files:** `diagnostics/core.in.rs`

After existing checks, add: scan emotional debt ledger for debts where `chapters_since_trigger > 10` and `paid = false`. Flag as `OverdueEmotionalDebt` diagnostic.

Commit: `feat: detect overdue emotional debt in diagnostics`

### Tasks 5-8: 4 New Evals

- `emotional_debt_extraction` — create chapter result with emotional keywords, verify cues extracted
- `emotional_debt_planner` — create unpaid debt near payoff, verify boost > 1.0
- `emotional_debt_todayfive` — create unpaid debts, verify guard detail includes count
- `emotional_debt_diagnostics` — create overdue debt, verify diagnostic produced

Register all 4. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 307

Commit: `feat: add 4 emotional debt evals`

### Task 9: Baseline
Change `303/303` to `307/307`. Commit: `chore: baseline after emotional debt`
