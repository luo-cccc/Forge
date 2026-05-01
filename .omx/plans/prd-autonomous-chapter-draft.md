# PRD: Autonomous Chapter Draft MVP

> Status update: this plan is no longer the primary product direction. After user clarification, Forge should prioritize the Cursor-style agent foundation in `.omx/plans/prd-agent-foundation.md`. Autonomous chapter drafting remains a secondary long-running tool capability to be connected after the agent observation, attention, tool registry, and editor-native suggestion loop are in place.

## Metadata

- Source spec: `.omx/specs/deep-interview-novel-agent-cursor-tool.md`
- Context snapshot: `.omx/context/novel-agent-cursor-tool-20260430T170032Z.md`
- Consensus: Planner revised, Architect APPROVE, Critic APPROVE
- Mode: RALPLAN deliberate
- Target repo: `C:\Users\Msi\Desktop\Forge`

## Goal

Implement a Cursor-like autonomous chapter draft workflow. A writer can type a request such as:

```text
帮我写第 3 章初稿
```

The app should resolve the target chapter, build bounded project context, generate a draft, save it safely, update outline state, and show visible progress without interrupting the active editor.

## Principles

1. Backend owns generation side effects: provider calls, chapter writes, and outline updates stay in Tauri/Rust.
2. Shared primitives over parallel paths: autonomous flow and legacy batch generation must use the same context/generation/save/update functions.
3. Bounded context always: every source has hard caps and prompt construction never includes the whole novel.
4. Conflict-safe writes: generated text never overwrites an open dirty editor buffer or a chapter changed since context was built.
5. Chinese-safe text handling: budgeting and truncation operate on valid Unicode boundaries, not byte offsets or whitespace counts.

## Decision Drivers

1. Correct persisted novel state under concurrent editor autosave and background generation.
2. Predictable context size and responsiveness for million-character projects.
3. Low-risk integration with existing Tauri commands, protocol constants, storage, Project Brain, and Hermes memory.

## In Scope

- Add a dedicated backend autonomous chapter generation service.
- Add four reusable primitives:
  - `build_chapter_context`
  - `generate_chapter_draft`
  - `save_generated_chapter`
  - `update_outline_after_generation`
- Add typed frontend/backend protocol for autonomous chapter generation.
- Add frontend open/dirty snapshot contract.
- Add compare-and-save conflict protection.
- Add Chinese-safe text budget utility.
- Add bounded source caps for outline, adjacent chapters/summaries, lorebook, RAG chunks, and user profile.
- Keep `batch_generate_chapter` as a compatibility wrapper over the new primitive pipeline where feasible.
- Show task progress and terminal states in the UI.

## Out Of Scope

- Full-book rewrite or refactor.
- Multi-chapter autonomous planning.
- Streaming generated text directly into the editor.
- User-edit merge/reconciliation UI.
- Cloud sync, collaboration, plugin marketplace.
- Broad UI redesign beyond progress/status UX.
- Provider architecture redesign.

## Options

### Option A: Dedicated Backend Autonomous Service

Chosen.

Pros:

- Clean long-running task boundary.
- Testable context, generation, save, outline, and event phases.
- Avoids editor action-tag mutation.
- Allows old `batch_generate_chapter` to delegate instead of drift.

Cons:

- More upfront structure than directly patching the old command.

### Option B: Harden `batch_generate_chapter`

Rejected as primary.

Pros:

- Smallest initial diff.
- Current OutlinePanel already uses it.

Cons:

- Keeps autonomous behavior inside a legacy batch-shaped API.
- Harder to add task IDs, CAS conflicts, typed progress, and source caps cleanly.

### Option C: Frontend-Orchestrated Flow

Rejected.

Pros:

- Fast UI prototype.

Cons:

- Frontend would coordinate storage mutation, provider calls, conflict policy, and outline writes.
- Higher stale-state and overwrite risk.

## Backend Primitive Contracts

### `build_chapter_context`

Purpose: Build bounded, inspectable context for one target chapter.

Inputs:

- `request_id`
- `target_chapter_title` or `target_chapter_number`
- `user_instruction`
- optional budget overrides

Outputs:

- target chapter metadata
- `base_revision` for CAS
- prompt/context string
- selected source records with `original_chars`, `included_chars`, and `truncated`
- budget report and warnings

Errors:

- `TARGET_CHAPTER_NOT_FOUND`
- `TARGET_CHAPTER_AMBIGUOUS`
- `INSTRUCTION_EMPTY`
- `CONTEXT_BUDGET_EXCEEDED`
- `STORAGE_READ_FAILED`

Rules:

- Must not call the model.
- Must not mutate storage.
- Must use Chinese-safe truncation.
- Must return a `base_revision` for later save checks.

Default caps:

- total context: 24,000 Unicode chars
- instruction: 1,000 chars
- outline: 6,000 chars
- previous chapter/summaries: 5,000 chars total, max 2 chapters
- next chapter/summary: 2,000 chars, max 1 chapter
- target existing text: 3,000 chars
- lorebook/world notes: 5,000 chars total, max 4 entries
- character/style notes: 4,000 chars total, max 6 entries
- RAG chunks: top K only, truncate per chunk, include IDs/scores in stats
- event/debug preview: 2,000 chars, never full prompt by default

### `generate_chapter_draft`

Purpose: Generate chapter text from a prebuilt context.

Inputs:

- `request_id`
- built context
- generation options: model/provider defaults, temperature, output cap

Outputs:

- generated content
- finish reason
- usage estimate and provider metadata
- context base revision

Errors:

