# Agent-Writer

Agent-Writer is a local AI writing desktop app for long-form fiction. It combines a Tiptap editor, a Tauri/Rust harness, project files, lorebook retrieval, chapter generation, script review, and a lightweight local memory/RAG layer.

## Stack

- Desktop shell: Tauri 2
- Frontend: React, TypeScript, Vite, Tailwind CSS, Zustand, Tiptap
- Backend: Rust workspace with `src-tauri` and `agent-harness-core`
- Storage: local app data files, SQLite via `rusqlite`, JSON vector store
- LLM API: OpenAI-compatible chat completions and embeddings endpoints

## Main Features

- Multi-chapter writing workspace with auto-save.
- Tiptap rich-text editor with inline AI commands.
- Agent side panel with streaming responses.
- Action tags for editor operations: insert, replace, and lorebook search.
- Lorebook for characters, locations, and world details.
- Outline/beat sheet with background chapter generation.
- Script Doctor review with inline highlighted comments.
- Project Brain RAG over saved chapter chunks.
- Entity graph, storyboard, and pacing analysis views.
- Hermes-inspired memory for interaction history and learned writing rules.
- API key storage through the OS keychain and diagnostic log export.

## Project Layout

```text
.
├─ src/                    React frontend
│  ├─ components/          App panels and views
│  ├─ extensions/          Tiptap marks
│  ├─ App.tsx              Three-column application shell
│  └─ store.ts             Zustand state
├─ src-tauri/              Tauri app and product-specific Rust commands
├─ agent-harness-core/     Shared Rust harness modules
├─ public/                 Static frontend assets
└─ docs/P*.md              Phase development plans outside this repo
```

## Configuration

The app first tries to read the OpenAI-compatible API key from the OS keychain under provider `openai`. It can also fall back to `OPENAI_API_KEY` from the environment.

Optional environment variables:

```text
OPENAI_API_KEY=...
OPENAI_API_BASE=https://api.openai.com/v1
OPENAI_MODEL=gpt-4o-mini
```

Embedding currently uses `text-embedding-3-small`.

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

Check Rust workspace:

```powershell
cargo check --workspace
```

Run tests:

```powershell
cargo test --workspace
```

Lint frontend:

```powershell
npm run lint
```

## Current Architecture Notes

The repository has already been split into a Rust workspace, but `src-tauri/src/lib.rs` still contains much of the product-specific orchestration. `agent-harness-core` holds reusable pieces such as action parsing, config types, memory, routing, planner types, and vector search.

The current protocol still uses Tauri invoke commands and event streams. Frontend action handling is intentionally simple and XML tag based, with tags such as `<ACTION_INSERT>...</ACTION_INSERT>`.

## Known Gaps

- The Tauri backend needs further modularization.
- Event names and command names should be centralized.
- Planner/DAG execution is not yet fully wired into the main agent loop.
- Hierarchical summaries have storage support but not a complete generation pipeline.
- Storyboard physical file reordering needs a safer persistence model.
