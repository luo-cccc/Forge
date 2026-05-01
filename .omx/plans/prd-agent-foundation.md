# PRD: Cursor-Style Agent Foundation

## Metadata

- Supersedes primary direction of: `.omx/plans/prd-autonomous-chapter-draft.md`
- Related ADR: `.omx/plans/adr-agent-foundation.md`
- Related test spec: `.omx/plans/test-spec-agent-foundation.md`
- Target repo: `C:\Users\Msi\Desktop\Forge`
- Product priority: Agent and architecture foundation first; writing features second.

## Goal

Build Forge into a Cursor-style writing agent platform, not a collection of writing tools. The first product milestone is an always-present agent loop that observes the editor, understands project context, decides when to help, calls tools, and presents low-friction suggestions while the user is writing.

Writing output remains important, but it is no longer the architectural center. Chapter drafting, Script Doctor, Project Brain, lorebook, and outline features should become tools that the agent can call, not separate product centers.

## Product North Star

The user should feel:

> I am writing with an AI partner that understands my project and quietly works beside me.

The agent should not wait only for explicit chat commands. It should notice useful moments, prepare context in the background, and surface help in the text flow without stealing control.

## Current Gap

Repository evidence shows the current app is still mostly passive:

- `src/components/EditorPanel.tsx` listens to editor updates mainly for autosave.
- `src/components/AgentPanel.tsx` invokes agent work after user submit.
- `src/components/InlineCommandBubble.tsx` appears after keyboard command.
- `src/protocol.ts` has command/event contracts for chat, stream chunks, batch status, and chapter generation, but no proactive suggestion protocol.
- `agent-harness-core/src/actions.rs` supports `ACTION_INSERT`, `ACTION_REPLACE`, and `ACTION_SEARCH`, which are command execution primitives rather than observation/suggestion primitives.

## Principles

1. Agent loop before writing features: observation, attention, planning, tool use, and suggestion delivery come before more writing endpoints.
2. User remains in control: proactive suggestions are previewed, accepted, ignored, or dismissed; the agent does not silently edit active text.
3. Low-disruption UX: suggestions appear in the editor flow, not only in a side chat panel.
4. Bounded context by default: every proactive tick has strict context, time, and token budgets.
5. Tool abstraction over feature silos: existing writing features become callable agent tools.
6. Inspectable behavior: every proactive suggestion has reason, source hints, and a traceable event path.

## In Scope

- Editor observation model.
- Agent attention and trigger policy.
- Proactive suggestion protocol.
- Agent state machine and event bus.
- Tool registry for existing capabilities.
- Context builder for active writing windows.
- Suggestion UI surfaces in the editor.
- User control primitives: accept, reject, snooze, disable, explain.
- Observability for agent decisions.
- Tests for trigger policy, budgets, protocol, and editor safety.

## Out Of Scope For This Foundation Milestone

- High-quality chapter prose generation as the primary milestone.
- Full-book autonomous rewrite.
- Complex multi-agent team orchestration inside the product UI.
- Cloud sync and collaboration.
- Plugin marketplace.
- Broad visual redesign unrelated to the agent loop.

## Core Concepts

### Editor Observation

A structured snapshot emitted from the editor, not a prompt:

- current chapter title and revision
- dirty state
- cursor position
- selected text
- current paragraph text
- nearby text window
- recent edit summary
- idle duration
- current outline node if known

### Agent Attention

A lightweight policy decides whether the agent should act:

- user paused after meaningful typing
- user selected text
- paragraph changed substantially
- chapter switch occurred
- likely continuity risk detected
- user is idle and current section has known outline/lore context

The policy can also decide not to act. Silence is a valid output.

### Proactive Suggestion

A suggestion is not an edit. It is a typed proposal:

- `continue`: next sentence / paragraph continuation
- `revise`: local rewrite proposal
- `continuity`: possible contradiction or relevant prior fact
- `lore`: relevant setting reminder
- `structure`: outline/beat alignment hint
- `question`: concise clarification prompt

Every suggestion carries:

- stable id
- target range or cursor anchor
- confidence
- reason
- source summaries
- preview text
- available actions

### Tool Registry

Existing capabilities become tools:

- load current chapter
- load outline node
- search lorebook
- query Project Brain
- inspect user drift profile
- analyze selected text
- generate bounded continuation
- generate chapter draft, as a secondary long-running tool

Tools must declare:

- inputs
- output type
- context cost
- side effects
- timeout
- whether user approval is required

## Architecture Requirements

### Frontend

- Add `AgentObservation` and `AgentSuggestion` types to `src/protocol.ts`.
- Extend Zustand store with:
  - latest observation
  - agent mode: off, passive, proactive
  - suggestion queue
  - accepted/rejected suggestion history
  - cooldown/snooze state
- Update `EditorPanel` to produce debounced observations.
- Add editor-native suggestion surface, likely near cursor or paragraph edge.
- Keep `AgentPanel` as transcript/trace, not the only agent surface.

### Backend

- Add an agent runtime service separate from chapter generation.
- Add command/event pair:
  - `agent_observe`
  - `agent-suggestion`
- Add attention policy as pure testable logic.
- Add context builder for current editor window.
- Add tool registry around existing storage, lorebook, Project Brain, and LLM calls.
- Return typed suggestions; do not emit raw text-only actions for proactive mode.

### Safety

- No automatic write to active editor from proactive flow.
- No full-novel prompt loading.
- No provider call on every keystroke.
- Budget and cooldown defaults must prevent runaway cost.
- Suggestions must be dismissible and disableable.

## Acceptance Criteria

- Typing in the editor produces debounced `AgentObservation` snapshots without blocking input.
- After a meaningful pause, the agent can emit either no-op or one typed suggestion.
- Suggestions appear in the editor surface, not only the chat panel.
- User can accept, reject, snooze, or explain a suggestion.
- Accepting a suggestion is the only path that mutates the editor.
- Agent suggestions include reason and source summary.
- Existing lorebook/RAG/outline capabilities are callable through a tool registry abstraction.
- Trigger policy, context budget, and suggestion reducer are covered by unit tests.
- Proactive mode can be disabled.
- Validation passes:
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `npm run lint`
  - `npm run build`

## Implementation Sequence

1. Define protocol and state model:
   - `src/protocol.ts`
   - `src/store.ts`
2. Add editor observation producer:
   - `src/components/EditorPanel.tsx`
   - `src/App.tsx`
3. Add suggestion UI surface:
   - new `src/components/AgentSuggestionOverlay.tsx`
   - integrate with editor accept/reject paths
4. Add backend agent runtime:
   - new `src-tauri/src/agent_runtime.rs`
   - command registration in `src-tauri/src/lib.rs`
5. Add tool registry:
   - wrap storage, lorebook, Project Brain, outline, and bounded LLM continuation tools
6. Add tests:
   - pure Rust tests for attention policy and budgets
   - frontend type/build validation
7. Only after the loop is working, reconnect autonomous chapter drafting as a long-running tool.

## Roadmap Priority

### P0: Agent Foundation

- Observation loop
- Attention policy
- Suggestion protocol
- Tool registry
- Editor-native suggestion UX

### P1: Agent Tooling

- Use lorebook, outline, RAG, and user profile from the agent runtime.
- Add traces and explanations.
- Add per-project agent settings.

### P2: Long-Running Agent Tasks

- Chapter draft generation
- Multi-chapter planning
- Continuity pass

## Non-Blocking Caveats

- Existing autonomous chapter generation work can be retained, but it should not remain the main architecture driver.
- Current code has useful building blocks, but the missing primitive is the proactive agent loop.
