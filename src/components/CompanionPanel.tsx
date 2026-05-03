import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events } from "../protocol";
import type {
  WriterAgentStatus,
  WriterAgentLedgerSnapshot,
  AgentProposal,
  BackupTarget,
  FileBackupInfo,
  OperationApproval,
  OperationResult,
  ProposalFeedback,
  ProjectStorageDiagnostics,
  StoryMode,
  StoryDebtSnapshot,
  StoryDebtEntry,
  StoryReviewQueueEntry,
  StoryContractSummary,
  ChapterMissionSummary,
  WriterAgentTraceSnapshot,
  WriterProposalTrace,
  WriterOperation,
} from "../protocol";

interface CompanionPanelProps {
  mode: StoryMode;
  onApplyOperation?: (operation: WriterOperation) => Promise<ApplyOperationResult>;
}

interface ApplyOperationResult {
  applied: boolean;
  saved: boolean;
  revision?: string;
  savedContent?: string;
  chapterTitle?: string;
  error?: string;
}

async function recordOperationDurableSave(
  proposalId: string | undefined,
  operation: WriterOperation,
  saveResult: string,
  savedContent?: string,
  chapterTitle?: string,
  chapterRevision?: string,
) {
  if (!proposalId) return;
  await invoke(Commands.recordWriterOperationDurableSave, {
    proposalId,
    operation,
    saveResult,
    savedContent,
    chapterTitle,
    chapterRevision,
  });
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

function isEnhancedGhost(proposal: AgentProposal): boolean {
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

function isEditorTextOperation(
  operation: WriterOperation,
): operation is Extract<WriterOperation, { kind: "text.insert" | "text.replace" }> {
  return operation.kind === "text.insert" || operation.kind === "text.replace";
}

function primaryOperation(proposal: AgentProposal): WriterOperation | undefined {
  return proposal.alternatives?.[0]?.operation ?? proposal.operations[0];
}

function queuePrimaryOperation(entry: StoryReviewQueueEntry): WriterOperation | undefined {
  return entry.operations[0];
}

function debtPrimaryOperation(entry: StoryDebtEntry): WriterOperation | undefined {
  return entry.operations[0];
}

function canonUpdateOperation(operations: WriterOperation[]): WriterOperation | undefined {
  return operations.find((operation) => operation.kind === "canon.update_attribute");
}

function operationLabel(operation: WriterOperation): string {
  if (operation.kind === "promise.resolve") return "Resolve";
  if (operation.kind === "promise.defer") return "Defer";
  if (operation.kind === "promise.abandon") return "Abandon";
  if (operation.kind === "canon.update_attribute") return "Update Canon";
  if (operation.kind === "text.replace") return "Apply Fix";
  if (operation.kind === "text.insert") return "Insert";
  return "Apply";
}

function operationApproval(
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

function nextChapterLabel(chapter?: string | null): string {
  const match = chapter?.match(/(\d+)(?!.*\d)/);
  return match ? `Chapter-${Number(match[1]) + 1}` : "later chapter";
}

function severityClass(severity: StoryReviewQueueEntry["severity"]): string {
  if (severity === "error") return "border-danger/40 bg-danger/10";
  if (severity === "warning") return "border-accent/30 bg-accent-subtle/20";
  return "border-border-subtle bg-bg-raised";
}

function severityBadgeClass(severity: StoryReviewQueueEntry["severity"]): string {
  if (severity === "error") return "bg-danger/20 text-danger";
  if (severity === "warning") return "bg-accent-subtle text-accent";
  return "bg-bg-deep text-text-muted";
}

function storageStatusClass(status: string): string {
  if (status === "ok") return "text-success";
  if (status === "missing") return "text-text-muted";
  if (status === "error") return "text-danger";
  return "text-accent";
}

function formatBytes(bytes?: number): string {
  if (bytes === undefined) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function mergeProposal(prev: AgentProposal[], incoming: AgentProposal): AgentProposal[] {
  const incomingSlot = proposalSlotKey(incoming);
  const existingIndex = prev.findIndex((proposal) => proposalSlotKey(proposal) === incomingSlot);
  if (existingIndex < 0) return [incoming, ...prev].slice(0, 20);

  const existing = prev[existingIndex];
  if (!shouldReplaceProposal(existing, incoming)) return prev;

  const next = prev.filter((_, index) => index !== existingIndex);
  return [incoming, ...next].slice(0, 20);
}

function emptyStoryContractDraft(): StoryContractDraft {
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

function storyContractDraftFromSummary(
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

function emptyChapterMissionDraft(): ChapterMissionDraft {
  return {
    mission: "",
    mustInclude: "",
    mustNot: "",
    expectedEnding: "",
    status: "in_progress",
    sourceRef: "author",
  };
}

function chapterMissionDraftFromSummary(
  mission: ChapterMissionSummary | null | undefined,
): ChapterMissionDraft {
  return {
    mission: mission?.mission ?? "",
    mustInclude: mission?.mustInclude ?? "",
    mustNot: mission?.mustNot ?? "",
    expectedEnding: mission?.expectedEnding ?? "",
    status: mission?.status || "in_progress",
    sourceRef: mission?.sourceRef || "author",
  };
}

function hasStoryContractContent(draft: StoryContractDraft): boolean {
  return Object.values(draft).some((value) => value.trim().length > 0);
}

function hasChapterMissionContent(draft: ChapterMissionDraft): boolean {
  return [
    draft.mission,
    draft.mustInclude,
    draft.mustNot,
    draft.expectedEnding,
  ].some((value) => value.trim().length > 0);
}

type SecondBrainTone = "neutral" | "accent" | "danger" | "success";

interface SecondBrainItem {
  label: string;
  value: string;
  detail?: string;
  tone: SecondBrainTone;
}

interface StoryContractDraft {
  title: string;
  genre: string;
  targetReader: string;
  readerPromise: string;
  first30ChapterPromise: string;
  mainConflict: string;
  structuralBoundary: string;
  toneContract: string;
}

interface ChapterMissionDraft {
  mission: string;
  mustInclude: string;
  mustNot: string;
  expectedEnding: string;
  status: string;
  sourceRef: string;
}

function compactLine(text: string | undefined, fallback: string, max = 96): string {
  const cleaned = (text ?? "").replace(/\s+/g, " ").trim();
  if (!cleaned) return fallback;
  return cleaned.length > max ? `${cleaned.slice(0, max - 1)}...` : cleaned;
}

function firstDebt(
  storyDebt: StoryDebtSnapshot | null,
  categories: StoryDebtEntry["category"][],
): StoryDebtEntry | undefined {
  return storyDebt?.entries.find((entry) =>
    entry.status === "open" && categories.includes(entry.category)
  );
}

function scopedDecision(ledger: WriterAgentLedgerSnapshot | null, currentChapter: string) {
  return ledger?.recentDecisions.find((decision) =>
    currentChapter
      ? decision.scope === currentChapter || decision.scope === "manual request"
      : decision.scope === "manual request"
  );
}

function proposalForArc(proposals: AgentProposal[]): AgentProposal | undefined {
  return proposals.find((proposal) =>
    proposal.kind === "style_note" || proposal.kind === "chapter_structure"
  );
}

function secondBrainToneClass(tone: SecondBrainTone): string {
  if (tone === "danger") return "border-danger/40 bg-danger/10";
  if (tone === "accent") return "border-accent/30 bg-accent-subtle/20";
  if (tone === "success") return "border-success/30 bg-success/10";
  return "border-border-subtle bg-bg-raised";
}

function secondBrainValueClass(tone: SecondBrainTone): string {
  if (tone === "danger") return "text-danger";
  if (tone === "accent") return "text-accent";
  if (tone === "success") return "text-success";
  return "text-text-primary";
}

function latestContextProposal(trace: WriterAgentTraceSnapshot | null): WriterProposalTrace | undefined {
  return trace?.recentProposals.find((proposal) => proposal.contextBudget);
}

function contextBudgetTone(trace: WriterAgentTraceSnapshot | null): SecondBrainTone {
  const budget = latestContextProposal(trace)?.contextBudget;
  if (!budget) return "neutral";
  if (budget.used > budget.totalBudget) return "danger";
  if (budget.sourceReports.some((source) => source.truncated)) return "accent";
  return "success";
}

function formatContextBudgetValue(proposal: WriterProposalTrace | undefined): string {
  const budget = proposal?.contextBudget;
  if (!budget) return "No trace yet";
  return `${budget.used}/${budget.totalBudget} chars`;
}

function formatContextBudgetDetail(proposal: WriterProposalTrace | undefined): string {
  const budget = proposal?.contextBudget;
  if (!budget) return "Waiting for the next context-backed agent proposal.";
  const truncated = budget.sourceReports.filter((source) => source.truncated).length;
  const pct = budget.totalBudget > 0
    ? Math.round((budget.used / budget.totalBudget) * 100)
    : 0;
  return `${budget.task} · ${budget.sourceReports.length} sources · ${pct}% used · ${truncated} truncated`;
}

function formatRate(value: number | undefined): string {
  if (value === undefined || Number.isNaN(value)) return "0%";
  return `${Math.round(value * 100)}%`;
}

function missionStatusLabel(status: string | undefined): string {
  if (status === "completed") return "Done";
  if (status === "drifted") return "Drift";
  if (status === "needs_review") return "Review";
  if (status === "in_progress") return "Active";
  return status || "Draft";
}

function missionStatusTone(status: string | undefined): SecondBrainTone {
  if (status === "completed") return "success";
  if (status === "drifted" || status === "needs_review") return "danger";
  if (status === "in_progress") return "accent";
  return "accent";
}

function sourceBudgetClass(truncated: boolean): string {
  return truncated ? "text-accent" : "text-text-muted";
}

function latestTaskPacket(trace: WriterAgentTraceSnapshot | null) {
  return trace?.taskPackets?.[0];
}

function guardModeLabel(
  trace: WriterAgentTraceSnapshot | null,
  storyDebt: StoryDebtSnapshot | null,
  isAgentThinking: boolean,
): string {
  if (isAgentThinking) return "checking";
  if ((storyDebt?.openCount ?? 0) > 0) return "watching";
  if (latestTaskPacket(trace)?.foundationComplete) return "aligned";
  return "quiet";
}

function guardModeTone(
  trace: WriterAgentTraceSnapshot | null,
  storyDebt: StoryDebtSnapshot | null,
  isAgentThinking: boolean,
): SecondBrainTone {
  if (isAgentThinking) return "accent";
  if ((storyDebt?.canonRiskCount ?? 0) > 0 || (storyDebt?.missionCount ?? 0) > 0) return "danger";
  if ((storyDebt?.openCount ?? 0) > 0) return "accent";
  if (latestTaskPacket(trace)?.foundationComplete) return "success";
  return "neutral";
}

function guardModeDetail(trace: WriterAgentTraceSnapshot | null, storyDebt: StoryDebtSnapshot | null): string {
  const packet = latestTaskPacket(trace);
  const debtCount = storyDebt?.openCount ?? 0;
  if (debtCount > 0) return `${debtCount} story point${debtCount === 1 ? "" : "s"} need protection before the next move.`;
  if (trace?.productMetrics?.feedbackCount) {
    return `Recent acceptance ${formatRate(trace.productMetrics.proposalAcceptanceRate)} · saves ${formatRate(trace.productMetrics.durableSaveSuccessRate)}.`;
  }
  if (packet) {
    return "Grounded on the current chapter, memory, and book-level promise.";
  }
  return "No active risk surfaced.";
}

function buildSecondBrainItems(
  ledger: WriterAgentLedgerSnapshot | null,
  storyDebt: StoryDebtSnapshot | null,
  proposals: AgentProposal[],
  currentChapter: string,
  trace: WriterAgentTraceSnapshot | null,
  isAgentThinking: boolean,
): SecondBrainItem[] {
  const contractDebt = firstDebt(storyDebt, ["story_contract"]);
  const missionDebt = firstDebt(storyDebt, ["chapter_mission"]);
  const canonRisk = firstDebt(storyDebt, ["canon_risk", "timeline_risk"]);
  const promiseDebt = firstDebt(storyDebt, ["promise"]);
  const pacingDebt = firstDebt(storyDebt, ["pacing"]);
  const decision = scopedDecision(ledger, currentChapter);
  const arcProposal = proposalForArc(proposals);
  const openPromise = ledger?.openPromises[0];
  const canonRule = ledger?.canonRules[0];
  const storyContract = ledger?.storyContract;
  const nextBeat = ledger?.nextBeat;
  const latestResult =
    (currentChapter
      ? ledger?.recentChapterResults.find((result) => result.chapterTitle === currentChapter)
      : undefined) ?? ledger?.recentChapterResults[0];
  const chapterMission =
    ledger?.activeChapterMission ??
    ledger?.chapterMissions.find((mission) => mission.chapterTitle === currentChapter);
  const hasStoryContract = Boolean(storyContract && (
    storyContract.readerPromise ||
    storyContract.first30ChapterPromise ||
    storyContract.mainConflict ||
    storyContract.genre
  ));
  const contractNeedsReview = Boolean(storyContract && (
    storyContract.genre.includes("待定") ||
    storyContract.mainConflict.includes("待明确") ||
    storyContract.readerPromise.includes("保持主线清晰")
  ));

  const sceneGoal = contractDebt ?? missionDebt ?? canonRisk ?? promiseDebt ?? pacingDebt;
  const sceneValue = sceneGoal
    ? compactLine(sceneGoal.title, "Resolve current story debt", 72)
    : nextBeat
      ? compactLine(nextBeat.goal, "Continue the next beat", 72)
    : decision
      ? compactLine(decision.title, "Continue current decision", 72)
      : currentChapter
        ? `Advance ${currentChapter}`
        : "No chapter loaded";
  const sceneDetail = sceneGoal
    ? compactLine(sceneGoal.message, "Review the active story issue")
    : nextBeat
      ? compactLine(
          [
            nextBeat.carryovers[0] && `Carry: ${nextBeat.carryovers[0]}`,
            nextBeat.blockers[0] && `Block: ${nextBeat.blockers[0]}`,
          ]
            .filter(Boolean)
            .join(" · "),
          `Next beat for ${nextBeat.chapterTitle}`,
        )
    : decision
      ? compactLine(decision.rationale || decision.scope, "Follow the latest creative decision")
      : "Keep drafting while preserving canon and unresolved promises.";

  const promiseValue = openPromise
    ? compactLine(openPromise.title, "Open promise", 72)
    : promiseDebt
      ? compactLine(promiseDebt.title, "Open promise", 72)
      : "No open promise";
  const promiseDetail = openPromise
    ? compactLine(
        openPromise.expectedPayoff
          ? `${openPromise.description} -> ${openPromise.expectedPayoff}`
          : openPromise.description,
        "Payoff not set",
      )
    : promiseDebt
      ? compactLine(promiseDebt.message, "Resolve or defer")
      : "Ledger has no unresolved promise.";

  const canonValue = canonRisk
    ? compactLine(canonRisk.title, "Canon risk", 72)
    : canonRule
      ? compactLine(canonRule.category, "Canon rule", 72)
      : "No canon risk";
  const canonDetail = canonRisk
    ? compactLine(canonRisk.message, "Resolve before accepting new text")
    : canonRule
      ? compactLine(canonRule.rule, "Respect active canon rule")
      : "No active conflict flagged.";

  const arcValue = pacingDebt
    ? compactLine(pacingDebt.title, "Pacing issue", 72)
    : arcProposal
      ? compactLine(arcProposal.kind.replace("_", " "), "Arc note", 72)
      : "No pacing debt";
  const arcDetail = pacingDebt
    ? compactLine(pacingDebt.message, "Adjust beat or structure")
    : arcProposal
      ? compactLine(arcProposal.preview, arcProposal.rationale || "Review current scene movement")
      : "Current arc has no flagged drag or missing beat.";

  const contractValue = contractDebt
    ? compactLine(contractDebt.title, "Story contract guard", 72)
    : hasStoryContract
    ? compactLine(
        storyContract?.readerPromise ||
          storyContract?.mainConflict ||
          storyContract?.first30ChapterPromise ||
          storyContract?.genre,
        "Story contract",
        72,
      )
    : "No story contract";
  const contractDetail = contractDebt
    ? compactLine(contractDebt.message, "Review book-level boundary before continuing")
    : hasStoryContract
    ? compactLine(
        [
          storyContract?.genre && `Genre: ${storyContract.genre}`,
          storyContract?.first30ChapterPromise && `First 30: ${storyContract.first30ChapterPromise}`,
          storyContract?.mainConflict && `Conflict: ${storyContract.mainConflict}`,
        ]
          .filter(Boolean)
          .join(" · "),
        "Book-level promise is active.",
      )
    : "Set the book-level promise so the agent can judge local choices against the whole novel.";
  const missionValue = missionDebt
    ? compactLine(missionDebt.title, "Chapter mission guard", 72)
    : chapterMission
    ? compactLine(
        `${missionStatusLabel(chapterMission.status)} · ${chapterMission.mission || chapterMission.expectedEnding}`,
        "Current chapter mission",
        72,
      )
    : currentChapter
      ? "No chapter mission"
      : "No chapter loaded";
  const missionDetail = missionDebt
    ? compactLine(missionDebt.message, "Resolve or update this chapter mission")
    : chapterMission
    ? compactLine(
        [
          chapterMission.mustInclude && `Must: ${chapterMission.mustInclude}`,
          chapterMission.mustNot && `Avoid: ${chapterMission.mustNot}`,
          chapterMission.expectedEnding && `End: ${chapterMission.expectedEnding}`,
        ]
          .filter(Boolean)
          .join(" · "),
        "Mission is active for the current chapter.",
      )
    : currentChapter
      ? "Add or seed a mission so local suggestions know what this chapter must accomplish."
      : "Open a chapter to bind the agent to a concrete mission.";
  const resultValue = latestResult
    ? compactLine(latestResult.summary, "Saved chapter result", 72)
    : "No saved result";
  const resultDetail = latestResult
    ? compactLine(
        [
          latestResult.newClues.length > 0 && `Clues: ${latestResult.newClues.join(", ")}`,
          latestResult.newConflicts[0] && `Conflict: ${latestResult.newConflicts[0]}`,
          latestResult.promiseUpdates[0] && `Promise: ${latestResult.promiseUpdates[0]}`,
        ]
          .filter(Boolean)
          .join(" · "),
        `From ${latestResult.chapterTitle}`,
      )
    : "Save a chapter to turn actual written outcomes into future context.";

  const allItems: SecondBrainItem[] = [
    {
      label: "Agent Guard",
      value: guardModeLabel(trace, storyDebt, isAgentThinking),
      detail: guardModeDetail(trace, storyDebt),
      tone: guardModeTone(trace, storyDebt, isAgentThinking),
    },
    {
      label: "Book Contract",
      value: contractValue,
      detail: contractDetail,
      tone: contractDebt ? "danger" : hasStoryContract && !contractNeedsReview ? "success" : "accent",
    },
    {
      label: "Chapter Mission",
      value: missionValue,
      detail: missionDetail,
      tone: missionDebt ? "danger" : chapterMission ? missionStatusTone(chapterMission.status) : "accent",
    },
    {
      label: "Last Result",
      value: resultValue,
      detail: resultDetail,
      tone: latestResult ? "success" : "accent",
    },
    {
      label: "Scene Goal",
      value: sceneValue,
      detail: sceneDetail,
      tone: sceneGoal || nextBeat ? "accent" : "neutral",
    },
    {
      label: "Open Promise",
      value: promiseValue,
      detail: promiseDetail,
      tone: openPromise || promiseDebt ? "accent" : "success",
    },
    {
      label: "Canon Risk",
      value: canonValue,
      detail: canonDetail,
      tone: canonRisk ? "danger" : "success",
    },
    {
      label: "Arc / Pacing",
      value: arcValue,
      detail: arcDetail,
      tone: pacingDebt || arcProposal ? "accent" : "success",
    },
  ];
  const priority = new Map<string, number>([
    ["Agent Guard", 100],
    ["Chapter Mission", missionDebt ? 95 : 70],
    ["Open Promise", openPromise || promiseDebt ? 90 : 45],
    ["Canon Risk", canonRisk ? 85 : 40],
    ["Book Contract", contractDebt ? 80 : hasStoryContract ? 35 : 75],
    ["Scene Goal", sceneGoal || nextBeat ? 65 : 30],
    ["Arc / Pacing", pacingDebt || arcProposal ? 60 : 20],
    ["Last Result", latestResult ? 25 : 15],
  ]);
  return allItems
    .sort((left, right) => (priority.get(right.label) ?? 0) - (priority.get(left.label) ?? 0))
    .slice(0, 5);
}

export const CompanionPanel: React.FC<CompanionPanelProps> = ({ mode, onApplyOperation }) => {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const currentChapterRevision = useAppStore((s) => s.currentChapterRevision);
  const agentMode = useAppStore((s) => s.agentMode);
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);

  const [status, setStatus] = useState<WriterAgentStatus | null>(null);
  const [ledger, setLedger] = useState<WriterAgentLedgerSnapshot | null>(null);
  const [storageDiagnostics, setStorageDiagnostics] = useState<ProjectStorageDiagnostics | null>(null);
  const [chapterBackups, setChapterBackups] = useState<FileBackupInfo[]>([]);
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [reviewQueue, setReviewQueue] = useState<StoryReviewQueueEntry[]>([]);
  const [storyDebt, setStoryDebt] = useState<StoryDebtSnapshot | null>(null);
  const [trace, setTrace] = useState<WriterAgentTraceSnapshot | null>(null);
  const [activeTab, setActiveTab] = useState<"status" | "foundation" | "queue" | "promises" | "canon" | "decisions" | "audit">("status");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [operationError, setOperationError] = useState<string | null>(null);
  const [contractDraft, setContractDraft] = useState<StoryContractDraft>(() => emptyStoryContractDraft());
  const [missionDraft, setMissionDraft] = useState<ChapterMissionDraft>(() => emptyChapterMissionDraft());
  const [foundationSaveState, setFoundationSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [foundationDirty, setFoundationDirty] = useState(false);
  const foundationChapterRef = useRef(currentChapter);

  const refreshStatus = useCallback(async () => {
    try {
      const [nextStatus, nextLedger, nextProposals, nextReviewQueue, nextStoryDebt, nextTrace] = await Promise.all([
        invoke<WriterAgentStatus>(Commands.getWriterAgentStatus),
        invoke<WriterAgentLedgerSnapshot>(Commands.getWriterAgentLedger),
        invoke<AgentProposal[]>(Commands.getWriterAgentPendingProposals),
        invoke<StoryReviewQueueEntry[]>(Commands.getStoryReviewQueue),
        invoke<StoryDebtSnapshot>(Commands.getStoryDebtSnapshot),
        invoke<WriterAgentTraceSnapshot>(Commands.getWriterAgentTrace, { limit: 12 }),
      ]);
      invoke<ProjectStorageDiagnostics>(Commands.getProjectStorageDiagnostics)
        .then(setStorageDiagnostics)
        .catch(() => setStorageDiagnostics(null));
      if (currentChapter) {
        const target: BackupTarget = { kind: "chapter", title: currentChapter };
        invoke<FileBackupInfo[]>(Commands.listFileBackups, { target })
          .then((backups) => setChapterBackups(backups.slice(0, 5)))
          .catch(() => setChapterBackups([]));
      } else {
        setChapterBackups([]);
      }
      setStatus(nextStatus);
      setLedger(nextLedger);
      setReviewQueue(nextReviewQueue);
      setStoryDebt(nextStoryDebt);
      setTrace(nextTrace);
      if (foundationChapterRef.current !== currentChapter) {
        foundationChapterRef.current = currentChapter;
        setFoundationDirty(false);
        setFoundationSaveState("idle");
        setMissionDraft(emptyChapterMissionDraft());
      }
      setContractDraft((prev) =>
        foundationDirty || hasStoryContractContent(prev)
          ? prev
          : storyContractDraftFromSummary(nextLedger.storyContract),
      );
      const activeMission =
        nextLedger.activeChapterMission ??
        nextLedger.chapterMissions.find((mission) => mission.chapterTitle === currentChapter);
      setMissionDraft((prev) =>
        foundationDirty || hasChapterMissionContent(prev) ? prev : chapterMissionDraftFromSummary(activeMission),
      );
      setProposals((prev) => {
        const merged = nextProposals.reduce((acc, proposal) => mergeProposal(acc, proposal), prev);
        return merged.filter((proposal) =>
          nextProposals.some((pending) => pending.id === proposal.id)
        );
      });
    } catch {
      // kernel not initialized yet
    }
  }, [currentChapter, foundationDirty]);

  useEffect(() => {
    const initial = setTimeout(refreshStatus, 0);
    const interval = setInterval(refreshStatus, 5000);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [refreshStatus]);

  useEffect(() => {
    const interval = setInterval(() => setNowMs(Date.now()), 1000);
    return () => clearInterval(interval);
  }, []);

  const applyApprovedOperation = useCallback(async (
    operation: WriterOperation,
    proposalId?: string,
  ): Promise<boolean> => {
    if (!isEditorTextOperation(operation)) return true;

    const result = await onApplyOperation?.(operation);
    if (!result?.applied) {
      setOperationError(result?.error ?? "The editor could not apply this operation.");
      return false;
    }

    if (!result.saved) {
      setOperationError(result.error ?? "The editor applied this operation, but it was not saved. Feedback was not recorded.");
      return false;
    }

    await recordOperationDurableSave(
      proposalId,
      operation,
      result.revision ? `editor_save:${result.revision}` : "editor_save:ok",
      result.savedContent,
      result.chapterTitle,
      result.revision,
    );
    return true;
  }, [onApplyOperation]);

  useEffect(() => {
    // Listen for new proposals from the kernel
    const fn = listen<AgentProposal>(Events.agentProposal, (event) => {
      setProposals((prev) => mergeProposal(prev, event.payload));
    });
    return () => { fn.then((f) => f()); };
  }, []);

  const recordFeedback = useCallback(async (
    proposalId: string,
    action: ProposalFeedback["action"],
    finalText?: string,
    reason?: string,
  ) => {
    const feedback: ProposalFeedback = {
      proposalId,
      action,
      finalText,
      reason,
      createdAt: nowMs,
    };
    try {
      const nextStatus = await invoke<WriterAgentStatus>(Commands.applyProposalFeedback, { feedback });
      setStatus(nextStatus);
      setProposals((prev) => prev.filter((p) => p.id !== proposalId));
      setReviewQueue((prev) => prev.filter((entry) => entry.proposalId !== proposalId));
      const nextLedger = await invoke<WriterAgentLedgerSnapshot>(Commands.getWriterAgentLedger);
      setLedger(nextLedger);
      const nextStoryDebt = await invoke<StoryDebtSnapshot>(Commands.getStoryDebtSnapshot);
      setStoryDebt(nextStoryDebt);
    } catch (e) {
      console.error("Proposal feedback failed:", e);
    }
  }, [nowMs]);

  const handleFeedback = useCallback(async (proposalId: string, action: ProposalFeedback["action"]) => {
    await recordFeedback(proposalId, action);
  }, [recordFeedback]);

  const handleApplyProposal = useCallback(async (proposal: AgentProposal) => {
    setOperationError(null);
    const operation = primaryOperation(proposal);
    if (!operation) {
      await recordFeedback(proposal.id, "accepted", proposal.preview, "Accepted proposal without executable operation.");
      return;
    }

    const currentRevision = currentChapterRevision ?? "";

    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision,
        approval: operationApproval(
          "companion_proposal",
          `Author applied proposal: ${proposal.kind}`,
          proposal.id,
        ),
      });

      if (!result.success) {
        const message = result.error?.message ?? "Operation was rejected by the kernel.";
        setOperationError(message);
        if (result.error?.code === "conflict") {
          await recordFeedback(proposal.id, "snoozed", undefined, message);
        }
        return;
      }

      const applied = await applyApprovedOperation(operation, proposal.id);
      if (!applied) {
        return;
      }

      const finalText = isEditorTextOperation(operation) ? operation.text : proposal.preview;
      await recordFeedback(proposal.id, "accepted", finalText);
    } catch (e) {
      setOperationError(String(e));
    }
  }, [applyApprovedOperation, currentChapterRevision, recordFeedback]);

  const handleApplyQueueEntry = useCallback(async (entry: StoryReviewQueueEntry) => {
    setOperationError(null);
    const operation = queuePrimaryOperation(entry);
    if (!operation) {
      await recordFeedback(entry.proposalId, "accepted", entry.message, "Accepted queue item without executable operation.");
      return;
    }

    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision: currentChapterRevision ?? "",
        approval: operationApproval(
          "story_review_queue",
          `Author applied review queue item: ${entry.category}`,
          entry.proposalId,
        ),
      });

      if (!result.success) {
        const message = result.error?.message ?? "Operation was rejected by the kernel.";
        setOperationError(message);
        if (result.error?.code === "conflict") {
          await recordFeedback(entry.proposalId, "snoozed", undefined, message);
        }
        return;
      }

      const applied = await applyApprovedOperation(operation, entry.proposalId);
      if (!applied) {
        return;
      }

      const finalText = isEditorTextOperation(operation) ? operation.text : entry.message;
      await recordFeedback(entry.proposalId, "accepted", finalText);
    } catch (e) {
      setOperationError(String(e));
    }
  }, [applyApprovedOperation, currentChapterRevision, recordFeedback]);

  const handleApplyQueueOperation = useCallback(async (
    entry: StoryReviewQueueEntry,
    operation: WriterOperation,
    feedbackReason: string,
  ) => {
    setOperationError(null);
    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision: currentChapterRevision ?? "",
        approval: operationApproval("story_review_queue", feedbackReason, entry.proposalId),
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "Operation was rejected by the kernel.");
        return;
      }

      const applied = await applyApprovedOperation(operation, entry.proposalId);
      if (!applied) {
        return;
      }

      const finalText = isEditorTextOperation(operation) ? operation.text : entry.message;
      await recordFeedback(entry.proposalId, "accepted", finalText, feedbackReason);
    } catch (e) {
      setOperationError(String(e));
    }
  }, [applyApprovedOperation, currentChapterRevision, recordFeedback]);

  const handlePromiseLedgerOperation = useCallback(async (operation: WriterOperation) => {
    setOperationError(null);
    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision: currentChapterRevision ?? "",
        approval: operationApproval(
          "promise_ledger",
          `Author updated promise ledger: ${operation.kind}`,
        ),
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "Could not update this promise.");
        return;
      }
      await refreshStatus();
    } catch (e) {
      setOperationError(String(e));
    }
  }, [currentChapterRevision, refreshStatus]);

  const handleApplyDebtOperation = useCallback(async (
    entry: StoryDebtEntry,
    operation: WriterOperation,
    feedbackReason: string,
  ) => {
    setOperationError(null);
    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision: currentChapterRevision ?? "",
        approval: operationApproval("story_debt", feedbackReason, entry.relatedReviewIds[0]),
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "Could not apply this story debt action.");
        return;
      }

      const applied = await applyApprovedOperation(operation, entry.relatedReviewIds[0]?.replace(/^review_/, ""));
      if (!applied) {
        return;
      }

      const proposalId = entry.relatedReviewIds[0]?.replace(/^review_/, "");
      if (proposalId) {
        const finalText = isEditorTextOperation(operation) ? operation.text : entry.message;
        await recordFeedback(proposalId, "accepted", finalText, feedbackReason);
      } else {
        await refreshStatus();
      }
    } catch (e) {
      setOperationError(String(e));
    }
  }, [applyApprovedOperation, currentChapterRevision, recordFeedback, refreshStatus]);


  const handleIgnoreDebtEntry = useCallback(async (entry: StoryDebtEntry) => {
    const proposalId = entry.relatedReviewIds[0]?.replace(/^review_/, "");
    if (proposalId) {
      await recordFeedback(proposalId, "rejected", undefined, "Ignored from story debt summary.");
    }
  }, [recordFeedback]);

  const handleSaveFoundation = useCallback(async () => {
    setOperationError(null);
    setFoundationSaveState("saving");
    const projectId = status?.projectId ?? ledger?.storyContract?.projectId ?? "local-project";
    const operations: WriterOperation[] = [];

    if (hasStoryContractContent(contractDraft)) {
      operations.push({
        kind: "story_contract.upsert",
        contract: {
          projectId,
          title: contractDraft.title.trim(),
          genre: contractDraft.genre.trim(),
          targetReader: contractDraft.targetReader.trim(),
          readerPromise: contractDraft.readerPromise.trim(),
          first30ChapterPromise: contractDraft.first30ChapterPromise.trim(),
          mainConflict: contractDraft.mainConflict.trim(),
          structuralBoundary: contractDraft.structuralBoundary.trim(),
          toneContract: contractDraft.toneContract.trim(),
        },
      });
    }

    if (currentChapter && hasChapterMissionContent(missionDraft)) {
      operations.push({
        kind: "chapter_mission.upsert",
        mission: {
          projectId,
          chapterTitle: currentChapter,
          mission: missionDraft.mission.trim(),
          mustInclude: missionDraft.mustInclude.trim(),
          mustNot: missionDraft.mustNot.trim(),
          expectedEnding: missionDraft.expectedEnding.trim(),
          status: missionDraft.status.trim() || "in_progress",
          sourceRef: missionDraft.sourceRef.trim() || "author",
        },
      });
    }

    if (operations.length === 0) {
      setFoundationSaveState("error");
      setOperationError("Fill at least one Story Contract or Chapter Mission field before saving.");
      return;
    }

    try {
      for (const operation of operations) {
        const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
          operation,
          currentRevision: currentChapterRevision ?? "",
          approval: operationApproval(
            "foundation_editor",
            `Author saved foundation memory: ${operation.kind}`,
          ),
        });
        if (!result.success) {
          throw new Error(result.error?.message ?? "Foundation operation was rejected by the kernel.");
        }
      }
      setFoundationDirty(false);
      setFoundationSaveState("saved");
      await refreshStatus();
    } catch (e) {
      setFoundationSaveState("error");
      setOperationError(String(e));
    }
  }, [
    contractDraft,
    currentChapter,
    currentChapterRevision,
    ledger?.storyContract?.projectId,
    missionDraft,
    refreshStatus,
    status?.projectId,
  ]);

  const handleRestoreLatestChapterBackup = useCallback(async () => {
    if (!currentChapter || chapterBackups.length === 0) return;
    setOperationError(null);
    try {
      const target: BackupTarget = { kind: "chapter", title: currentChapter };
      await invoke(Commands.restoreFileBackup, {
        target,
        backupId: chapterBackups[0].id,
      });
      const revision = await invoke<string>(Commands.getChapterRevision, { title: currentChapter });
      window.dispatchEvent(new CustomEvent(Events.chapterRestored, {
        detail: { title: currentChapter, revision },
      }));
      await refreshStatus();
    } catch (e) {
      setOperationError(String(e));
    }
  }, [chapterBackups, currentChapter, refreshStatus]);

  const pendingProposals = proposals.filter((p) => {
    return p.expiresAt === undefined || p.expiresAt === 0 || nowMs < p.expiresAt;
  });
  const visibleReviewQueue = reviewQueue.filter((entry) => (
    entry.status === "pending"
    && (entry.expiresAt === undefined || entry.expiresAt === 0 || nowMs < entry.expiresAt)
  ));
  const visibleProposals =
    mode === "write"
      ? pendingProposals.filter((proposal) => proposal.priority === "urgent").slice(0, 3)
      : pendingProposals;
  const secondBrainItems = buildSecondBrainItems(
    ledger,
    storyDebt,
    pendingProposals,
    currentChapter,
    trace,
    isAgentThinking,
  );
  const availableTabs =
    mode === "write"
      ? (["status"] as const)
      : mode === "review"
        ? (["foundation", "queue", "promises", "canon", "decisions", "audit"] as const)
        : (["status", "foundation", "promises", "canon", "decisions", "audit"] as const);
  const effectiveTab =
    mode === "write"
      ? "status"
      : mode === "review" && activeTab === "status"
        ? "queue"
        : mode !== "review" && activeTab === "queue"
          ? "status"
          : activeTab;

  return (
    <div className="flex flex-col h-full bg-bg-surface">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border-subtle">
        <div className="flex items-center justify-between mb-2">
          <span className="font-display text-sm tracking-wider text-text-primary">
            {mode === "write" ? "Writing Companion" : mode === "review" ? "Story Review" : "Agent Evidence"}
          </span>
          <span className={`w-2 h-2 rounded-full ${
            isAgentThinking ? "bg-accent animate-pulse" : "bg-success"
          }`} />
        </div>
        {status && mode !== "write" && (
          <div className="grid grid-cols-2 gap-2 text-xs text-text-muted">
            <div>
              <span className="block text-text-secondary">Observations</span>
              <span className="font-mono">{status.observationCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">Proposals</span>
              <span className="font-mono">{status.proposalCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">Open Promises</span>
              <span className="font-mono text-accent">{status.openPromiseCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">Feedback</span>
              <span className="font-mono">{status.totalFeedbackEvents}</span>
            </div>
          </div>
        )}
      </div>

      {/* Tabs */}
      {availableTabs.length > 1 && (
        <div className="flex border-b border-border-subtle">
          {availableTabs.map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`flex-1 py-2 text-xs tracking-wide transition-colors ${
                effectiveTab === tab
                  ? "text-accent border-b border-accent"
                  : "text-text-muted hover:text-text-secondary"
              }`}
            >
              {tab === "status"
                ? "状态"
                : tab === "foundation"
                  ? "地基"
                : tab === "queue"
                  ? "队列"
                : tab === "promises"
                  ? "伏笔"
                  : tab === "canon"
                    ? "设定"
                    : tab === "decisions"
                      ? "决策"
                      : "审计"}
            </button>
          ))}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-3 space-y-2">
        {effectiveTab === "status" && (
          <div className="space-y-3">
            {mode !== "write" && agentMode !== "proactive" && (
              <div className="p-3 rounded bg-accent-subtle/30 border border-accent/20 text-xs text-text-secondary">
                Agent is in {agentMode} mode. Switch to Proactive for ambient suggestions.
              </div>
            )}
            {operationError && (
              <div className="p-2 rounded bg-danger/10 border border-danger/30 text-xs text-danger">
                {operationError}
              </div>
            )}
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2 text-xs">
                <span className="font-medium text-text-secondary">What It Is Guarding</span>
                {mode !== "write" && (
                  <span className="text-[10px] text-text-muted">
                    {storyDebt?.openCount ?? 0} open · {ledger?.openPromises.length ?? 0} promises
                  </span>
                )}
              </div>
              <div className="grid grid-cols-1 gap-2 2xl:grid-cols-2">
                {secondBrainItems.map((item) => (
                  <div
                    key={item.label}
                    className={`min-w-0 rounded border p-2 text-xs ${secondBrainToneClass(item.tone)}`}
                  >
                    <div className="mb-1 text-[10px] uppercase tracking-wide text-text-muted">
                      {item.label}
                    </div>
                    <div className={`truncate font-medium ${secondBrainValueClass(item.tone)}`} title={item.value}>
                      {item.value}
                    </div>
                    {item.detail && (
                      <div className="mt-1 line-clamp-2 text-[10px] leading-snug text-text-secondary" title={item.detail}>
                        {item.detail}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
            {mode !== "write" && (
              <div className={`rounded border p-2 text-xs ${secondBrainToneClass(contextBudgetTone(trace))}`}>
                {(() => {
                  const latestContextTrace = latestContextProposal(trace);
                  const latestContextBudget = latestContextTrace?.contextBudget;
                  return (
                    <>
                      <div className="mb-1 flex items-center justify-between gap-2">
                        <span className="text-[10px] uppercase tracking-wide text-text-muted">
                          Evidence Trace
                        </span>
                        <span className="font-mono text-[10px] text-text-muted">
                          {latestContextTrace?.kind ?? "idle"}
                        </span>
                      </div>
                      <div className={`font-medium ${secondBrainValueClass(contextBudgetTone(trace))}`}>
                        {formatContextBudgetValue(latestContextTrace)}
                      </div>
                      <div className="mt-1 text-[10px] leading-snug text-text-secondary">
                        {formatContextBudgetDetail(latestContextTrace)}
                      </div>
                      {latestContextBudget && (
                        <div className="mt-2 space-y-1">
                          {latestContextBudget.sourceReports.slice(0, 4).map((source) => (
                            <div
                              key={`${latestContextTrace.id}-${source.source}`}
                              className="flex items-center justify-between gap-2 rounded bg-bg-deep px-1.5 py-1"
                            >
                              <span className={`truncate ${sourceBudgetClass(source.truncated)}`} title={source.source}>
                                {source.source}
                              </span>
                              <span className="shrink-0 font-mono text-[10px] text-text-muted">
                                {source.provided}/{source.requested}
                              </span>
                            </div>
                          ))}
                        </div>
                      )}
                    </>
                  );
                })()}
              </div>
            )}
            {mode !== "write" && (
              <div className="text-xs text-text-muted">
                <div className="mb-2 text-text-secondary font-medium">Active Scene</div>
                <div className="p-2 rounded bg-bg-raised border border-border-subtle">
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate">{currentChapter || "No chapter loaded"}</span>
                    {chapterBackups.length > 0 && (
                      <button
                        onClick={handleRestoreLatestChapterBackup}
                        className="shrink-0 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-[10px] text-text-secondary hover:border-accent/40 hover:text-accent"
                        title={chapterBackups[0].filename}
                      >
                        Restore latest
                      </button>
                    )}
                  </div>
                  {chapterBackups.length > 0 && (
                    <div className="mt-1 text-[10px] text-text-muted">
                      {chapterBackups.length} recent backups · latest {formatBytes(chapterBackups[0].bytes)}
                    </div>
                  )}
                </div>
              </div>
            )}
            {mode !== "write" && storageDiagnostics && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2 text-xs">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Project Storage</span>
                  <span className={storageDiagnostics.healthy ? "text-success" : "text-danger"}>
                    {storageDiagnostics.healthy ? "healthy" : "needs attention"}
                  </span>
                </div>
                <div className="mb-2 min-w-0 text-[10px] text-text-muted">
                  <div className="truncate" title={storageDiagnostics.projectDataDir}>
                    {storageDiagnostics.projectName} · {storageDiagnostics.projectId}
                  </div>
                </div>
                <div className="space-y-1">
                  {storageDiagnostics.files.map((file) => (
                    <div key={file.label} className="flex items-center justify-between gap-2">
                      <span className="truncate text-text-secondary" title={file.path}>
                        {file.label}
                      </span>
                      <span className="shrink-0 font-mono text-[10px] text-text-muted">
                        {file.recordCount ?? "-"} · {formatBytes(file.bytes)} · b{file.backupCount}
                      </span>
                      <span className={`shrink-0 text-[10px] ${storageStatusClass(file.status)}`}>
                        {file.status}
                      </span>
                    </div>
                  ))}
                  {storageDiagnostics.databases.map((db) => (
                    <div key={db.label} className="border-t border-border-subtle pt-1">
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate text-text-secondary" title={db.path}>
                          {db.label}
                        </span>
                        <span className="shrink-0 font-mono text-[10px] text-text-muted">
                          v{db.userVersion ?? "-"} · {formatBytes(db.bytes)}
                        </span>
                        <span className={`shrink-0 text-[10px] ${storageStatusClass(db.status)}`}>
                          {db.status}
                        </span>
                      </div>
                      {db.tableCounts.length > 0 && (
                        <div className="mt-1 flex flex-wrap gap-1">
                          {db.tableCounts.slice(0, 5).map((table) => (
                            <span key={`${db.label}-${table.table}`} className="rounded bg-bg-deep px-1 py-0.5 text-[10px] text-text-muted">
                              {table.table}: {table.rows}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
                {[...storageDiagnostics.files, ...storageDiagnostics.databases]
                  .filter((item) => item.error)
                  .slice(0, 2)
                  .map((item) => (
                    <div key={`${item.label}-error`} className="mt-2 rounded border border-danger/30 bg-danger/10 p-1.5 text-[10px] text-danger">
                      {item.label}: {item.error}
                    </div>
                  ))}
              </div>
            )}
            {visibleProposals.length > 0 && (
              <div>
                <div className="text-xs text-text-secondary font-medium mb-2">
                  {mode === "write" ? "Urgent Story Guards" : "Pending Proposals"} ({visibleProposals.length})
                </div>
                {visibleProposals.slice(0, mode === "write" ? 3 : 5).map((p) => (
                  <div key={p.id} className={`p-2 rounded border mb-1 text-xs ${
                    p.priority === "urgent" ? "border-danger/40 bg-danger/10" :
                    p.priority === "normal" ? "border-accent/30 bg-accent-subtle/20" :
                    "border-border-subtle bg-bg-raised"
                  }`}>
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-text-primary font-medium">{p.kind}</span>
                      <div className="flex items-center gap-1">
                        {isEnhancedGhost(p) && (
                          <span className="px-1.5 py-0.5 rounded text-[10px] bg-success/10 text-success">
                            Enhanced
                          </span>
                        )}
                        <span className={`px-1.5 py-0.5 rounded text-[10px] ${
                          p.priority === "urgent" ? "bg-danger/20 text-danger" :
                          p.priority === "normal" ? "bg-accent-subtle text-accent" :
                          "bg-bg-raised text-text-muted"
                        }`}>{p.priority}</span>
                      </div>
                    </div>
                    <p className="text-text-muted mb-2">{p.preview}</p>
                    {mode !== "write" && p.rationale && (
                      <p className="text-text-secondary italic mb-1">{p.rationale}</p>
                    )}
                    {mode !== "write" && p.evidence.length > 0 && (
                      <div className="mb-2 space-y-1">
                        {p.evidence.map((e, i) => (
                          <div key={i} className="p-1.5 rounded bg-bg-deep border border-border-subtle">
                            <span className="text-[10px] text-text-muted">{e.source}: </span>
                            <span className="text-[10px] text-text-secondary">{e.snippet}</span>
                          </div>
                        ))}
                      </div>
                    )}
                    {mode !== "write" && primaryOperation(p) && (
                      <div className="mb-2 rounded bg-bg-deep border border-border-subtle p-1.5 text-[10px] text-text-muted">
                        {primaryOperation(p)?.kind}
                        {p.alternatives.length > 1 ? ` · ${p.alternatives.length} branches` : ""}
                      </div>
                    )}
                    <div className="flex gap-1">
                      <button
                        onClick={() => handleApplyProposal(p)}
                        className="px-2 py-1 text-[10px] rounded bg-accent-subtle text-accent border border-accent/40 hover:bg-accent/20"
                      >
                        Apply
                      </button>
                      <button
                        onClick={() => handleFeedback(p.id, "rejected")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                      >
                        Reject
                      </button>
                      <button
                        onClick={() => handleFeedback(p.id, "snoozed")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                      >
                        Snooze
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {effectiveTab === "foundation" && (
          <div className="space-y-3 text-xs">
            {operationError && (
              <div className="p-2 rounded bg-danger/10 border border-danger/30 text-xs text-danger">
                {operationError}
              </div>
            )}
            <div className="rounded border border-border-subtle bg-bg-raised p-3">
              <div className="mb-2 flex items-center justify-between gap-2">
                <div>
                  <div className="font-medium text-text-primary">Story Contract</div>
                  <div className="text-[10px] text-text-muted">Book-level promise every agent action must obey.</div>
                </div>
                <span className="shrink-0 text-[10px] text-text-muted">
                  {ledger?.storyContract?.updatedAt ? "saved" : "draft"}
                </span>
              </div>
              <div className="grid grid-cols-1 gap-2">
                <input
                  value={contractDraft.title}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, title: event.target.value }));
                  }}
                  className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Title"
                />
                <div className="grid grid-cols-2 gap-2">
                  <input
                    value={contractDraft.genre}
                    onChange={(event) => {
                      setFoundationDirty(true);
                      setContractDraft((prev) => ({ ...prev, genre: event.target.value }));
                    }}
                    className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                    placeholder="Genre"
                  />
                  <input
                    value={contractDraft.targetReader}
                    onChange={(event) => {
                      setFoundationDirty(true);
                      setContractDraft((prev) => ({ ...prev, targetReader: event.target.value }));
                    }}
                    className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                    placeholder="Target reader"
                  />
                </div>
                <textarea
                  value={contractDraft.readerPromise}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, readerPromise: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Reader promise"
                />
                <textarea
                  value={contractDraft.first30ChapterPromise}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, first30ChapterPromise: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="First 30 chapters promise"
                />
                <textarea
                  value={contractDraft.mainConflict}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, mainConflict: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Main conflict"
                />
                <textarea
                  value={contractDraft.structuralBoundary}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, structuralBoundary: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Boundaries / forbidden moves"
                />
                <textarea
                  value={contractDraft.toneContract}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, toneContract: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Tone and style floor"
                />
              </div>
            </div>

            <div className="rounded border border-border-subtle bg-bg-raised p-3">
              <div className="mb-2 flex items-center justify-between gap-2">
                <div>
                  <div className="font-medium text-text-primary">Chapter Mission</div>
                  <div className="text-[10px] text-text-muted">{currentChapter || "No chapter selected"}</div>
                </div>
                <select
                  value={missionDraft.status}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, status: event.target.value }));
                  }}
                  className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-[10px] text-text-secondary outline-none focus:border-accent"
                >
                  <option value="in_progress">Active</option>
                  <option value="needs_review">Review</option>
                  <option value="completed">Done</option>
                  <option value="drifted">Drift</option>
                </select>
              </div>
              <div className="grid grid-cols-1 gap-2">
                <textarea
                  value={missionDraft.mission}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, mission: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="What this chapter must accomplish"
                />
                <textarea
                  value={missionDraft.mustInclude}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, mustInclude: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Must include"
                />
                <textarea
                  value={missionDraft.mustNot}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, mustNot: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Must not reveal or do"
                />
                <textarea
                  value={missionDraft.expectedEnding}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, expectedEnding: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="Expected ending state"
                />
              </div>
            </div>

            <div className="sticky bottom-0 flex items-center justify-between gap-2 border-t border-border-subtle bg-bg-surface pt-2">
              <span className={`text-[10px] ${
                foundationSaveState === "error"
                  ? "text-danger"
                  : foundationSaveState === "saved"
                    ? "text-success"
                    : "text-text-muted"
              }`}>
                {foundationSaveState === "saving"
                  ? "Saving foundation..."
                  : foundationSaveState === "saved"
                    ? "Foundation saved"
                    : foundationSaveState === "error"
                      ? "Foundation save failed"
                      : foundationDirty
                        ? "Unsaved foundation edits"
                        : "Foundation is synced"}
              </span>
              <button
                onClick={handleSaveFoundation}
                disabled={foundationSaveState === "saving"}
                className="rounded bg-accent px-3 py-1 text-xs text-bg-deep transition-colors hover:bg-accent/80 disabled:opacity-60"
              >
                Save Foundation
              </button>
            </div>
          </div>
        )}

        {effectiveTab === "queue" && (
          <div className="space-y-2 text-xs">
            {operationError && (
              <div className="p-2 rounded bg-danger/10 border border-danger/30 text-xs text-danger">
                {operationError}
              </div>
            )}
            {storyDebt && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Story Debt</span>
                  <span className="text-[10px] text-text-muted">
                    {storyDebt.chapterTitle || currentChapter || "project"}
                  </span>
                </div>
                <div className="mt-2 grid grid-cols-6 gap-1 text-center">
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-text-primary">{storyDebt.openCount}</div>
                    <div className="text-[10px] text-text-muted">open</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-danger">{storyDebt.contractCount}</div>
                    <div className="text-[10px] text-text-muted">contract</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-danger">{storyDebt.missionCount}</div>
                    <div className="text-[10px] text-text-muted">mission</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-danger">{storyDebt.canonRiskCount}</div>
                    <div className="text-[10px] text-text-muted">canon</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-accent">{storyDebt.promiseCount}</div>
                    <div className="text-[10px] text-text-muted">promise</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-text-secondary">{storyDebt.pacingCount}</div>
                    <div className="text-[10px] text-text-muted">pacing</div>
                  </div>
                </div>
                {storyDebt.entries.slice(0, 3).map((entry) => {
                  const primaryDebtOperation = debtPrimaryOperation(entry);
                  const canonOperation = canonUpdateOperation(entry.operations);
                  const secondaryOperations = entry.operations.filter((operation) =>
                    operation !== primaryDebtOperation && operation !== canonOperation
                  );
                  return (
                    <div key={entry.id} className="mt-2 border-t border-border-subtle pt-2">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-text-secondary">{entry.title}</span>
                        <span className={`rounded px-1.5 py-0.5 text-[10px] ${severityBadgeClass(entry.severity)}`}>
                          {entry.category}
                        </span>
                      </div>
                      <p className="mt-1 line-clamp-2 text-text-muted">{entry.message}</p>
                      {(entry.operations.length > 0 || entry.relatedReviewIds.length > 0) && (
                        <div className="mt-2 flex flex-wrap gap-1">
                          {primaryDebtOperation && (
                            <button
                              onClick={() => handleApplyDebtOperation(entry, primaryDebtOperation, `${operationLabel(primaryDebtOperation)} from story debt summary.`)}
                              className="px-2 py-1 text-[10px] rounded bg-accent-subtle text-accent border border-accent/40 hover:bg-accent/20"
                            >
                              {operationLabel(primaryDebtOperation)}
                            </button>
                          )}
                          {canonOperation && (
                            <button
                              onClick={() => handleApplyDebtOperation(entry, canonOperation, "Updated canon from story debt.")}
                              className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-secondary border border-border-subtle hover:text-accent hover:border-accent/40"
                            >
                              Update Canon
                            </button>
                          )}
                          {secondaryOperations.map((operation) => (
                            <button
                              key={`${entry.id}-${operation.kind}`}
                              onClick={() => handleApplyDebtOperation(entry, operation, `${operationLabel(operation)} from story debt summary.`)}
                              className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-secondary border border-border-subtle hover:text-accent hover:border-accent/40"
                            >
                              {operationLabel(operation)}
                            </button>
                          ))}
                          {entry.relatedReviewIds.length > 0 && (
                            <button
                              onClick={() => handleIgnoreDebtEntry(entry)}
                              className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-muted border border-border-subtle hover:bg-bg-surface"
                            >
                              Ignore
                            </button>
                          )}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
            {visibleReviewQueue.length === 0 && (
              <div className="rounded bg-bg-raised border border-border-subtle p-3 text-text-muted">
                No story review items waiting.
              </div>
            )}
            {visibleReviewQueue.map((entry) => {
              const operation = queuePrimaryOperation(entry);
              const canonOperation = canonUpdateOperation(entry.operations);
              const secondaryOperations = entry.operations.filter((item) =>
                item !== operation && item !== canonOperation
              );
              return (
                <div key={entry.id} className={`rounded border p-2 ${severityClass(entry.severity)}`}>
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="font-medium text-text-primary">{entry.title}</div>
                      <div className="mt-0.5 text-[10px] text-text-muted">{entry.category}</div>
                    </div>
                    <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] ${severityBadgeClass(entry.severity)}`}>
                      {entry.severity}
                    </span>
                  </div>
                  <p className="mt-2 text-text-secondary">{entry.message}</p>
                  {entry.evidence.length > 0 && (
                    <div className="mt-2 space-y-1">
                      {entry.evidence.slice(0, 3).map((evidence, index) => (
                        <div key={`${entry.id}-${index}`} className="rounded bg-bg-deep border border-border-subtle p-1.5">
                          <span className="text-[10px] text-text-muted">{evidence.source}: </span>
                          <span className="text-[10px] text-text-secondary">{evidence.snippet}</span>
                        </div>
                      ))}
                    </div>
                  )}
                  {operation && (
                    <div className="mt-2 rounded bg-bg-deep border border-border-subtle p-1.5 text-[10px] text-text-muted">
                      {operation.kind}
                    </div>
                  )}
                  <div className="mt-2 flex flex-wrap gap-1">
                    <button
                      onClick={() => handleApplyQueueEntry(entry)}
                      className="px-2 py-1 text-[10px] rounded bg-accent-subtle text-accent border border-accent/40 hover:bg-accent/20"
                    >
                      {operation ? operationLabel(operation) : "Apply"}
                    </button>
                    <button
                      onClick={() => handleFeedback(entry.proposalId, "rejected")}
                      className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                    >
                      Ignore
                    </button>
                    <button
                      onClick={() => handleFeedback(entry.proposalId, "snoozed")}
                      className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                    >
                      Snooze
                    </button>
                    {canonOperation && (
                      <button
                        onClick={() => handleApplyQueueOperation(entry, canonOperation, "Updated canon instead of changing text.")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-secondary border border-border-subtle hover:text-accent hover:border-accent/40"
                      >
                        Update Canon
                      </button>
                    )}
                    {secondaryOperations.map((item) => (
                      <button
                        key={`${entry.id}-${item.kind}`}
                        onClick={() => handleApplyQueueOperation(entry, item, `${operationLabel(item)} from review queue.`)}
                        className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-secondary border border-border-subtle hover:text-accent hover:border-accent/40"
                      >
                        {operationLabel(item)}
                      </button>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {effectiveTab === "promises" && (
          <div className="space-y-2 text-xs">
            {(ledger?.openPromises.length ?? 0) === 0 && (
              <p className="text-text-muted">No open plot promises recorded yet.</p>
            )}
            {ledger?.openPromises.map((promise) => {
              const chapter = currentChapter ?? (promise.expectedPayoff || "current chapter");
              const operations: WriterOperation[] = [
                { kind: "promise.resolve", promiseId: String(promise.id), chapter },
                {
                  kind: "promise.defer",
                  promiseId: String(promise.id),
                  chapter,
                  expectedPayoff: nextChapterLabel(chapter),
                },
                {
                  kind: "promise.abandon",
                  promiseId: String(promise.id),
                  chapter,
                  reason: `Author decided '${promise.title}' no longer needs payoff in the current story shape.`,
                },
              ];
              return (
                <div key={promise.id} className="rounded bg-bg-raised border border-border-subtle p-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-text-primary">{promise.title}</span>
                    <span className="text-[10px] text-text-muted">{promise.kind}</span>
                  </div>
                  <p className="mt-1 text-text-secondary">{promise.description}</p>
                  <div className="mt-2 text-[10px] text-text-muted">
                    {promise.introducedChapter || "unknown"} {"->"} {promise.expectedPayoff || "unset"}
                  </div>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {operations.map((operation) => (
                      <button
                        key={`${promise.id}-${operation.kind}`}
                        onClick={() => handlePromiseLedgerOperation(operation)}
                        className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-secondary border border-border-subtle hover:text-accent hover:border-accent/40"
                      >
                        {operationLabel(operation)}
                      </button>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {effectiveTab === "canon" && (
          <div className="space-y-2 text-xs">
            {(ledger?.canonEntities.length ?? 0) === 0 && (ledger?.canonRules.length ?? 0) === 0 && (
              <p className="text-text-muted">No canon entities or rules recorded yet.</p>
            )}
            {ledger?.canonRules.map((rule) => (
              <div key={`${rule.category}-${rule.rule}`} className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">{rule.category}</span>
                  <span className="text-[10px] text-text-muted">
                    p{rule.priority} · {rule.status}
                  </span>
                </div>
                <p className="mt-1 text-text-secondary">{rule.rule}</p>
              </div>
            ))}
            {ledger?.canonEntities.map((entity) => (
              <div key={`${entity.kind}-${entity.name}`} className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">{entity.name}</span>
                  <span className="text-[10px] text-text-muted">
                    {entity.kind} · {Math.round(entity.confidence * 100)}%
                  </span>
                </div>
                {entity.summary && (
                  <p className="mt-1 line-clamp-3 text-text-secondary">{entity.summary}</p>
                )}
                {Object.entries(entity.attributes).length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {Object.entries(entity.attributes).map(([key, value]) => (
                      <span key={key} className="rounded bg-bg-deep px-1.5 py-0.5 text-[10px] text-text-muted">
                        {key}: {String(value)}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {effectiveTab === "decisions" && (
          <div className="space-y-2 text-xs">
            {(ledger?.recentDecisions.length ?? 0) === 0 && (
              <p className="text-text-muted">No creative decisions recorded yet.</p>
            )}
            {ledger?.recentDecisions.map((decision) => (
              <div key={`${decision.createdAt}-${decision.title}`} className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">{decision.title}</span>
                  <span className="text-[10px] text-text-muted">{decision.decision}</span>
                </div>
                <p className="mt-1 text-text-secondary">{decision.rationale || decision.scope}</p>
                <div className="mt-1 text-[10px] text-text-muted">{decision.scope}</div>
              </div>
            ))}
          </div>
        )}

        {effectiveTab === "audit" && (
          <div className="space-y-2 text-xs">
            {(trace?.recentProposals.length ?? 0) === 0 &&
              (trace?.taskPackets.length ?? 0) === 0 &&
              (ledger?.memoryAudit.length ?? 0) === 0 &&
              (trace?.contextRecalls.length ?? ledger?.contextRecalls.length ?? 0) === 0 && (
              <p className="text-text-muted">No agent audit events yet.</p>
            )}
            {(trace?.taskPackets.length ?? 0) > 0 && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Why It Spoke</span>
                  <span className="text-[10px] text-text-muted">
                    {trace?.taskPackets.length ?? 0} packets
                  </span>
                </div>
                <div className="space-y-1.5">
                  {trace?.taskPackets.slice(0, 4).map((packet) => (
                    <div key={packet.id} className="rounded border border-border-subtle bg-bg-deep p-2">
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-text-secondary" title={packet.objective}>
                          {packet.task} · {packet.foundationComplete ? "grounded" : "partial"}
                        </span>
                        <span className="shrink-0 font-mono text-[10px] text-text-muted">
                          {packet.requiredContextCount} ctx · {packet.beliefCount} beliefs
                        </span>
                      </div>
                      <p className="mt-1 line-clamp-2 text-[10px] text-text-muted">
                        {packet.objective}
                      </p>
                      <div className="mt-2 flex flex-wrap gap-1">
                        <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                          {packet.scope}
                        </span>
                        <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                          {packet.maxSideEffectLevel}
                        </span>
                        <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                          {packet.feedbackCheckpointCount} checks
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {((trace?.contextRecalls.length ?? ledger?.contextRecalls.length ?? 0) > 0) && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Context Recall</span>
                  <span className="text-[10px] text-text-muted">
                    {trace?.contextRecalls.length ?? ledger?.contextRecalls.length ?? 0} tracked
                  </span>
                </div>
                <div className="space-y-1.5">
                  {(trace?.contextRecalls ?? ledger?.contextRecalls ?? []).slice(0, 6).map((recall) => (
                    <div key={`${recall.source}-${recall.reference}`} className="rounded border border-border-subtle bg-bg-deep p-2">
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-text-secondary" title={recall.reference}>
                          {recall.source} · {recall.reference}
                        </span>
                        <span className="shrink-0 font-mono text-[10px] text-accent">
                          x{recall.recallCount}
                        </span>
                      </div>
                      <p className="mt-1 line-clamp-2 text-[10px] text-text-muted">{recall.snippet}</p>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {(trace?.recentProposals.length ?? 0) > 0 && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Context Trace</span>
                  <span className="text-[10px] text-text-muted">
                    {trace?.recentProposals.filter((proposal) => proposal.contextBudget).length ?? 0} budgeted
                  </span>
                </div>
                <div className="space-y-2">
                  {trace?.recentProposals.slice(0, 6).map((proposal) => {
                    const budget = proposal.contextBudget;
                    const truncated = budget?.sourceReports.filter((source) => source.truncated).length ?? 0;
                    return (
                      <div key={proposal.id} className="rounded border border-border-subtle bg-bg-deep p-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="truncate font-medium text-text-secondary" title={proposal.previewSnippet}>
                            {proposal.kind} · {proposal.state}
                          </span>
                          <span className="shrink-0 font-mono text-[10px] text-text-muted">
                            {budget ? `${budget.used}/${budget.totalBudget}` : "no budget"}
                          </span>
                        </div>
                        <div className="mt-1 line-clamp-2 text-[10px] text-text-muted">
                          {proposal.previewSnippet || proposal.observationId}
                        </div>
                        {budget && (
                          <>
                            <div className="mt-2 flex flex-wrap gap-1">
                              <span className={`rounded px-1.5 py-0.5 text-[10px] ${truncated > 0 ? "bg-accent-subtle text-accent" : "bg-success/10 text-success"}`}>
                                {budget.task}
                              </span>
                              <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                                wasted {budget.wasted}
                              </span>
                              <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                                truncated {truncated}
                              </span>
                            </div>
                            <div className="mt-2 space-y-1">
                              {budget.sourceReports.slice(0, 5).map((source) => (
                                <div key={`${proposal.id}-${source.source}`} className="rounded border border-border-subtle bg-bg-surface p-1.5">
                                  <div className="flex items-center justify-between gap-2">
                                    <span className={`truncate ${sourceBudgetClass(source.truncated)}`} title={source.source}>
                                      {source.source}
                                    </span>
                                    <span className="shrink-0 font-mono text-[10px] text-text-muted">
                                      {source.provided}/{source.requested}
                                    </span>
                                  </div>
                                  <div className="mt-1 line-clamp-2 text-[10px] leading-snug text-text-secondary">
                                    {source.reason}
                                  </div>
                                  {source.truncationReason && (
                                    <div className="mt-1 line-clamp-2 text-[10px] leading-snug text-accent">
                                      {source.truncationReason}
                                    </div>
                                  )}
                                </div>
                              ))}
                            </div>
                          </>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
            {(ledger?.memoryAudit.length ?? 0) > 0 && (
              <div className="text-[10px] font-medium uppercase tracking-wide text-text-muted">
                Memory Audit
              </div>
            )}
            {ledger?.memoryAudit.map((entry) => (
              <div key={`${entry.proposalId}-${entry.createdAt}`} className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">{entry.title}</span>
                  <span className="text-[10px] text-text-muted">{entry.action}</span>
                </div>
                <div className="mt-1 text-[10px] text-text-muted">
                  {entry.kind} · {new Date(entry.createdAt).toLocaleString()}
                </div>
                {entry.evidence && (
                  <p className="mt-1 rounded bg-bg-deep border border-border-subtle p-1.5 text-text-secondary">
                    {entry.evidence}
                  </p>
                )}
                {entry.rationale && (
                  <p className="mt-1 text-text-muted">{entry.rationale}</p>
                )}
                {entry.reason && (
                  <p className="mt-1 text-text-secondary">Reason: {entry.reason}</p>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
