import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events } from "../protocol";
import type {
  WriterAgentStatus,
  WriterAgentLedgerSnapshot,
  AgentProposal,
  OperationResult,
  ProposalFeedback,
  StoryMode,
  StoryDebtSnapshot,
  StoryDebtEntry,
  StoryReviewQueueEntry,
  WriterOperation,
} from "../protocol";

interface CompanionPanelProps {
  mode: StoryMode;
  onApplyOperation?: (operation: WriterOperation) => boolean;
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

function mergeProposal(prev: AgentProposal[], incoming: AgentProposal): AgentProposal[] {
  const incomingSlot = proposalSlotKey(incoming);
  const existingIndex = prev.findIndex((proposal) => proposalSlotKey(proposal) === incomingSlot);
  if (existingIndex < 0) return [incoming, ...prev].slice(0, 20);

  const existing = prev[existingIndex];
  if (!shouldReplaceProposal(existing, incoming)) return prev;

  const next = prev.filter((_, index) => index !== existingIndex);
  return [incoming, ...next].slice(0, 20);
}

export const CompanionPanel: React.FC<CompanionPanelProps> = ({ mode, onApplyOperation }) => {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const currentChapterRevision = useAppStore((s) => s.currentChapterRevision);
  const agentMode = useAppStore((s) => s.agentMode);
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);

