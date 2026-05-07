# Perf Optimization Sprint 1: Context Efficiency — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Reduce chapter generation latency by eliminating redundant context rebuilds — CompiledInput enters prompt, chapter gen gets ContextSpine, FocusPack incremental refresh.

**Architecture:** 3 work items → 4 gates. No new tables. Changes to context assembly and generation pipeline.

**Quality redline:** Anchor carry must not regress. Repair rate must not rise.

---

### Task 1: CompiledInput Enters Chapter Generation Prompt

**Files:** `context.in.rs`, `pipeline/main.in.rs`

`CompiledInput` already exists in `BuiltChapterContext`. Wire it into the chapter generation prompt:
- In `context.in.rs`, find where the prompt system message is built. Append a compact `CompiledInput` block at the end.
- Format: `"\n---\n## 本章生成计划\n意图: {intent_text}\n证据: {selected_evidence}\n规则: {rule_stack}\n---"`.
- Only append when `compiled_input.is_some()`.

**Gate:** `chapter_generation_compiled_input_enters_prompt` — verify CompiledInput fields appear in built prompt context.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: wire CompiledInput into chapter generation prompt`

### Task 2: Chapter Generation Context Spine

**Files:** `context.in.rs`, `context/spine.in.rs`, `pipeline/main.in.rs`

Adapt the existing `ContextSpine` (used for ghost/observation contexts) for chapter generation:
- Add method `build_chapter_generation_spine(memory, target, compiled_input) -> ContextSpine`.
- Layers:
  - `FrozenPrefix`: system chapter-generation contract, output protocol
  - `ProjectStablePrefix`: Story Contract, Author Style, long-term Canon/Promise short summaries
  - `FocusPack`: current chapter mission, result feedback, next beat, story impact radius, selected Project Brain evidence
  - `HotBuffer`: current user instruction, target existing text, explicit override summary
- In `build_chapter_context()`, use the spine instead of building prompt from scratch.
- Add `cache_stability_report()` to the built context: track stable prefix chars, dynamic tail chars.

**Gate:** `chapter_generation_stable_prefix_reuse` — verify stable prefix is not rebuilt for consecutive same-volume chapters.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add ContextSpine-based chapter generation context assembly`

### Task 3: FocusPack Incremental Refresh

**Files:** `context.in.rs`, `pipeline/main.in.rs`

Add `FocusState` tracking to avoid rebuilding FocusPack when nothing changed:
- Track: current chapter title, current scene id, result feedback hash, next beat hash, story impact radius hash.
- On each generation request, compare current state with previous. If none of the tracked fields changed, reuse existing FocusPack.
- Only rebuild FocusPack + HotBuffer when:
  - chapter switch
  - scene switch
  - selected evidence materially changed
  - result feedback / next beat changed
  - story impact radius materially changed

Add `pub focus_pack_rebuild_count: usize` to `BuiltChapterContext`.

**Gate:** `chapter_generation_focus_pack_rebuild_only` — simulate 3 consecutive same-chapter generations, verify only 1 FocusPack rebuild.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add FocusPack incremental refresh for chapter generation`

### Tasks 4-7: 4 New Evals + Telemetry

**Files:** Create 4 eval files + register

- `compiled_input_prompt_eval` — BuiltChapterContext with CompiledInput → verify prompt contains intent/evidence/rules
- `stable_prefix_reuse_eval` — 2 consecutive same-volume generations → verify stable prefix chars constant
- `focus_rebuild_eval` — 3 same-chapter generations → verify focus_pack_rebuild_count = 1
- `spine_telemetry_eval` — verify BuiltChapterContext has cache_stability fields (stable_prefix_chars, dynamic_tail_chars)

Register all 4. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 311

Commit: `feat: add 4 context efficiency evals`

### Task 8: Baseline

Change `307/307` to `311/311`. Commit: `chore: baseline after perf sprint 1`
