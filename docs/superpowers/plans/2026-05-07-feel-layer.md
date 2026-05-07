# Feel Layer Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Make the Companion panel readable by a novice author — no technical jargon, clear action prompts, human-readable state.

**Architecture:** 6 items: 3 frontend-only (CompanionPanel.tsx text changes), 2 backend tweaks (today_five.in.rs labels + save message), 1 eval.

**Tech Stack:** TypeScript (React), Rust

---

### Task 1: TodayFive Labels — De-Jargonify

**Files:** `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

Change the `label` fields in TodayFive items:
- `"Agent Guard"` → `"今日状态"`
- `"Book Contract"` → `"全书承诺"`
- `"Chapter Mission"` → `"本章目标"`
- `"Open Promise"` → `"待兑现线索"`
- `"Next Move"` → `"下一步"`

Change `tone` values to human-readable:
- `"danger"` → `"⚠️ 需要注意"`
- `"accent"` → `"📝 提个醒"`
- `"success"` → `"✅ 一切正常"`

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: de-jargonify TodayFive labels and tones`

---

### Task 2: Save Feedback — Human-Readable Summary

**Files:** `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

Add a one-sentence chapter-completion summary to the guard item detail, using actual entity data from memory:
- Character count changes, promise advancement, relationship updates
- Format: `"本章写了 X 字，推进了 Y 条线索"`

This enriches the existing guard_detail with entity-level summary.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add human-readable chapter summary to TodayFive guard`

---

### Task 3: Ghost Text — User-Facing Hints

**Files:** `src/components/EditorPanel.tsx` or `src/extensions/GhostText.ts`

Add a visible hint near ghost text: `Tab 接受 · Esc 忽略`. If already present, verify it's visible and clear. If not, add a small tooltip or fixed-position hint bar when ghost text is active.

If the hint already exists in some form, just ensure it's always visible when ghost text is rendered (not hidden behind hover).

Verify: `npx tsc --noEmit` passes.
Commit: `feat: ensure ghost text interaction hints are always visible`

---

### Task 4: Save Toast — Entity-Level Feedback

**Files:** `src/components/CompanionPanel.tsx` or the save handler in `App.tsx`

After save, replace generic "Settlement applied" message with a simple summary using TodayFive data already available in state. Example: `"第 5 章已保存 · 林墨的线索已更新"`.

Minimal implementation: read the existing `todayFiveSummary` state and extract the promise item's detail text for the toast.

Verify: `npx tsc --noEmit` passes.
Commit: `feat: show entity-level feedback in save confirmation`

---

### Task 5: Next Chapter Readiness Indicator

**Files:** `src/components/CompanionPanel.tsx`

Add a simple readiness indicator near the "next" slot or a generation button area:
- Green `✅ 可以继续` — no active debt or all debts are low-risk
- Yellow `📝 建议先处理` — has open canon risk or medium-priority debt
- Red `⚠️ 有阻断问题` + reason — met cognitive gate block or high-risk continuity issue

This reads `todayFiveSummary.items[0].tone` and the `guard` item detail to determine state.

Verify: `npx tsc --noEmit` passes.
Commit: `feat: add next-chapter readiness indicator`

---

### Task 6: Baseline

Update `scripts/verification-baseline.cjs` if needed, run `npm run build` to verify frontend, run `npm run verify`.

Commit: `chore: final baseline after feel layer sprint`
