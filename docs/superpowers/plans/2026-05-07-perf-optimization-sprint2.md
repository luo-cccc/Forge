# Perf Optimization Sprint 2: Strategy & Selection — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Further reduce chapter generation latency by: making previous-chapter fulltext opt-in (risk-gated), using StoryImpactRadius for evidence selection, and upgrading preflight to a strategy selector.

**Architecture:** 3 work items → 4 gates. No new tables. Quality redline: anchor carry + repair rate must not regress.

---

### Task 1: Previous Chapter — Structured Result First, Fulltext on Risk

**Files:** `context.in.rs`

Modify `build_chapter_context()`:
- Default: read `ChapterResultSummary` + `NextBeat` + `reader_takeaway` + `settlement delta` instead of previous chapter fulltext.
- Upgrade to fulltext only when:
  - `continuity_risk == "high"` (from story debt snapshot)
  - `unresolved_debt_density > 3` (open promises / recent chapters > 3)
  - `previous_structured_evidence_insufficient` (result summary is None or empty)
- Add `previous_fulltext_upgrade_count: usize` and `previous_fulltext_upgrade_reason: String` to telemetry.

The spine's `FocusPack` already contains result feedback and next beat. The change is: don't add the full previous chapter text to the prompt unless risk-gated.

**Gate:** `chapter_generation_previous_fulltext_upgrade_only_on_risk` — verify that default build does NOT include fulltext, but high-risk scenario does.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: gate previous chapter fulltext on continuity risk`

### Task 2: StoryImpactRadius as Evidence Selection Pre-Filter

**Files:** `context.in.rs`, `context_relevance/typed_filter.in.rs`

In `build_chapter_context()`, before the existing lore/RAG/previous-chapter evidence selection:
- Call `story_impact_radius` (already exists in writer_agent) to get impacted nodes for the current chapter.
- Use the impacted node IDs as a cheap pre-filter: only include evidence that relates to impacted characters, relationships, knowledge items, or scenes.
- Extend `typed_filter.in.rs` with a `story_impact_filter()` that takes impact radius nodes and returns a filtered evidence list.
- Fallback: if impact radius is empty or unavailable, fall back to the existing typed filter.

Add `impact_scoped: bool` and `impact_filtered_count: usize` to telemetry.

**Gate:** `chapter_generation_story_impact_scoped_recall` — verify impact-scoped recall returns fewer but more relevant results than unfiltered recall.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: use StoryImpactRadius as evidence selection pre-filter`

### Task 3: Preflight → Generation Strategy Selector

**Files:** `pipeline/main.in.rs`, `types_and_utils.in.rs`

Add `GenerationStrategy` enum:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStrategy {
    InteractiveFastDraft,    // low risk, fast path
    InteractiveSafeDraft,    // moderate risk, standard path
    BackgroundLongChapter,   // high risk or long chapter, full path
    RepairHeavyMode,         // very high repair history, extra checks
}
```

In preflight (before building chapter context), select strategy based on:
- `context_total_chars < 8000 && impact_truncated == false && repair_rate_low` → `InteractiveFastDraft`
- `context_total_chars > 15000 || impact_truncated` → `BackgroundLongChapter`
- `repair_count_recent > 2` → `RepairHeavyMode`
- Default → `InteractiveSafeDraft`

Add `generation_strategy` to `BuiltChapterContext` and `ChapterGenerationEvent`.

**Gate:** `chapter_generation_strategy_selection_consistency` — verify that different context sizes produce different strategy selections.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add generation strategy selector to preflight`

### Tasks 4-7: 4 New Evals

Create these eval files:
- `agent-evals/src/evals/previous_fulltext_gate.rs` — default build without fulltext, high-risk build with fulltext
- `agent-evals/src/evals/impact_scoped_recall.rs` — impact filter returns subset of unfiltered results
- `agent-evals/src/evals/strategy_selection.rs` — different context sizes → different strategies
- `agent-evals/src/evals/anchor_regression.rs` — verify anchor carry doesn't degrade after changes

Register all 4. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 315
Commit: `feat: add 4 strategy selection evals`

### Task 8: Baseline
Change `311/311` to `315/315`. Commit: `chore: baseline after perf sprint 2`
