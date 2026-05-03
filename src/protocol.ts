export const Commands = {
  abortEditorPrediction: "abort_editor_prediction",
  analyzeChapter: "analyze_chapter",
  analyzePacing: "analyze_pacing",
  agentObserve: "agent_observe",
  applyProposalFeedback: "apply_proposal_feedback",
  approveWriterOperation: "approve_writer_operation",
  recordWriterOperationDurableSave: "record_writer_operation_durable_save",
  askAgent: "ask_agent",
  generateParallelDrafts: "generate_parallel_drafts",
  askProjectBrain: "ask_project_brain",
  batchGenerateChapter: "batch_generate_chapter",
  checkApiKey: "check_api_key",
  createChapter: "create_chapter",
  deleteLoreEntry: "delete_lore_entry",
  deleteOutlineNode: "delete_outline_node",
  exportDiagnosticLogs: "export_diagnostic_logs",
  exportWriterAgentTrajectory: "export_writer_agent_trajectory",
  generateChapterAutonomous: "generate_chapter_autonomous",
  getAgentDomainProfile: "get_agent_domain_profile",
  getAgentKernelStatus: "get_agent_kernel_status",
  getProjectBrainKnowledgeGraph: "get_project_brain_knowledge_graph",
  getProjectStorageDiagnostics: "get_project_storage_diagnostics",
  getWriterAgentLedger: "get_writer_agent_ledger",
  getWriterAgentPendingProposals: "get_writer_agent_pending_proposals",
  getWriterAgentStatus: "get_writer_agent_status",
  getWriterAgentTrace: "get_writer_agent_trace",
  getWriterAgentInspectorTimeline: "get_writer_agent_inspector_timeline",
  getWriterAgentCompanionTimelineSummary: "get_writer_agent_companion_timeline_summary",
  getStoryReviewQueue: "get_story_review_queue",
  getStoryDebtSnapshot: "get_story_debt_snapshot",
  getAgentTools: "get_agent_tools",
  getEffectiveAgentToolInventory: "get_effective_agent_tool_inventory",
  getChapterRevision: "get_chapter_revision",
  getLorebook: "get_lorebook",
  getOutline: "get_outline",
  getProjectGraphData: "get_project_graph_data",
  loadChapter: "load_chapter",
  listFileBackups: "list_file_backups",
  readProjectDir: "read_project_dir",
  recordImplicitGhostRejection: "record_implicit_ghost_rejection",
  reportEditorState: "report_editor_state",
  reportSemanticLintState: "report_semantic_lint_state",
  reorderOutlineNodes: "reorder_outline_nodes",
  renameChapterFile: "rename_chapter_file",
  restoreFileBackup: "restore_file_backup",
  saveChapter: "save_chapter",
  saveLoreEntry: "save_lore_entry",
  saveOutlineNode: "save_outline_node",
  setApiKey: "set_api_key",
} as const;

export const Events = {
  agentChainOfThought: "agent-chain-of-thought",
  agentEpiphany: "agent-epiphany",
  agentError: "agent-error",
  agentProposal: "agent-proposal",
  agentSuggestion: "agent-suggestion",
  agentSearchStatus: "agent-search-status",
  agentStreamChunk: "agent-stream-chunk",
  agentStreamEnd: "agent-stream-end",
  batchStatus: "batch-status",
  chapterGeneration: "chapter-generation",
  editorGhostChunk: "editor-ghost-chunk",
  editorGhostEnd: "editor-ghost-end",
  editorSemanticLint: "editor-semantic-lint",
  editorEntityCard: "editor-entity-card",
  editorHoverHint: "editor-hover-hint",
  inlineWriterOperation: "inline-writer-operation",
  chapterRestored: "chapter-restored",
  projectFileRestored: "project-file-restored",
  storyboardMarker: "storyboard-marker",
} as const;

export interface ChapterRestored {
  title: string;
  revision: string;
}

export interface ProjectFileRestored {
  kind: "lorebook" | "outline" | "project_brain";
}

export interface StreamChunk {
  content: string;
}

export interface StreamEnd {
  reason: string;
}

