# Forge Code Quality 全面治理 — Design Spec

Date: 2026-05-05 | Branch: main

## Goal

Systematic code quality overhaul across the Forge Rust workspace: clippy zero, large file splitting, architecture cleanup. Three sprints, each independently mergeable.

## Sprint 1: Clippy Zero + Gate

| Warning | File | Fix |
|---------|------|-----|
| `result_large_err` (4x) | chapter_generation.rs | Box large error variants |
| `too_many_arguments` (8/7) | chapter_generation.rs | Extract config struct |
| `nonminimal_bool` | commands/writer_agent.rs | Simplify boolean logic |
| `unnecessary_sort_by` | storage.rs | Use sort_by_key |

Plus: `[lints.clippy] warnings = "deny"` in Cargo.toml to prevent regression.

## Sprint 2: Large File Splitting

| File | Lines | Target |
|------|-------|--------|
| memory.rs | 3382 | memory/{model,vector_store,chunk_ops,recall,consolidation}.rs |
| chapter_generation.rs | 2313 | chapter_generation/{error,context,pipeline,validation,save_ops,outline}.rs |
| brain_service.rs | 2004 | brain_service/{index,search,chunk_writer,project_brain}.rs |
| kernel/tests.rs | 1910 | kernel/tests/{context_pack,proposals,observations,run_loop,...}_tests.rs |
| context.rs | 1450 | context/{...}.rs |
| storage.rs | 1434 | storage/{...}.rs |

Principle: each child file 200-400 lines, high cohesion, low coupling.

## Sprint 3: Architecture Cleanup

- Move 12 `kernel_*.rs` flat files into `kernel/` sub-module
- Merge duplicates (kernel_proposals + proposals, kernel_run_loop + run_loop)
- Organize writer_agent/mod.rs with section comments
- Expand lib.rs public API surface

## Verification

Each sprint: `cargo test --workspace` passes + `cargo clippy --workspace` zero warnings.
