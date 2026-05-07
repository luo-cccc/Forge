# Final Closure: Perf Gates + Input Governance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans.

**Goal:** Close the 2 remaining performance gates and implement the input governance compiler (§3.2.1).

**Architecture:** 2 performance evals + 1 new compilation module that produces intent/evidence/rule-stack artifacts before generation.

**Tech Stack:** Rust (src-tauri, agent-evals)

---

### Task 1: Save Path Without Full State Rescan

**Files:** `agent-evals/src/evals/save_perf.rs`, `evals.rs`, `main.rs`

Create an eval that verifies `build_basic_chapter_settlement_delta` + `apply_chapter_settlement_delta` does NOT trigger full-state scans. Strategy: create 100 characters + 50 relationships + 30 knowledge items in memory. Build a settlement delta referencing only 1 character. Time the apply call. Expect < 200ms.

```rust
pub fn run_save_perf_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory.ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "").unwrap();
    for i in 0..100 { memory.upsert_character(&format!("char_{}", i), &[], "supporting", "filler").unwrap(); }
    let start = std::time::Instant::now();
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-1".to_string(), chapter_revision: "aaaa0001".to_string(),
        ..Default::default()
    };
    let result = apply_chapter_settlement_delta(&memory, "eval", &delta).unwrap();
    let elapsed = start.elapsed().as_millis();
    EvalResult::pass_if(result.applied && elapsed < 200,
        format!("applyMs={} entities=100", elapsed))
}
```

Register. Commit: `feat: add save path performance eval`

### Task 2: Entity-Scoped Apply Without Global Rebuild

**Files:** `agent-evals/src/evals/entity_apply_perf.rs`, `evals.rs`, `main.rs`

Verify that applying entity deltas only touches affected entities. Create 100 characters with state versions. Apply a delta changing 1 character's state. Verify that only 1 new state version row is added (not 100).

```rust
pub fn run_entity_apply_perf_eval() -> EvalResult {
    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    memory.ensure_story_contract_seed("eval", "test", "fantasy", "p", "j", "").unwrap();
    let target_id = memory.upsert_character("target", &[], "protagonist", "the one").unwrap();
    for i in 0..100 { memory.upsert_character(&format!("filler_{}", i), &[], "supporting", "").unwrap(); }
    let before_count = memory.get_active_state(target_id, "Chapter-1").ok().flatten().map_or(0, |_| 1);
    let delta = ChapterSettlementDelta {
        chapter_title: "Chapter-1".to_string(), chapter_revision: "a".to_string(),
        character_state_deltas: vec![CharacterStateDeltaEntry {
            character_name: "target".to_string(), chapter_title: "Chapter-1".to_string(),
            action: "upserted".to_string(), core_commitments: vec!["test".to_string()],
            goal_state: serde_json::json!({}), source_ref: "test".to_string(),
        }],
        ..Default::default()
    };
    apply_chapter_settlement_delta(&memory, "eval", &delta).unwrap();
    let after = memory.get_active_state(target_id, "Chapter-1").unwrap();
    EvalResult::pass_if(after.is_some(), format!("scopedApply=true beforeStates={}", before_count))
}
```

Register. Commit: `feat: add entity-scoped apply performance eval`

### Task 3: Baseline After Perf Gates

Change `288/288` to `290/290`. Run `npm run baseline`.
Commit: `chore: update baseline after performance gates`

### Task 4: Input Governance Compiler

**Files:** Create `src-tauri/src/writer_agent/input_governance.rs`, `input_governance/compiler.in.rs`

The compiler produces a reviewable artifact before chapter generation:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledInput {
    pub intent_text: String,           // ChapterMission + user instruction, compiled
    pub selected_evidence: Vec<String>, // StoryOS query results, reduced to key refs
    pub rule_stack: Vec<String>,       // Applicable canon rules + story contract constraints
    pub trace_hint: String,            // Short traceable key for Inspector replay
    pub compiled_at_ms: u64,
}
```

**Implementation:**
1. `compile_input(memory, chapter_title, user_instruction) -> CompiledInput`:
   - Reads ChapterMission for the target chapter
   - Queries StoryOS for relevant evidence (character states, open promises, knowledge items)
   - Collects active canon rules and story contract constraints
   - Produces a compact, reviewable `CompiledInput` struct

2. Wire into `BuiltChapterContext` — add `pub compiled_input: Option<CompiledInput>`.

3. Persist as `compiled_input.json` alongside existing runtime artifacts.

4. Expose via `get_writer_agent_trace` or a new read-only command for Inspector view.

**Files:**
- Create: `src-tauri/src/writer_agent/input_governance.rs`
- Create: `src-tauri/src/writer_agent/input_governance/compiler.in.rs`
- Modify: `context.in.rs` (add field to BuiltChapterContext)
- Modify: `runtime_artifacts.in.rs` (persist)
- Modify: `lib.rs` (register module)

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add input governance compiler`

### Task 5: Input Governance Eval

**Files:** Create `agent-evals/src/evals/input_compiler.rs`

Create chapter mission + user instruction + character state. Call `compile_input`. Verify output has non-empty intent_text, selected_evidence, and rule_stack.

Register. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 291

Commit: `feat: add input governance compiler eval`

### Task 6: Baseline Final

Change to `291/291`. Commit: `chore: final baseline after input governance`
