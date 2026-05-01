export const Commands = {
  analyzeChapter: "analyze_chapter",
  analyzePacing: "analyze_pacing",
  agentObserve: "agent_observe",
  askAgent: "ask_agent",
  askProjectBrain: "ask_project_brain",
  batchGenerateChapter: "batch_generate_chapter",
  checkApiKey: "check_api_key",
  createChapter: "create_chapter",
  deleteLoreEntry: "delete_lore_entry",
  deleteOutlineNode: "delete_outline_node",
  exportDiagnosticLogs: "export_diagnostic_logs",
  generateChapterAutonomous: "generate_chapter_autonomous",
  getApiKey: "get_api_key",
  getAgentTools: "get_agent_tools",
  getChapterRevision: "get_chapter_revision",
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
  agentSuggestion: "agent-suggestion",
  agentSearchStatus: "agent-search-status",
  agentStreamChunk: "agent-stream-chunk",
  agentStreamEnd: "agent-stream-end",
  batchStatus: "batch-status",
  chapterGeneration: "chapter-generation",
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

export type AgentMode = "off" | "passive" | "proactive";

export type AgentObservationReason =
  | "user_typed"
  | "selection_change"
  | "chapter_switch"
  | "idle_tick";

export interface AgentTextRange {
  from: number;
  to: number;
}

export interface AgentSelection extends AgentTextRange {
  text: string;
}

export interface AgentObservation {
  id: string;
  mode: AgentMode;
  reason: AgentObservationReason;
  createdAt: number;
  chapterTitle?: string;
  chapterRevision?: string;
  dirty: boolean;
  cursorPosition: number;
  selection?: AgentSelection;
  currentParagraph: string;
  nearbyText: string;
  recentEditSummary: string;
  idleMs: number;
  snoozedUntil?: number;
  outlineChapterTitle?: string;
}

export type AgentSuggestionKind =
  | "continue"
  | "revise"
  | "continuity"
  | "lore"
  | "structure"
  | "question";

export type AgentSuggestionAction = "accept" | "reject" | "snooze" | "explain";

export interface AgentSourceSummary {
  sourceType: string;
  label: string;
  summary: string;
  originalChars: number;
  includedChars: number;
  truncated: boolean;
}

export interface AgentSuggestion {
  id: string;
  requestId: string;
  observationId: string;
  kind: AgentSuggestionKind;
  targetRange?: AgentTextRange;
  anchorPosition?: number;
  confidence: number;
  reason: string;
  sourceSummaries: AgentSourceSummary[];
  previewText: string;
  actions: AgentSuggestionAction[];
  createdAt: number;
}

export interface AgentObserveResult {
  requestId: string;
  observationId: string;
  decision: "noop" | "suggestion";
  reason: string;
  suggestionId?: string;
}

export interface AgentToolDescriptor {
  name: string;
  inputType: string;
  outputType: string;
  sideEffectLevel: "none" | "read" | "provider_call" | "write";
  requiresApproval: boolean;
  timeoutMs: number;
  contextCostChars: number;
}

export interface FrontendChapterStateSnapshot {
  openChapterTitle?: string;
  openChapterRevision?: string;
  dirty: boolean;
}

export interface ChapterContextBudget {
  totalChars?: number;
  instructionChars?: number;
  outlineChars?: number;
  previousChaptersChars?: number;
  nextChapterChars?: number;
  targetExistingChars?: number;
  lorebookChars?: number;
  userProfileChars?: number;
  ragChars?: number;
  previousChapterCount?: number;
  nextChapterCount?: number;
  lorebookEntryCount?: number;
  userProfileEntryCount?: number;
  ragChunkCount?: number;
}

export interface GenerateChapterAutonomousPayload {
  requestId?: string;
  targetChapterTitle?: string;
  targetChapterNumber?: number;
  userInstruction: string;
  budget?: ChapterContextBudget;
  frontendState?: FrontendChapterStateSnapshot;
  saveMode?: "create_if_missing" | "replace_if_clean" | "save_as_draft";
  chapterSummaryOverride?: string;
}

export interface ChapterContextSource {
  sourceType: string;
  id: string;
  label: string;
  originalChars: number;
  includedChars: number;
  truncated: boolean;
  score?: number;
}

export interface ChapterContextBudgetReport {
  maxChars: number;
  includedChars: number;
  sourceCount: number;
  truncatedSourceCount: number;
  warnings: string[];
}

export interface ChapterGenerationError {
  code: string;
  message: string;
  recoverable: boolean;
  details?: string;
}

export interface ChapterGenerationConflict {
  reason: string;
  baseRevision: string;
  currentRevision: string;
  openChapterTitle?: string;
  dirty: boolean;
  draftTitle?: string;
}

export interface SavedGeneratedChapter {
  chapterTitle: string;
  newRevision: string;
  savedMode: "created" | "replaced" | "draft_copy" | string;
}

export type ChapterGenerationPhase =
  | "chapter_generation_started"
  | "chapter_generation_context_built"
  | "chapter_generation_progress"
  | "chapter_generation_conflict"
  | "chapter_generation_completed"
  | "chapter_generation_failed";

export interface ChapterGenerationEvent {
  requestId: string;
  phase: ChapterGenerationPhase;
  status: "running" | "done" | "conflict" | "error" | string;
  message: string;
  progress: number;
  targetChapterTitle?: string;
  sources?: ChapterContextSource[];
  budget?: ChapterContextBudgetReport;
  saved?: SavedGeneratedChapter;
  conflict?: ChapterGenerationConflict;
  error?: ChapterGenerationError;
  warnings: string[];
}

export interface ChapterGenerationStart {
  requestId: string;
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
