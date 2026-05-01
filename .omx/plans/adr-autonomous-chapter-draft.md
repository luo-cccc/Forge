# ADR: Autonomous Chapter Draft Architecture

> Status update: superseded as the primary architecture driver by `.omx/plans/adr-agent-foundation.md`. The chapter draft architecture can remain useful as a future agent tool, but the platform center is now the proactive agent runtime.

## Decision

Implement a dedicated backend autonomous chapter generation service built from shared primitives, with typed frontend protocol and compare-and-save conflict protection.

## Drivers

- Protect user edits during long-running generation.
- Keep generation side effects backend-owned.
- Make context composition bounded, testable, and reusable.
- Avoid duplicate legacy/autonomous generation logic.
- Support million-character Chinese projects without whole-novel prompt loading.

## Alternatives Considered

### Harden only `batch_generate_chapter`

Lower upfront cost, but keeps generation, context, save, outline update, events, and errors inside one monolithic command. It risks duplicate logic and weak conflict handling.

### Frontend orchestration

Fast to prototype, but unsafe for persistence and provider side effects. It would make stale editor state and overwrite bugs more likely.

### Dedicated backend primitive service

More structure, but creates clear boundaries for context building, generation, save conflict policy, outline update, and progress events.

## Why Chosen

The MVP’s core risk is not UI polish; it is safely mutating durable novel files while the frontend editor has its own autosave behavior. A backend service with shared primitives gives one auditable path for generation and persistence.

## Consequences

- Requires backend refactor around existing `batch_generate_chapter`.
- Requires typed frontend protocol additions.
- Requires frontend dirty/open chapter state reporting.
- Enables later cancellation, draft variants, progress improvements, multi-chapter workflows, and context previews without rewriting persistence rules.

## Pre-Mortem

### Silent overwrite of user edits

Cause: frontend dirty state ignored or stale revision save.

Mitigation: frontend dirty snapshot plus backend revision/CAS check. Either can block replacement.

Test: dirty open chapter and revision mismatch cases.

### Chinese prompt corruption or under-budgeting

Cause: byte slicing or whitespace token estimate on Chinese text.

Mitigation: Unicode char-boundary budget utility and hard source caps.

Test: Chinese/mixed/emoji truncation tests and large-context integration.

### Two generation paths drift

Cause: autonomous command and `batch_generate_chapter` keep separate prompt/save logic.

Mitigation: legacy command delegates to shared primitives or is explicitly deprecated after frontend migration.

Test: wrapper/deprecation test and code review gate confirming no duplicate context/model path.

## Execution Staffing

### Ralph

Recommended when one persistent owner should implement sequentially.

Sequence:

1. Map exact current surfaces.
2. Add backend text-budget utility and primitive contracts.
3. Add autonomous command and event schema.
4. Convert `batch_generate_chapter` to wrapper/deprecation path.
5. Wire frontend protocol and dirty/open state.
6. Add tests and observability.
7. Run final verification and code review.

### Team

Use only if running from an environment that supports the desired team surface.

Lanes:

- Backend executor: primitives, command orchestration, wrapper/deprecation.
- Frontend executor: protocol types, command invocation, event handling, dirty/open contract.
- Test engineer: unit/integration/stress tests and smoke checklist.
- Verifier/reviewer: conflict policy, logs, no duplicate path, final acceptance.

Shared-file caution:

- `src/protocol.ts` should have one owner.
- `src-tauri/src/lib.rs` and command registration should have one backend owner.
