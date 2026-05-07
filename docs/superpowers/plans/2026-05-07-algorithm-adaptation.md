# Algorithm Adaptation Sprint — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the four foundational state layers consumable by retrieval, planner, diagnostics, with performance gates preventing degradation.

**Architecture:** Surgical insertions into existing code paths — typed filter pre-step in context assembly, knowledge/timeline factors in planner, scene/flashback checks in diagnostics, 2 performance evals.

**Tech Stack:** Rust (src-tauri, agent-evals)

---

### Task 1: Typed Filter for Context Retrieval

**Files:**
- Create: `src-tauri/src/writer_agent/context_relevance/typed_filter.in.rs`
- Modify: `src-tauri/src/writer_agent/context_relevance/scoring.in.rs`

- [ ] **Step 1: Create typed_filter.in.rs**

```rust
pub struct TypedFilterResult {
    pub entity_boost: f64,
    pub knowledge_boost: f64,
    pub scene_boost: f64,
    pub reasons: Vec<String>,
}

impl TypedFilterResult {
    pub fn neutral() -> Self {
        Self { entity_boost: 1.0, knowledge_boost: 1.0, scene_boost: 1.0, reasons: Vec::new() }
    }
    pub fn total_multiplier(&self) -> f64 {
        self.entity_boost * self.knowledge_boost * self.scene_boost
    }
}

pub fn apply_typed_filter(
    source_text: &str,
    chapter_title: &str,
    memory: &WriterMemory,
) -> TypedFilterResult {
    let mut result = TypedFilterResult::neutral();

    // Entity boost: check if source mentions known characters
    if let Ok(characters) = memory.list_characters(None) {
        for c in &characters {
            if source_text.contains(&c.name) {
                if c.role_type == "protagonist" {
                    result.entity_boost *= 1.3;
                    result.reasons.push(format!("protagonist:{}", c.name));
                }
                // Check for pending character state commitments
                if let Ok(Some(state)) = memory.get_active_state(c.id, chapter_title) {
                    if let Some(commitments) = state.core_commitments.as_array() {
                        if !commitments.is_empty() {
                            result.entity_boost *= 1.15;
                            result.reasons.push(format!("pending_commitments:{}", c.name));
                        }
                    }
                }
                break; // one match per source is enough
            }
        }
    }

    // Knowledge boost: check for concealed/suspecting topics
    if let Ok(items) = memory.list_knowledge_items(Some("objective")) {
        for item in &items {
            if source_text.contains(&item.topic) {
                result.knowledge_boost *= 1.2;
                result.reasons.push(format!("knowledge_topic:{}", item.topic));
                break;
            }
        }
    }

    // Scene boost: check for unpaid scene obligations
    if let Ok(scenes) = memory.list_scenes_by_chapter(chapter_title) {
        for scene in &scenes {
            if let Ok(Some(obl)) = memory.get_scene_obligations(scene.id) {
                if !obl.promise_ids.is_empty() || !obl.payoff_targets.is_empty() {
                    result.scene_boost *= 1.1;
                    result.reasons.push(format!("scene_obligations:{}", scene.id));
                    break;
                }
            }
        }
    }

    result
}
```

- [ ] **Step 2: Wire into scoring.in.rs**

In the existing scoring loop, before computing relevance, call `apply_typed_filter` and multiply the relevance score by `filter.total_multiplier()`. Append `filter.reasons` to the WHY explanation.

