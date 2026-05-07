# North Star Last Mile — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Close the gap between the North Star promise and the actual user experience — onboarding, retrospective, story overview, and trust-building.

**Architecture:** 4 items, all leveraging existing backend data. 1 new backend endpoint + 3 frontend changes. 4 evals.

---

### Task 1: Project Onboarding — First Launch Companion

**Files:** `CompanionPanel.tsx`, `today_five.in.rs`

When a project has 0 chapters (detectable from today_five data), show a welcome message instead of the normal 5 items:

```
欢迎使用 Forge 🚀

我是你的写作伙伴。我会：
• 记住你书中的角色、线索和承诺
• 在写下一章时提醒你该注意什么
• 帮你发现故事中的矛盾和缺失

开始写第一章吧——先写一个开头，然后点"生成下一章"。
```

Backend: in `today_five_summary()`, detect if project is empty (0 chapters, 0 characters). If so, set a flag `is_onboarding: true`. Frontend checks this flag and renders the welcome message.

Commit: `feat: add project onboarding welcome message`

### Task 2: Story Overview — Cross-Chapter Summary

**Files:** `today_five.in.rs`, `CompanionPanel.tsx`

Add a 6th section to the Companion (expandable/collapsible) showing a story overview:

```
📖 故事概览
角色们: 林墨(主角) · 张三(配角) · 共12人
线索进展: 3条待兑现 / 5条已兑现
情绪轨迹: 第1章 好奇 → 第3章 紧张 → 第5章 感动
最近揭示: 第4章 林墨发现了北境宗主的身份
```

Backend: add a `story_snapshot()` method to the kernel that aggregates entity counts, promise stats, emotional beats from recent chapters, and latest reveal events.

Frontend: add a toggle section at the top or bottom of Companion showing this snapshot.

Commit: `feat: add cross-chapter story overview`

### Task 3: Retrospective — "What Did We Just Do?"

**Files:** `today_five.in.rs`, `CompanionPanel.tsx`

Add a "retrospective mode" the user can trigger — a summary of what changed in the last session. This consumes existing settlement data:

```
📝 本次写作回顾
写了 3 章 (第3-5章)，共 10,200 字
推进了 2 条线索：林墨揭开了密信来源，张三与李四和解
新增了 1 个角色：王五
1 个风险需要注意：寒玉戒指下落仍未交代
```

Backend: add `recent_session_summary()` — reads the last N chapter settlements and aggregates changes.

Commit: `feat: add session retrospective summary`

### Task 4: Trust Building — "What I Learned From You"

**Files:** `today_five.in.rs`, `CompanionPanel.tsx`

Add a small trust indicator in the guard item detail:

```
📊 了解你的写作习惯
接受的建议: 12/15 (80%)
忽略的提醒: 3/8 (38%) — 已将相似提醒降级
喜欢的风格: 对话中有潜台词 · 动作描写简洁
```

Backend: read feedback stats from `WriterMemory` (accepted/rejected proposal counts, style preferences, ignored warning counts). Append to guard detail.

Commit: `feat: add trust-building feedback stats to Companion`

### Tasks 5-8: 4 New Evals

- `onboarding_contract` — empty project → today_five has is_onboarding=true
- `story_snapshot_contract` — project with 5 chapters → snapshot has chars, promises, reveals
- `retrospective_contract` — session with 3 chapters → summary has word count, promises advanced
- `trust_stats_contract` — project with feedback data → stats show acceptance rate

Register all 4. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 319

Commit: `feat: add 4 north star last mile evals`

### Task 9: Baseline
Change `315/315` to `319/319`. Commit: `chore: baseline after north star last mile`
