# Test Spec: Agent Foundation

## Goal

Verify the Cursor-style agent foundation: editor observation, attention policy, bounded context, tool calls, typed suggestions, user-controlled application, and nonblocking UX.

## Unit Tests

### Observation Model

- Builds an observation with chapter title, revision, dirty flag, cursor position, selected text, current paragraph, and nearby text window.
- Truncates nearby text by Unicode characters, not bytes.
- Does not include the full chapter when the chapter exceeds the observation budget.
- Marks observations as user-typed, selection-change, chapter-switch, or idle-tick.

### Attention Policy

- Returns no-op for short/noisy edits.
- Returns no-op while cooldown is active.
- Returns no-op when proactive mode is disabled.
- Triggers on meaningful pause after paragraph-length change.
- Triggers on selected text when the user pauses.
- Triggers on possible continuity-risk marker from local heuristics.
- Enforces provider-call rate limits.

### Context Budget

- Caps current paragraph, nearby window, outline, lorebook snippets, RAG snippets, and user profile.
- Reports original chars, included chars, and truncation per source.
- Does not use whitespace token count as Chinese budget proxy.

### Tool Registry

- Registers existing tools with stable names.
- Declares side-effect level for each tool.
- Rejects side-effecting tools unless user-approved or explicitly allowed.
- Times out tools that exceed policy.
- Returns typed tool errors.

### Suggestion Reducer

- Adds a suggestion to queue.
- Replaces stale suggestion for the same anchor.
- Accept applies only the target suggestion.
- Reject removes suggestion and records rejection.
- Snooze suppresses new suggestions for configured duration.
- Explain returns reason/source summary without applying text.

## Integration Tests

- Editor update produces observation after debounce.
- Observation invokes backend `agent_observe`.
- Backend emits either no-op or `agent-suggestion`.
- Suggestion renders in editor surface.
- Accepting suggestion mutates editor content once.
- Rejecting suggestion leaves editor content unchanged.
- AgentPanel receives trace events but is not required for applying suggestions.
- Lorebook/RAG/outline tools are callable from agent runtime.
- Provider failure yields typed suggestion failure or no-op, not UI lockup.

## E2E / Smoke

- Open a chapter and type a paragraph.
- Stop typing for the configured pause interval.
- Confirm the editor stays responsive.
- Confirm a low-disruption suggestion appears near the active paragraph or cursor.
- Press accept action; confirm text is inserted exactly once.
- Press reject action; confirm no text mutation.
- Disable proactive mode; confirm no further suggestions appear.
- Re-enable passive mode; confirm traces can appear without editor mutation.

## Stress Tests

- 1,000,000 Chinese character synthetic project.
- 100 chapters.
- 1,000+ RAG chunks.
- 100 editor observations over 5 minutes.
- No whole-novel prompt construction.
- No unbounded suggestion queue growth.
- No provider call per keystroke.

## Observability Checks

- Every suggestion has id, request id, observation id, reason, confidence, and source summary.
- Logs contain phase, duration, tool names, source counts, and terminal status.
- Logs do not include full prompt or full chapter content by default.
- Rejected suggestions are counted for future policy tuning.

## Required Verification Commands

```powershell
cargo check --workspace
cargo test --workspace
npm run lint
npm run build
```

## Manual Review Gate

Before implementing additional writing features, verify:

- The agent loop works without any chapter-generation feature.
- The editor-native suggestion UX exists.
- Existing writing features are accessible as tools, not only as panels.
