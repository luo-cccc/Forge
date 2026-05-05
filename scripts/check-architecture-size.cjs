const fs = require("fs");
const path = require("path");
const ts = require("typescript");

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
    label: "Companion panel proposal helpers",
    file: path.join("src", "components", "CompanionPanel.proposal.ts"),
    maxLines: 300,
    rationale: "Pure proposal / operation helpers extracted from CompanionPanel — no hooks, no JSX.",
  },
  {
    label: "Companion panel contract helpers",
    file: path.join("src", "components", "CompanionPanel.contract.ts"),
    maxLines: 200,
    rationale: "Story contract / chapter mission draft types and factories — no hooks, no JSX.",
  },
  {
    label: "Companion panel brain helpers",
    file: path.join("src", "components", "CompanionPanel.brain.ts"),
    maxLines: 500,
    rationale: "Second brain display helpers — no hooks, no JSX, pure display logic.",
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

checkCompanionHelpersBoundary();
checkEvalRootBoundary();

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

function checkCompanionHelpersBoundary() {
  const helperFiles = [
    path.join("src", "components", "CompanionPanel.proposal.ts"),
    path.join("src", "components", "CompanionPanel.contract.ts"),
    path.join("src", "components", "CompanionPanel.brain.ts"),
  ];

  for (const relativePath of helperFiles) {
    const absolutePath = path.join(repoRoot, relativePath);
    if (!fs.existsSync(absolutePath)) {
      failures.push(`${relativePath}: missing`);
      continue;
    }
    const source = fs.readFileSync(absolutePath, "utf8");
    const ast = ts.createSourceFile(absolutePath, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
    const boundaryFailures = [];

    walk(ast, (node) => {
      if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node) || ts.isJsxFragment(node)) {
        boundaryFailures.push("contains JSX");
        return;
      }

      if (
        ts.isImportDeclaration(node) &&
        ts.isStringLiteral(node.moduleSpecifier) &&
        node.moduleSpecifier.text === "react"
      ) {
        boundaryFailures.push("imports React");
      }

    if (ts.isCallExpression(node)) {
      const expression = node.expression;
      if (ts.isIdentifier(expression) && /^use[A-Z0-9]/.test(expression.text)) {
        boundaryFailures.push(`calls hook-like function ${expression.text}`);
      }
      if (ts.isPropertyAccessExpression(expression)) {
        const receiver = expression.expression.getText(ast);
        const method = expression.name.text;
        if (
          (receiver === "window" || receiver === "document" || receiver === "localStorage" || receiver === "sessionStorage") ||
          (receiver === "console" && method !== "warn") ||
          (receiver === "Date" && method === "now") ||
          method === "addEventListener" ||
          method === "removeEventListener" ||
          method === "dispatchEvent" ||
          method === "setItem" ||
          method === "removeItem" ||
          method === "clear"
        ) {
          boundaryFailures.push(`uses side-effect API ${receiver}.${method}`);
        }
      }
    }

    if (ts.isNewExpression(node)) {
      const expression = node.expression.getText(ast);
      if (expression === "Date") {
        boundaryFailures.push("constructs Date");
      }
    }
  });

    const unique = [...new Set(boundaryFailures)];
    if (unique.length > 0) {
      failures.push(`${relativePath}: helper boundary violated (${unique.join(", ")})`);
    } else {
      console.log(`ok   ${relativePath}: helper boundary has no hooks, JSX, or side-effect APIs.`);
    }
  }
}

function checkEvalRootBoundary() {
  const evalSrcDir = path.join(repoRoot, "agent-evals", "src");
  const allowedRootFiles = new Set(["evals.rs", "fixtures.rs", "main.rs"]);
  if (!fs.existsSync(evalSrcDir)) {
    failures.push("agent-evals/src: missing");
    return;
  }

  const rootRustFiles = fs
    .readdirSync(evalSrcDir)
    .filter((entry) => entry.endsWith(".rs"))
    .sort();
  const unexpected = rootRustFiles.filter((entry) => !allowedRootFiles.has(entry));
  if (unexpected.length > 0) {
    failures.push(
      `agent-evals/src: root eval implementation files must live under evals/ (${unexpected.join(", ")})`,
    );
  } else {
    console.log("ok   agent-evals/src: eval implementations are isolated under evals/.");
  }
}

function walk(node, visit) {
  visit(node);
  ts.forEachChild(node, (child) => walk(child, visit));
}
