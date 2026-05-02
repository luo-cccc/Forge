const fs = require("fs");
const path = require("path");

const componentPath = path.join(__dirname, "..", "src", "components", "CompanionPanel.tsx");
const source = fs.readFileSync(componentPath, "utf8");

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
