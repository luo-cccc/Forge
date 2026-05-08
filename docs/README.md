# Documentation Map

Last updated: 2026-05-08

## Active Docs

- [`README.md`](../README.md): product overview, architecture, setup, and verification baseline.
- [`plan.md`](../plan.md): Forge project whitepaper. Defines the product thesis, invariants, state model, validation bars, and the archived execution program.
- [`project-status.md`](./project-status.md): current implementation status, stable capabilities, cleanliness decisions, and remaining gaps.
- [`real-provider-tuning.md`](./real-provider-tuning.md): real-provider tuning history and latency/profile notes.
- [`UNWRAP_AUDIT.md`](./UNWRAP_AUDIT.md): unwrap audit notes.

## Historical Archive

- `superpowers/plans/`: dated implementation plans kept for historical context.
- `superpowers/specs/`: dated design specs that reference the whitepaper sections in `plan.md`.
- Historical docs may preserve milestone-era wording or older verification counts. Use `plan.md` and `project-status.md` as the current interpretation layer.

## Generated / Ignored Outputs

- `reports/`: generated eval and benchmark outputs; ignored from git.
- `dist/`, `target/`, `node_modules/`: local build artifacts and dependencies; ignored from git.

## Cleanup Policy

- Prefer adding ongoing reference material under `docs/` only when it owns a durable product or engineering decision.
- Treat dated `superpowers/` docs as archive-first material unless a historical correction is required.
- When verification counts change intentionally, update the baseline blocks in `README.md` and `project-status.md` via `npm run baseline`.