export interface EditorStatePayload {
  requestId: string;
  prefix: string;
  suffix: string;
  cursorPosition: number;
  textCursorPosition?: number;
  paragraph: string;
  chapterTitle?: string;
  chapterRevision?: string;
  editorDirty?: boolean;
}

export interface SemanticLintPayload {
  requestId: string;
  paragraph: string;
  paragraphFrom: number;
  cursorPosition: number;
  chapterTitle?: string;
}

export interface EditorGhostChunk {
  requestId: string;
  proposalId?: string;
  operation?: WriterOperation;
  cursorPosition: number;
  content: string;
  intent?: string;
  candidates?: EditorGhostCandidate[];
  replace?: boolean;
}

export interface EditorGhostCandidate {
  id: string;
  label: string;
  text: string;
}

export interface EditorGhostEnd {
  requestId: string;
  cursorPosition: number;
  reason: "complete" | "cancelled" | "error" | string;
}

export interface EditorSemanticLint {
  requestId: string;
  cursorPosition: number;
  from: number;
  to: number;
  message: string;
  severity: "info" | "warning" | "error" | string;
}

export interface InlineWriterOperationEvent {
  requestId: string;
  proposal: AgentProposal;
  operation: WriterOperation;
}

export interface EditorEntityCard {
  keyword: string;
  content: string;
  chapter: string;
}

export interface EditorHoverHint {
  message: string;
  from: number;
  to: number;
}

export interface StoryboardMarker {
  chapter: string;
  message: string;
  level: string;
}

export interface ParallelDraft {
  id: string;
  label: string;
  text: string;
}

export interface ParallelDraftPayload {
  prefix: string;
  suffix: string;
  paragraph: string;
  selectedText: string;
  chapterTitle?: string;
}

export interface SearchStatus {
  keyword: string;
  round: number;
}

export interface AgentError {
  message: string;
  source: string;
  error?: {
    code?: string;
    message?: string;
    recoverable?: boolean;
    details?: {
      providerBudget?: WriterProviderBudgetReport;
      [key: string]: unknown;
    };
  };
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

export type StoryMode = "write" | "review" | "explore" | "inspect";

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
  description: string;
  inputType: string;
  outputType: string;
  sideEffectLevel: "none" | "read" | "provider_call" | "write" | "external";
  requiresApproval: boolean;
  timeoutMs: number;
  contextCostChars: number;
  tags: string[];
  stage: "observe" | "plan" | "context" | "execute" | "reflect";
  source: string;
  supportedIntents: Array<
    "chat" | "retrieve_knowledge" | "analyze_text" | "generate_content" | "execute_plan" | "linter"
  >;
  enabledByDefault: boolean;
  inputSchema?: unknown;
}

export type EffectiveToolStatus =
  | "allowed"
  | "disabled"
  | "intent_mismatch"
  | "side_effect_too_high"
  | "missing_tag"
  | "approval_required"
  | "permission_denied";

export interface EffectiveToolEntry {
  descriptor: AgentToolDescriptor;
  allowed: boolean;
  status: EffectiveToolStatus;
  reason?: string;
}

export interface EffectiveToolInventory {
  allowed: AgentToolDescriptor[];
  blocked: EffectiveToolEntry[];
}

export interface AgentKernelStatus {
  toolGeneration: number;
  toolCount: number;
  effectiveToolCount: number;
  blockedToolCount: number;
  modelCallableToolCount: number;
  approvalRequiredToolCount: number;
  writeToolCount: number;
  domainId: string;
  capabilityCount: number;
  qualityGateCount: number;
  traceEnabled: boolean;
}

export interface AgentDomainCapability {
  id: string;
  label: string;
  description: string;
  stage: AgentToolDescriptor["stage"];
  intents: AgentToolDescriptor["supportedIntents"];
  contextSources: string[];
  qualityChecks: string[];
  priority: number;
}

export interface AgentContextPriority {
  sourceType: string;
  priority: number;
  maxChars: number;
  required: boolean;
}

