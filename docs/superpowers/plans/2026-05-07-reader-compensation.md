# Reader Compensation Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Per-chapter settlement produces a ReaderTakeaway — what the reader felt, expects, and is missing. Feeds TodayFive and promise planner.

**Architecture:** 1 new settlement delta field + 1 TodayFive enrichment + 1 planner factor + 3 evals. No new tables.

---

### Task 1: ReaderTakeaway Type + Settlement Integration

**Files:** `types_and_utils.in.rs`, `settlement.in.rs`, `settlement_apply.rs`

Add `ReaderTakeaway` struct to `types_and_utils.in.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReaderTakeaway {
    pub emotional_beat: String,       // "紧张", "感动", "好奇"
    pub expectation: String,          // "期待林墨揭露真相"
    pub unresolved_lack: String,      // "还没交代寒玉戒指的下落"
}
```

Add `pub reader_takeaway: Option<ReaderTakeaway>` to `ChapterSettlementDelta`.

In `settlement.in.rs` extraction, derive a minimal takeaway:
- `emotional_beat` from chapter_result.summary sentiment keywords
- `expectation` from open promises nearest to payoff
- `unresolved_lack` from stale promises (>5 chapters)

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add ReaderTakeaway to settlement delta`

### Task 2: TodayFive — Reader Expectations

**Files:** `today_five.in.rs`

Enrich the "next" slot with reader expectation from the last chapter's takeaway. If no takeaway exists, preserve existing behavior.

```rust
let reader_hint = latest_settlement.and_then(|s| s.reader_takeaway.as_ref())
    .map(|t| format!("读者期待: {}", t.expectation))
    .unwrap_or_default();
```

Append to the next slot's value or detail.

Commit: `feat: enrich TodayFive next slot with reader expectation`

### Task 3: Promise Planner — Reader-Driven Promise Creation

**Files:** `promise_planner.rs`

Add a factor that boosts promise priority when it matches current reader expectations. If a promise's description or payoff text contains the same keywords as the reader takeaway's `expectation`, boost ×1.2.

```rust
fn reader_expectation_boost(promise: &PlotPromiseSummary, takeaway: &Option<ReaderTakeaway>) -> f64 {
    if let Some(t) = takeaway {
        if promise.description.contains(&t.expectation) || promise.expected_payoff.contains(&t.expectation) {
            return 1.2;
        }
    }
    1.0
}
```

Commit: `feat: boost promises matching reader expectations`

### Tasks 4-6: 3 New Evals

- `reader_takeaway_contract` — create chapter, extract settlement, verify takeaway is Some with non-empty fields
- `reader_todayfive_contract` — verify TodayFive next slot includes reader expectation text
- `reader_planner_contract` — create promise matching takeaway expectation, verify boost > 1.0

Register all 3. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 303

Commit: `feat: add 3 reader compensation evals`

### Task 7: Baseline

Change `300/300` to `303/303`. Commit: `chore: baseline after reader compensation`
