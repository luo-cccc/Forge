const fs = require("fs");
const path = require("path");
const ts = require("typescript");

const componentPath = path.join(__dirname, "..", "src", "components", "CompanionPanel.tsx");
const helpersPath = path.join(__dirname, "..", "src", "components", "CompanionPanel.helpers.ts");
const appPath = path.join(__dirname, "..", "src", "App.tsx");
const inspectorPath = path.join(__dirname, "..", "src", "components", "WriterInspectorPanel.tsx");
const source = fs.readFileSync(componentPath, "utf8");
const helpersSource = fs.readFileSync(helpersPath, "utf8");
const mergedSource = source + "\n" + helpersSource;
const appSource = fs.readFileSync(appPath, "utf8");
const inspectorSource = fs.readFileSync(inspectorPath, "utf8");

const componentAst = ts.createSourceFile(componentPath, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const appAst = ts.createSourceFile(appPath, appSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const inspectorAst = ts.createSourceFile(inspectorPath, inspectorSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);

function normalize(text) {
  return text.replace(/\s+/g, " ");
}

function walk(node, visit) {
  visit(node);
  ts.forEachChild(node, (child) => walk(child, visit));
}

function hasVariableInitializer(ast, name, predicate) {
  let found = false;
  walk(ast, (node) => {
    if (found || !ts.isVariableDeclaration(node)) return;
    if (!ts.isIdentifier(node.name) || node.name.text !== name || !node.initializer) return;
    found = predicate(normalize(node.initializer.getText()));
  });
  return found;
}

function hasJsxTag(ast, tagName) {
  let found = false;
  walk(ast, (node) => {
    if (found) return;
    if (
      ts.isJsxSelfClosingElement(node) &&
      ts.isIdentifier(node.tagName) &&
      node.tagName.text === tagName
    ) {
      found = true;
    }
    if (
      ts.isJsxOpeningElement(node) &&
      ts.isIdentifier(node.tagName) &&
      node.tagName.text === tagName
    ) {
      found = true;
    }
  });
  return found;
}

function ancestorHasGuard(node, guardPattern) {
  let current = node.parent;
  while (current) {
    if (
      ts.isJsxExpression(current) ||
      ts.isBinaryExpression(current) ||
      ts.isConditionalExpression(current) ||
      ts.isParenthesizedExpression(current)
    ) {
      if (guardPattern.test(normalize(current.getText()))) return true;
    }
    if (ts.isFunctionLike(current)) return false;
    current = current.parent;
  }
  return false;
}

function hasGuardedNeedle(ast, needlePattern, guardPattern) {
  let found = false;
  walk(ast, (node) => {
    if (found) return;
    if (!needlePattern.test(normalize(node.getText()))) return;
    found = ancestorHasGuard(node, guardPattern);
  });
  return found;
}

function hasString(ast, pattern) {
  let found = false;
  walk(ast, (node) => {
    if (found) return;
    if (
      (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) &&
      pattern.test(node.text)
    ) {
      found = true;
    }
  });
  return found;
}

const modeNotWrite = /\bmode\s*!==\s*"write"/;

const checks = [
  {
    name: "write mode exposes only the quiet status surface",
    pass: hasVariableInitializer(
      componentAst,
      "availableTabs",
      (text) => /\bmode\s*===\s*"write"\s*\?\s*\(\s*\[\s*"status"\s*\]\s*as const\s*\)/.test(text),
    ),
  },
  {
    name: "tabs are hidden when there is only one author-facing surface",
    pass: /availableTabs\.length\s*>\s*1\s*&&/.test(source),
  },
  {
    name: "evidence trace is kept out of default writing mode",
    pass: hasGuardedNeedle(componentAst, /Evidence Trace/, modeNotWrite),
  },
  {
    name: "project storage diagnostics are kept out of default writing mode",
    pass: hasGuardedNeedle(componentAst, /Project Storage/, modeNotWrite),
  },
  {
    name: "write-mode proposals do not expose rationale by default",
    pass: hasGuardedNeedle(componentAst, /\bp\.rationale\b/, modeNotWrite),
  },
  {
    name: "write-mode proposals do not expose evidence cards by default",
    pass: hasGuardedNeedle(componentAst, /\bp\.evidence\.length\b/, modeNotWrite),
  },
  {
    name: "write-mode proposals do not expose operation internals by default",
    pass: hasGuardedNeedle(componentAst, /primaryOperation\s*\(\s*p\s*\)/, modeNotWrite),
  },
  {
    name: "guard detail avoids raw task-packet counters in author view",
    pass: !/\$\{packet\.task\}\s*guard is using/.test(source),
  },
  {
    name: "write-mode guard summarizes product metrics instead of raw traces",
    pass:
      /Recent acceptance/.test(mergedSource) &&
      /productMetrics\.proposalAcceptanceRate/.test(mergedSource) &&
      !/operationLifecycle\.map/.test(mergedSource),
  },
  {
    name: "internal timeline has a dedicated inspect mode",
    pass:
      hasString(appAst, /^inspect$/) &&
      hasJsxTag(appAst, "WriterInspectorPanel") &&
      /storyMode\s*===\s*"inspect"/.test(appSource),
  },
  {
    name: "inspector uses the backend inspector timeline command",
    pass:
      /Commands\.getWriterAgentInspectorTimeline/.test(inspectorSource) &&
      /WriterInspectorTimeline/.test(inspectorSource),
  },
  {
    name: "inspector keeps failure and provider budget details out of companion tabs",
    pass:
      hasString(inspectorAst, /^failure$/) &&
      /Provider Budget/.test(inspectorSource) &&
      !/getWriterAgentInspectorTimeline/.test(source),
  },
  {
    name: "inspector owns save completed and save feedback latency details",
    pass:
      hasString(inspectorAst, /^save_completed$/) &&
      /writer\.save_completed/.test(inspectorSource) &&
      /averageSaveToFeedbackMs/.test(inspectorSource) &&
      !/writer\.save_completed/.test(source),
  },
  {
    name: "inspector owns proposal context budget drilldown",
    pass:
      /Proposal Context Budgets/.test(inspectorSource) &&
      /sourceReports/.test(inspectorSource) &&
      /contextBudgetToneClass/.test(inspectorSource) &&
      !/Proposal Context Budgets/.test(source),
  },
  {
    name: "inspector owns failure recovery action chips",
    pass:
      /recoveryActionsForFailure/.test(inspectorSource) &&
      /Review budget/.test(inspectorSource) &&
      /Open failures/.test(inspectorSource) &&
      !/Review budget/.test(source),
  },
  {
    name: "inspector owns task receipt details",
    pass:
      hasString(inspectorAst, /^task_receipt$/) &&
      /Latest Receipt/.test(inspectorSource) &&
      /Open receipts/.test(inspectorSource) &&
      !/task_receipt/.test(source) &&
      !/Latest Receipt/.test(source),
  },
  {
    name: "inspector owns task artifact details",
    pass:
      hasString(inspectorAst, /^task_artifact$/) &&
      /Latest Artifact/.test(inspectorSource) &&
      /Open artifacts/.test(inspectorSource) &&
      !/task_artifact/.test(source) &&
      !/Latest Artifact/.test(source),
  },
  {
    name: "inspector owns metacognitive recovery action chips",
    pass:
      /metacognitiveRecoveryActions/.test(inspectorSource) &&
      /Open Review/.test(inspectorSource) &&
      /Inspect Diagnostics/.test(inspectorSource) &&
      /setStoryMode/.test(inspectorSource) &&
      !/Metacognitive Gate/.test(source) &&
      !/Inspect Diagnostics/.test(source),
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
