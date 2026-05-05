# Forge Writer Agent

Forge is a local-first, Cursor-style writing agent for long-form fiction. The editor is the surface; the product is the persistent co-writer underneath it: a project-aware agent that observes the manuscript, remembers creative decisions, protects continuity, proposes typed operations, and learns from author feedback.

## Product Positioning

Forge is not a generic writing tool with AI buttons. It is meant to become a writing partner:

- A second brain for the book: canon, promises, chapter missions, decisions, and style memory stay available across sessions.
- A second novelist at the desk: the agent can continue, warn, draft, revise, and explain why.
- A quiet companion: the default right rail shows the few things the agent is guarding, not a noisy chat transcript.
- A Cursor-like workflow for prose: observations become proposals, proposals become inspectable operations, and feedback feeds the next turn.

## Current Architecture

```text
src/                         React + Tiptap writing surfaces
  App.tsx                    Three-column shell and chapter switching
  components/                Editor, companion panel, outline, lore, review views
  extensions/                Tiptap marks/plugins for ghost text, previews, lint, anchors
  protocol.ts                Tauri command/event and agent protocol types

src-tauri/                   Product-specific Tauri backend
  src/lib.rs                 Root module wiring, Tauri setup, command registration
  src/commands/              Tauri command handlers and command audit surface
  src/app_state.rs           Shared AppState, memory DB initialization, kernel seed
  src/writer_agent/          Writer Agent Kernel
  src/writer_agent/kernel.rs Kernel facade and stable public API
  src/writer_agent/kernel/   Stateful kernel implementation modules
  src/chapter_generation.rs  Autonomous chapter generation and save protection
  src/storage.rs             Local project storage and bounded backups

agent-harness-core/          Reusable agent runtime foundation
  provider/                  OpenAI-compatible provider abstraction
  agent_loop.rs              Tool-calling loop
  compaction.rs              Context compaction
  task_packet.rs             Five-axis task contract
  tool_registry.rs           Tool inventory and side-effect policy

agent-evals/                 Regression evals for writer-agent behavior, organized by evals/* responsibility modules
docs/                        Architecture plans and project status
scripts/                     Local static checks
```

## Foundation Features

- `TaskPacket` contract for core agent actions: objective, context, beliefs, tool policy, success criteria, and feedback checkpoints.
- Story Contract / Book Contract context for genre, reader promise, main conflict, boundaries, and tone.
- Chapter Mission tracking for what the current chapter must advance, include, avoid, and resolve.
- Result Feedback Loop on save: chapter summaries, state changes, new/settled promises, conflicts, and next-beat handoff.
- Promise Ledger for unresolved topics, emotional debt, object whereabouts, and payoff expectations.
- Companion Panel quiet mode: current guard state, chapter mission, open promises, canon risk, arc/pacing, and next step.
- Typed `WriterOperation` flow for text, canon, promise, style, story contract, chapter mission, and outline changes.
- Effective Tool Inventory and allowlist filtering so model-visible tools respect side-effect limits.
- Append-only WriterRunEventStore for privacy-preserving replay of observations, context packs, provider starts, tool calls, proposals, operation lifecycle, feedback, saves, diagnostics, and failures.
- Inspect mode for internal timeline review, failure evidence, provider budget reports, post-write diagnostics, proposal context budgets, and persisted context pressure trends.
- Trajectory export as JSONL for observations, proposals, feedback, task packets, state snapshots, product metrics, and Trace Viewer compatible local replay.

## Local Storage And Secrets

- Chapters, outline, lorebook, project brain, and writer memory live in local app data.
- API keys are stored through the OS keychain under provider `openai`; the renderer only checks whether a key exists.
- The app can also read `OPENAI_API_KEY` from the environment for development.
- Production CSP is restrictive; Vite localhost and `unsafe-eval` are only present in `devCsp`.
- `reports/`, `dist/`, `target/`, `node_modules/`, `.env`, and worktrees are local/generated and ignored.

Optional provider environment variables:

```text
OPENAI_API_KEY=...
OPENAI_API_BASE=https://openrouter.ai/api/v1
OPENAI_MODEL=deepseek/deepseek-v4-flash
```

## Development

Install dependencies:

```powershell
npm install
```

Run the web frontend only:

```powershell
npm run dev
```

Run the Tauri desktop app:

```powershell
npm run tauri dev
```

Build the frontend:

```powershell
npm run build
```

Run frontend lint:

```powershell
npm run lint
```

Run the P2 companion-surface guard:

```powershell
npm run check:p2
```

Run the P2 write-mode render guard:

```powershell
npm run check:p2-render
```

Clean generated eval reports:

```powershell
npm run clean:reports
```

Run Rust tests:

```powershell
cargo test -p agent-writer
```

Run writer-agent evals:

```powershell
cargo run -p agent-evals
```

## Verification Baseline

Before pushing foundation changes, run:

```powershell
npm run verify
```

Expected current baseline. This block is generated from `scripts/verification-baseline.cjs`; update it with `npm run baseline` when verification counts intentionally change.

<!-- verification-baseline:start -->
- `cargo test -p agent-harness-core`: 88 tests passing
- `cargo test -p agent-writer`: 209 tests passing
- `cargo run -p agent-evals`: 200/200 evals passing
- `npm run check:p2`: 18/18 checks passing
- `npm run check:p2-render`: write-mode DOM guard passing
- `npm run check:audit`: 56 commands, 0 issues
- `npm run check:architecture`: 7/7 files within budget, eval root guard passing
- `npm run lint`: passing
- `npm run build`: passing
- `cargo fmt --all -- --check`: passing
- `git diff --check`: passing
<!-- verification-baseline:end -->

`cargo run -p agent-evals` writes local reports under `reports/`; `npm run verify` cleans generated eval reports before checking whitespace.

## Current Engineering Priorities

P0/P1 foundation work comes before new UI:

- Keep manuscript persistence transactional: dirty state, chapter switching, autosave, and accepted feedback must not diverge.
- Keep the Writer Agent Kernel as the owner of agent intelligence; the frontend renders observations and proposals.
- Keep all model actions typed and reviewable through `WriterOperation`.
- Keep memory grounded in author feedback and saved manuscript results.
- Keep generated reports and build outputs out of git.

See [Project Status](docs/project-status.md) for the latest architecture state and remaining gaps.
