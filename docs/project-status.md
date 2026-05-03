# Project Status

Last updated: 2026-05-03

## Positioning

Forge is a Cursor-style writing agent for novels, not a generic writing tool. The editor is only the workbench. The product value is the persistent co-writer that knows the book, tracks commitments, watches continuity, proposes typed changes, and learns from what the author accepts or rejects.

## P0 Status (May 2026): Unified Writer Agent Kernel Brain

P0 is complete:

- **P0.1 (Unified Run Loop)**: `AgentLoop::new` now lives only behind `WriterAgentKernel.prepare_task_run()` / `WriterAgentPreparedRun.run()`. `ask_agent` in lib.rs calls the kernel path, with no direct agent-loop construction in the command layer. `WriterAgentRunRequest` / `WriterAgentRunResult` types are re-exported through `writer_agent::kernel` and implemented in `writer_agent/kernel_run_loop.rs`.
- **P0.2 (Unified Action Lifecycle)**: `WriterOperationLifecycleState` (Proposed → Approved → Applied → DurablySaved → FeedbackRecorded) and `WriterOperationLifecycleTrace` track full lifecycle. `apply_feedback()` enforces durable-save-before-feedback for positive feedback. All write-capable operations push lifecycle entries.
- **P0.3 (Command Boundary Audit)**: 50 `#[tauri::command]` functions classified by risk level (destructive/manuscript-write/memory-write/provider-call/credential/read-only). Static audit check at `scripts/check-command-audit.cjs` runs as part of `npm run verify` and covers `pub async fn` command handlers. All legacy direct-save commands reference `audit_project_file_write`.

## P1 Status (May 2026): Trust Contract And Product Validation

P1 is in progress:

- Story Contract quality now has explicit Missing / Vague / Usable / Strong states and vague contracts are excluded from context packs.
- Chapter Mission save calibration can mark completed, drifted, or needs_review states based on save observations.
- Promise Ledger now classifies promise kinds including plot promise, emotional debt, object whereabouts, character commitment, mystery clue, and relationship tension.
- Writer Agent trajectory now exports derived product metrics such as proposal acceptance rate, ignored suggestion rate, promise recall hit rate, canon false-positive rate, chapter mission completion rate, durable save success rate, and save-to-feedback latency. It also exports a first multi-session `writer.product_metrics_trend` event derived from persisted run events.
- P4 has started with an append-only `WriterRunEventStore`: observation, context-pack-built, model-started, tool-called, task-packet, proposal, operation-lifecycle, feedback, and failure events are recorded with monotonic sequence numbers, persisted in WriterMemory SQLite, and exported through trajectory JSONL as `writer.run_event`. `ToolExecutor` now has an optional audit sink, and Tauri has a reusable `writer_tool_audit_sink` that maps direct executor calls to the same privacy-preserving `writer.tool_called` event shape.
- P4 Planning / Review mode now has a backend read-only task path: `WriterAgentTask::PlanningReview` uses a dedicated context profile, AnalyzeText task packet, read-only project tool inventory, no memory-write feedback contract, and a prompt that outputs objective understanding, evidence, risks, candidate actions, and author-confirmation questions.
- P4 WriterTaskReceipt / failure evidence first slice is implemented for chapter generation: built chapter contexts carry a `WriterTaskReceipt`, saves validate receipt task/chapter/revision/evidence/artifact before writing, receipt mismatch blocks writes, and structured `WriterFailureEvidenceBundle` records provider/save/receipt failures into `writer.error` run events and trajectory export.
- P4 memory correction / reinforcement first slice now reuses reviewed feedback: accepted memory candidates create reinforcement signals, rejected/edited memory candidates create correction signals, correction suppresses future same-slot memory extraction even after prior reinforcement, and reviewable memory candidates now emit `writer.memory_candidate_created` run events without writing Canon/Promise ledgers before author approval.
- P4 Project Brain knowledge index / graph now builds `knowledge_index.json` from vector chunks, outline, and lorebook; nodes carry source refs, keywords, and summaries, shared-keyword graph edges preserve evidence refs, knowledge index file reads have a path guard, and the Graph view has a Project Brain mode backed by the read-only `get_project_brain_knowledge_graph` command. The Graph view also has first-layer node kind filtering, source/keyword/summary/relation search, selected-node neighbor highlighting, and reference/back-reference navigation.
- P4 Project Brain embedding provider boundaries now have a first-stage profile: provider id, model, expected dimensions, input limit, batch limit, and retry limit are explicit; `OPENAI_EMBEDDING_INPUT_LIMIT_CHARS` controls input trimming with a guarded default; chapter ingestion and Project Brain queries share the wrapper; batch reports distinguish complete, partial, and empty results with requested/embedded/skipped/truncated/error counts.
- P4 isolated research / diagnostic subtask first slice now creates per-subtask artifact workspaces under `agent_subtasks/<subtask_id>/artifacts`, enforces relative artifact paths, applies research/diagnostic/drafting tool policies, returns evidence-only results with attempted writes captured as blocked operation kinds, records started/completed subtasks as `writer.subtask_started` / `writer.subtask_completed` run events without evidence snippets or absolute artifact paths, and exposes a Subtasks filter in Inspect mode.
- P4 Inspector / trajectory slice now derives an inspector-only timeline from trace snapshots, exposes a dedicated frontend Inspect mode for internal timeline filtering, failure details, failure recovery navigation chips, provider-budget reports, save_completed events, save-to-feedback latency, multi-session save-to-feedback trends, proposal-level context budget drilldown, post-write diagnostics, and context-source pressure, keeps the default companion summary free of task packets/raw run events/operation lifecycle internals, adds redaction warning plus `local_only` metadata to trajectory exports, and can export a Claude-Code-style / HF Agent Trace Viewer compatible JSONL bridge while preserving Forge's native schema.
- P4 provider budget now estimates token/cost budgets for long provider tasks, returns allowed/warn/approval-required/blocked decisions with remediation, gates chapter draft generation before the real provider call with `PROVIDER_BUDGET_APPROVAL_REQUIRED` failure evidence when approval is needed, surfaces an Explore-mode approval/retry card for chapter generation, validates the retry against the approved task/model/token/cost ceiling, and records chapter-generation budget reports as `writer.provider_budget` run events for trajectory replay. Project Brain answer generation now has a chat-provider preflight too: it records `project_brain_query` budget reports before `stream_chat`, blocks approval-required calls, writes a provider failure bundle for Inspector/trajectory, and accepts a matching approval credential on retry. Manual requests now get a first-round AgentLoop provider preflight before streaming; approval-required requests record `manual_request` budget reports/failure bundles, are blocked before the real provider call, and can be retried from the same Explore approval card with a matching approval credential. Chapter generation, Project Brain, and manual request record `writer.model_started` only after budget approval gates pass and before the real provider call starts.
- P4 post-write diagnostics first slice now records `WriterPostWriteDiagnosticReport` after save observations and accepted inline/proposal durable saves, including severity/category counts, evidence refs, remediation, `writer.post_write_diagnostics` run events, linked `writer.save_completed` events, trace snapshot entries, trajectory export events, and Companion Audit summaries.
- P4 external tool remediation first slice now adds structured remediation to `ToolExecution` failures for unregistered tools, approval/permission denial, missing binary/resource, workspace unavailable, unknown tool/agent, doom-loop detection, and generic handler failures; those failures can now become `WriterFailureEvidenceBundle` records and Inspector `failure` timeline events.
- Companion Panel write mode remains quiet and now summarizes acceptance/save health instead of exposing raw operation traces.
- `agent-evals` now includes 10 long-form product scenario checks in `agent-evals/src/product_scenarios.rs`, covering multi-chapter promise tracking, result feedback handoff, payoff timing, resolved-promise quieting, object whereabouts, mission drift, canon guardrails, style feedback, decision metrics, and context explainability. More realistic fixtures are still needed before product value can be called proven.
- Writer Agent context relevance ranking now prioritizes Canon / Promise ledger slices by mission, next beat, result feedback, recent decisions, cursor-local story signals, open promises, and lightweight scene-type matches, with `WHY writing_relevance` explanations on retrieved entries. Project Brain / vector chunks are now reranked after semantic retrieval using the same writing-focus, scene-type, and avoid-term signals; standalone `query_project_brain` also injects active WriterMemory focus into retrieval and rerank. Scene-type explanations prioritize setup/payoff and reveal over generic action/description signals, and Project Brain rerank now extracts author-project terms from indexed keywords/phrase boundaries.

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
- Canon / Promise memory candidate quality gates now run on the real local-save and LLM proposal paths: vague candidates are rejected, duplicates are deduped before writes, conflicting canon candidates become explicit continuity review proposals, and same-entity non-conflicting attribute additions use narrow `canon.update_attribute` approval operations instead of whole-entity upserts. Style preference writes now reject vague, duplicate, same-key reverse-polarity, and same-taxonomy-slot reverse-polarity entries before they can pollute the style ledger, while same-direction slot updates merge into a normalized style key.
- Canon / Promise context slices and Project Brain / vector chunks now use writing-relevance ranking instead of plain mention matching, fixed ledger order, or raw semantic similarity only, so current-plot entities, payoff-relevant promises, mission-relevant RAG chunks, and scene-type-relevant chunks are surfaced with explicit relevance reasons. Standalone Project Brain queries now combine the user query with active chapter mission, recent result feedback, next beat, and recent decisions before final writing-relevance rerank; mission `must_not` phrases are parsed up to boundaries such as "盖过" / "取代" / "抢走" so forbidden distractions do not get boosted while boundary-after positive targets, adjacent old-clue payoff terms, and author-project terms can still rank.
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
- A static command boundary audit classifies all 49 Tauri commands by risk level and verifies audit coverage for write paths.
- Product metrics are derived from trace data and emitted in trajectory JSONL as `writer.product_metrics`.
- Writer run events now have a persisted replay path via `writer_run_events`, with `writer_agent:append_only_run_event_store` covering monotonic seq replay and trajectory export; failure bundles are recorded as `writer.error` run events, durable saves as `writer.save_completed`, reviewable memory candidates as `writer.memory_candidate_created`, WriterOperation approval/rejection decisions as `writer.approval_decided`, real writer/chapter-generation context assembly as `writer.context_pack_built` without storing raw manuscript context text, provider-call starts as `writer.model_started` without storing prompts or model output, and manual AgentLoop plus audited direct `ToolExecutor` calls as `writer.tool_called` without storing raw args or tool output.
- Inspector timeline views now separate debug/internal replay from the default companion summary. The frontend Inspect mode reads `get_writer_agent_inspector_timeline` and `get_writer_agent_trace`, with filters for failure, save_completed, run event, task packet, lifecycle, context recall, and product metrics, plus side summaries for provider budget, latest failure/save, save-to-feedback latency, proposal context budgets, post-write diagnostics, and context-source pressure. Failure cards and the latest-failure summary now provide read-only recovery navigation chips into budget, save, task-packet, run-event, context, and failure views. Trajectory export warns about manuscript/project-memory/feedback leakage before any sharing and now has both native Forge JSONL and Trace Viewer compatible local export options.