  const [status, setStatus] = useState<WriterAgentStatus | null>(null);
  const [ledger, setLedger] = useState<WriterAgentLedgerSnapshot | null>(null);
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [reviewQueue, setReviewQueue] = useState<StoryReviewQueueEntry[]>([]);
  const [storyDebt, setStoryDebt] = useState<StoryDebtSnapshot | null>(null);
  const [activeTab, setActiveTab] = useState<"status" | "queue" | "promises" | "canon" | "decisions" | "audit">("status");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [operationError, setOperationError] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const [nextStatus, nextLedger, nextProposals, nextReviewQueue, nextStoryDebt] = await Promise.all([
        invoke<WriterAgentStatus>(Commands.getWriterAgentStatus),
        invoke<WriterAgentLedgerSnapshot>(Commands.getWriterAgentLedger),
        invoke<AgentProposal[]>(Commands.getWriterAgentPendingProposals),
        invoke<StoryReviewQueueEntry[]>(Commands.getStoryReviewQueue),
        invoke<StoryDebtSnapshot>(Commands.getStoryDebtSnapshot),
      ]);
      setStatus(nextStatus);
      setLedger(nextLedger);
      setReviewQueue(nextReviewQueue);
      setStoryDebt(nextStoryDebt);
      setProposals((prev) => {
        const merged = nextProposals.reduce((acc, proposal) => mergeProposal(acc, proposal), prev);
        return merged.filter((proposal) =>
          nextProposals.some((pending) => pending.id === proposal.id)
        );
      });
    } catch {
      // kernel not initialized yet
    }
  }, []);

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
      });

      if (!result.success) {
        const message = result.error?.message ?? "Operation was rejected by the kernel.";
        setOperationError(message);
        if (result.error?.code === "conflict") {
          await recordFeedback(proposal.id, "snoozed", undefined, message);
        }
        return;
      }

      const applied = isEditorTextOperation(operation)
        ? onApplyOperation?.(operation) ?? false
        : true;
      if (!applied) {
        setOperationError("The editor could not apply this operation.");
        return;
      }

      const finalText = isEditorTextOperation(operation) ? operation.text : proposal.preview;
      await recordFeedback(proposal.id, "accepted", finalText);
    } catch (e) {
      setOperationError(String(e));
    }
  }, [currentChapterRevision, onApplyOperation, recordFeedback]);

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
      });

      if (!result.success) {
        const message = result.error?.message ?? "Operation was rejected by the kernel.";
        setOperationError(message);
        if (result.error?.code === "conflict") {
          await recordFeedback(entry.proposalId, "snoozed", undefined, message);
        }
        return;
      }

      const applied = isEditorTextOperation(operation)
        ? onApplyOperation?.(operation) ?? false
        : true;
      if (!applied) {
        setOperationError("The editor could not apply this operation.");
        return;
      }

      const finalText = isEditorTextOperation(operation) ? operation.text : entry.message;
      await recordFeedback(entry.proposalId, "accepted", finalText);
    } catch (e) {
      setOperationError(String(e));
    }
  }, [currentChapterRevision, onApplyOperation, recordFeedback]);

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
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "Operation was rejected by the kernel.");
        return;
      }

      const applied = isEditorTextOperation(operation)
        ? onApplyOperation?.(operation) ?? false
        : true;
      if (!applied) {
        setOperationError("The editor could not apply this operation.");
        return;
      }

      const finalText = isEditorTextOperation(operation) ? operation.text : entry.message;
      await recordFeedback(entry.proposalId, "accepted", finalText, feedbackReason);
    } catch (e) {
      setOperationError(String(e));
    }
  }, [currentChapterRevision, onApplyOperation, recordFeedback]);

  const handlePromiseLedgerOperation = useCallback(async (operation: WriterOperation) => {
    setOperationError(null);
    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision: currentChapterRevision ?? "",
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
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "Could not apply this story debt action.");
        return;
      }

      const applied = isEditorTextOperation(operation)
        ? onApplyOperation?.(operation) ?? false
        : true;
      if (!applied) {
        setOperationError("The editor could not apply this operation.");
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
  }, [currentChapterRevision, onApplyOperation, recordFeedback, refreshStatus]);


  const handleIgnoreDebtEntry = useCallback(async (entry: StoryDebtEntry) => {
    const proposalId = entry.relatedReviewIds[0]?.replace(/^review_/, "");
    if (proposalId) {
      await recordFeedback(proposalId, "rejected", undefined, "Ignored from story debt summary.");
    }
  }, [recordFeedback]);

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
  const availableTabs =
    mode === "write"
      ? (["status", "promises", "canon"] as const)
      : mode === "review"
        ? (["queue", "promises", "canon", "decisions", "audit"] as const)
        : (["status", "promises", "canon", "decisions", "audit"] as const);
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
            {mode === "write" ? "Story Guard" : mode === "review" ? "Story Review" : "Explore Context"}
          </span>
          <span className={`w-2 h-2 rounded-full ${
            isAgentThinking ? "bg-accent animate-pulse" : "bg-success"
          }`} />
        </div>
        {status && (
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

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-3 space-y-2">
        {effectiveTab === "status" && (
          <div className="space-y-3">
            {mode !== "write" && agentMode !== "proactive" && (
              <div className="p-3 rounded bg-accent-subtle/30 border border-accent/20 text-xs text-text-secondary">
                Agent is in {agentMode} mode. Switch to Proactive for ambient suggestions.
              </div>
            )}
            {mode === "write" && (
              <div className="p-2 rounded bg-bg-raised border border-border-subtle text-xs text-text-muted">
                Write mode keeps the agent quiet. Only urgent story-truth issues surface here.
              </div>
            )}
            {operationError && (
              <div className="p-2 rounded bg-danger/10 border border-danger/30 text-xs text-danger">
                {operationError}
              </div>
            )}
            <div className="text-xs text-text-muted">
              <div className="mb-2 text-text-secondary font-medium">Active Scene</div>
              <div className="p-2 rounded bg-bg-raised border border-border-subtle">
                {currentChapter || "No chapter loaded"}
              </div>
            </div>
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
                    {p.rationale && (
                      <p className="text-text-secondary italic mb-1">{p.rationale}</p>
                    )}
                    {p.evidence.length > 0 && (
                      <div className="mb-2 space-y-1">
                        {p.evidence.map((e, i) => (
                          <div key={i} className="p-1.5 rounded bg-bg-deep border border-border-subtle">
                            <span className="text-[10px] text-text-muted">{e.source}: </span>
                            <span className="text-[10px] text-text-secondary">{e.snippet}</span>
                          </div>
                        ))}
                      </div>
                    )}
                    {primaryOperation(p) && (
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
                <div className="mt-2 grid grid-cols-4 gap-1 text-center">
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-text-primary">{storyDebt.openCount}</div>
                    <div className="text-[10px] text-text-muted">open</div>
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
            {(ledger?.canonEntities.length ?? 0) === 0 && (
              <p className="text-text-muted">No canon entities recorded yet.</p>
            )}
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
            {(ledger?.memoryAudit.length ?? 0) === 0 && (
              <p className="text-text-muted">No memory audit events yet.</p>
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
