# Project Status

Last updated: 2026-05-03

## Positioning

Forge is a Cursor-style writing agent for novels, not a generic writing tool. The editor is only the workbench. The product value is the persistent co-writer that knows the book, tracks commitments, watches continuity, proposes typed changes, and learns from what the author accepts or rejects.

## P0 Status (May 2026): Unified Writer Agent Kernel Brain

P0 is complete:

- **P0.1 (Unified Run Loop)**: `AgentLoop::new` now lives only behind `WriterAgentKernel.prepare_task_run()` / `WriterAgentPreparedRun.run()`. `ask_agent` in lib.rs calls the kernel path, with no direct agent-loop construction in the command layer. `WriterAgentRunRequest` / `WriterAgentRunResult` types are re-exported through `writer_agent::kernel` and implemented in `writer_agent/kernel_run_loop.rs`.
- **P0.2 (Unified Action Lifecycle)**: `WriterOperationLifecycleState` (Proposed → Approved → Applied → DurablySaved → FeedbackRecorded) and `WriterOperationLifecycleTrace` track full lifecycle. `apply_feedback()` enforces durable-save-before-feedback for positive feedback. All write-capable operations push lifecycle entries.
- **P0.3 (Command Boundary Audit)**: 47 `#[tauri::command]` functions classified by risk level (destructive/manuscript-write/memory-write/provider-call/credential/read-only). Static audit check at `scripts/check-command-audit.cjs` runs as part of `npm run verify` and covers `pub async fn` command handlers. All legacy direct-save commands reference `audit_project_file_write`.

## P1 Status (May 2026): Trust Contract And Product Validation

P1 is in progress:

- Story Contract quality now has explicit Missing / Vague / Usable / Strong states and vague contracts are excluded from context packs.
- Chapter Mission save calibration can mark completed, drifted, or needs_review states based on save observations.
- Promise Ledger now classifies promise kinds including plot promise, emotional debt, object whereabouts, character commitment, mystery clue, and relationship tension.
- Writer Agent trajectory now exports derived product metrics such as proposal acceptance rate, ignored suggestion rate, promise recall hit rate, canon false-positive rate, chapter mission completion rate, durable save success rate, and save-to-feedback latency.
- Companion Panel write mode remains quiet and now summarizes acceptance/save health instead of exposing raw operation traces.
- `agent-evals` now includes 10 long-form product scenario checks in `agent-evals/src/product_scenarios.rs`, covering multi-chapter promise tracking, result feedback handoff, payoff timing, resolved-promise quieting, object whereabouts, mission drift, canon guardrails, style feedback, decision metrics, and context explainability. More realistic fixtures are still needed before product value can be called proven.
- Writer Agent context relevance ranking now prioritizes Canon / Promise ledger slices by mission, next beat, result feedback, recent decisions, cursor-local story signals, open promises, and lightweight scene-type matches, with `WHY writing_relevance` explanations on retrieved entries. Project Brain / vector chunks are now reranked after semantic retrieval using the same writing-focus, scene-type, and avoid-term signals; standalone `query_project_brain` also injects active WriterMemory focus into retrieval and rerank.

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
  - derived product metrics