## Current Verification Baseline

The expected local baseline is:

- `cargo test -p agent-writer`: 183 passing
- `cargo test -p agent-harness-core`: 80 passing
- `cargo run -p agent-evals`: 138/138 passing
- `npm run check:p2`: 15/15 passing
- `npm run check:audit`: 50 commands, 0 issues
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
- Product metrics now include a first persisted multi-session trend derived from `writer_run_events`, and context source trends are derived from trace data; the remaining gap is calibrating these trends against real long author sessions rather than synthetic history (P1).
- P2.1 context pack debugging now exposes backend source trends in `WriterAgentTraceSnapshot.contextSourceTrends` and Inspect-mode proposal context budget drilldown, with eval coverage for CursorPrefix and ChapterMission budget participation. The remaining gap is real long-session calibration.
- P2.2 memory-write gates now cover Canon / Promise proposal creation, safe same-entity attribute merge proposals, foundation quality guards, and Style memory validation with a lightweight taxonomy for dialogue subtext, prose sentence length, exposition density, sensory description, POV distance, action clarity, chapter hooks, and tone/voice. Style preference writes now support polarity-aware same-slot merging into normalized `style:<slot>` keys; remaining Style work is richer author-editable taxonomy and UI review of merged preferences.
- P2.3 context relevance now covers Writer Agent ledger context for Canon / Promise slices plus Project Brain / vector chunk rerank, including standalone `query_project_brain` WriterMemory focus injection. Eval coverage now includes ordinary semantic-similarity distractors, scene-type relevance signals, active-mission focus overriding a surface-similar query, query-only retrieval misses, avoid-term noise suppression over multi-chapter Project Brain fixtures, preservation of old-clue payoff chunks near avoid terms, complex must_not boundary parsing, and author-project term recall/explanation. The remaining retrieval gap is calibration on real author projects rather than synthetic fixtures.
- P2.4-P2.6 architecture splitting is complete for the current plan: `lib.rs` is glue-only, `writer_agent/kernel.rs` is a facade/state owner with implementation blocks split into focused modules, and `agent-evals/src/evals.rs` is split into responsibility-based eval modules. Further splitting should be driven by new feature pressure rather than line-count targets.
- P4 backend and first frontend slices now cover replayable run events, Planning / Review read-only mode, chapter-generation TaskReceipt/failure evidence bundles, memory correction/reinforcement signals and memory-candidate-created events for reviewed memory candidates, operation approval-decision events, context-pack-built events for real writer/chapter-generation work, model-started events after provider-budget gates, manual AgentLoop and audited direct ToolExecutor tool-called events, Project Brain knowledge index/path guard plus first Graph-view visualization/filter/reference navigation, Project Brain embedding provider profile/input-limit/batch-report boundaries, isolated research/diagnostic subtask workspace boundaries plus subtask started/completed run events and Inspect filtering, inspector-only timeline views, companion-safe timeline summaries, a dedicated Inspect mode for timeline/failure/failure-recovery navigation/provider-budget/save_completed/save-to-feedback/multi-session metric trend/proposal-context-budget/post-write/context-pressure debugging, trajectory redaction warnings plus Trace Viewer compatible JSONL export and product-metrics-trend events, provider budget reports with chapter-generation preflight/run events, Project Brain chat-provider preflight/run events/failure bundles, manual-request first-round provider preflight/run events/failure bundles, Explore-mode budget approval/retry propagation for chapter generation, Project Brain, and manual request, post-write diagnostic reports for save observations and accepted text operations with Companion Audit and Inspect summaries plus linked save-completed events, external tool failure remediation mapped into failure bundles/Inspector failure events, and Research subtask tool/provider failure bundles with subtask evidence. Remaining P4 gaps are wiring the new ToolExecutor audit sink into future real external/public-source tool entrypoints, deeper Project Brain graph cross-reference actions and real-source calibration, local/remote embedding provider registry plus provider-specific calibration and chunk source/version history, real subtask run-loop automation, provider-budget enforcement before external research plus multi-round provider cost tracking, real external-public-source provider/tool integration, and longer continuous-writing fixtures.