- `CONTEXT_INVALID`
- `PROVIDER_NOT_CONFIGURED`
- `PROVIDER_TIMEOUT`
- `PROVIDER_RATE_LIMITED`
- `PROVIDER_CALL_FAILED`
- `MODEL_OUTPUT_EMPTY`
- `MODEL_OUTPUT_TOO_LARGE`

Rules:

- Must not save content.
- Must validate non-empty output.
- Default output cap: 12,000 chars; hard cap: 30,000 chars.
- Provider timeout default: 120 seconds.
- Max one retry for transient provider/network failure.

### `save_generated_chapter`

Purpose: Persist generated text only when revision and frontend dirty/open policy allow it.

Inputs:

- `request_id`
- target chapter metadata
- generated content
- `base_revision`
- `save_mode`: `create_if_missing`, `replace_if_clean`, or `save_as_draft`
- frontend state snapshot:
  - `open_chapter_id` or title
  - `open_chapter_revision`
  - `dirty`

Outputs:

- saved chapter title/id
- new revision
- saved mode: `created`, `replaced`, or `draft_copy`
- optional conflict details

Conflict policy:

- If target chapter is open and frontend reports `dirty: true`, do not replace.
- If stored revision differs from `base_revision`, do not replace.
- If `create_if_missing` is requested and target exists, return conflict.
- `save_as_draft` fallback is allowed only when explicitly requested.
- Never silently overwrite user edits.

Errors:

- `SAVE_CONFLICT`
- `CHAPTER_DELETED`
- `STORAGE_WRITE_FAILED`
- `CONTENT_EMPTY`
- `CONTENT_TOO_LARGE`

### `update_outline_after_generation`

Purpose: Update outline status after a successful save.

Inputs:

- `request_id`
- target chapter metadata
- saved chapter revision
- optional generated summary
- optional outline base revision

Outputs:

- outline revision
- changed flag
- warnings

Rules:

- Runs only after chapter save succeeds.
- If outline update fails, do not roll back saved chapter.
- Emit partial-success warning.
- Recommended status for MVP: `drafted`, with compatibility mapping from existing `generated` if needed.

Errors:

- `OUTLINE_NOT_FOUND`
- `OUTLINE_REVISION_MISMATCH`
- `OUTLINE_UPDATE_FAILED`

## Protocol Contract

Add to `src/protocol.ts`:

- `Commands.generateChapterAutonomous = "generate_chapter_autonomous"`
- `Events.chapterGeneration = "chapter-generation"`

Add TypeScript interfaces:

- `GenerateChapterAutonomousPayload`
- `FrontendChapterStateSnapshot`
- `ChapterContextBudget`
- `ChapterGenerationEvent`
- `ChapterGenerationStartedEvent`
- `ChapterGenerationContextBuiltEvent`
- `ChapterGenerationProgressEvent`
- `ChapterGenerationConflictEvent`
- `ChapterGenerationCompletedEvent`
- `ChapterGenerationFailedEvent`
- `ChapterGenerationError`

Minimum event phases:

- `chapter_generation_started`
- `chapter_generation_context_built`
- `chapter_generation_progress`
- `chapter_generation_conflict`
- `chapter_generation_completed`
- `chapter_generation_failed`

Every event carries `requestId`.

## Frontend Contract

- `AgentPanel` detects high-confidence chapter drafting requests such as `帮我写第 3 章初稿`.
- If target is ambiguous, fall back to normal chat or ask for clarification.
- On autonomous generation, pass frontend snapshot:
  - current open chapter
  - current known revision if available
  - dirty flag
- Listen for `chapter-generation` events and render a compact progress timeline.
- Conflict events must not mutate editor content.
- Generated chapter should load only when the user selects it.

## Compatibility Plan

`batch_generate_chapter` should become a compatibility wrapper where feasible:

1. Keep existing command and event behavior for OutlinePanel compatibility.
2. Internally call the same primitive pipeline used by `generate_chapter_autonomous`.
3. Avoid a second prompt/context/model/save implementation path.
4. If wrapper is not feasible, deprecate only after frontend references are removed and a typed deprecation result exists.

## Chinese-Safe Budgeting

Replace generation-path byte slicing and whitespace token estimates with a shared utility:

- count via Unicode char iteration
- truncate only at valid character boundaries
- prefer paragraph/sentence boundaries including `。！？；\n`
- fallback to char boundary
- return original chars, included chars, truncated flag
- never use `split_whitespace().count()` as the preflight budget proxy for Chinese prose

## Acceptance Criteria

- `generate_chapter_autonomous` exists and returns typed success/error state.
- Four backend primitives exist and are independently testable.
- `batch_generate_chapter` delegates to the new primitive pipeline or is explicitly deprecated with no active frontend dependency.
- `src/protocol.ts` defines command payloads, event union, error codes, budget types, and frontend dirty snapshot.
- Dirty/open editor and revision conflicts cannot overwrite user edits.
- Chinese-safe budgeting replaces unsafe truncation in generation context paths.
- Hard context source caps are enforced and reported.
- Progress is visible for context, provider call/drafting, save, outline update, completion/error/conflict.
- A project of roughly 1,000,000 Chinese chars, 100 chapters, and 1,000+ chunks does not require whole-novel prompt loading and remains responsive in context-building tests.
- Validation commands pass:
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `npm run lint`
  - `npm run build`

## Non-Blocking Caveats

- `package.json` currently has no JS test script. Do not claim frontend automated tests unless a test harness is added.
- Frontend validation for this plan is `npm run lint`, `npm run build`, and manual/event-contract smoke verification.
- This native Codex surface is outside tmux; use native subagents here, or run OMX team only from an attached OMX CLI runtime.