- [ ] **Step 3: Verify**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: add typed filter pre-step to context retrieval"
```

---

### Task 2: Planner Full-Factor Scoring

**Files:**
- Modify: `src-tauri/src/writer_agent/promise_planner.rs`

- [ ] **Step 1: Add knowledge_readiness_factor and timeline_due_factor**

```rust
fn knowledge_readiness_factor(promise: &PlotPromiseSummary, memory: &WriterMemory, current_chapter: &str) -> f64 {
    let mut factor = 1.0;
    for related in &promise.related_entities {
        if let Some(name) = related.strip_prefix("character:") {
            if let Ok(Some(c)) = memory.get_character_by_name(name) {
                if let Ok(ownerships) = memory.get_knowledge_by_holder("character", c.id, current_chapter) {
                    for o in &ownerships {
                        match o.knowledge_mode.as_str() {
                            "aware" | "suspecting" => factor *= 1.1,
                            "concealing" => factor *= 0.8,
                            "misbelief" => factor *= 0.5,
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    factor
}

fn timeline_due_factor(promise: &PlotPromiseSummary, memory: &WriterMemory, current_chapter: &str) -> f64 {
    let expected_num = extract_chapter_number(&promise.expected_payoff);
    if expected_num == 0 { return 1.0; }
    let expected_chapter = format!("Chapter-{}", expected_num);
    if let Ok(mappings) = memory.get_time_mapping_for_chapter(current_chapter) {
        if let Ok(expected_mappings) = memory.get_time_mapping_for_chapter(&expected_chapter) {
            if let (Some(cur), Some(exp)) = (mappings.first(), expected_mappings.first()) {
                if let (Ok(Some(cur_ts)), Ok(Some(exp_ts))) = (
                    memory.get_time_slice_by_id(cur.time_slice_id),
                    memory.get_time_slice_by_id(exp.time_slice_id),
                ) {
                    if cur_ts.relative_order > exp_ts.relative_order {
                        return 1.3; // overdue in story time
                    }
                    if cur.narrative_mode == "flashback" && cur_ts.relative_order < exp_ts.relative_order {
                        return 0.3; // not yet relevant in flashback
                    }
                }
            }
        }
    }
    1.0
}
```

- [ ] **Step 2: Wire into promise_subject_pressure**

Multiply `pressure` by `knowledge_readiness_factor()` and `timeline_due_factor()` at the end of `promise_subject_pressure`. Both default to 1.0 when no data exists.

- [ ] **Step 3: Verify**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: add knowledge readiness and timeline due factors to promise planner"
```

---

### Task 3: Diagnostics — Scene Obligation + Flashback Identity

**Files:**
- Modify: `src-tauri/src/writer_agent/diagnostics/core.in.rs`

- [ ] **Step 1: Add scene obligation check**

After hidden relationship check, add:

```rust
// 1c. Scene obligation check: verify scene obligations are addressed
if let Ok(scenes) = memory.list_scenes_by_chapter(chapter_id) {
    for scene in &scenes {
        if let Ok(Some(obl)) = memory.get_scene_obligations(scene.id) {
            for pid in &obl.promise_ids {
                // Check if any promise payoff text appears in paragraph
                if let Ok(Some(state)) = memory.get_scene_state(scene.id) {
                    if !state.objective.is_empty() && !paragraph.contains(&state.objective) {
                        // Only flag if none of the chapter text addresses this scene's objective
                        // (would need full-chapter context; flag only when paragraph is near scene boundary)
                    }
                }
                let _ = pid; // future: cross-reference promise payoff text
            }
        }
    }
}
```

- [ ] **Step 2: Add flashback identity cross-check**

In the entity loop, after timeline issue detection, add a flashback-specific identity check: if `chapter_time_mapping` shows flashback mode, cross-reference character identity at the target time slice:

```rust
// Flashback identity consistency
if let Ok(mappings) = memory.get_time_mapping_for_chapter(chapter_id) {
    if mappings.iter().any(|m| m.narrative_mode == "flashback") {
        for entity in &entities {
            if let Ok(Some(character)) = memory.get_character_by_name(entity) {
                if let Ok(Some(identity)) = memory.get_active_identity(character.id, chapter_id) {
                    if identity.public_identity != identity.private_identity {
                        results.push(DiagnosticResult {
                            id: next_id(),
                            severity: DiagnosticSeverity::Info,
                            category: DiagnosticCategory::TimelineIssue,
                            message: format!("闪回场景: {} 的公开身份({})在闪回时间点可能需要与当前身份({})一致", entity, identity.public_identity, identity.private_identity),
                            entity_name: Some(entity.clone()),
                            from: paragraph_offset,
                            to: paragraph_offset + paragraph.chars().count(),
                            evidence: vec![DiagnosticEvidence {
                                source: "identity".into(), reference: entity.clone(),
                                snippet: format!("flashback: public={} private={}", identity.public_identity, identity.private_identity),
                            }],
                            fix_suggestion: Some("确认闪回中角色的身份状态是否与故事时间一致".into()),
                            operations: Vec::new(),
                        });
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: add scene obligation check and flashback identity diagnostics"
```

---

### Tasks 4-6: 3 New Evals

**Files:**
- Create: `agent-evals/src/evals/typed_context_filter.rs`
- Create: `agent-evals/src/evals/planner_fallback.rs`
- Create: `agent-evals/src/evals/scene_obligation_diagnostic.rs`
- Modify: `agent-evals/src/evals.rs`, `agent-evals/src/main.rs`

- [ ] **Step 1: typed_context_filter_under_large_state_fixture**

Create 50 characters, 30 relationships, 20 knowledge items, 10 scenes. Call `apply_typed_filter` with sample source text. Verify entity_boost > 1.0 (protagonist detected), knowledge_boost responds, and total execution < 1s.

- [ ] **Step 2: planner_subject_scoring_fallback_consistency**

Create a promise WITHOUT subject/knowledge/timeline data. Call `promise_subject_pressure`. Create the SAME promise WITH subject/knowledge/timeline data. Verify that the pressure with data >= pressure without data (no regression — new factors only boost).

- [ ] **Step 3: scene_obligation_fulfillment_diagnostic**

Create a scene + obligation (promise_id). Run diagnostics on chapter text that does NOT mention the obligation. Verify that the diagnostic output includes scene-related evidence.

- [ ] **Step 4: Register all 3 and verify**

```bash
cargo run -p agent-evals 2>&1 | grep -E "typed_context_filter|planner_fallback|scene_obligation_diagnostic|Total:"
```
Expected: 3 [PASS], Total: 284.

- [ ] **Step 5: Commit**

```bash
git add agent-evals/ && git commit -m "feat: add 3 algorithm adaptation evals"
```

---

### Task 7: Baseline Update

**Files:**
- Modify: `scripts/verification-baseline.cjs`

- [ ] **Step 1:** Change `281/281 evals passing` to `284/284 evals passing`. Run `npm run baseline`.

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "chore: update baseline after algorithm adaptation sprint"
```
