# Project Status

Last updated: 2026-05-02

## Positioning

Forge is a Cursor-style writing agent for novels, not a generic writing tool. The editor is only the workbench. The product value is the persistent co-writer that knows the book, tracks commitments, watches continuity, proposes typed changes, and learns from what the author accepts or rejects.

## What Is Solid Now

- Rust workspace is rooted at `C:\Users\Msi\Desktop\Forge`; the authoritative lockfile is the root `Cargo.lock`.
- `agent-harness-core` contains reusable agent runtime pieces: provider abstraction, tool registry, tool executor, compaction, permissions, prompt/cache helpers, vector DB, run trace, task packets, and ambient loop primitives.
- `src-tauri/src/writer_agent` contains the product-specific Writer Agent Kernel:
  - observations
  - intent
  - context packing
  - canon diagnostics
  - typed operations
  - proposal queue
  - feedback
  - memory
  - trajectory export
- The five foundation axes are represented by `TaskPacket` and enforced in trace/eval paths.
- Chapter generation records task packets and feeds successful generated chapters into the Result Feedback Loop.
- Story Contract, Chapter Mission, Result Feedback Loop, Promise Ledger, and Companion Panel quiet mode are implemented enough to be active product foundations.
- Production CSP is no longer null and no longer allows localhost or `unsafe-eval`.
- Frontend API-key handling does not expose the stored raw key.
- Recent save-flow hardening prevents accepted text feedback from being recorded before text is durably saved.
- Recent editor hardening prevents stale autosaves from marking newer edits clean, blocks wrong-chapter inline operations, and stops chapter switching if current chapter save fails.
- Storage writes now use per-target write guards and unique temporary files instead of a shared `.tmp` path, reducing concurrent autosave/generation/restore collision risk.
- Inline AI operations now render as decoration-based previews and only enter manuscript text after acceptance and a successful save.
- Story Contract and current Chapter Mission now have a Foundation editing surface in the Companion Panel; saves go through typed WriterOperations into Writer Agent memory.
- Write-capable WriterOperations for memory, promises, foundation, canon, and outline now require surfaced approval context and record an approval decision before execution.
- Legacy direct file-write commands for chapters, lore, outline, backup restore, and chapter rename now record Writer Agent audit decisions after successful writes.

## Current Verification Baseline

The expected local baseline is:

- `cargo test -p agent-writer`: 148 passing
- `cargo run -p agent-evals`: 46/46 passing
- `npm run check:p2`: 8/8 passing
- `npm run lint`: passing
- `npm run build`: passing
- `git diff --check`: passing

`cargo run -p agent-evals` writes a local report under `reports/`; report directories are ignored and should not be committed.

## Cleanliness Decisions

- Keep root `Cargo.lock`.
- Remove and ignore `src-tauri/Cargo.lock`; it is a duplicate member lockfile because the workspace root is the repository root.
- Keep `.env`, `node_modules`, `dist`, `target`, `.worktrees`, and report directories ignored.
- Do not run broad `git clean -X` in this repo because it would delete `.env`, dependencies, build caches, and worktrees.

## Remaining Gaps

- `src-tauri/src/lib.rs` is still too large and should be split into command modules after the save-flow risks are fully closed.
- `ask_agent` manual requests now create WriterObservations, run Writer Agent Kernel observation, use ManualRequest context packs, persist manual exchanges, and then execute through the older agent loop; the remaining gap is retiring that execution layer once the kernel can own the full run loop.
- Story Contract and Chapter Mission now have basic authoring/editing UX; the remaining gap is richer guidance, validation, and per-chapter navigation for missions.
- Tool policy now has surfaced approval context for WriterOperation writes and audit coverage for legacy direct save commands; the remaining gap is richer policy rules per operation class and eventually routing more saves through typed operations.
