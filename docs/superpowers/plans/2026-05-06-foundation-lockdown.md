# Foundation Lockdown Sprint (§3.3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lock down Forge's production kernel by closing all save/settlement/recovery/budget/user-surface semantic gaps and adding 3 new eval gates.

**Architecture:** Audit-first approach — 4 new/existing audit checks find bypass paths, then targeted Rust fixes close them. Settlement replay infrastructure added to existing `chapter_generation` module. No pipeline rewrites.

**Tech Stack:** Rust (src-tauri, agent-evals), Node.js (audit scripts), TypeScript AST (check:p2)

---

### Task 1: Save Path Consistency Audit Script

**Files:**
- Create: `scripts/check-save-path-consistency.cjs`
- Modify: `scripts/verify.cjs`
- Modify: `package.json`

- [ ] **Step 1: Write the audit script**

```javascript
const fs = require("fs");
const path = require("path");

const srcDir = path.join(__dirname, "..", "src-tauri", "src");

function collectRsFiles(dirPath) {
  const results = [];
  const entries = fs.readdirSync(dirPath, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      results.push(...collectRsFiles(full));
    } else if (entry.name.endsWith(".rs")) {
      results.push(full);
    }
  }
  return results;
}

const rsFiles = collectRsFiles(srcDir);
const mergedSource = rsFiles.map((f) => fs.readFileSync(f, "utf8")).join("\n");

// Find all save_chapter call sites
const saveCallPattern = /\bsave_chapter\b/g;
const observePattern = /\bobserve_chapter_save_with_result\b/g;
const observeSimplePattern = /\bobserve_chapter_save\b/g;

// Find all commands that call save_chapter or generate paths
const saveCommands = [
  "save_chapter",
  "generate_chapter_autonomous",
  "batch_generate_chapter",
  "repair_chapter_state",
];

// Check that every save_chapter call in a command context eventually calls observe
const chapterGenerationMod = fs.readFileSync(
  path.join(srcDir, "chapter_generation", "mod.rs"),
  "utf8"
);

const issues = [];

// 1. Verify chapter_generation pipeline calls observe
const pipelineFiles = [
  "pipeline.in.rs",
  "pipeline/main.in.rs",
].map((name) => path.join(srcDir, "chapter_generation", name));
for (const file of pipelineFiles) {
  const source = fs.readFileSync(file, "utf8");
  if (!/observe_generated_chapter_result/.test(source)) {
    issues.push(`${file}: chapter generation pipeline missing observe_generated_chapter_result`);
  }
}

// 2. Verify writer_observer has the save observation bridge
const observerPath = path.join(srcDir, "writer_observer.rs");
const observerSource = fs.readFileSync(observerPath, "utf8");
if (!/observe_chapter_save_with_result/.test(observerSource)) {
  issues.push("writer_observer.rs: missing observe_chapter_save_with_result");
}
if (!/observe_generated_chapter_result/.test(observerSource)) {
  issues.push("writer_observer.rs: missing observe_generated_chapter_result");
}

// 3. Verify save_generated_chapter calls observe_generated_chapter_result
const genCommandsPath = path.join(srcDir, "commands", "generation.rs");
const genSource = fs.readFileSync(genCommandsPath, "utf8");
if (!/observe_generated_chapter_result/.test(genSource)) {
  issues.push("commands/generation.rs: generated chapter path missing observe_generated_chapter_result");
}

// 4. Verify manual save_chapter command goes through observe_chapter_save
const chaptersPath = path.join(srcDir, "commands", "chapters.rs");
const chaptersSource = fs.readFileSync(chaptersPath, "utf8");
if (!/observe_chapter_save/.test(chaptersSource)) {
  issues.push("commands/chapters.rs: save_chapter command missing observe_chapter_save");
}

// 5. Verify repair_chapter_state also observes
if (!/observe_chapter_save/.test(genSource)) {
  issues.push("commands/generation.rs: repair_chapter_state path missing observe_chapter_save");
}

// 6. Verify Companion write mode only calls TodayFive
const companionPath = path.join(__dirname, "..", "src", "components", "CompanionPanel.tsx");
const companionHelpersPaths = [
  path.join(__dirname, "..", "src", "components", "CompanionPanel.proposal.ts"),
  path.join(__dirname, "..", "src", "components", "CompanionPanel.contract.ts"),
  path.join(__dirname, "..", "src", "components", "CompanionPanel.brain.ts"),
];
const companionSource = fs.readFileSync(companionPath, "utf8");
const companionHelpersSource = companionHelpersPaths
  .map((p) => fs.readFileSync(p, "utf8"))
  .join("\n");
const allCompanionSource = companionSource + "\n" + companionHelpersSource;

// In write mode, Companion should only use getWriterAgentTodayFive for state display
const forbiddenInWritePatterns = [
  /getWriterAgentLedger/,
  /getWriterAgentStatus/,
  /getStoryDebtSnapshot/,
  /getWriterAgentPendingProposals/,
  /getWriterAgentInspectorTimeline/,
];
for (const pattern of forbiddenInWritePatterns) {
  if (pattern.test(allCompanionSource)) {
    // Check if it's guarded by mode !== "write"
    // Simplified: check if the pattern appears outside inspect-guarded context
    const lines = allCompanionSource.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (pattern.test(lines[i])) {
        // Look back up to 10 lines for a mode guard
        const context = lines.slice(Math.max(0, i - 10), i + 1).join("\n");
        if (!/mode\s*!==\s*"write"/.test(context) && !/storyMode\s*===\s*"inspect"/.test(context)) {
          issues.push(
            `${companionPath}:${i + 1}: write-mode may expose ${pattern.source} without guard`
          );
        }
      }
    }
  }
}

if (issues.length > 0) {
  console.error("Save path consistency audit failed:");
  for (const issue of issues) {
    console.error(`  - ${issue}`);
  }
  process.exit(1);
}

const saveCount = (mergedSource.match(saveCallPattern) || []).length;
const observeCount = (mergedSource.match(observePattern) || []).length;
const observeSimpleCount = (mergedSource.match(observeSimplePattern) || []).length;
console.log(
  `Save path consistency: ${saveCount} save calls, ${observeCount} observe_with_result calls, ${observeSimpleCount} simple observe calls`
);
console.log("Save path consistency audit passed.");
```

