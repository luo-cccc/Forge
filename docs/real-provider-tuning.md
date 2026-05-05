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
- The dedicated repeated-runs chapter probe now lives at `scripts/chapter-stability-probe.cjs`. With identical frozen inputs, `chapter3` showed `latencyStddev≈6.1s`, `charsStddev≈97`, and `anchorCarryRate` ranging from `0.6` to `1.0`, which indicates both provider jitter and prompt-level instability. `chapter4` showed `latencyStddev≈7.1s`, but much lower content spread (`charsStddev≈36`, `anchorCarryRate 0.8..1.0`), which points more strongly to provider jitter than prompt drift.
- The `chapter3` probe also surfaced the concrete instability pattern: the same input swings between a compact “enter the gate, meet the reflection” scene and a wider “old debt history + account-book exposition + gate rules” scene. That spread is wider than `chapter4`, so the next prompt/context change should target chapter3’s context shape specifically instead of lowering chapter-wide budgets again.
- After restructuring chapter generation context for chapter3-style scenes, the repeated-runs probe improved materially. `chapter3` moved from `latencyStddev≈6.1s` and `charsStddev≈97` down to about `1.4s` and `70` respectively, while `anchorCarryRate` stayed within `0.6..1.0`. `chapter4` remained high-jitter on latency (`≈15.0s`) with comparatively stable content spread, which strengthens the conclusion that chapter3 had a real prompt/context instability component while chapter4 is still mostly provider-driven.

## Next Calibration Targets

- Run the same scenario against at least one longer real author project and one different provider/model before hardening the defaults further.
- Calibrate the new anchor-carry metric against real generated chapters and author judgments; the current version is deterministic and useful, but still heuristic.
- Capture provider usage and TTFT for streaming paths so Context Spine / prompt-cache work can be tuned against real latency rather than local fingerprints alone.
- Narrow the `chapter3` prompt/context shape first; its repeated-run spread shows prompt instability on top of provider jitter.
