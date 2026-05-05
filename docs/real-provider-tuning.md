# Real Provider Tuning Log

Last updated: 2026-05-05

This log records sanitized evidence from local real-provider runs. It deliberately excludes API keys, raw generated prose, full prompts, and manuscript text. Raw local metrics live under ignored `reports/` files and should not be committed.

## Environment

- API base: OpenRouter-compatible endpoint.
- Model: `deepseek/deepseek-v4-flash`.
- Embedding model: `text-embedding-3-small`.
- Scenario: 5-chapter "镜中墟" author-session simulation.
- Operations per run: chapter draft, analysis, ghost preview, A/B/C parallel draft, manual rewrite, JSON extraction, and embedding for each chapter.

## Current Profile Decision

- Disable provider-scoped reasoning by default for short/structured profiles: JSON, ghost preview, analysis, parallel draft, manual rewrite, tool continuation, and Project Brain stream.
- Disable provider-scoped reasoning by default for chapter draft on OpenRouter as the current latency-first default.
- Keep the setting overridable with `OPENAI_CHAPTER_DRAFT_DISABLE_REASONING` because the A/B result shows an anchor-recall tradeoff, not an absolute quality win.
- Strengthen the chapter prompt so active anchors must be carried through scene action, dialogue, consequence, or payoff pressure, not mentioned only as labels.
- Keep `analysis` and `parallel_draft` as explicit on-demand commands; tune their token budgets first before introducing extra trigger gates.

## Sanitized Runs

| Run | Chapter reasoning | Avg chat latency | P95 chat latency | Avg draft chars | Min anchor hit rate | Min anchor carry rate | JSON valid | A/B/C valid | Hook rate | Provider failures |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2026-05-05T11:56Z | Disabled | 5.1s | 17.5s | 662 | 0.8 | n/a | 1.0 | 1.0 | 1.0 | 0 |
| 2026-05-05T12:14Z | Enabled | 6.9s | 25.0s | 571 | 0.8 | n/a | 1.0 | 1.0 | 1.0 | 0 |
| 2026-05-05T12:18Z | Disabled | 5.2s | 19.9s | 564 | 0.6 | n/a | 1.0 | 1.0 | 1.0 | 0 |
| 2026-05-05T12:54Z | Disabled | 5.1s | 13.4s | 591 | 0.8 | 0.8 | 1.0 | 1.0 | 1.0 | 0 |
| 2026-05-05T13:32Z | Disabled (`chapter maxTokens=640`) | 4.5s | 12.5s | 681 | 0.8 | 0.8 | 1.0 | 1.0 | 1.0 | 0 |

## Evidence-Based Findings

- JSON empty-output failures were caused by reasoning-token budget consumption in earlier runs. Disabling OpenRouter reasoning for JSON fixed the observed JSON validity issue in the later 5-chapter runs.
- Short profile schema stability is acceptable in the current measured setup: JSON validity, A/B/C branch validity, and hook detection all stayed at 1.0 in the recorded runs.
- Chapter reasoning disabled lowered average latency in both disabled-vs-enabled comparisons, but the latest disabled run had lower minimum anchor hit rate. The project should not claim a final optimum yet.
- The remaining bottleneck is latency tail. Even with reasoning disabled, one JSON call reached about 31.7s in the latest run, so provider/network variance and profile-specific retries still need observation.
- First-pass anchor carry scoring is now covered by `writer_agent:anchor_carry_metric`. It catches the concrete failure mode exposed by real testing: a draft can mention every anchor while carrying none of them through action, dialogue, consequence, or payoff pressure.
- The real-provider calibration chain now has two levels: a Rust `api_integration_tests::real_author_session_three_chapter_smoke` gate for repeatable opt-in regression checks, and the ignored local 5-chapter runner for richer tuning metrics. The latest 5-chapter run with chapter reasoning disabled and carry scoring enabled reached `minAnchorCarryRate=0.8` with `p95ChatLatencyMs=13398`.
- The long-session runner now lives at `scripts/real-author-session-runner.cjs` and reads shared anchor-carry heuristics from `config/anchor-carry-heuristics.json`, so Node calibration and Rust scoring no longer drift from two separate rule lists.
- The long-session runner and Rust runtime now also share profile defaults from `config/llm-request-profiles.json`, so chapter/ghost/analysis/parallel/manual profile baselines no longer diverge between local calibration and product code.
- The current best measured chapter-draft profile is `maxTokens=640` with the stronger anchor-participation prompt. In the latest 5-chapter run it reached `avgChatLatencyMs=4498`, `p95ChatLatencyMs=12499`, `minAnchorHitRate=0.8`, and `minAnchorCarryRate=0.8`, with no findings raised by the runner.
- Targeted real-provider probes for on-demand tools showed `analysis.maxTokens=384` and `parallel_draft.maxTokens=512` are the best current tradeoff. `analysis` kept useful output around `avgChars=282` while lowering repeated-run `p95` versus the older 768-token setting in end-to-end runs, and `parallel_draft=512` outperformed both `768` and `384` in focused latency probes without hurting the A/B/C output format.

## Next Calibration Targets

- Run the same scenario against at least one longer real author project and one different provider/model before hardening the defaults further.
- Calibrate the new anchor-carry metric against real generated chapters and author judgments; the current version is deterministic and useful, but still heuristic.
- Capture provider usage and TTFT for streaming paths so Context Spine / prompt-cache work can be tuned against real latency rather than local fingerprints alone.
- Split analysis and parallel draft behind explicit user actions if latency tail remains above the acceptable write-flow threshold.