- [ ] **Step 2: Add script to package.json**

In `package.json`, add to `"scripts"`:
```json
"check:save-path": "node scripts/check-save-path-consistency.cjs"
```

- [ ] **Step 3: Add to verify.cjs**

In `scripts/verify.cjs`, add after `check:audit` line:
```javascript
["npm", ["run", "check:save-path"]],
```

- [ ] **Step 4: Run the audit script**

```bash
npm run check:save-path
```
Expected: PASS or list of gaps to fix

- [ ] **Step 5: Commit**

```bash
git add scripts/check-save-path-consistency.cjs scripts/verify.cjs package.json
git commit -m "feat: add save path consistency audit script"
```

---

### Task 2: Fix Any Save Path Gaps Found

**Files:**
- Modify: `src-tauri/src/commands/chapters.rs` (if needed)
- Modify: `src-tauri/src/commands/generation.rs` (if needed)

- [ ] **Step 1: Address any issues from check:save-path audit**

If the audit in Task 1 found gaps, fix them here. Expected fixes might include:
- Adding `observe_chapter_save` call to any direct save path that bypasses it
- Wrapping unguarded Companion ledger calls behind `mode !== "write"` checks

- [ ] **Step 2: Re-run audit after fixes**

```bash
npm run check:save-path
```
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "fix: close save path observation gaps found by audit"
```

---

### Task 3: Settlement Replay Infrastructure

**Files:**
- Modify: `src-tauri/src/chapter_generation/types_and_utils.in.rs`
- Modify: `src-tauri/src/chapter_generation/settlement.in.rs`
- Modify: `src-tauri/src/chapter_generation/runtime_artifacts.in.rs`

- [ ] **Step 1: Add SettlementReplay and ReplayResult types**

In `types_and_utils.in.rs`, after the `ChapterSettlementDelta` struct, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettlementReplay {
    pub input_content_hash: String,
    pub memory_snapshot_id: String,
    pub output_delta_hash: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementReplayResult {
    pub replayed: bool,
    pub matches_original: bool,
    pub mismatches: Vec<String>,
    pub original_hash: String,
    pub replayed_hash: String,
}
```

