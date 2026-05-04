const fs = require("fs");
const path = require("path");

const repoRoot = path.join(__dirname, "..");

const budgets = [
  {
    label: "Tauri root glue",
    file: path.join("src-tauri", "src", "lib.rs"),
    maxLines: 220,
    rationale: "Root Tauri module should stay limited to module wiring, setup, and command registration.",
  },
  {
    label: "Writer kernel facade",
    file: path.join("src-tauri", "src", "writer_agent", "kernel.rs"),
    maxLines: 650,
    rationale: "Kernel facade should own state and public API while implementation lives in focused modules.",
  },
  {
    label: "Eval facade",
    file: path.join("agent-evals", "src", "evals.rs"),
    maxLines: 120,
    rationale: "Eval facade should only keep shared helpers and module exports.",
  },
  {
    label: "Companion panel surface",
    file: path.join("src", "components", "CompanionPanel.tsx"),
    maxLines: 2100,
    rationale: "Main companion panel should stay below its split budget after helper extraction.",
  },
  {
    label: "Companion panel helpers",
    file: path.join("src", "components", "CompanionPanel.helpers.ts"),
    maxLines: 350,
    rationale: "Extracted helpers should remain small enough to review directly.",
  },
];

const failures = [];

for (const budget of budgets) {
  const absolutePath = path.join(repoRoot, budget.file);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${budget.file}: missing`);
    continue;
  }

  const lineCount = countLines(fs.readFileSync(absolutePath, "utf8"));
  const status = lineCount <= budget.maxLines ? "ok" : "over";
  console.log(
    `${status.padEnd(4)} ${budget.file}: ${lineCount}/${budget.maxLines} lines - ${budget.rationale}`,
  );

  if (lineCount > budget.maxLines) {
    failures.push(`${budget.file}: ${lineCount}/${budget.maxLines} lines`);
  }
}

if (failures.length > 0) {
  console.error("\nArchitecture size guard failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(`Architecture size guard passed (${budgets.length}/${budgets.length} files within budget).`);

function countLines(source) {
  if (source.length === 0) {
    return 0;
  }

  const normalized = source.endsWith("\n") ? source.slice(0, -1) : source;
  return normalized.split(/\r?\n/).length;
}