- The five foundation axes are represented by `TaskPacket` and enforced in trace/eval paths.
- All Tauri command handlers now live under `src-tauri/src/commands/*`; `src-tauri/src/lib.rs` currently has 0 `#[tauri::command]` handlers.
- Shared app state and startup memory initialization now live in `src-tauri/src/app_state.rs`, including AppState, lock helpers, Hermes/Writer memory DB opening, legacy DB migration, and kernel seed logic.
- Semantic lint payload/event handling and lore/diagnostic lint logic now live in `src-tauri/src/semantic_lint.rs`.
- Manual context injection, user profile reads, chapter embedding, skill extraction, and LLM memory candidate generation now live in `src-tauri/src/memory_context.rs`.
- Agent/editor/manual observation payloads and WriterObservation conversion now live in `src-tauri/src/observation_bridge.rs`.
- Editor realtime ghost rendering, ambient output forwarding, editor prediction cleanup, realtime cowrite gating, and LLM ghost proposal flow now live in `src-tauri/src/editor_realtime.rs`.
- API key resolution, path helpers, event constants, event payloads, Agent status payloads, project write audit helpers, and chapter-save observation/canon-refresh/context-render helpers now live in focused root helper modules (`api_key.rs`, `app_paths.rs`, `events.rs`, `event_payloads.rs`, `agent_status.rs`, `project_audit.rs`, `writer_observer.rs`).
- The root lib test suite now lives in `src-tauri/src/tests.rs`; `src-tauri/src/lib.rs` is down to roughly 170 lines of module wiring, Tauri setup, and command registration.
- Writer Agent kernel splitting is now complete for the P2 pass. The facade in `src-tauri/src/writer_agent/kernel.rs` is down to roughly 450 lines and retains the stable public API while implementation-heavy responsibilities live in focused sibling/nested modules.
- TaskPacket construction, context budget trace conversion, and trace state expiry helpers now live in `src-tauri/src/writer_agent/kernel_task_packet.rs` while preserving the existing `writer_agent::kernel::build_task_packet_for_observation` export path.
- Trace-derived product metrics now live in `src-tauri/src/writer_agent/kernel_metrics.rs`, while preserving the existing `writer_agent::kernel::WriterProductMetrics` export path.
- Proposal lifecycle helpers now live in `src-tauri/src/writer_agent/kernel_proposals.rs`, covering replacement decisions, priority ordering, and expiry checks with focused unit coverage.
- Ghost proposal helpers now live in `src-tauri/src/writer_agent/kernel_ghost.rs`, covering local continuation drafts, three-branch alternatives, continuation sanitization, and context evidence mapping with focused unit coverage.
- Memory feedback and slot helpers now live in `src-tauri/src/writer_agent/kernel_memory_feedback.rs`, covering proposal slot keys, suppression keys, memory extraction preferences, and memory audit/feedback recording with focused unit coverage.
- Memory candidate extraction, LLM candidate parsing, promise/canon candidate proposal construction, dedupe, sentence splitting, and memory-candidate quality validation now live in `src-tauri/src/writer_agent/kernel_memory_candidates.rs`.
- Canon / Promise memory candidate quality gates now run on the real local-save and LLM proposal paths: vague candidates are rejected, duplicates are deduped before writes, conflicting canon candidates become explicit continuity review proposals, and same-entity non-conflicting attribute additions use narrow `canon.update_attribute` approval operations instead of whole-entity upserts. Style preference writes now reject vague, duplicate, same-key conflicting, and same-taxonomy-slot conflicting entries before they can pollute the style ledger.
- Canon / Promise context slices and Project Brain / vector chunks now use writing-relevance ranking instead of plain mention matching, fixed ledger order, or raw semantic similarity only, so current-plot entities, payoff-relevant promises, mission-relevant RAG chunks, and scene-type-relevant chunks are surfaced with explicit relevance reasons. Standalone Project Brain queries now combine the user query with active chapter mission, recent result feedback, next beat, and recent decisions before final writing-relevance rerank; mission `must_not` phrases are treated as avoid signals so forbidden distractions do not get boosted as relevant context, while adjacent old-clue payoff terms can still rank.
- Kernel stateful implementation blocks now live under `src-tauri/src/writer_agent/kernel/`: observation handling, context-pack accessors, run-loop methods, proposal creation/registration, feedback, operation execution, snapshots, trace recording, and kernel tests.
- Writer Agent run-loop data types and `WriterAgentPreparedRun` now live in `src-tauri/src/writer_agent/kernel_run_loop.rs` while preserving the existing `writer_agent::kernel::*` export path.
- `agent-evals/src/evals.rs` is now a small module facade; the former large eval file is split into focused modules under `agent-evals/src/evals/` for intent, canon, ghost/feedback, context, tool policy, run loop, task packet, foundation, mission, promise, story debt, and trajectory coverage.
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
- Product metrics are derived from trace data and emitted in trajectory JSONL as `writer.product_metrics`.

## Current Verification Baseline

The expected local baseline is:

- `cargo test -p agent-writer`: 168 passing
- `cargo test -p agent-harness-core`: 79 passing
- `cargo run -p agent-evals`: 96/96 passing
- `npm run check:p2`: 9/9 passing
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

- `src-tauri/src/lib.rs` has completed the command-handler, AppState, semantic-lint, memory/context, observation-bridge, editor-realtime, utility, event, audit, writer-observer, and root-test splits. Remaining `lib.rs` content is mostly final app setup and command registration glue.
- Story Contract and Chapter Mission now have basic authoring/editing UX; the remaining gap is richer guidance, validation, and per-chapter navigation for missions (P1).
- Tool policy now has surfaced approval context for WriterOperation writes and audit coverage for legacy direct save commands; the remaining gap is richer policy rules per operation class (P1).
- Companion Panel should continue moving debug/audit internals into a dedicated inspector, even though write mode now hides raw traces by default (P1).
- Product validation now has the first 10 long-form scenario evals; the remaining gap is making those fixtures closer to real author sessions and tracking failures over longer sessions (P1).
- Product metrics are currently derived locally from trace data; the remaining gap is richer per-session metric history and a debug view for trend inspection (P1).
- P2.2 memory-write gates now cover Canon / Promise proposal creation, safe same-entity attribute merge proposals, foundation quality guards, and Style memory validation with a lightweight taxonomy for dialogue subtext, prose sentence length, exposition density, sensory description, POV distance, action clarity, chapter hooks, and tone/voice. Remaining Style work is richer author-editable taxonomy and polarity-aware preference merging.
- P2.3 context relevance now covers Writer Agent ledger context for Canon / Promise slices plus Project Brain / vector chunk rerank, including standalone `query_project_brain` WriterMemory focus injection. Eval coverage now includes ordinary semantic-similarity distractors, scene-type relevance signals, active-mission focus overriding a surface-similar query, query-only retrieval misses, avoid-term noise suppression over a multi-chapter Project Brain fixture, and preservation of old-clue payoff chunks near avoid terms. The remaining retrieval gap is real author-project fixtures for candidate-pool recall, noise suppression, and avoid-term weight calibration.
- P2.4-P2.6 architecture splitting is complete for the current plan: `lib.rs` is glue-only, `writer_agent/kernel.rs` is a facade/state owner with implementation blocks split into focused modules, and `agent-evals/src/evals.rs` is split into responsibility-based eval modules. Further splitting should be driven by new feature pressure rather than line-count targets.
