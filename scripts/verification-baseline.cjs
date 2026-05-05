const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const baselineItems = [
  ["cargo test -p agent-harness-core", "81 tests passing"],
  ["cargo test -p agent-writer", "198 tests passing"],
  ["cargo run -p agent-evals", "200/200 evals passing"],
  ["npm run check:p2", "18/18 checks passing"],
  ["npm run check:p2-render", "write-mode DOM guard passing"],
  ["npm run check:audit", "56 commands, 0 issues"],
  ["npm run check:architecture", "7/7 files within budget, eval root guard passing"],
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
const isWindows = process.platform === "win32";
const cargoCommand = isWindows ? "cargo.exe" : "cargo";
const nodeCommand = isWindows ? "node.exe" : "node";
const npmCommand = isWindows ? "cmd.exe" : "npm";

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

function commandExpectation(command) {
  const found = baselineItems.find(([itemCommand]) => itemCommand === command);
  if (!found) {
    throw new Error(`Missing baseline command: ${command}`);
  }
  return found[1];
}

function expectedCount(command, pattern) {
  const expectation = commandExpectation(command);
  const match = expectation.match(pattern);
  if (!match) {
    throw new Error(`Cannot parse baseline expectation for ${command}: ${expectation}`);
  }
  return Number(match[1]);
}

function runCaptured(command, args) {
  const result = spawnSync(command, args, {
    cwd: path.join(__dirname, ".."),
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status ?? "unknown"}\n${output}`,
    );
  }
  return output;
}

function runNpmCaptured(script) {
  if (isWindows) {
    return runCaptured(npmCommand, ["/d", "/c", `npm run ${script}`]);
  }
  return runCaptured(npmCommand, ["run", script]);
}

function countCargoTests(packageName) {
  const output = runCaptured(cargoCommand, ["test", "-p", packageName, "--", "--list"]);
  return output
    .split(/\r?\n/)
    .filter((line) => line.trim().endsWith(": test")).length;
}

function checkLiveBaseline() {
  const failures = [];

  const harnessExpected = expectedCount("cargo test -p agent-harness-core", /^(\d+) tests passing$/);
  const harnessActual = countCargoTests("agent-harness-core");
  if (harnessActual !== harnessExpected) {
    failures.push(
      `cargo test -p agent-harness-core: expected ${harnessExpected} tests, found ${harnessActual}`,
    );
  }

  const writerExpected = expectedCount("cargo test -p agent-writer", /^(\d+) tests passing$/);
  const writerActual = countCargoTests("agent-writer");
  if (writerActual !== writerExpected) {
    failures.push(
      `cargo test -p agent-writer: expected ${writerExpected} tests, found ${writerActual}`,
    );
  }

  const evalOutput = runCaptured(cargoCommand, ["run", "-p", "agent-evals"]);
  const evalExpected = commandExpectation("cargo run -p agent-evals").match(
    /^(\d+)\/(\d+) evals passing$/,
  );
  if (!evalExpected) {
    failures.push("cargo run -p agent-evals: cannot parse baseline expectation");
  } else {
    const totalMatch = evalOutput.match(/Total:\s+(\d+)\s+\|\s+Passed:\s+(\d+)\s+\|\s+Failed:\s+(\d+)/);
    if (!totalMatch) {
      failures.push("cargo run -p agent-evals: cannot parse eval output totals");
    } else {
      const [, total, passed, failed] = totalMatch.map(Number);
      if (
        total !== Number(evalExpected[2]) ||
        passed !== Number(evalExpected[1]) ||
        failed !== 0
      ) {
        failures.push(
          `cargo run -p agent-evals: expected ${evalExpected[1]}/${evalExpected[2]} passing, found ${passed}/${total} passing, failed ${failed}`,
        );
      }
    }
  }

  const auditOutput = runNpmCaptured("check:audit");
  const auditExpected = commandExpectation("npm run check:audit").match(/^(\d+) commands, (\d+) issues$/);
  const auditActual = auditOutput.match(/Command Boundary Audit:\s+(\d+) commands,\s+(\d+) issues/);
  if (!auditExpected || !auditActual) {
    failures.push("npm run check:audit: cannot parse baseline or command output");
  } else if (
    Number(auditActual[1]) !== Number(auditExpected[1]) ||
    Number(auditActual[2]) !== Number(auditExpected[2])
  ) {
    failures.push(
      `npm run check:audit: expected ${auditExpected[1]} commands, ${auditExpected[2]} issues; found ${auditActual[1]} commands, ${auditActual[2]} issues`,
    );
  }

  const architectureOutput = runNpmCaptured("check:architecture");
  const architectureExpected = commandExpectation("npm run check:architecture").match(
    /^(\d+)\/(\d+) files within budget(?:, eval root guard passing)?$/,
  );
  const architectureActual = architectureOutput.match(
    /Architecture size guard passed \((\d+)\/(\d+) files within budget\)/,
  );
  if (!architectureExpected || !architectureActual) {
    failures.push("npm run check:architecture: cannot parse baseline or command output");
  } else if (
    Number(architectureActual[1]) !== Number(architectureExpected[1]) ||
    Number(architectureActual[2]) !== Number(architectureExpected[2])
  ) {
    failures.push(
      `npm run check:architecture: expected ${architectureExpected[1]}/${architectureExpected[2]} files; found ${architectureActual[1]}/${architectureActual[2]}`,
    );
  }
  if (
    commandExpectation("npm run check:architecture").includes("eval root guard passing") &&
    !architectureOutput.includes("agent-evals/src: eval implementations are isolated under evals/.")
  ) {
    failures.push("npm run check:architecture: eval root guard did not run");
  }

  runCaptured(nodeCommand, ["scripts/clean-eval-reports.cjs"]);

  if (failures.length > 0) {
    console.error("Verification baseline live check failed:");
    for (const failure of failures) {
      console.error(`- ${failure}`);
    }
    process.exit(1);
  }

  console.log("Verification baseline live check passed.");
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
  checkLiveBaseline();
} else if (mode === "--check-live") {
  checkLiveBaseline();
} else {
  console.error(`Unknown mode: ${mode}`);
  process.exit(1);
}