export interface AgentDomainProfile {
  id: string;
  name: string;
  description: string;
  capabilities: AgentDomainCapability[];
  contextPriorities: AgentContextPriority[];
  qualityGates: string[];
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
  providerBudgetApproval?: WriterProviderBudgetApproval;
}

export interface AskAgentContextPayload {
  chapterTitle?: string;
  chapterRevision?: string;
  cursorPosition?: number;
  dirty?: boolean;
  mode?: "chat" | "inline_operation" | string;
  requestId?: string;
  providerBudgetApproval?: WriterProviderBudgetApproval;
}

export interface AskProjectBrainPayload {
  query: string;
  providerBudgetApproval?: WriterProviderBudgetApproval;
}

export type WriterProviderBudgetTask =
  | "chapter_generation"
  | "batch_generation"
  | "project_brain_query"
  | "project_brain_rebuild"
  | "external_research"
  | "manual_request"
  | "ghost_preview"
  | string;

export type WriterProviderBudgetDecision =
  | "allowed"
  | "warn"
  | "approval_required"
  | "blocked"
  | string;

export interface WriterProviderBudgetReport {
  task: WriterProviderBudgetTask;
  model: string;
  estimatedInputTokens: number;
  requestedOutputTokens: number;
  estimatedTotalTokens: number;
  estimatedCostMicros: number;
  maxTotalTokensWithoutApproval: number;
  maxEstimatedCostMicrosWithoutApproval: number;
  decision: WriterProviderBudgetDecision;
  approvalRequired: boolean;
  reasons: string[];
  remediation: string[];
}

export interface WriterProviderBudgetApproval {
  task: WriterProviderBudgetTask;
  model: string;
  approvedTotalTokens: number;
  approvedCostMicros: number;
  approvedAtMs: number;
  source: string;
}

