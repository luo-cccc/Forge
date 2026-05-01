# Deep Interview Spec: Cursor-Like Novel Agent

## Metadata

- Profile: standard
- Rounds: 7
- Final ambiguity: 14%
- Threshold: 20%
- Context type: brownfield
- Context snapshot: `.omx/context/novel-agent-cursor-tool-20260430T170032Z.md`
- Transcript summary: `.omx/interviews/novel-agent-cursor-tool-20260430T170032Z.md`

## Intent

Build the current Agent-Writer/Forge project toward a Cursor-like tool for novel creation. The product should feel easy for nontechnical writers: a user gives a natural-language chapter/book-level task, and the agent understands the project, retrieves the right context, plans, executes, and reports progress without requiring technical operation.

The core value is Cursor-like convenience applied to long-form fiction, not just a chat panel beside an editor.

## Desired Outcome

First-pass MVP: autonomous chapter draft generation.

Example user request:

```text
帮我写第 3 章初稿。
```

The agent should:

1. Understand the task and locate the target chapter.
2. Read the target chapter outline/beat from `outline.json`.
3. Retrieve prior continuity from adjacent chapter content or existing summaries.
4. Retrieve relevant lorebook entries.
5. Retrieve relevant Project Brain/RAG chunks.
6. Retrieve user writing preferences/style profile where available.
7. Build a bounded plan for drafting.
8. Generate the chapter in visible phases.
9. Save the chapter atomically.
10. Update outline status.
11. Notify the user and update visible UI state without interrupting unrelated current editing.

## In Scope

- A chapter-level autonomous task flow for draft generation.
- Context retrieval across outline, prior chapter/summary, lorebook, Project Brain RAG, and user memory/profile.
- Visible progress events for task planning, context retrieval, drafting sections, polishing, saving, and completion.
- Safe atomic write to the chapter file.
- Outline status update after successful draft generation.
- Nonblocking editor behavior if the user is editing another chapter.
- Large-project robustness work needed for million-character-scale novels.
- Internal refactors needed to support the above flow cleanly.

## Out Of Scope / Non-Goals

- Full-book rewrite/refactor in the first pass.
- Automatic overwrite of currently edited chapter content.
- Cloud sync or multi-user collaboration.
- Plugin marketplace.
- Complex visual workflow builder.
- Broad UI redesign beyond necessary progress/status UX.
- New provider architecture unless required by existing settings.
- Packaging/deployment work unless needed for validation.

## Decision Boundaries

The implementer may decide without further confirmation:

- UI progress/status presentation.
- Backend command and event naming.
- Local JSON or SQLite schema additions.
- Adjustments to existing chapter generation logic.
- Outline status values and transition details.
- Implementation sequencing and internal architecture inside the first-pass scope.

Must confirm before:

- Expanding beyond autonomous chapter-draft workflow.
- Adding cloud/collaboration/plugin marketplace features.
- Introducing destructive overwrite behavior for content currently open in the editor.

## Constraints

- Brownfield evolution of `C:\Users\Msi\Desktop\Forge`.
- Preserve existing local-first desktop architecture unless a narrow change is required.
- Do not load an entire million-character novel into a single prompt or hot UI path.
- Prefer bounded context windows, summaries, RAG, and adjacent chapter reads.
- Task execution should be nonblocking and observable through frontend progress.
- File writes should be atomic.
- Existing docs in `C:\Users\Msi\Desktop\docs` remain relevant planning context, especially P13-P15.

## Testable Acceptance Criteria

- Given a project with about 1,000,000 Chinese characters, at least 100 chapters, and 1,000+ RAG chunks, running a request like `帮我写第 N 章初稿` does not crash the app.
- The frontend remains responsive during the autonomous generation task.
- The task does not load the whole novel into a prompt.
- The context package contains only bounded relevant inputs: target outline, adjacent chapter or summary, relevant lorebook entries, relevant RAG chunks, and user style/profile data.
- Progress is visible for at least task understanding, context retrieval, drafting, polishing/saving, and completion/error.
- On success, the target chapter file is written atomically and the outline node status is updated.
- If the user is editing another chapter, that editor content is not replaced or interrupted.
- Existing validation commands should pass after implementation: `cargo check --workspace`, `cargo test --workspace`, `npm run lint`, `npm run build`.

## Assumptions Exposed And Resolved

- Initial ambiguity: "Cursor-like" could mean inline edits, whole-project awareness, autonomous execution, or IDE-like project management.
- Resolution: all are part of the long-term direction, but first-pass scope is autonomous chapter/book-level task execution.
- Scope pressure: implementation could expand into every Cursor-like feature.
- Resolution: first pass is only the autonomous chapter-draft closed loop.
- Robustness pressure: success is not only functional generation.
- Resolution: million-character-scale stability is a hard requirement.

## Pressure-Pass Findings

The interview revisited scope and success standards after the user initially said all Cursor-like values are core. The result narrowed execution to one closed loop: autonomous chapter draft generation. The user then pushed acceptance criteria toward large-project stability, which changes implementation priorities: context budgeting and nonblocking task orchestration are as important as generation quality.

## Brownfield Evidence Vs Inference

Evidence:

- README describes a local AI writing desktop app for long-form fiction.
- Existing features include Tiptap editor, streaming agent panel, lorebook, outline, chapter generation, Script Doctor, Project Brain RAG, entity graph, storyboard, pacing analysis, Hermes memory.
- Recent code has centralized protocol constants and split storage, LLM runtime, and Project Brain services.
- Current storage/vector code has whole-file JSON reads in some paths, so million-character robustness needs attention.

Inference:

- The next implementation likely needs a dedicated autonomous task service around chapter drafting.
- Planner/DAG wiring and hierarchical summaries are likely important to make large-project context safe.
- Project Brain/vector storage may need scaling improvements or bounded loading if 1,000+ chunks becomes a hot path.

## Technical Context Findings

Likely touchpoints:

- `src/components/AgentPanel.tsx`
- `src/components/EditorPanel.tsx`
- `src/components/ProjectTree.tsx`
- `src/protocol.ts`
- `src-tauri/src/lib.rs`
- `src-tauri/src/storage.rs`
- `src-tauri/src/brain_service.rs`
- `src-tauri/src/llm_runtime.rs`
- `agent-harness-core/src/planner.rs`
- `agent-harness-core/src/hermes_memory.rs`
- `agent-harness-core/src/vector_db.rs`

Likely implementation themes:

- Add a high-level autonomous chapter drafting command/service.
- Emit structured task progress events rather than only freeform stream chunks.
- Build a bounded context package.
- Persist/consume chapter summaries to avoid reading whole prior text at scale.
- Keep editor state independent from background generation.
- Validate with a seeded large-project fixture or synthetic stress test.

## Condensed Transcript

The user wants a Cursor-like novel writing agent for nontechnical users. The chosen next-stage MVP is a chapter/book-level autonomous task flow, specifically `帮我写第 3 章初稿`. The AI should retrieve outline, prior continuity, lorebook, RAG, and user preference context, show progress while drafting, save the chapter atomically, update outline state, notify the user, and not interrupt current editing. The user delegates first-pass implementation decisions and non-goal judgment. The hard acceptance standard is stability for projects around 1,000,000 Chinese characters, 100+ chapters, and 1,000+ RAG chunks.

## Handoff Recommendation

Use `$ralplan` next for architecture and test-shape validation before execution:

```text
$plan --consensus --direct .omx/specs/deep-interview-novel-agent-cursor-tool.md
```

This is preferable because the feature touches frontend UX, Tauri command/event contracts, storage, RAG/context budgeting, and performance validation.
