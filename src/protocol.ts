export const Commands = {
  analyzeChapter: "analyze_chapter",
  analyzePacing: "analyze_pacing",
  askAgent: "ask_agent",
  askProjectBrain: "ask_project_brain",
  batchGenerateChapter: "batch_generate_chapter",
  checkApiKey: "check_api_key",
  createChapter: "create_chapter",
  deleteLoreEntry: "delete_lore_entry",
  deleteOutlineNode: "delete_outline_node",
  exportDiagnosticLogs: "export_diagnostic_logs",
  getApiKey: "get_api_key",
  getLorebook: "get_lorebook",
  getOutline: "get_outline",
  getProjectGraphData: "get_project_graph_data",
  harnessEcho: "harness_echo",
  loadChapter: "load_chapter",
  readProjectDir: "read_project_dir",
  reorderOutlineNodes: "reorder_outline_nodes",
  renameChapterFile: "rename_chapter_file",
  saveChapter: "save_chapter",
  saveLoreEntry: "save_lore_entry",
  saveOutlineNode: "save_outline_node",
  setApiKey: "set_api_key",
} as const;

export const Events = {
  agentChainOfThought: "agent-chain-of-thought",
  agentEpiphany: "agent-epiphany",
  agentError: "agent-error",
  agentSearchStatus: "agent-search-status",
  agentStreamChunk: "agent-stream-chunk",
  agentStreamEnd: "agent-stream-end",
  batchStatus: "batch-status",
} as const;

export interface StreamChunk {
  content: string;
}

export interface StreamEnd {
  reason: string;
}

export interface SearchStatus {
  keyword: string;
  round: number;
}

export interface AgentError {
  message: string;
  source: string;
}

export interface Epiphany {
  id: number;
  skill: string;
  category: string;
}

export interface ChainOfThoughtStep {
  step: number;
  total: number;
  description: string;
  status: string;
}

export interface ParsedAction {
  kind: "insert" | "replace";
  content: string;
}

export const ACTION_RE = /<ACTION_(INSERT|REPLACE)>(.*?)<\/ACTION_\1>/gs;

export function extractActions(buffer: string): {
  actions: ParsedAction[];
  cleanText: string;
} {
  const actions: ParsedAction[] = [];
  const cleanText = buffer.replace(ACTION_RE, (_, kind: string, content: string) => {
    actions.push({ kind: kind.toLowerCase() as ParsedAction["kind"], content });
    return "";
  });
  return { actions, cleanText };
}
