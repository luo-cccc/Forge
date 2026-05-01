# Test Spec: Autonomous Chapter Draft MVP

> Status update: this is now a secondary tool-level test spec. The primary foundation test plan is `.omx/plans/test-spec-agent-foundation.md`.

## Unit Tests

### Context Builder

- Resolves target chapter by title.
- Resolves target chapter by number from outline order.
- Errors on missing or ambiguous target.
- Enforces max source count.
- Enforces total char cap.
- Includes target outline/beat.
- Selects bounded adjacent summaries/chapters.
- Selects bounded relevant lorebook entries.
- Selects bounded relevant RAG chunks with IDs/scores.
- Includes user profile/style data within cap.
- Returns `base_revision`.
- Reports `original_chars`, `included_chars`, and `truncated`.

### Generation

- Rejects empty model output.
- Rejects output above hard max.
- Maps timeout, rate limit, provider failure, and missing config to typed errors.
- Records provider/model metadata without logging full prompt/content.

### Save/Conflict

- Creates missing chapter.
- Replaces only when revision matches and frontend dirty is false.
- Rejects dirty open chapter.
- Rejects revision mismatch.
- Supports `save_as_draft` conflict fallback only when requested.
- Rejects empty or oversized content.
- Uses atomic write.

### Outline Update

- Updates status after save.
- Does not run before save success.
- Returns partial success/warning when outline update fails.
- Does not roll back saved chapter on outline failure.

### Chinese-Safe Text Budgeting

- Truncates Chinese text without invalid UTF-8.
- Handles Chinese punctuation boundaries.
- Handles mixed Chinese/English.
- Handles emoji and multi-byte characters.
- Reports char counts correctly.
- Does not use whitespace token count for Chinese preflight budgets.

## Integration Tests

- `generate_chapter_autonomous` success path emits deterministic events and persists chapter.
- Conflict path emits `chapter_generation_conflict` and does not overwrite target.
- Provider failure emits exactly one typed failed event.
- Event order:
  1. `chapter_generation_started`
  2. `chapter_generation_context_built`
  3. provider progress
  4. save progress
  5. outline progress
  6. completed/conflict/failed
- `batch_generate_chapter` delegates to the shared primitive pipeline or returns typed deprecation once unused.
- Outline update failure yields saved chapter plus warning/partial success.
- Duplicate same-chapter tasks are rejected, serialized, or conflict deterministically.

## E2E / Smoke

No JS test script exists yet, so automated frontend E2E is not part of the MVP unless a harness is added.

Manual/event-contract smoke:

- Type `帮我写第 3 章初稿` in AgentPanel.
- Confirm progress timeline appears.
- Confirm current editor content remains unchanged while another chapter is generated.
- Confirm generated chapter appears when selected later.
- Confirm dirty/open target chapter produces conflict UI and no overwrite.
- Confirm legacy OutlinePanel generation still works or shows typed deprecation state.

## Stress Tests

- Synthetic project around 1,000,000 Chinese chars, 100 chapters, and 1,000+ RAG chunks.
- Context building respects all caps and does not include whole novel.
- Large lorebook/notes over budget produce truncation warnings, not crashes.
- 20 sequential requests with unique request IDs show no event cross-talk.
- 5 concurrent requests for same chapter produce at most one replace; others conflict/serialize/save-as-draft according to chosen policy.
- Provider timeout/rate-limit returns recoverable typed error.

## Observability Checks

- Logs include request ID, phase durations, source counts, truncation counts, provider/model, save mode, conflict reason, terminal status.
- Logs do not include full prompt or full generated content by default.
- Events include request ID and typed terminal state.

## Required Verification Commands

```powershell
cargo check --workspace
cargo test --workspace
npm run lint
npm run build
```
