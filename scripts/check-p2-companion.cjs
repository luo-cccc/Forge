const fs = require("fs");
const path = require("path");

const componentPath = path.join(__dirname, "..", "src", "components", "CompanionPanel.tsx");
const appPath = path.join(__dirname, "..", "src", "App.tsx");
const inspectorPath = path.join(__dirname, "..", "src", "components", "WriterInspectorPanel.tsx");
const source = fs.readFileSync(componentPath, "utf8");
const appSource = fs.readFileSync(appPath, "utf8");
const inspectorSource = fs.readFileSync(inspectorPath, "utf8");

const checks = [
  {
    name: "write mode exposes only the quiet status surface",
    pass: source.includes('mode === "write"\n      ? (["status"] as const)'),
  },
  {
    name: "tabs are hidden when there is only one author-facing surface",
    pass: source.includes("{availableTabs.length > 1 && ("),
  },
  {
    name: "evidence trace is kept out of default writing mode",
    pass: source.includes('{mode !== "write" && (\n              <div className={`rounded border p-2 text-xs ${secondBrainToneClass(contextBudgetTone(trace))}`}>'),
  },
  {
    name: "project storage diagnostics are kept out of default writing mode",
    pass: source.includes('{mode !== "write" && storageDiagnostics && ('),
  },
  {
    name: "write-mode proposals do not expose rationale by default",
    pass: source.includes('{mode !== "write" && p.rationale && ('),
  },
  {
    name: "write-mode proposals do not expose evidence cards by default",
    pass: source.includes('{mode !== "write" && p.evidence.length > 0 && ('),
  },
  {
    name: "write-mode proposals do not expose operation internals by default",
    pass: source.includes('{mode !== "write" && primaryOperation(p) && ('),
  },
  {
    name: "guard detail avoids raw task-packet counters in author view",
    pass: !source.includes("${packet.task} guard is using"),
  },
  {
    name: "write-mode guard summarizes product metrics instead of raw traces",
    pass:
      source.includes("Recent acceptance") &&
      source.includes("productMetrics.proposalAcceptanceRate") &&
      !source.includes("operationLifecycle.map"),
  },
  {
    name: "internal timeline has a dedicated inspect mode",
    pass:
      appSource.includes('"inspect"') &&
      appSource.includes("<WriterInspectorPanel />") &&
      appSource.includes('storyMode === "inspect"'),
  },
  {
    name: "inspector uses the backend inspector timeline command",
    pass:
      inspectorSource.includes("Commands.getWriterAgentInspectorTimeline") &&
      inspectorSource.includes("WriterInspectorTimeline"),
  },
  {
    name: "inspector keeps failure and provider budget details out of companion tabs",
    pass:
      inspectorSource.includes('"failure"') &&
      inspectorSource.includes("Provider Budget") &&
      !source.includes("getWriterAgentInspectorTimeline"),
  },
  {
    name: "inspector owns save completed and save feedback latency details",
    pass:
      inspectorSource.includes('"save_completed"') &&
      inspectorSource.includes("writer.save_completed") &&
      inspectorSource.includes("averageSaveToFeedbackMs") &&
      !source.includes("writer.save_completed"),
  },
  {
    name: "inspector owns proposal context budget drilldown",
    pass:
      inspectorSource.includes("Proposal Context Budgets") &&
      inspectorSource.includes("sourceReports") &&
      inspectorSource.includes("contextBudgetToneClass") &&
      !source.includes("Proposal Context Budgets"),
  },
  {
    name: "inspector owns failure recovery action chips",
    pass:
      inspectorSource.includes("recoveryActionsForFailure") &&
      inspectorSource.includes("Review budget") &&
      inspectorSource.includes("Open failures") &&
      !source.includes("Review budget"),
  },
];

const failed = checks.filter((check) => !check.pass);
if (failed.length > 0) {
  console.error("P2 companion checks failed:");
  for (const check of failed) {
    console.error(`- ${check.name}`);
  }
  process.exit(1);
}

console.log(`P2 companion checks passed (${checks.length}/${checks.length}).`);