export interface WriterFailureEvidenceBundle {
  category: string;
  code: string;
  message: string;
  recoverable: boolean;
  taskId?: string | null;
  evidenceRefs: string[];
  details: unknown;
  remediation: string[];
  createdAtMs: number;
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
  evidence?: WriterFailureEvidenceBundle;
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

// === Patch Review System (OpenCode apply_patch pattern) ===

export interface TextPatch {
  id: string;
  from: number;
  to: number;
  replacement: string;
  description: string;
  severity: "info" | "warning" | "suggestion";
  original: string;
}

export interface PatchSet {
  patches: TextPatch[];
  requestId: string;
  baseText: string;
  createdAt: number;
}

export type PatchStatus = "pending" | "accepted" | "rejected";

// === Agent Loop Events (matches Rust AgentLoopEvent) ===

export interface AgentLoopEventPayload {
  kind: string;
  intent?: string;
  tool?: string;
  args?: unknown;
  result?: {
    tool_name: string;
    error: string | null;
    duration_ms: number;
  };
  content?: string;
  message?: string;
  round?: number;
  before_tokens?: number;
  after_tokens?: number;
  compacted_count?: number;
  rounds?: number;
  tool_calls?: number;
  tokens_used?: number;
}

// === Editor Events for Ambient Agents ===

export interface EditorEventPayload {
  kind: string;
  idle_ms?: number;
  chapter?: string;
  paragraph?: string;
  cursor_position?: number;
  from?: number;
  to?: number;
  text?: string;
  content_length?: number;
  revision?: string;
  keywords?: string[];
  change_summary?: string;
  full_text_snippet?: string;
}


// === Writer Agent Protocol (replaces XML action tags) ===

export interface WriterObservation {
  id: string;
  createdAt: number;
  source: "editor" | "outline" | "lorebook" | "chapter_save" | "manual_request";
  reason: "typed" | "idle" | "selection" | "chapter_switch" | "save" | "explicit";
  projectId: string;
  chapterTitle?: string;
  chapterRevision?: string;
  cursor?: { from: number; to: number };
  selection?: { from: number; to: number; text: string };
  prefix: string;
  suffix: string;
  paragraph: string;
  fullTextDigest?: string;
  editorDirty: boolean;
}

export interface AgentProposal {
  id: string;
  observationId: string;
  kind: "ghost" | "parallel_draft" | "continuity_warning" | "canon_update"
    | "style_note" | "plot_promise" | "chapter_structure" | "story_contract" | "chapter_mission" | "question";
  priority: "ambient" | "normal" | "urgent";
  target?: { from: number; to: number };
  preview: string;
  operations: WriterOperation[];
  rationale: string;
  evidence: EvidenceRef[];
  risks: string[];
  alternatives: ProposalAlternative[];
  confidence: number;
  expiresAt?: number;
}

export interface ProposalAlternative {
  id: string;
  label: string;
  preview: string;
  operation?: WriterOperation;
  rationale: string;
}

export interface EvidenceRef {
  source: "lorebook" | "outline" | "chapter_text" | "canon" | "style_ledger" | "promise_ledger" | "story_contract" | "chapter_mission" | "author_feedback";
  reference: string;
  snippet: string;
}

export interface StoryReviewQueueEntry {
  id: string;
  proposalId: string;
  category: AgentProposal["kind"];
  severity: "info" | "warning" | "error";
  title: string;
  message: string;
  target?: { from: number; to: number };
  evidence: EvidenceRef[];
  operations: WriterOperation[];
  status: "pending" | "accepted" | "ignored" | "snoozed" | "expired";
  createdAt: number;
  expiresAt?: number;
}

export interface StoryDebtSnapshot {
  chapterTitle?: string;
  total: number;
  openCount: number;
  contractCount: number;
  missionCount: number;
  canonRiskCount: number;
  promiseCount: number;
  pacingCount: number;
  entries: StoryDebtEntry[];
}

export interface StoryDebtEntry {
  id: string;
  chapterTitle?: string;
  category: "story_contract" | "chapter_mission" | "canon_risk" | "timeline_risk" | "promise" | "pacing" | "memory" | "question";
  severity: "info" | "warning" | "error";
  status: "open" | "snoozed" | "stale";
  title: string;
  message: string;
  evidence: EvidenceRef[];
  relatedReviewIds: string[];
  operations: WriterOperation[];
  createdAt: number;
}

export type WriterOperation =
  | { kind: "text.insert"; chapter: string; at: number; text: string; revision: string }
  | { kind: "text.replace"; chapter: string; from: number; to: number; text: string; revision: string }
  | { kind: "text.annotate"; chapter: string; from: number; to: number; message: string; severity: string }
  | { kind: "canon.upsert_entity"; entity: unknown }
  | { kind: "canon.update_attribute"; entity: string; attribute: string; value: string; confidence: number }
  | { kind: "canon.upsert_rule"; rule: unknown }
  | { kind: "promise.add"; promise: unknown }
  | { kind: "promise.resolve"; promiseId: string; chapter: string }
  | { kind: "promise.defer"; promiseId: string; chapter: string; expectedPayoff: string }
  | { kind: "promise.abandon"; promiseId: string; chapter: string; reason: string }
  | { kind: "style.update_preference"; key: string; value: string }
  | { kind: "story_contract.upsert"; contract: StoryContractOperationInput }
  | { kind: "chapter_mission.upsert"; mission: ChapterMissionOperationInput }
  | { kind: "outline.update"; nodeId: string; patch: unknown };

export interface OperationError {
  code: string;
  message: string;
}

export interface OperationResult {
  success: boolean;
  operation: WriterOperation;
  error?: OperationError;
  revisionAfter?: string;
}

export interface OperationApproval {
  source: string;
  actor: string;
  reason: string;
  proposalId?: string;
  surfacedToUser: boolean;
  createdAt: number;
}

export interface RecordOperationDurableSavePayload {
  proposalId?: string;
  operation: WriterOperation;
  saveResult: string;
  savedContent?: string;
  chapterTitle?: string;
  chapterRevision?: string;
}

export interface ProposalFeedback {
  proposalId: string;
  action: "accepted" | "rejected" | "edited" | "snoozed" | "explained";
  finalText?: string;
  reason?: string;
  createdAt: number;
}

export interface WriterAgentStatus {
  projectId: string;
  sessionId: string;
  activeChapter: string | null;
  observationCount: number;
  proposalCount: number;
  openPromiseCount: number;
  pendingProposals: number;
  totalFeedbackEvents: number;
}

export interface ProjectStorageDiagnostics {
  projectId: string;
  projectName: string;
  appDataDir: string;
  projectDataDir: string;
  checkedAt: number;
  healthy: boolean;
  files: StorageFileDiagnostic[];
  databases: SqliteDatabaseDiagnostic[];
}

export type BackupTarget =
  | { kind: "lorebook" }
  | { kind: "outline" }
  | { kind: "project_brain" }
  | { kind: "chapter"; title: string };

export interface FileBackupInfo {
  id: string;
  filename: string;
  path: string;
  bytes: number;
  modifiedAt: number;
}

export interface StorageFileDiagnostic {
  label: string;
  path: string;
  exists: boolean;
  bytes?: number;
  recordCount?: number;
  backupCount: number;
  status: string;
  error?: string;
}

export interface SqliteDatabaseDiagnostic {
  label: string;
  path: string;
  exists: boolean;
  bytes?: number;
  userVersion?: number;
  quickCheck?: string;
  tableCounts: SqliteTableCount[];
  status: string;
  error?: string;
}

export interface SqliteTableCount {
  table: string;
  rows: number;
}

export interface CanonEntitySummary {
  kind: string;
  name: string;
  summary: string;
  attributes: Record<string, unknown>;
  confidence: number;
}

export interface CanonRuleSummary {
  rule: string;
  category: string;
  priority: number;
  status: string;
}

export interface PlotPromiseSummary {
  id: number;
  kind: string;
  title: string;
  description: string;
  introducedChapter: string;
  lastSeenChapter: string;
  lastSeenRef: string;
  expectedPayoff: string;
  priority: number;
}

export interface CreativeDecisionSummary {
  scope: string;
  title: string;
  decision: string;
  rationale: string;
  createdAt: string;
}

export interface StoryContractSummary {
  projectId: string;
  title: string;
  genre: string;
  targetReader: string;
  readerPromise: string;
  first30ChapterPromise: string;
  mainConflict: string;
  structuralBoundary: string;
  toneContract: string;
  updatedAt: string;
}

export interface StoryContractOperationInput {
  projectId: string;
  title: string;
  genre: string;
  targetReader: string;
  readerPromise: string;
  first30ChapterPromise: string;
  mainConflict: string;
  structuralBoundary: string;
  toneContract: string;
}

export interface ChapterMissionSummary {
  id: number;
  projectId: string;
  chapterTitle: string;
  mission: string;
  mustInclude: string;
  mustNot: string;
  expectedEnding: string;
  status: string;
  sourceRef: string;
  updatedAt: string;
}

export interface ChapterMissionOperationInput {
  projectId: string;
  chapterTitle: string;
  mission: string;
  mustInclude: string;
  mustNot: string;
  expectedEnding: string;
  status: string;
  sourceRef: string;
}

export interface ChapterResultSummary {
  id: number;
  projectId: string;
  chapterTitle: string;
  chapterRevision: string;
  summary: string;
  stateChanges: string[];
  characterProgress: string[];
  newConflicts: string[];
  newClues: string[];
  promiseUpdates: string[];
  canonUpdates: string[];
  sourceRef: string;
  createdAt: number;
}

export interface NextBeatSummary {
  chapterTitle: string;
  goal: string;
  carryovers: string[];
  blockers: string[];
  sourceRefs: string[];
}

export interface MemoryAuditEntry {
  proposalId: string;
  kind: string;
  action: string;
  title: string;
  evidence: string;
  rationale: string;
  reason?: string;
  createdAt: number;
}

export interface WriterAgentLedgerSnapshot {
  storyContract?: StoryContractSummary | null;
  activeChapterMission?: ChapterMissionSummary | null;
  chapterMissions: ChapterMissionSummary[];
  recentChapterResults: ChapterResultSummary[];
  nextBeat?: NextBeatSummary | null;
  canonEntities: CanonEntitySummary[];
  canonRules: CanonRuleSummary[];
  openPromises: PlotPromiseSummary[];
  recentDecisions: CreativeDecisionSummary[];
  memoryAudit: MemoryAuditEntry[];
  contextRecalls: ContextRecallSummary[];
}

export interface WriterAgentTraceSnapshot {
  recentObservations: WriterObservationTrace[];
  taskPackets: WriterTaskPacketTrace[];
  recentProposals: WriterProposalTrace[];
  recentFeedback: WriterFeedbackTrace[];
  operationLifecycle: WriterOperationLifecycleTrace[];
  runEvents: WriterRunEvent[];
  postWriteDiagnostics: WriterPostWriteDiagnosticReport[];
  contextSourceTrends: WriterContextSourceTrend[];
  contextRecalls: ContextRecallSummary[];
  productMetrics: WriterProductMetrics;
  productMetricsTrend: WriterProductMetricsTrend;
}

export interface WriterPostWriteDiagnosticReport {
  observationId: string;
  chapterTitle?: string | null;
  chapterRevision?: string | null;
  totalCount: number;
  errorCount: number;
  warningCount: number;
  infoCount: number;
  categories: WriterPostWriteDiagnosticCategoryCount[];
  diagnostics: WriterPostWriteDiagnosticItem[];
  sourceRefs: string[];
  remediation: string[];
  createdAtMs: number;
}

export interface WriterPostWriteDiagnosticCategoryCount {
  category: string;
  count: number;
}

export interface WriterPostWriteDiagnosticItem {
  diagnosticId: string;
  severity: "Error" | "Warning" | "Info" | string;
  category: string;
  message: string;
  target: { from: number; to: number };
  evidenceRefs: string[];
  fixSuggestion?: string | null;
  operationCount: number;
}

export interface WriterProductMetrics {
  proposalCount: number;
  feedbackCount: number;
  acceptedCount: number;
  rejectedCount: number;
  editedCount: number;
  snoozedCount: number;
  explainedCount: number;
  ignoredCount: number;
  positiveFeedbackCount: number;
  negativeFeedbackCount: number;
  proposalAcceptanceRate: number;
  ignoredRepeatedSuggestionRate: number;
  manualAskConvertedToOperationRate: number;
  promiseRecallHitRate: number;
  canonFalsePositiveRate: number;
  chapterMissionCompletionRate: number;
  durableSaveSuccessRate: number;
  averageSaveToFeedbackMs?: number | null;
}

export interface WriterProductMetricsTrend {
  sourceEventCount: number;
  sessionCount: number;
  overallAverageSaveToFeedbackMs?: number | null;
  recentAverageSaveToFeedbackMs?: number | null;
  previousAverageSaveToFeedbackMs?: number | null;
  saveToFeedbackDeltaMs?: number | null;
  recentSessions: WriterProductMetricSessionTrend[];
}

export interface WriterProductMetricSessionTrend {
  sessionId: string;
  firstEventAt: number;
  lastEventAt: number;
  eventCount: number;
  proposalCount: number;
  feedbackCount: number;
  acceptedCount: number;
  rejectedCount: number;
  editedCount: number;
  ignoredCount: number;
  proposalAcceptanceRate: number;
  durableSaveSuccessRate: number;
  averageSaveToFeedbackMs?: number | null;
  saveFeedbackSampleCount: number;
}

export interface WriterOperationLifecycleTrace {
  proposalId?: string | null;
  operationKind: string;
  sourceTask?: string | null;
  approvalSource?: string | null;
  affectedScope?: string | null;
  state: "proposed" | "approved" | "applied" | "durably_saved" | "feedback_recorded" | "rejected" | string;
  saveResult?: string | null;
  feedbackResult?: string | null;
  createdAt: number;
}

export interface WriterRunEvent {
  seq: number;
  tsMs: number;
  projectId: string;
  sessionId: string;
  taskId?: string | null;
  eventType: string;
  sourceRefs: string[];
  data: unknown;
}

export type WriterTimelineAudience = "companion" | "inspector";

export type WriterTimelineEventKind =
  | "observation"
  | "task_packet"
  | "proposal"
  | "feedback"
  | "operation_lifecycle"
  | "run_event"
  | "failure"
  | "context_recall"
  | "product_metrics";

export interface WriterTimelineEvent {
  audience: WriterTimelineAudience;
  kind: WriterTimelineEventKind;
  label: string;
  tsMs: number;
  taskId?: string | null;
  sourceRefs: string[];
  summary: string;
  detail?: unknown;
}

export interface WriterInspectorTimeline {
  audience: WriterTimelineAudience;
  includesInternalTrace: boolean;
  events: WriterTimelineEvent[];
}

export interface WriterTaskPacketTrace {
  id: string;
  observationId: string;
  task: string;
  objective: string;
  scope: string;
  intent?: string;
  requiredContextCount: number;
  beliefCount: number;
  successCriteriaCount: number;
  maxSideEffectLevel: string;
  feedbackCheckpointCount: number;
  foundationComplete: boolean;
  packet: TaskPacket;
}

export interface TaskPacket {
  id: string;
  objective: string;
  scope: string;
  scopeRef?: string;
  intent?: "chat" | "retrieve_knowledge" | "analyze_text" | "generate_content" | "execute_plan" | "linter";
  constraints: string[];
  successCriteria: string[];
  beliefs: TaskBelief[];
  requiredContext: RequiredContext[];
  toolPolicy: ToolPolicyContract;
  feedback: FeedbackContract;
  createdAtMs: number;
}

export interface TaskBelief {
  subject: string;
  statement: string;
  confidence: number;
  source?: string;
}

export interface RequiredContext {
  sourceType: string;
  purpose: string;
  maxChars: number;
  required: boolean;
}

export interface ToolPolicyContract {
  maxSideEffectLevel: AgentToolDescriptor["sideEffectLevel"];
  allowApprovalRequired: boolean;
  requiredToolTags: string[];
}

export interface FeedbackContract {
  expectedSignals: string[];
  checkpoints: string[];
  memoryWrites: string[];
}

export interface WriterObservationTrace {
  id: string;
  createdAt: number;
  reason: string;
  chapterTitle?: string;
  paragraphSnippet: string;
}

export interface WriterProposalTrace {
  id: string;
  observationId: string;
  kind: string;
  priority: string;
  state: string;
  confidence: number;
  previewSnippet: string;
  evidence: EvidenceRef[];
  contextBudget?: ContextBudgetTrace;
}

export interface ContextBudgetTrace {
  task: string;
  used: number;
  totalBudget: number;
  wasted: number;
  sourceReports: ContextSourceBudgetTrace[];
}

export interface ContextSourceBudgetTrace {
  source: string;
  requested: number;
  provided: number;
  truncated: boolean;
  reason: string;
  truncationReason?: string;
}

export interface WriterContextSourceTrend {
  source: string;
  appearances: number;
  providedCount: number;
  truncatedCount: number;
  droppedCount: number;
  totalRequested: number;
  totalProvided: number;
  averageProvided: number;
  lastReason?: string;
  lastTruncationReason?: string;
}

export interface WriterFeedbackTrace {
  proposalId: string;
  action: string;
  reason?: string;
  createdAt: number;
}

export interface ContextRecallSummary {
  source: string;
  reference: string;
  snippet: string;
  recallCount: number;
  firstRecalledAt: number;
  lastRecalledAt: number;
  lastObservationId: string;
  lastProposalId: string;
}

export const WriterAgentCommands = {
  getWriterAgentStatus: "get_writer_agent_status",
  getWriterAgentLedger: "get_writer_agent_ledger",
  getStoryReviewQueue: "get_story_review_queue",
  getStoryDebtSnapshot: "get_story_debt_snapshot",
  getWriterAgentInspectorTimeline: "get_writer_agent_inspector_timeline",
  getWriterAgentCompanionTimelineSummary: "get_writer_agent_companion_timeline_summary",
  agentObserve: "agent_observe",
  applyProposalFeedback: "apply_proposal_feedback",
  approveWriterOperation: "approve_writer_operation",
  recordWriterOperationDurableSave: "record_writer_operation_durable_save",
  recordImplicitGhostRejection: "record_implicit_ghost_rejection",
} as const;