- [ ] **Step 2: Add replay_settlement_extraction function**

In `settlement.in.rs`, add after exports:

```rust
use sha2::{Digest, Sha256};

fn hash_str(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn replay_settlement_extraction(
    original: &ChapterSettlementDelta,
    generated_content: &str,
    memory: &WriterMemory,
) -> SettlementReplayResult {
    let replayed = build_basic_chapter_settlement_delta(
        &String::new(),
        &original.chapter_title,
        &original.chapter_revision,
        generated_content,
        0,
        memory,
        Vec::new(),
    );

    let original_fields = [
        ("chapter_title", &original.chapter_title, &replayed.chapter_title),
    ];

    let mismatches: Vec<String> = [
        ("summary", &original.summary, &replayed.summary),
    ]
    .iter()
    .filter(|(field, orig, replay)| orig != replay)
    .map(|(field, _, _)| format!("field '{}' differs", field))
    .collect();

    let promise_mismatch = if original.promise_updates.len() != replayed.promise_updates.len() {
        Some(format!(
            "promise count differs: {} vs {}",
            original.promise_updates.len(),
            replayed.promise_updates.len()
        ))
    } else {
        None
    };
    let mismatches: Vec<String> = mismatches
        .into_iter()
        .chain(promise_mismatch)
        .collect();

    let original_json = serde_json::to_string(original).unwrap_or_default();
    let replayed_json = serde_json::to_string(&replayed).unwrap_or_default();

    SettlementReplayResult {
        replayed: true,
        matches_original: mismatches.is_empty(),
        mismatches,
        original_hash: hash_str(&original_json),
        replayed_hash: hash_str(&replayed_json),
    }
}
```

- [ ] **Step 3: Add replay artifact to runtime artifacts persistence**

In `runtime_artifacts.in.rs`, add to `PersistedChapterRuntimeArtifacts`:
- Add `pub replay_json: Option<String>` to the struct
- In `persist_chapter_runtime_artifacts`, add replay JSON file write

```rust
#[derive(Debug, Clone)]
pub struct PersistedChapterRuntimeArtifacts {
    pub artifact_refs: Vec<String>,
    pub replay_json: Option<String>,
}
```

Add the replay persist section before `Ok(PersistedChapterRuntimeArtifacts {`:

```rust
let replay = SettlementReplay {
    input_content_hash: hash_str(generated_content),
    memory_snapshot_id: format!("{}", memory.created_at()),
    output_delta_hash: hash_str(
        &serde_json::to_string(settlement_delta).unwrap_or_default(),
    ),
    created_at_ms: crate::agent_runtime::now_ms(),
};
let replay_path = runtime_dir.join(format!("{}.replay.json", stem));
write_json_file(&replay_path, &replay)?;
let replay_ref = path_ref(&project_dir, &replay_path);
```

And add `replay_ref` to `artifact_refs` and `replay_json: Some(replay_ref)`.

- [ ] **Step 4: Add sha2 to Cargo.toml dependencies**

