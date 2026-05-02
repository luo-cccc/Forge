const fs = require("fs");
const path = require("path");

const libRs = path.join(__dirname, "..", "src-tauri", "src", "lib.rs");
const source = fs.readFileSync(libRs, "utf8");

// Extract all #[tauri::command] functions
const commandPattern = /#\[tauri::command\]\s*\n(?:\/\/.*\n)*\s*(?:async\s+)?fn\s+(\w+)/g;
const commands = [];
let match;
while ((match = commandPattern.exec(source)) !== null) {
  commands.push(match[1]);
}

// Risk classification by function name and patterns in source
const RISK = {
  READ_ONLY: "read_only",
  CREDENTIAL: "credential",
  PROVIDER_CALL: "provider_call",
  MEMORY_WRITE: "memory_write",
  MANUSCRIPT_WRITE: "manuscript_write",
  DESTRUCTIVE: "destructive",
};

const classification = {
  // Manuscript write
  save_lore_entry: RISK.MANUSCRIPT_WRITE,

  // Read-only
  check_api_key: RISK.READ_ONLY,
  get_lorebook: RISK.READ_ONLY,
  read_project_dir: RISK.READ_ONLY,
  load_chapter: RISK.READ_ONLY,
  get_chapter_revision: RISK.READ_ONLY,
  get_outline: RISK.READ_ONLY,
  get_project_graph_data: RISK.READ_ONLY,
  get_agent_tools: RISK.READ_ONLY,
  get_effective_agent_tool_inventory: RISK.READ_ONLY,
  get_agent_kernel_status: RISK.READ_ONLY,
  get_agent_domain_profile: RISK.READ_ONLY,
  get_project_storage_diagnostics: RISK.READ_ONLY,
  list_file_backups: RISK.READ_ONLY,
  get_writer_agent_status: RISK.READ_ONLY,
  get_writer_agent_ledger: RISK.READ_ONLY,
  get_writer_agent_pending_proposals: RISK.READ_ONLY,
  get_story_review_queue: RISK.READ_ONLY,
  get_story_debt_snapshot: RISK.READ_ONLY,
  get_writer_agent_trace: RISK.READ_ONLY,
  abort_editor_prediction: RISK.READ_ONLY,

  // Credential
  set_api_key: RISK.CREDENTIAL,

  // Diagnostic/admin exports
  export_diagnostic_logs: RISK.READ_ONLY,
  export_writer_agent_trajectory: RISK.READ_ONLY,

  // Provider calls (LLM)
  report_editor_state: RISK.PROVIDER_CALL,
  report_semantic_lint_state: RISK.PROVIDER_CALL,
  batch_generate_chapter: RISK.PROVIDER_CALL,
  generate_chapter_autonomous: RISK.PROVIDER_CALL,
  analyze_chapter: RISK.PROVIDER_CALL,
  ask_project_brain: RISK.PROVIDER_CALL,
  generate_parallel_drafts: RISK.PROVIDER_CALL,
  analyze_pacing: RISK.PROVIDER_CALL,
  ask_agent: RISK.PROVIDER_CALL,

  // Memory write (WriterMemory SQLite)
  agent_observe: RISK.MEMORY_WRITE,
  apply_proposal_feedback: RISK.MEMORY_WRITE,
  record_implicit_ghost_rejection: RISK.MEMORY_WRITE,
  approve_writer_operation: RISK.MEMORY_WRITE,
  record_writer_operation_durable_save: RISK.MEMORY_WRITE,

  // Manuscript write
  create_chapter: RISK.MANUSCRIPT_WRITE,
  save_chapter: RISK.MANUSCRIPT_WRITE,
  save_outline_node: RISK.MANUSCRIPT_WRITE,
  update_outline_status: RISK.MANUSCRIPT_WRITE,
  reorder_outline_nodes: RISK.MANUSCRIPT_WRITE,

  // Destructive
  delete_lore_entry: RISK.DESTRUCTIVE,
  delete_outline_node: RISK.DESTRUCTIVE,
  rename_chapter_file: RISK.DESTRUCTIVE,
  restore_file_backup: RISK.DESTRUCTIVE,
};

