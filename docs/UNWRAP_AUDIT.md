# Production `.unwrap()` Audit

**Date:** 2026-05-05
**Scope:** `src-tauri/`, `agent-harness-core/`, `agent-evals/` workspace crates
**Method:** `awk` per-file scan, terminating at first `#[cfg(test)]` marker — excludes test-block unwraps

## Summary

| Severity | Count |
|----------|------:|
| CRITICAL | 0 |
| HIGH | 0 |
| MEDIUM | 0 |
| **LOW** | **5** |
| **Total** | **5** |

The earlier coarse grep reported 122 prod unwraps. After excluding `#[cfg(test)]` blocks within mixed source files, the true production count is **5**, all in low-risk patterns where panic is mathematically impossible or guarded by upstream invariants.

## Findings

### LOW-1 — `feedback_methods.in.rs:15`

```rust
serde_json::to_string(alternatives).unwrap(),
```

- **File:** `src-tauri/src/writer_agent/memory/feedback_methods.in.rs`
- **Argument type:** `&[String]` (owned slice of plain strings)
- **Failure mode:** `serde_json::to_string` only fails on serialization-trait errors; `&[String]` cannot trigger any.
- **Recommendation:** Leave as-is. Optionally swap to `.unwrap_or_default()` for defense-in-depth.

### LOW-2 — `feedback_methods.in.rs:17`

```rust
serde_json::to_string(sources).unwrap()
```

- Same pattern as LOW-1; same disposition.

### LOW-3 — `canon_methods.in.rs:11`

```rust
let aliases_json = serde_json::to_string(aliases).unwrap();
```

- **File:** `src-tauri/src/writer_agent/memory/canon_methods.in.rs`
- **Argument type:** `&[String]`
- **Failure mode:** identical to LOW-1.
- **Recommendation:** Leave as-is.

### LOW-4 — `context_fetcher.rs:83`

```rust
cache.last_updated = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64;
```

- **File:** `src-tauri/src/ambient_agents/context_fetcher.rs`
- **Failure mode:** `duration_since(UNIX_EPOCH)` only fails when the system clock is set before 1970-01-01. Not realistic on any deployment target.
- **Recommendation:** Replace with `.expect("system clock must be after 1970-01-01")` for documentation value.

### LOW-5 — `kernel.rs:426`

```rust
session_id: format!(
    "session-{}",
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
),
```

- **File:** `src-tauri/src/writer_agent/kernel.rs`
- **Failure mode:** identical to LOW-4.
- **Recommendation:** Replace with `.expect("system clock must be after 1970-01-01")`.

## Disposition

- All 5 are **LOW** severity — leave as-is or swap to `.expect("...")` with rationale.
- **No `?` propagation needed** in any of the 5 sites: callers do not have a meaningful Result path for "system clock is impossible" or "JSON serialization of `&[String]` failed."
- The 67-file / 122-call earlier grep figure was inflated by `#[cfg(test)]` blocks in mixed source files (e.g., `tool_registry.rs`, `kernel.rs`, `agent_loop.rs`).

## Verification Command

```bash
for f in <files-listed-above>; do
  awk 'BEGIN{cfg=0} /#\[cfg\(test\)\]/{cfg=1} /\.unwrap\(\)/ && !cfg {print FNR": "$0}' "$f"
done
```

Reproducing this command on the four files above must continue to yield exactly 5 hits. Any new hit triggers re-audit.
