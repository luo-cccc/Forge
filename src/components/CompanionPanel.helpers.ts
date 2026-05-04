import type {
  AgentProposal,
  ChapterMissionSummary,
  OperationApproval,
  StoryContractSummary,
  StoryDebtEntry,
  StoryReviewQueueEntry,
  WriterOperation,
} from "../protocol";

export interface StoryContractDraft {
  title: string;
  genre: string;
  targetReader: string;
  readerPromise: string;
  first30ChapterPromise: string;
  mainConflict: string;
  structuralBoundary: string;
  toneContract: string;
}

export interface ChapterMissionDraft {
  mission: string;
  mustInclude: string;
  mustNot: string;
  expectedEnding: string;
  status: string;
  sourceRef: string;
  blockedReason: string;
  retiredHistory: string;
}

function memoryOperationSlot(operation: WriterOperation | undefined): string | null {
  if (operation?.kind === "canon.upsert_entity") {
    const entity = operation.entity as { kind?: unknown; name?: unknown };
    if (typeof entity.kind === "string" && typeof entity.name === "string") {
      return `memory|canon|${entity.kind}|${entity.name}`;
    }
  }
  if (operation?.kind === "promise.add") {
    const promise = operation.promise as { kind?: unknown; title?: unknown };
    if (typeof promise.kind === "string" && typeof promise.title === "string") {
      return `memory|promise|${promise.kind}|${promise.title}`;
    }
  }
  return null;
}

function proposalSlotKey(proposal: AgentProposal): string {
  const target = proposal.target ? `${proposal.target.from}:${proposal.target.to}` : "none";
  if (proposal.kind === "ghost") {
    return `${proposal.observationId}|${proposal.kind}|${target}`;
  }

  const memorySlot = memoryOperationSlot(primaryOperation(proposal));
  if (memorySlot) return memorySlot;

  const evidence = proposal.evidence[0];
  const evidenceKey = evidence ? `${evidence.source}:${evidence.reference}` : "";
  const previewKey = proposal.preview.replace(/\s+/g, " ").slice(0, 80);
  return `${proposal.observationId}|${proposal.kind}|${target}|${evidenceKey}|${previewKey}`;
}

export function isEnhancedGhost(proposal: AgentProposal): boolean {
  return proposal.kind === "ghost" && proposal.rationale.includes("LLM增强续写");
}

function priorityWeight(priority: AgentProposal["priority"]): number {
  if (priority === "urgent") return 2;
  if (priority === "normal") return 1;
  return 0;
}

function shouldReplaceProposal(existing: AgentProposal, incoming: AgentProposal): boolean {
  if (isEnhancedGhost(incoming) && !isEnhancedGhost(existing)) return true;
  if (priorityWeight(incoming.priority) > priorityWeight(existing.priority)) return true;
  return incoming.confidence > existing.confidence + 0.05;
}

export function mergeProposal(prev: AgentProposal[], incoming: AgentProposal): AgentProposal[] {
  const incomingSlot = proposalSlotKey(incoming);
  const existingIndex = prev.findIndex((proposal) => proposalSlotKey(proposal) === incomingSlot);
  if (existingIndex < 0) return [incoming, ...prev].slice(0, 20);

  const existing = prev[existingIndex];
  if (!shouldReplaceProposal(existing, incoming)) return prev;

  const next = prev.filter((_, index) => index !== existingIndex);
  return [incoming, ...next].slice(0, 20);
}

export function isEditorTextOperation(
  operation: WriterOperation,
): operation is Extract<WriterOperation, { kind: "text.insert" | "text.replace" }> {
  return operation.kind === "text.insert" || operation.kind === "text.replace";
}

export function primaryOperation(proposal: AgentProposal): WriterOperation | undefined {
  return proposal.alternatives?.[0]?.operation ?? proposal.operations[0];
}

export function queuePrimaryOperation(entry: StoryReviewQueueEntry): WriterOperation | undefined {
  return entry.operations[0];
}

export function debtPrimaryOperation(entry: StoryDebtEntry): WriterOperation | undefined {
  return entry.operations[0];
}

export function canonUpdateOperation(operations: WriterOperation[]): WriterOperation | undefined {
  return operations.find((operation) => operation.kind === "canon.update_attribute");
}

