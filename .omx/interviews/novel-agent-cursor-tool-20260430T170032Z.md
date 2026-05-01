# Deep Interview Transcript Summary: novel-agent-cursor-tool

- Profile: standard
- Context type: brownfield
- Final ambiguity: 14%
- Threshold: 20%
- Context snapshot: `.omx/context/novel-agent-cursor-tool-20260430T170032Z.md`

## Outcome

The user wants the existing Agent-Writer/Forge project to evolve toward a Cursor-like novel creation tool for nontechnical writers. The first execution target is not inline text replacement; it is an autonomous chapter/book-level workflow where the user gives a natural-language task, the agent retrieves project context, plans steps, executes continuously, shows progress, and writes results back safely.

## Key Requirements

- MVP task: user says `帮我写第 3 章初稿`.
- Agent retrieves bounded context from outline, prior chapter or summaries, lorebook, Project Brain RAG chunks, and user style/profile memory.
- Agent emits visible progress through the frontend so the user sees reading, retrieval, drafting, polishing, saving, and outline-update stages.
- Agent writes generated chapter content atomically to the project chapter file.
- Agent updates outline status and shows a completion notification.
- Agent does not interrupt or overwrite the currently edited chapter if the user is editing elsewhere.
- Must support million-character-scale projects without crashing or freezing.

## Non-Goals

- No full-book rewrite/refactor in the first pass.
- No automatic overwrite of currently edited chapter content.
- No cloud sync or multi-user collaboration.
- No plugin marketplace.
- No complex visual workflow builder.
- No broad UI redesign beyond necessary progress/status UX.
- No new provider architecture unless required by existing settings.
- No packaging/deployment work unless needed for validation.

## Decision Boundaries

Within the first-pass autonomous chapter-draft workflow, implementation decisions are delegated:

- UI progress/status presentation.
- Backend command and event naming.
- Local JSON or SQLite schema additions.
- Adjusting existing chapter generation logic.
- Outline status values and transition details.
- Implementation sequencing and internal architecture.

Must confirm before expanding into:

- Scope beyond the autonomous chapter-draft workflow.
- Cloud/collaboration/plugin marketplace features.
- Destructive overwrite behavior for active user-edited content.

## Acceptance Bar

The validated robustness target is enough for first-pass planning:

- Around 1,000,000 Chinese characters.
- At least 100 chapters.
- 1,000+ RAG chunks.
- Running `生成第 N 章初稿` must not crash, freeze the UI, or load the whole novel into one prompt.
- The flow must finish save + status update using summaries, adjacent chapters, relevant RAG/lorebook, and bounded context budgets.

## Transcript

1. Asked what "Cursor-like" means for novel writing. User said editor-native changes, whole-project context, autonomous multi-step work, and IDE-like project management are all core because the product targets simple interaction for nontechnical users.
2. Asked for the first minimum closed loop. User selected chapter/book-level tasks where the AI retrieves context, plans, executes, and shows results.
3. Asked for a concrete example. User described `帮我写第 3 章初稿`, with outline lookup, prior continuity, lorebook, RAG, user preference injection, visible progress, atomic file write, outline update, toast, and file-tree update.
4. Asked non-goals. User delegated judgment to the agent.
5. Asked decision boundaries. User answered all first-pass implementation decisions may be made autonomously.
6. Asked minimum acceptance standard. User said it must support writing million-word/character-scale novels without crashing.
7. Proposed a measurable robustness definition. User confirmed it was sufficient.
