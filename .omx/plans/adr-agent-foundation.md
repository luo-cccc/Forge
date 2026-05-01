# ADR: Agent Foundation Architecture

## Decision

Re-center Forge around an editor-observing, proactive agent runtime. Writing features become tools behind the agent runtime rather than independent product centers.

## Status

Accepted as the new primary direction after user clarification:

> 重点是完善这个agent和架构，agent的功能，夯实agent和架构基础，写作时次要的

## Drivers

- Forge should feel like Cursor for writing: AI present in the text flow, not only in a chat panel.
- The current code has many useful writing tools, but lacks a proactive trigger and suggestion mechanism.
- Architecture must support future domains and workflows, not only chapter drafting.
- Nontechnical users need the agent to decide when and how to help without tool selection burden.

## Decision Details

Add a dedicated agent runtime layer with five responsibilities:

1. Observe editor and project state.
2. Decide whether attention is warranted.
3. Build bounded context.
4. Call registered tools.
5. Emit typed suggestions and traces.

The runtime must be separate from:

- chat streaming
- one-off inline command bubble
- chapter generation
- individual analysis panels

Those capabilities can remain, but they become tools or presentation surfaces for the agent.

## Alternatives Considered

### Option A: Continue autonomous chapter drafting as primary

Rejected as primary.

It improves a useful long-running task, but it does not create the Cursor-like feeling of AI working beside the user while they type.

### Option B: Expand AgentPanel chat features

Rejected as primary.

It keeps the user in a chat-first mental model. Cursor-like behavior requires editor-native observation and suggestion surfaces.

### Option C: Frontend-only proactive suggestions

Rejected as primary.

It can prototype UI quickly, but it would fragment context building, tool use, policy, and observability. The agent runtime should own decisions and tool orchestration.

### Option D: Dedicated agent runtime with editor observation

Chosen.

It creates a stable foundation for proactive suggestions, long-running tasks, context retrieval, tool use, traceability, and future agent behaviors.

## Consequences

- New protocol types are required for observations and suggestions.
- `EditorPanel` becomes a source of observations, not only an editor/autosave surface.
- `AgentPanel` becomes trace/transcript, not the whole agent UX.
- Backend needs an agent runtime module and tool registry.
- Existing chapter draft work is demoted to a P2 tool behind the agent.

## Required Boundaries

- Proactive suggestions cannot silently mutate editor content.
- Provider calls cannot run on every keystroke.
- Observation payloads must be bounded.
- Agent traces must not leak full private prompt/context by default.
- Tool registry must mark side effects and approval requirements.

## Pre-Mortem

### Failure: The app becomes noisy

Cause: every pause produces suggestions.

Mitigation: attention policy supports no-op, cooldowns, confidence thresholds, and per-project mode settings.

### Failure: Agent feels like chat in disguise

Cause: suggestions only appear in AgentPanel.

Mitigation: editor-native suggestion overlay is required acceptance criteria.

### Failure: Cost or latency spikes

Cause: LLM calls on frequent editor updates.

Mitigation: local attention policy first; provider calls only after debounce, threshold, and budget checks.

### Failure: Tool features drift again

Cause: new capabilities are added as panels/commands instead of agent tools.

Mitigation: new agent-facing features must declare tool metadata: input, output, side effects, timeout, approval.

## Follow-Ups

1. Implement `agent_runtime.rs`.
2. Add `AgentObservation`, `AgentSuggestion`, and `AgentToolCall` protocol types.
3. Add editor suggestion overlay.
4. Wrap existing lorebook, outline, Project Brain, and chapter generation as tools.
5. Add tests for attention policy and suggestion reducer.