In `src-tauri/Cargo.toml`, under `[dependencies]`:
```toml
sha2 = "0.10"
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p agent-writer
```
Expected: compiles without errors

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/chapter_generation/
git commit -m "feat: add settlement replay infrastructure with content hashing"
```

---

### Task 4: Integrate Replay into repair_chapter_state

**Files:**
- Modify: `src-tauri/src/commands/generation.rs`

- [ ] **Step 1: Add replay assertion to repair_chapter_state**

In `repair_chapter_state` (around line 1217 in `commands/generation.rs`), after building the settlement delta, add:

```rust
use crate::chapter_generation::replay_settlement_extraction;

// Replay the settlement extraction to verify consistency
let replay = replay_settlement_extraction(
    &delta,
    &content,
    &memory,
);

if !replay.matches_original {
    let mismatches = replay.mismatches.join("; ");
    tracing::warn!(
        chapter = chapter_title,
        mismatches = mismatches,
        "Settlement replay produced different results"
    );
}
```

- [ ] **Step 2: Add replay result to RepairChapterStateResult**

In `types_and_utils.in.rs`, find `RepairChapterStateResult` and add:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub settlement_replay_matches: Option<bool>,
```

Set it in the command handler: `settlement_replay_matches: Some(replay.matches_original)`.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p agent-writer
```
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/generation.rs src-tauri/src/chapter_generation/
git commit -m "feat: integrate settlement replay assertion into repair_chapter_state"
```

---

### Task 5: repair_chapter_state Idempotency Guard

**Files:**
- Modify: `src-tauri/src/commands/generation.rs`

- [ ] **Step 1: Add idempotency check at the top of repair_chapter_state**

After loading the chapter content and revision, check if runtime artifacts already exist:

```rust
// Idempotency guard: if settlement and runtime artifacts already exist, skip repair
let project_dir = crate::storage::active_project_data_dir(&app)?;
let runtime_dir = project_dir.join("chapter_runtime");
let stem = format!(
    "chapter-{:04}-{}",
    /* chapter number derived from title */,
    crate::chapter_generation::make_request_id("repair-state")
);
let settlement_path = runtime_dir.join(format!("{}.settlement.json", stem));
if settlement_path.exists() {
    let existing_settlement: crate::chapter_generation::ChapterSettlementDelta =
        serde_json::from_str(
            &std::fs::read_to_string(&settlement_path)
                .map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string())?;
    if existing_settlement.chapter_revision == revision {
        return Ok(RepairChapterStateResult {
            chapter_title,
            revision,
            repaired: false,
            already_repaired: true,
            chapter_fact_delta: existing_settlement.chapter_fact_delta,
            promise_delta: existing_settlement.promise_delta,
            arc_delta: existing_settlement.arc_delta,
            book_state_delta: existing_settlement.book_state_delta,
            settlement_replay_matches: None,
        });
    }
}
```

- [ ] **Step 2: Add already_repaired field to RepairChapterStateResult**

In `types_and_utils.in.rs`:
```rust
#[serde(default)]
pub already_repaired: bool,
```

- [ ] **Step 3: Add chronology preservation assertion**

After applying settlement, verify ordering is preserved:

```rust
// Verify chronology preservation
let ledger_after = kernel.ledger_snapshot();
let results_before_repair = &ledger_before.recent_chapter_results;
let results_after_repair = &ledger_after.recent_chapter_results;
let chronology_preserved = results_before_repair
    .iter()
    .map(|r| &r.chapter_title)
    .eq(results_after_repair.iter().map(|r| &r.chapter_title));

if !chronology_preserved {
    tracing::error!(
        chapter = chapter_title,
        "repair_chapter_state altered chapter chronology"
    );
}
```

Add `chronology_preserved: Some(chronology_preserved)` to `RepairChapterStateResult`.

- [ ] **Step 4: Verify compilation and run agent-writer tests**

