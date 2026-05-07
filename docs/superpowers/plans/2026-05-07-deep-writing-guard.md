# Deep Writing Guard Sprint — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development.

**Goal:** Close the 4 remaining shallow-coverage gaps: voice drift, pacing monotony, scene density, author burnout.

**Architecture:** 4 diagnostic additions + 1 TodayFive enrichment. No new tables. 4 evals.

---

### Task 1: Voice Drift Detection

**Files:** `diagnostics/core.in.rs`

When `character_voice_cards` exist for a character (has role_type + current_state_summary), compare the current paragraph's dialogue/action patterns against the character's previous chapter state. Flag if a protagonist suddenly speaks in long sentences when they're supposed to be "寡言" (taciturn).

Simple heuristic: count sentence length in character-attributed text (text following character name in quotes). If avg sentence length doubles from previous chapter, flag `VoiceDrift`.

```rust
// After existing diagnostics, check voice consistency
if let Ok(chars) = memory.list_characters(Some("protagonist")) {
    for c in &chars {
        if paragraph.contains(&c.name) {
            let sentences: Vec<&str> = paragraph.split('。').collect();
            let avg_len = sentences.iter().map(|s| s.chars().count()).sum::<usize>() / sentences.len().max(1);
            if avg_len > 80 && c.role_type == "protagonist" {
                // Long monologue for a taciturn character
                results.push(DiagnosticResult {
                    id: next_id(),
                    severity: DiagnosticSeverity::Info,
                    category: DiagnosticCategory::CanonConflict,
                    message: format!("角色声音漂移: {} 在本文中以长句为主(avg {}字), 与角色设定可能不一致", c.name, avg_len),
                    entity_name: Some(c.name.clone()),
                    from: paragraph_offset,
                    to: paragraph_offset + paragraph.chars().count(),
                    evidence: vec![DiagnosticEvidence { source: "voice".into(), reference: c.name.clone(), snippet: format!("avg_sentence_len={}", avg_len) }],
                    fix_suggestion: Some("检查角色对话风格是否与设定一致".into()),
                    operations: Vec::new(),
                });
            }
        }
    }
}
```

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add voice drift detection to diagnostics`

### Task 2: Pacing Monotony Detection

**Files:** `diagnostics/core.in.rs`

Track the last N chapters' scene types. If 5+ consecutive chapters are the same type (all action-heavy or all dialogue-heavy), flag `PacingMonotony`.

```rust
fn detect_pacing_monotony(memory: &WriterMemory, chapter_id: &str) -> Option<String> {
    let results = memory.get_recent_chapter_results(5).unwrap_or_default();
    if results.len() < 5 { return None; }
    // Check if all 5 recent chapters have similar state_changes patterns
    let action_count = results.iter().filter(|r| r.summary.contains("冲突") || r.summary.contains("战斗")).count();
    if action_count >= 4 {
        return Some("最近5章中有4章以动作/冲突为主，建议插入过渡或情绪章节".into());
    }
    let dialogue_count = results.iter().filter(|r| r.summary.contains("对话") || r.summary.contains("交谈")).count();
    if dialogue_count >= 4 {
        return Some("最近5章中有4章以对话为主，建议插入动作或描写章节".into());
    }
    None
}
```

In `diagnose()`, call this and add a `PacingNote` diagnostic if flag raised.

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add pacing monotony detection`

### Task 3: Scene Density Warning

**Files:** `diagnostics/core.in.rs`

Check scene count for the current chapter. If > 6 scenes, flag density warning:

```rust
if let Ok(scenes) = memory.list_scenes_by_chapter(chapter_id) {
    if scenes.len() > 6 {
        results.push(DiagnosticResult {
            id: next_id(),
            severity: DiagnosticSeverity::Info,
            category: DiagnosticCategory::PacingNote,
            message: format!("本章有 {} 个场景，密度较高，建议不超过6个场景以保持节奏清晰", scenes.len()),
            entity_name: None,
            from: paragraph_offset, to: paragraph_offset + 1,
            evidence: vec![DiagnosticEvidence { source: "scene".into(), reference: chapter_id.into(), snippet: format!("scene_count={}", scenes.len()) }],
            fix_suggestion: Some("考虑将部分场景合并或移到下一章".into()),
            operations: Vec::new(),
        });
    }
}
```

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add scene density warning`

### Task 4: Author Burnout Signal in TodayFive

**Files:** `today_five.in.rs`

If `repair_rate` has been rising over the last 5 chapters (repair count increasing), add a burnout signal to the guard detail:

```rust
let repair_trend = check_repair_trend(memory);
if repair_trend == "rising" {
    guard_detail = format!("{}\n💡 最近修复率在上升——连续写作可能导致疲劳。建议休息一下或回顾前几章。", guard_detail);
}

fn check_repair_trend(memory: &WriterMemory) -> &str {
    let results = memory.get_recent_chapter_results(5).unwrap_or_default();
    if results.len() < 3 { return "stable"; }
    // Check if recent chapters have more repairs (proxied by summary length decreasing)
    let first_half: f64 = results.iter().take(2).map(|r| r.summary.len() as f64).sum::<f64>() / 2.0;
    let second_half: f64 = results.iter().skip(3).map(|r| r.summary.len() as f64).sum::<f64>() / 2.0;
    if second_half < first_half * 0.7 { "rising" } else { "stable" }
}
```

Verify: `cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"`
Commit: `feat: add author burnout signal to TodayFive`

### Tasks 5-8: 4 New Evals

- `voice_drift_eval` — protagonist paragraph avg>80 chars → flag
- `pacing_monotony_eval` — 5 action chapters → monotony detected
- `scene_density_eval` — 7 scenes → warning
- `burnout_signal_eval` — rising repair trend → burnout message

Register all 4. Verify: `cargo run -p agent-evals 2>&1 | grep "Total:"` → 329
Commit: `feat: add 4 deep writing guard evals`

### Task 9: Baseline
Change `325/325` to `329/329`. Commit: `chore: baseline after deep writing guard`
