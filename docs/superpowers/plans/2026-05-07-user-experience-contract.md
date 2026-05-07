# User Experience Contract Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Convert 8 implicit behavior rules into explicit contracts with eval gates — no new tables, no pipeline changes.

**Architecture:** Each rule = 1 code anchor point + 1 eval. 8 rules, 6 independent categories → 6 evals.

**Tech Stack:** Rust (src-tauri, agent-evals)

---

### Task 1: Companion Write-Mode Boundary Contract

Encode the 5-item-only rule as an eval that verifies `TodayFiveSummary` always has exactly 5 items with the correct slots: guard, contract, mission, promise, next. Each slot must have a non-empty label, value, detail, and valid tone.

**Eval:** `companion_write_mode_five_item_contract` — creates full kernel state, calls today_five_summary(), asserts items=5, slots match.

Commit: `feat: add Companion write-mode boundary contract eval`

---

### Task 2: TodayFive Sort Order Contract

Encode the priority-driven sort: entity pressure > knowledge concealment > scene obligation > promise urgency > chapter-level.

**Eval:** `today_five_sort_order_contract` — creates entities with varying pressure levels, knowledge items with concealment status, scenes with obligations, promises with different urgencies. Calls today_five_summary(). Asserts promise slot ranks entity-pressure promise highest.

Commit: `feat: add TodayFive sort order contract eval`

---

### Task 3: Auto-Repair vs Author-Confirm Boundary

Encode which failures auto-repair and which require author confirmation.

**Eval:** `auto_repair_vs_author_confirm_contract` — tests:
- Continuation triggered for < 3000 chars → auto-repair
- Compress triggered for > 4000 chars → auto-repair
- Hard compress > 4300 → requires author confirmation
- Contract violation → requires author confirmation
- Continuity block → requires author confirmation

Commit: `feat: add auto-repair vs author-confirm contract eval`

---

### Task 4: Interrupt vs Silent Contract

Encode when the agent interrupts vs stays silent.

**Eval:** `interrupt_vs_silent_contract` — tests:
- Generation failure → should interrupt (metacognitive gate blocks write)
- Save failure → should interrupt (save observation error)
- Ghost rejection (implicit) → should be silent (no Companion noise)
- Low-risk debt → should be silent (recorded in Inspect, not Companion)
- Canon conflict → should push warning to Companion

Commit: `feat: add interrupt vs silent contract eval`

---

### Task 5: Inspect Mode Boundary Contract

Encode what stays in Inspect vs what enters write mode.

**Eval:** `inspect_mode_boundary_contract` — verifies that raw trace events, task packets, operation lifecycles, metacognitive recovery actions, provider budget drilldown all stay in Inspect-only timeline and never leak into Companion write-mode summary.

Commit: `feat: add Inspect mode boundary contract eval`

---

### Task 6: Risk Prompt Contract

Encode when risk prompts fire.

**Eval:** `risk_prompt_contract` — tests:
- Canon conflict → Companion guard tone = "danger"
- OOC detected → Companion guard tone = "danger"
- Reveal timing conflict → Companion guard tone = "danger"
- No issues → Companion guard tone = "success" or "accent"

Commit: `feat: add risk prompt contract eval`

---

### Task 7: Baseline Update

Change `291/291` to `297/297`. Run `npm run baseline`.

Commit: `chore: update baseline after UX contract sprint`