export function operationLabel(operation: WriterOperation): string {
  if (operation.kind === "promise.resolve") return "Resolve";
  if (operation.kind === "promise.defer") return "Defer";
  if (operation.kind === "promise.abandon") return "Abandon";
  if (operation.kind === "canon.update_attribute") return "Update Canon";
  if (operation.kind === "text.replace") return "Apply Fix";
  if (operation.kind === "text.insert") return "Insert";
  return "Apply";
}

export function operationApproval(
  source: string,
  reason: string,
  proposalId?: string,
): OperationApproval {
  return {
    source,
    actor: "author",
    reason,
    proposalId,
    surfacedToUser: true,
    createdAt: Date.now(),
  };
}

export function nextChapterLabel(chapter?: string | null): string {
  const match = chapter?.match(/(\d+)(?!.*\d)/);
  return match ? `Chapter-${Number(match[1]) + 1}` : "later chapter";
}

export function severityClass(severity: StoryReviewQueueEntry["severity"]): string {
  if (severity === "error") return "border-danger/40 bg-danger/10";
  if (severity === "warning") return "border-accent/30 bg-accent-subtle/20";
  return "border-border-subtle bg-bg-raised";
}

export function severityBadgeClass(severity: StoryReviewQueueEntry["severity"]): string {
  if (severity === "error") return "bg-danger/20 text-danger";
  if (severity === "warning") return "bg-accent-subtle text-accent";
  return "bg-bg-deep text-text-muted";
}

export function storageStatusClass(status: string): string {
  if (status === "ok") return "text-success";
  if (status === "missing") return "text-text-muted";
  if (status === "error") return "text-danger";
  return "text-accent";
}

export function diagnosticSeverityClass(severity: string): string {
  if (severity === "Error" || severity === "error") return "bg-danger/20 text-danger";
  if (severity === "Warning" || severity === "warning") return "bg-accent-subtle text-accent";
  return "bg-bg-surface text-text-muted";
}

export function formatBytes(bytes?: number): string {
  if (bytes === undefined) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function emptyStoryContractDraft(): StoryContractDraft {
  return {
    title: "",
    genre: "",
    targetReader: "",
    readerPromise: "",
    first30ChapterPromise: "",
    mainConflict: "",
    structuralBoundary: "",
    toneContract: "",
  };
}

export function storyContractDraftFromSummary(
  contract: StoryContractSummary | null | undefined,
): StoryContractDraft {
  return {
    title: contract?.title ?? "",
    genre: contract?.genre ?? "",
    targetReader: contract?.targetReader ?? "",
    readerPromise: contract?.readerPromise ?? "",
    first30ChapterPromise: contract?.first30ChapterPromise ?? "",
    mainConflict: contract?.mainConflict ?? "",
    structuralBoundary: contract?.structuralBoundary ?? "",
    toneContract: contract?.toneContract ?? "",
  };
}

export function emptyChapterMissionDraft(): ChapterMissionDraft {
  return {
    mission: "",
    mustInclude: "",
    mustNot: "",
    expectedEnding: "",
    status: "active",
    sourceRef: "author",
    blockedReason: "",
    retiredHistory: "",
  };
}

export function chapterMissionDraftFromSummary(
  mission: ChapterMissionSummary | null | undefined,
): ChapterMissionDraft {
  return {
    mission: mission?.mission ?? "",
    mustInclude: mission?.mustInclude ?? "",
    mustNot: mission?.mustNot ?? "",
    expectedEnding: mission?.expectedEnding ?? "",
    status: mission?.status === "in_progress" ? "active" : mission?.status || "active",
    sourceRef: mission?.sourceRef || "author",
    blockedReason: mission?.blockedReason ?? "",
    retiredHistory: mission?.retiredHistory ?? "",
  };
}

export function hasStoryContractContent(draft: StoryContractDraft): boolean {
  return Object.values(draft).some((value) => value.trim().length > 0);
}

export function hasChapterMissionContent(draft: ChapterMissionDraft): boolean {
  return [
    draft.mission,
    draft.mustInclude,
    draft.mustNot,
    draft.expectedEnding,
  ].some((value) => value.trim().length > 0);
}
