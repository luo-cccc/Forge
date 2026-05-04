const fs = require("fs");
const path = require("path");

const baselineItems = [
  ["cargo test -p agent-harness-core", "80 tests passing"],
  ["cargo test -p agent-writer", "190 tests passing"],
  ["cargo run -p agent-evals", "156/156 evals passing"],
  ["npm run check:p2", "17/17 checks passing"],
  ["npm run check:audit", "52 commands, 0 issues"],
  ["npm run check:architecture", "5/5 files within budget"],
  ["npm run lint", "passing"],
  ["npm run build", "passing"],
  ["cargo fmt --all -- --check", "passing"],
  ["git diff --check", "passing"],
];

const docs = [
  path.join(__dirname, "..", "README.md"),
  path.join(__dirname, "..", "docs", "project-status.md"),
];

const startMarker = "<!-- verification-baseline:start -->";
const endMarker = "<!-- verification-baseline:end -->";

function renderBaseline() {
  return baselineItems
    .map(([command, expectation]) => `- \`${command}\`: ${expectation}`)
    .join("\n");
}

function replaceBlock(source, rendered) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker);
  if (start === -1 || end === -1 || end < start) {
    throw new Error(`Missing verification baseline markers`);
  }

  return [
    source.slice(0, start + startMarker.length),
    "\n",
    rendered,
    "\n",
    source.slice(end),
  ].join("");
}

function checkFile(file, rendered) {
  const source = fs.readFileSync(file, "utf8");
  const updated = replaceBlock(source, rendered);
  return source === updated;
}

function writeFile(file, rendered) {
  const source = fs.readFileSync(file, "utf8");
  const updated = replaceBlock(source, rendered);
  fs.writeFileSync(file, updated);
}

const mode = process.argv[2] ?? "--print";
const rendered = renderBaseline();

if (mode === "--print") {
  console.log(rendered);
} else if (mode === "--write") {
  for (const file of docs) {
    writeFile(file, rendered);
  }
  console.log(`Updated verification baseline in ${docs.length} docs.`);
} else if (mode === "--check") {
  const stale = docs.filter((file) => !checkFile(file, rendered));
  if (stale.length > 0) {
    console.error("Verification baseline docs are stale. Run `npm run baseline`.");
    for (const file of stale) {
      console.error(`- ${path.relative(path.join(__dirname, ".."), file)}`);
    }
    process.exit(1);
  }
  console.log(`Verification baseline docs are current (${docs.length}/${docs.length}).`);
} else {
  console.error(`Unknown mode: ${mode}`);
  process.exit(1);
}
