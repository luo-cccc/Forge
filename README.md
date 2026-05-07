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

## State Layers

### Entity State (Sprint A)
- Characters table with role types, aliases, and current state summaries — authoritative over canon_entities
- Character state versions: chapter-interval-valid snapshots of commitments, goals, and identity state
- Character relationships: pairwise edges with relation type, visibility, and chapter validity windows
- Promise subject binding: promises explicitly bound to characters/relationships/objects

### Knowledge & Identity (Sprint B)
- Knowledge items: objective/ambiguous/retconned truth states
- Knowledge ownership: per-character knowledge modes (aware/misbelief/suspecting/concealing) with chapter windows
- Identity layers: public vs private identity per character, with reveal tracking
- Reveal events: timestamped transitions from hidden to known

### Scene Orchestration (Sprint C)
- Scenes: chapter subdivision with sequence, type (scene/flashback/interlude), and summary
- Scene state: objective, participants, location, entry/exit states
- Scene obligations: promise/mission/payoff bindings per scene
- Scene results: outcome and consequence per scene

### Timeline (Sprint D)
- Story time slices: labeled time periods with relative ordering — independent of chapter order
- Chapter time mapping: per-chapter/scene narrative mode (present/flashback/flashforward/parallel)
- Timeline events: typed events keyed to time slices and subjects

### Algorithm Adaptation
- Typed filter pre-step in context retrieval: entity/knowledge/scene boosts before text rerank
- Multi-factor promise planner: subject pressure × knowledge readiness × timeline due × hook triage × reader expectation × emotional debt × rejection penalty
- Full-factor diagnostics: canon + OOC + timeline + knowledge visibility + identity conflict + scene obligation + emotional debt + flashback identity
- Input governance compiler: pre-generation intent/evidence/rule-stack compilation artifact

## Foundation Features