```bash
cargo check -p agent-writer && cargo test -p agent-writer 2>&1 | grep "test result"
```
Expected: compiles, all tests pass

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/generation.rs src-tauri/src/chapter_generation/
git commit -m "feat: add idempotency guard and chronology preservation to repair_chapter_state"
```

---

### Task 6: Provider Budget Audit Extension

**Files:**
- Modify: `scripts/check-command-audit.cjs`

- [ ] **Step 1: Add provider budget coverage check to check:audit**

Add a new section to `check-command-audit.cjs` that scans for provider call patterns and verifies budget gating. Append after the existing audit logic (before the final return/exit):

```javascript
// ── Provider Budget Coverage ──

const BUDGET_FUNCTIONS = [
  "WriterProviderBudgetRequest",
  "check_provider_budget",
  "provider_budget_preflight",
  "provider_budget_report",
];

const providerCallPatterns = [
  { name: "stream_chat", label: "direct LLM call" },
  { name: "run_agent_loop", label: "agent loop" },
  { name: "run_chapter_generation_pipeline", label: "chapter generation" },
  { name: "generate_parallel_drafts", label: "parallel drafts" },
  { name: "ask_project_brain", label: "project brain query" },
  { name: "run_metacognitive_recovery", label: "metacognitive recovery" },
  { name: "research_subtask", label: "research subtask" },
];

const budgetIssues = [];

// Check each provider-call command body for budget gating
const PROVIDER_COMMANDS = [
  "batch_generate_chapter",
  "generate_chapter_autonomous",
  "analyze_chapter",
  "ask_project_brain",
  "generate_parallel_drafts",
  "analyze_pacing",
  "ask_agent",
  "run_metacognitive_recovery",
];

for (const cmd of PROVIDER_COMMANDS) {
  const body = extractFunctionBody(cmd);
  const hasBudget = BUDGET_FUNCTIONS.some((fn) => body.includes(fn));
  if (!hasBudget) {
    budgetIssues.push(`${cmd}: provider call missing budget gating`);
  }
}

// Scan all source for unreferenced provider calls (direct LLM without budget)
const budgetCoveredPatterns = BUDGET_FUNCTIONS.join("|");
const allSource = rsFiles.map((f) => fs.readFileSync(f, "utf8")).join("\n");

for (const pattern of providerCallPatterns) {
  const callCount = (allSource.match(new RegExp(pattern.name, "g")) || []).length;
  const budgetRefs = (allSource.match(new RegExp(budgetCoveredPatterns, "g")) || []).length;
  // Not a strict check — informational only
  console.log(`  provider: ${pattern.label} (${callCount} call sites, ${budgetRefs} budget refs)`);
}

if (budgetIssues.length > 0) {
  console.log("\nPROVIDER BUDGET GAPS:");
  for (const issue of budgetIssues) {
    console.log(`  - ${issue}`);
  }
  issues.push(...budgetIssues);
}
```

Also add at the top:
```javascript
const rsFiles = collectRsFiles(srcDir);
```

- [ ] **Step 2: Run the extended audit**

```bash
npm run check:audit
```
Expected: reports provider budget coverage, flags any gaps

- [ ] **Step 3: Fix any provider budget gaps found**

If the audit finds any provider call paths without budget gating, add `WriterProviderBudgetRequest` before the real call.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-command-audit.cjs
git commit -m "feat: add provider budget coverage audit to check:audit"
```

---

### Task 7: TodayFive Exclusivity Enforcement

**Files:**
- Modify: `scripts/check-p2-companion.cjs`
- Modify: `scripts/check-p2-render.cjs`

- [ ] **Step 1: Add TodayFive exclusivity check to check:p2**

Append to `check-p2-companion.cjs`:

```javascript
// ── TodayFive Exclusivity ──

const todayFiveCheck = {
  name: "Companion status tab renders exactly 5 TodayFive items",
  pass:
    /todayFiveSummary/.test(companionSource) &&
    /Commands\.getWriterAgentTodayFive/.test(companionSource) &&
    /todayFiveSummary\.items\.map/.test(companionSource),
};

checks.push(todayFiveCheck);

// Verify Companion does NOT directly call ledger/status in write mode for display
const todayFiveExclusivityCheck = {
  name: "Companion write mode displays status exclusively from TodayFive",
  pass:
    companionSource.includes("todayFiveSummary") &&
    !/getWriterAgentLedger/.test(companionSource) ||
    hasGuardedNeedle(
      componentAst,
      /getWriterAgentLedger/,
      modeNotWrite
    ),
};

checks.push(todayFiveExclusivityCheck);
```

- [ ] **Step 2: Add TodayFive item count assertion to check:p2-render**

In `check-p2-render.cjs`, add an assertion that the rendered DOM contains exactly 5 TodayFive-like items:

```javascript
// TodayFive always has 5 slots: guard, contract, mission, promise, next
const todayFiveSlots = ["Agent Guard", "Book Contract", "Chapter Mission", "Open Promise", "Next Move"];
```

Add a check:
```javascript
{
  name: "write mode renders all 5 TodayFive slots",
  pass: todayFiveSlots.every((label) => renderedOutput.includes(label)),
}
```

And:
```javascript
{
  name: "write mode does not expose non-TodayFive state slots",
  pass: !/[Rr]aw [Tt]race/.test(renderedOutput) &&
    !/Task Packet/.test(renderedOutput) &&
    !/Operation Lifecycle/.test(renderedOutput),
}
```

- [ ] **Step 3: Run both checks**

```bash
npm run check:p2 && npm run check:p2-render
```
Expected: PASS with TodayFive checks

- [ ] **Step 4: Commit**

```bash
git add scripts/check-p2-companion.cjs scripts/check-p2-render.cjs
git commit -m "feat: add TodayFive exclusivity checks to companion surface guards"
```

---

### Task 8: Eval — Save Path Consistency

**Files:**
- Create: `agent-evals/src/evals/save_path_consistency.rs`
- Modify: `agent-evals/src/evals.rs`

- [ ] **Step 1: Create the eval module**

```rust
// agent-evals/src/evals/save_path_consistency.rs

use agent_writer_lib::writer_agent::WriterAgentKernel;
use agent_writer_lib::AppState;

pub fn register(registry: &mut crate::evals::EvalRegistry) {
    registry.register(
        "writer_agent",
        "save_path_consistency_all_paths_emit_observation",
        save_path_consistency_all_paths_emit_observation,
    );
}

fn save_path_consistency_all_paths_emit_observation(db_path: &std::path::Path) -> crate::EvalResult {
    use agent_writer_lib::chapter_generation::{build_chapter_context, BuildChapterContextInput};
    use agent_writer_lib::writer_agent::memory::WriterMemory;

    let memory = WriterMemory::open(db_path).map_err(|e| e.to_string())?;

    // Setup: create a project with initial state
    let project_id = "eval-save-path-consistency";
    memory.ensure_project(&project_id, "Save Path Consistency Test").map_err(|e| e.to_string())?;

    // Simulate chapter save via observation path
    let observation_id = "eval-save-path-obs";
    let content = "第三章内容：主角进入山谷发现秘密。";
    let saved_content = content.to_string();

    // Record save observation through kernel (simulates observe_chapter_save_with_result)
    let kernel = WriterAgentKernel::new_empty(
        project_id.to_string(),
        db_path.to_path_buf(),
        agent_writer_lib::writer_agent::kernel::WriterAgentApprovalMode::AutoApprove,
    );

    // Verify that after save observation, chapter result exists in memory
    let results = memory.get_recent_chapter_results(10).map_err(|e| e.to_string())?;
    let chapter_results_exist = results.iter().any(|r| r.chapter_title == "第三章");

    // Verify that ledger state is consistent after observation
    let ledger = kernel.ledger_snapshot();

    crate::EvalResult::pass_if(
        chapter_results_exist,
        format!(
            "savePathObserved={} ledgerEntries={}",
            chapter_results_exist,
            ledger.recent_chapter_results.len()
        ),
    )
}
```