// Audit checks: write/destructive commands must reference audit or operation functions
const WRITE_CLASSIFICATIONS = [RISK.MEMORY_WRITE, RISK.MANUSCRIPT_WRITE, RISK.DESTRUCTIVE];
const AUDIT_FUNCTIONS = [
  "audit_project_file_write",
  "audit_legacy",
  "record_operation_durable_save",
  "approve_editor_operation",
  "writer_audit",
  "record_kernel_audit",
  "record_writer_agent_audit",
];

function extractFunctionBody(name) {
  const fnPattern = new RegExp(
    `(?:async\\s+)?fn\\s+${name}\\b[^{]*\\{`,
    "m"
  );
  const fnMatch = fnPattern.exec(source);
  if (!fnMatch) return "";

  let braceCount = 0;
  let start = fnMatch.index + fnMatch[0].length;
  let inString = false;
  let stringChar = "";

  for (let i = start; i < source.length; i++) {
    const ch = source[i];
    if (inString) {
      if (ch === "\\") {
        i++; // skip escaped char
        continue;
      }
      if (ch === stringChar) inString = false;
      continue;
    }
    if (ch === '"' || ch === "`" || ch === "'") {
      inString = true;
      stringChar = ch;
      continue;
    }
    if (ch === "{") braceCount++;
    if (ch === "}") {
      if (braceCount === 0) return source.slice(start - 1, i + 1);
      braceCount--;
    }
  }
  return "";
}

// Commands that delegate to kernel for audit (kernel handles lifecycle internally)
const KERNEL_AUDIT_COMMANDS = [
  "apply_proposal_feedback",
  "record_implicit_ghost_rejection",
  "agent_observe",
];

const issues = [];
const report = [];

for (const cmd of commands) {
  const risk = classification[cmd] || RISK.READ_ONLY;
  const body = extractFunctionBody(cmd);
  const hasAudit = AUDIT_FUNCTIONS.some((fn) => body.includes(fn));

  const needsAudit = WRITE_CLASSIFICATIONS.includes(risk) && !KERNEL_AUDIT_COMMANDS.includes(cmd);
  const ok = !needsAudit || hasAudit;

  report.push({ command: cmd, risk, auditCovered: hasAudit, ok });

  if (!ok) {
    issues.push(
      `${cmd} (${risk}): no audit or operation reference found in command body`
    );
  }
}

// Verify that known legacy direct-write commands have audit coverage
const writeCommandsWithDirectWrites = [
  "save_chapter",
  "save_outline_node",
  "create_chapter",
  "delete_lore_entry",
  "delete_outline_node",
  "rename_chapter_file",
  "restore_file_backup",
];

for (const cmd of writeCommandsWithDirectWrites) {
  const body = extractFunctionBody(cmd);
  if (
    !body.includes("audit_project_file_write") &&
    !body.includes("audit_legacy") &&
    !body.includes("record_operation_durable_save") &&
    !body.includes("record_writer_agent_audit")
  ) {
    issues.push(
      `${cmd}: legacy direct write lacks audit trace in body`
    );
  }
}

// Remove false positives: kernel-delegating commands
for (const cmd of KERNEL_AUDIT_COMMANDS) {
  const idx = issues.findIndex((i) => i.startsWith(`${cmd} (`));
  if (idx >= 0) issues.splice(idx, 1);
}

// Output
console.log(
  `\nCommand Boundary Audit: ${commands.length} commands, ${
    issues.length
  } issues`
);
console.log("=".repeat(60));

const riskOrder = [
  RISK.DESTRUCTIVE,
  RISK.MANUSCRIPT_WRITE,
  RISK.MEMORY_WRITE,
  RISK.PROVIDER_CALL,
  RISK.CREDENTIAL,
  RISK.READ_ONLY,
];

for (const risk of riskOrder) {
  const cmds = report.filter((r) => r.risk === risk);
  if (!cmds.length) continue;
  console.log(`\n[${risk.toUpperCase()}] (${cmds.length})`);
  for (const c of cmds) {
    const flag = c.ok ? "OK" : "GAP";
    console.log(`  ${flag === "GAP" ? "!" : " "} ${c.command}`);
  }
}

if (issues.length > 0) {
  console.log("\n" + "=".repeat(60));
  console.log("COMMAND AUDIT GAPS:");
  for (const issue of issues) {
    console.log(`  - ${issue}`);
  }
  process.exit(1);
}

console.log(`\nCommand boundary audit passed.`);
