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

const saveCallPattern = /\bsave_chapter\b/g;
const observePattern = /\bobserve_chapter_save_with_result\b/g;
const observeSimplePattern = /\bobserve_chapter_save\b/g;

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

const forbiddenInWritePatterns = [
  /getWriterAgentLedger/,
  /getWriterAgentStatus/,
  /getStoryDebtSnapshot/,
  /getWriterAgentPendingProposals/,
  /getWriterAgentInspectorTimeline/,
];
for (const pattern of forbiddenInWritePatterns) {
  if (pattern.test(allCompanionSource)) {
    const lines = allCompanionSource.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (pattern.test(lines[i])) {
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