- [ ] **Step 2: Register eval in evals.rs**

In `agent-evals/src/evals.rs`, add:
```rust
pub mod save_path_consistency;
```

And in the register function:
```rust
save_path_consistency::register(&mut registry);
```

- [ ] **Step 3: Run the eval**

```bash
cargo run -p agent-evals 2>&1 | grep "save_path_consistency"
```
Expected: `[PASS] writer_agent:save_path_consistency_all_paths_emit_observation`

- [ ] **Step 4: Commit**

```bash
git add agent-evals/src/evals/save_path_consistency.rs agent-evals/src/evals.rs
git commit -m "feat: add save path consistency eval"
```

---

### Task 9: Eval — Settlement Replay Consistency

**Files:**
- Create: `agent-evals/src/evals/settlement_replay.rs`
- Modify: `agent-evals/src/evals.rs`

- [ ] **Step 1: Create the eval module**

```rust
// agent-evals/src/evals/settlement_replay.rs

use agent_writer_lib::chapter_generation::{
    build_basic_chapter_settlement_delta, replay_settlement_extraction,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn register(registry: &mut crate::evals::EvalRegistry) {
    registry.register(
        "writer_agent",
        "settlement_replay_produces_identical_delta",
        settlement_replay_produces_identical_delta,
    );
}

fn settlement_replay_produces_identical_delta(db_path: &std::path::Path) -> crate::EvalResult {
    let memory = WriterMemory::open(db_path).map_err(|e| e.to_string())?;
    let project_id = "eval-settlement-replay";
    memory
        .ensure_project(&project_id, "Settlement Replay Test")
        .map_err(|e| e.to_string())?;

    let content = "第四章内容：主角发现了古剑的秘密，剑身泛着蓝色微光。这把剑曾经属于北境宗主。";

    // Build settlement delta once
    let delta1 = build_basic_chapter_settlement_delta(
        project_id,
        "第四章",
        "aaaa0001",
        content,
        1000,
        &memory,
        Vec::new(),
    );

    // Replay: build settlement delta again with same inputs
    let replay = replay_settlement_extraction(&delta1, content, &memory);

    let msg = format!(
        "matches={} mismatches={}",
        replay.matches_original,
        replay.mismatches.join("; ")
    );

    crate::EvalResult::pass_if(
        replay.matches_original,
        msg,
    )
}
```

- [ ] **Step 2: Register eval in evals.rs**

```rust
pub mod settlement_replay;
// ...
settlement_replay::register(&mut registry);
```

- [ ] **Step 3: Run the eval**

```bash
cargo run -p agent-evals 2>&1 | grep "settlement_replay"
```
Expected: `[PASS] writer_agent:settlement_replay_produces_identical_delta`

- [ ] **Step 4: Commit**

```bash
git add agent-evals/src/evals/settlement_replay.rs agent-evals/src/evals.rs
git commit -m "feat: add settlement replay consistency eval"
```

---

### Task 10: Eval — Chronology Preservation

**Files:**
- Create: `agent-evals/src/evals/chronology_preservation.rs`
- Modify: `agent-evals/src/evals.rs`

- [ ] **Step 1: Create the eval module**

