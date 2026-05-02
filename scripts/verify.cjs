const { spawnSync } = require("child_process");

const isWindows = process.platform === "win32";
const steps = [
  ["npm", ["run", "lint"]],
  ["npm", ["run", "build"]],
  ["npm", ["run", "check:p2"]],
  [isWindows ? "cargo.exe" : "cargo", ["fmt", "--all", "--", "--check"]],
  [isWindows ? "cargo.exe" : "cargo", ["test", "-p", "agent-harness-core"]],
  [isWindows ? "cargo.exe" : "cargo", ["test", "-p", "agent-writer"]],
  [isWindows ? "cargo.exe" : "cargo", ["run", "-p", "agent-evals"]],
  [isWindows ? "node.exe" : "node", ["scripts/clean-eval-reports.cjs"]],
  [isWindows ? "git.exe" : "git", ["diff", "--check"]],
];

for (const [command, args] of steps) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  const result = run(command, args);

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function run(command, args) {
  if (isWindows && command === "npm") {
    return spawnSync("cmd.exe", ["/d", "/c", [command, ...args].join(" ")], {
      cwd: process.cwd(),
      stdio: "inherit",
    });
  }

  return spawnSync(command, args, {
    cwd: process.cwd(),
    stdio: "inherit",
  });
}
