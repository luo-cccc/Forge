# Writing Quality Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Make the model write better by giving it better input — curated context, writing checklist, author voice, emotional arc, character voice cards. No new rules, no new limits.

**Architecture:** 5 prompt-enrichment modules, all in context assembly. No new tables. 5 evals.

---

### Task 1: Writing Checklist from Story Impact

**Files:** `context.in.rs`

In `build_chapter_context()`, generate a writing checklist from story_impact nodes:

```rust
fn build_writing_checklist(memory: &WriterMemory, chapter_title: &str) -> Vec<String> {
    let mut items = Vec::new();
    // Top promises near payoff
    if let Ok(promises) = memory.get_open_promise_summaries() {
        for p in promises.iter().filter(|p| p.priority >= 6).take(3) {
            items.push(format!("兑现或推进线索: {}", p.title));
        }
    }
    // Active character arcs
    if let Ok(chars) = memory.list_characters(Some("protagonist")) {
        for c in chars.iter().take(2) {
            if let Ok(Some(state)) = memory.get_active_state(c.id, chapter_title) {
                if let Some(goals) = state.goal_state.as_object() {
                    for (k, v) in goals.iter().take(1) {
                        items.push(format!("{}: {} → {}", c.name, k, v));
                    }
                }
            }
        }
    }
    items
}
```

Inject at the top of the system prompt as "本章写作清单：\n- {item}\n- {item}".

Commit: `feat: add writing checklist from story impact to chapter prompt`

### Task 2: Curated Context — Top-N Instead of Full List

**Files:** `context.in.rs`, `context_relevance/typed_filter.in.rs`

Add a `select_top_context()` function that picks the TOP 3 most relevant items per category instead of including everything:

```rust
fn select_top_context(memory: &WriterMemory, chapter_title: &str) -> String {
    let mut lines = Vec::new();
    // Top 3 promises
    if let Ok(promises) = memory.get_open_promise_summaries() {
        let mut sorted = promises.clone();
        sorted.sort_by_key(|p| -p.priority);
        for p in sorted.iter().take(3) {
            lines.push(format!("线索: {} → {}", p.title, p.expected_payoff));
        }
    }
    // Top 3 knowledge items (concealed or suspecting only)
    // Top 3 character relationships
    lines.join("\n")
}
```

Inject as "## 关键信息\n{lines}" in the prompt, replacing the existing verbose context source list.

Commit: `feat: add curated top-N context selection to chapter prompt`

### Task 3: Author Voice Anchoring

**Files:** `context.in.rs`

Read the most recently saved chapter's first 200 chars and last 200 chars as style samples. Inject into prompt:

```rust
fn author_voice_sample(memory: &WriterMemory) -> String {
    let results = memory.get_recent_chapter_results(1).unwrap_or_default();
    if let Some(latest) = results.first() {
        let snippet: String = latest.summary.chars().take(300).collect();
        return format!("## 参考你的写作风格\n最近一章的风格示例：\n{}\n", snippet);
    }
    String::new()
}
```

Inject before the writing checklist. The model sees "write like THIS" rather than "don't write like THAT".

Commit: `feat: add author voice anchoring from recent chapters`

### Task 4: Emotional Arc Guidance

**Files:** `context.in.rs`

Read `reader_takeaway` from the previous chapter's settlement. Use it to set an emotional target for the current chapter:

```rust
fn emotional_arc_guidance(takeaway: Option<&ReaderTakeaway>) -> String {
    if let Some(t) = takeaway {
        format!("## 情感弧线\n上一章给读者的感受: {}。读者现在期待: {}。本章建议情绪走向: 开头延续{}，中段转折，结尾为下一章制造新的{}。",
            t.emotional_beat, t.expectation, t.emotional_beat, t.emotional_beat)
    } else {
        String::new()
    }
}
```

Inject after the writing checklist.

Commit: `feat: add emotional arc guidance from reader takeaway`

### Task 5: Character Voice Cards

**Files:** `context.in.rs`

For each character appearing in the current chapter (from story_impact or chapter mission), compile a compact voice card:

```rust
fn character_voice_cards(memory: &WriterMemory, chapter_title: &str) -> String {
    let mut cards = Vec::new();
    if let Ok(chars) = memory.list_characters(None) {
        for c in chars.iter().take(5) {
            let mut card = format!("{} ({}): ", c.name, c.role_type);
            if let Ok(Some(state)) = memory.get_active_state(c.id, chapter_title) {
                if let Some(goals) = state.goal_state.as_object() {
                    let goal_str: Vec<String> = goals.iter().map(|(k,v)| format!("{}={}", k, v)).collect();
                    card.push_str(&goal_str.join(", "));
                }
            }
            cards.push(card);
        }
    }
    if cards.is_empty() { return String::new(); }
    format!("## 角色速写\n{}", cards.join("\n"))
}
```

Inject after voice sample.

Commit: `feat: add character voice cards to chapter prompt`

### Tasks 6-10: 5 New Evals

Create 5 eval files:
- `writing_checklist_eval` — verify checklist has >= 1 item from open promises
- `curated_context_eval` — verify top-N filter returns <= 3 per category
- `voice_anchor_eval` — verify author_voice_sample returns non-empty for seeded memory
- `emotional_arc_eval` — verify arc guidance references emotional_beat from takeaway
- `voice_card_eval` — verify voice cards are generated for seeded characters

Register all 5. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 325
Commit: `feat: add 5 writing quality evals`

### Task 11: Baseline
Change `320/320` to `325/325`. Commit: `chore: baseline after writing quality sprint`