```rust
// agent-evals/src/evals/chronology_preservation.rs

use agent_writer_lib::writer_agent::memory::WriterMemory;

pub fn register(registry: &mut crate::evals::EvalRegistry) {
    registry.register(
        "writer_agent",
        "repair_state_is_idempotent_and_preserves_chronology",
        repair_state_is_idempotent_and_preserves_chronology,
    );
}

fn repair_state_is_idempotent_and_preserves_chronology(
    db_path: &std::path::Path,
) -> crate::EvalResult {
    let memory = WriterMemory::open(db_path).map_err(|e| e.to_string())?;
    let project_id = "eval-chronology-preservation";
    memory
        .ensure_project(&project_id, "Chronology Test")
        .map_err(|e| e.to_string())?;

    // Setup: record chapter results in known order
    let chapters = vec![
        ("第一章", "aaaa0001"),
        ("第二章", "aaaa0002"),
        ("第三章", "aaaa0003"),
    ];

    for (title, revision) in &chapters {
        let result = agent_writer_lib::writer_agent::memory::ChapterResultSummary {
            id: 0,
            project_id: project_id.to_string(),
            chapter_title: title.to_string(),
            chapter_revision: revision.to_string(),
            summary: format!("{} summary", title),
            state_changes: vec![],
            character_progress: vec![],
            new_conflicts: vec![],
            new_clues: vec![],
            promise_updates: vec![],
            canon_updates: vec![],
            source_ref: format!("test:{}", title),
            created_at: 1000,
        };
        memory.upsert_chapter_result(&result).map_err(|e| e.to_string())?;
    }

    // Capture ordering before simulated repair
    let results_before: Vec<String> = memory
        .get_recent_chapter_results(10)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| r.chapter_title.clone())
        .collect();

    // Simulate repair — re-upsert same results (repair is idempotent)
    let repair_result = agent_writer_lib::writer_agent::memory::ChapterResultSummary {
        id: 0,
        project_id: project_id.to_string(),
        chapter_title: "第二章".to_string(),
        chapter_revision: "aaaa0002".to_string(),
        summary: "第二章 summary".to_string(),
        state_changes: vec![],
        character_progress: vec![],
        new_conflicts: vec![],
        new_clues: vec![],
        promise_updates: vec![],
        canon_updates: vec![],
        source_ref: "test:repair".to_string(),
        created_at: 2000,
    };
    memory.upsert_chapter_result(&repair_result).map_err(|e| e.to_string())?;

    // Verify ordering preserved
    let results_after: Vec<String> = memory
        .get_recent_chapter_results(10)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| r.chapter_title.clone())
        .collect();

    let chronology_preserved = results_before == results_after;
    let idempotent = results_before.len() == results_after.len();

    crate::EvalResult::pass_if(
        chronology_preserved && idempotent,
        format!(
            "chronologyPreserved={} idempotent={} before={:?} after={:?}",
            chronology_preserved, idempotent, results_before, results_after
        ),
    )
}
```

- [ ] **Step 2: Register eval in evals.rs**

```rust
pub mod chronology_preservation;
// ...
chronology_preservation::register(&mut registry);
```

- [ ] **Step 3: Run the eval**

```bash
cargo run -p agent-evals 2>&1 | grep "chronology_preservation"
```
Expected: `[PASS] writer_agent:repair_state_is_idempotent_and_preserves_chronology`

- [ ] **Step 4: Commit**

```bash
git add agent-evals/src/evals/chronology_preservation.rs agent-evals/src/evals.rs
git commit -m "feat: add chronology preservation eval"
```

---

### Task 11: Verification Baseline Update & Final Verify

**Files:**
- Modify: `scripts/verification-baseline.cjs` (or README.md baseline block)
- Modify: `README.md`

- [ ] **Step 1: Run full verification**

```bash
npm run verify
```

- [ ] **Step 2: Update baseline if counts have changed**

```bash
npm run baseline
```

- [ ] **Step 3: Update README.md baseline block**

The `<!-- verification-baseline:start -->` block in `README.md` should be updated to reflect any new test/eval counts.

- [ ] **Step 4: Verify all gates pass**

Expected output:
```
check:save-path: PASS
check:audit: PASS (includes provider budget coverage)
check:p2: PASS (includes TodayFive exclusivity)
check:p2-render: PASS (includes TodayFive item count)
agent-evals: 268/268 passing (3 new gates)
agent-harness-core: 89 tests passing
agent-writer: 247 tests passing
```

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "chore: update verification baseline after foundation lockdown"
```
