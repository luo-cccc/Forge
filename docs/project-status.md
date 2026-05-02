# Project Status

Last updated: 2026-05-02

## Positioning

Forge is a Cursor-style writing agent for novels, not a generic writing tool. The editor is only the workbench. The product value is the persistent co-writer that knows the book, tracks commitments, watches continuity, proposes typed changes, and learns from what the author accepts or rejects.

## P0 Status (May 2026): Unified Writer Agent Kernel Brain

P0 is complete:

- **P0.1 (Unified Run Loop)**: `AgentLoop::new` now lives only in `writer_agent/kernel.rs`. `ask_agent` in lib.rs calls `kernel.prepare_task_run()` + `prepared_run.run()` — no direct agent-loop construction in the command layer. `WriterAgentRunRequest` / `WriterAgentRunResult` types are defined in the kernel.
- **P0.2 (Unified Action Lifecycle)**: `WriterOperationLifecycleState` (Proposed → Approved → Applied → DurablySaved → FeedbackRecorded) and `WriterOperationLifecycleTrace` track full lifecycle. `apply_feedback()` enforces durable-save-before-feedback for positive feedback. All write-capable operations push lifecycle entries.
- **P0.3 (Command Boundary Audit)**: 47 `#[tauri::command]` functions classified by risk level (destructive/manuscript-write/memory-write/provider-call/credential/read-only). Static audit check at `scripts/check-command-audit.cjs` runs as part of `npm run verify`. All legacy direct-save commands reference `audit_project_file_write`.

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
  - unified task execution (prepare_task_run / run_task)
  - operation lifecycle tracking
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
- Manual `ask_agent` requests now run through WriterAgentKernel.prepare_task_run() with ManualRequest tool boundary: project context tools only, no approval-required writes, no chapter-generation write tools.
- Story Contract and Chapter Mission writes now have kernel-level quality gates, so vague or incomplete foundation memory is rejected before it can pollute context packs.
- Operation lifecycle is tracked end-to-end: proposed → approved → applied → durably_saved → feedback_recorded, with durable-save-before-feedback enforcement.
- A static command boundary audit classifies all 47 Tauri commands by risk level and verifies audit coverage for write paths.

## Current Verification Baseline

The expected local baseline is:

- `cargo test -p agent-writer`: 153 passing
- `cargo test -p agent-harness-core`: 79 passing
- `cargo run -p agent-evals`: 51/51 passing
- `npm run check:p2`: 8/8 passing
- `npm run check:audit`: 47 commands, 0 issues
- `npm run lint`: passing
- `npm run build`: passing
- `cargo fmt --all -- --check`: passing
- `git diff --check`: passing

`npm run verify` runs all of the above. `cargo run -p agent-evals` writes a local report under `reports/`; report directories are ignored and should not be committed.

## Cleanliness Decisions

- Keep root `Cargo.lock`.
- Remove and ignore `src-tauri/Cargo.lock`; it is a duplicate member lockfile because the workspace root is the repository root.
- Keep `.env`, `node_modules`, `dist`, `target`, `.worktrees`, and report directories ignored.
- Do not run broad `git clean -X` in this repo because it would delete `.env`, dependencies, build caches, and worktrees.

## Remaining Gaps

- `src-tauri/src/lib.rs` is still too large and should be split into command modules after the save-flow risks are fully closed.
- Story Contract and Chapter Mission now have basic authoring/editing UX; the remaining gap is richer guidance, validation, and per-chapter navigation for missions (P1).
- Tool policy now has surfaced approval context for WriterOperation writes and audit coverage for legacy direct save commands; the remaining gap is richer policy rules per operation class (P1).
- Companion Panel needs to be further simplified to show only the top 3-5 highest-value signals (P1).
- No multi-chapter scenario evals yet (5+ chapter fixtures needed for P1 product validation).
- No product metrics recording yet (proposal acceptance rate, promise recall hit rate, etc.) (P1).
- `kernel.rs` (6834 lines), `lib.rs` (4039 lines), and `run_eval.rs` (3272 lines) need modular splitting (P2).
