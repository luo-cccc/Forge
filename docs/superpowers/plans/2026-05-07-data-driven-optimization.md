# Data-Driven Optimization Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Fix promise explosion, add confidence thresholds, harden length constraints, wire CompiledInput into stress test, validate strategy selector.

**Architecture:** 5 surgical changes. No new tables. 4 evals.

---

### Task 1: Promise Dedup by Topic

**Files:** `src-tauri/src/chapter_generation/settlement.in.rs`

Add `deduplicate_promise_candidates()` that merges promise candidates with the same `title` by picking the highest-confidence entry and discarding duplicates. Run BEFORE `materialize_promise_delta_entries()`.

```rust
fn deduplicate_promise_candidates(candidates: &mut Vec<ChapterPromiseExtractionCandidate>) {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut kept: Vec<ChapterPromiseExtractionCandidate> = Vec::new();
    for c in candidates.drain(..) {
        let key = c.title.clone();
        if let Some(&idx) = seen.get(&key) {
            if c.confidence > kept[idx].confidence {
                kept[idx] = c;
            }
        } else {
            seen.insert(key, kept.len());
            kept.push(c);
        }
    }
    *candidates = kept;
}
```

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: deduplicate promise candidates by topic in settlement extraction`

### Task 2: Promise Extraction Confidence Threshold

**Files:** `src-tauri/src/chapter_generation/settlement.in.rs`

After dedup, filter out promise candidates with confidence < 0.6. Also add a hard cap of 15 promise candidates per chapter to prevent explosion on very long chapters.

```rust
candidates.retain(|c| c.confidence >= 0.6);
candidates.truncate(15);
```

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add confidence threshold and cap to promise extraction`

### Task 3: Hard Length Constraint in Stress Test Prompt

**Files:** `agent-evals/src/bin/full_stress.rs`

Add explicit length guard to system prompt:
```
"严格输出 {}—{} 字。如果超过上限，立即在段落边界截断。"
```

Also add `max_tokens` estimation based on chars (1 char ≈ 2 tokens for Chinese).

Verify: `cargo check -p agent-evals --bin full_stress`
Commit: `feat: add hard length constraint to stress test prompt`

### Task 4: Wire CompiledInput into Stress Test

**Files:** `agent-evals/src/bin/full_stress.rs`

After chapter generation, build a minimal CompiledInput from the chapter data and include it in the system prompt for the NEXT chapter. This tests whether having structured context improves length compliance.

Add after each completed chapter:
```rust
let next_compiled = format!("上一章写了{}字。当前线索: {}条。角色状态: 林墨活跃。", chars, prom_count);
// Inject into next chapter's system prompt
```

Simple: append `next_compiled` to the next chapter's user message.

Verify: `cargo check -p agent-evals --bin full_stress`
Commit: `feat: wire chapter context into stress test prompt`

### Task 5: Strategy Selector Validation Eval

**Files:** `agent-evals/src/evals/strategy_validation.rs`

Back-test the strategy selector against the stress test data pattern:
- chars=5771, repair=0 → should select `BackgroundLongChapter`
- chars=2991, repair=0 → should select `InteractiveSafeDraft` (under, not fast)
- chars=3737, repair=0 → should select `InteractiveFastDraft`

Register eval. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 320

Commit: `feat: add strategy selector validation eval`

### Task 6: Baseline
Change `319/319` to `320/320`. Commit: `chore: baseline after data-driven optimization`