- `TaskPacket` contract for core agent actions: objective, context, beliefs, tool policy, success criteria
- Story Contract / Chapter Mission / Result Feedback Loop for chapter-level contract enforcement
- Promise Ledger: plot promises, emotional debt, object whereabouts, character commitments, mystery clues
- `ChapterContract`: 3500±500 chars enforced at draft+save; continuation/compress/hard_compress phases
- Supervised Sprint v2: pause/resume/checkpoint/budget ceiling for multi-chapter advancement
- VectorDB: ANN + BM25 hybrid search, <5ms @ 50K chunks
- Story OS: 3-tier query (hot/warm/cold), context assembly <5ms @ Chapter 500
- Typed `WriterOperation`: 40+ variants covering text, canon, promise, character, relationship, knowledge, identity, scene, timeline
- Companion Panel: 5-item TodayFiveSummary (guard/contract/mission/promise/next), de-jargonified labels
- Instinct mode: full timeline, failure evidence, provider budget, diagnostics, context pressure
- Append-only WriterRunEventStore + trajectory JSONL export
- Input governance compiler: pre-generation intent/evidence/rule-stack artifact
- Feedback learning: planner penalty on rejected kinds, ghost boost on accepted styles, diagnostic severity demotion on ignored warnings
- Reader Compensation: per-chapter emotional beat, expectation, unresolved lack projections
- Emotional Debt: pressure cue extraction, payoff boost, overdue detection
- Per-phase generation progress events to frontend

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
OPENAI_CHAT_TEMPERATURE=0.7
OPENAI_JSON_TEMPERATURE=0.0
OPENAI_CHAT_MAX_TOKENS=4096
OPENAI_JSON_MAX_TOKENS=1024
OPENAI_CHAPTER_DRAFT_TEMPERATURE=0.75
OPENAI_CHAPTER_DRAFT_MAX_TOKENS=6000
OPENAI_GHOST_PREVIEW_TEMPERATURE=0.55
OPENAI_GHOST_PREVIEW_MAX_TOKENS=160
OPENAI_ANALYSIS_TEMPERATURE=0.2
OPENAI_ANALYSIS_MAX_TOKENS=384
OPENAI_PARALLEL_DRAFT_TEMPERATURE=0.85
OPENAI_PARALLEL_DRAFT_MAX_TOKENS=512
OPENAI_MANUAL_REWRITE_TEMPERATURE=0.6
OPENAI_MANUAL_REWRITE_MAX_TOKENS=512
OPENAI_TOOL_CONTINUATION_TEMPERATURE=0.7
OPENAI_TOOL_CONTINUATION_MAX_TOKENS=2048
OPENAI_PROJECT_BRAIN_TEMPERATURE=0.3
OPENAI_PROJECT_BRAIN_MAX_TOKENS=4096
OPENAI_EMBEDDING_MODEL=text-embedding-3-small
OPENAI_EMBEDDING_INPUT_LIMIT_CHARS=8000
OPENAI_CHAT_DISABLE_REASONING=false
OPENAI_JSON_DISABLE_REASONING=true
OPENAI_CHAPTER_DRAFT_DISABLE_REASONING=true
OPENAI_GHOST_PREVIEW_DISABLE_REASONING=true
OPENAI_ANALYSIS_DISABLE_REASONING=true
OPENAI_PARALLEL_DRAFT_DISABLE_REASONING=true
OPENAI_MANUAL_REWRITE_DISABLE_REASONING=true
OPENAI_TOOL_CONTINUATION_DISABLE_REASONING=true
OPENAI_PROJECT_BRAIN_DISABLE_REASONING=true
```

The `*_DISABLE_REASONING` controls are provider-scoped. Forge currently sends OpenRouter reasoning controls only when `OPENAI_API_BASE` contains `openrouter.ai`; other OpenAI-compatible providers receive the standard chat payload.

Real provider integration tests are opt-in so normal verification stays offline and deterministic:

```powershell
$env:FORGE_REAL_API_TESTS="1"
$env:OPENAI_CHAT_MAX_TOKENS="256"
$env:OPENAI_JSON_MAX_TOKENS="256"
$env:OPENAI_PROJECT_BRAIN_MAX_TOKENS="96"
cargo test -p agent-writer api_integration_tests::chat_text_chinese_capability -- --nocapture
```

These tests require a real `OPENAI_API_KEY`; do not use them in CI without an explicit budget and secret policy.

Latest local real-provider tuning note, recorded on 2026-05-06 against OpenRouter `deepseek/deepseek-v4-flash`: the default long-chapter profiles now use `chapter_draft.maxTokens=5200`, `chapter_continuation.maxTokens=2200`, and `chapter_compress.maxTokens=5200` so `ChapterContract` can target `3500 +/- 500` Chinese characters. The latest explicit 30-chapter opt-in gate passed with `chapterCount=30`, `avgAnchorHit=0.8733`, `minAnchorCarryRate=0.60`, and `avgChars=1096`; that gate uses a shorter regression prompt, so treat it as continuity/anchor evidence, not proof of 3500-character real-model chapter stability. The earlier 2026-05-05 5-chapter "镜中墟" runs remain in `docs/real-provider-tuning.md` as latency/profile history. Both the runner and the Rust runtime read the same profile baseline and anchor-carry heuristics from `config/`.

For chapter stability debugging there is also a dedicated repeated-runs probe: `npm run probe:chapter-stability`. It freezes the chapter 3/4 inputs and repeats the same provider call so provider jitter and prompt instability can be measured separately.

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
- `cargo test -p agent-harness-core`: 89 tests passing
- `cargo test -p agent-writer`: 247 tests passing
- `cargo run -p agent-evals`: 307/307 evals passing
- `npm run check:p2`: 20/20 checks passing
- `npm run check:p2-render`: write-mode DOM guard passing
- `npm run check:save-path`: passed
- `npm run check:audit`: 74 commands, 0 issues
- `npm run check:architecture`: 14/14 files within budget, eval root guard passing
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
