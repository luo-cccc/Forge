import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events, type SprintProgress } from "../protocol";
import {
  canonUpdateOperation,
  debtPrimaryOperation,
  diagnosticSeverityClass,
  formatBytes,
  isEditorTextOperation,
  isEnhancedGhost,
  mergeProposal,
  nextChapterLabel,
  operationApproval,
  operationLabel,
  primaryOperation,
  queuePrimaryOperation,
  severityBadgeClass,
  severityClass,
  storageStatusClass,
} from "./CompanionPanel.proposal";
import {
  chapterMissionDraftFromSummary,
  emptyChapterMissionDraft,
  emptyStoryContractDraft,
  hasChapterMissionContent,
  hasStoryContractContent,
  storyContractDraftFromSummary,
  validateChapterMissionStatusExplanation,
  type ChapterMissionDraft,
  type StoryContractDraft,
} from "./CompanionPanel.contract";
import {
  contextBudgetTone,
  formatContextBudgetDetail,
  formatContextBudgetValue,
  latestContextProposal,
  memoryReliabilityPercent,
  memoryReliabilityTone,
  postWriteReportCounts,
  postWriteReportLabel,
  postWriteReportTone,
  secondBrainToneClass,
  secondBrainValueClass,
  sourceBudgetClass,
} from "./CompanionPanel.brain";
import type {
  WriterAgentStatus,
  WriterAgentLedgerSnapshot,
  AgentProposal,
  BackupTarget,
  FileBackupInfo,
  OperationResult,
  ProposalFeedback,
  ProjectStorageDiagnostics,
  StoryMode,
  StoryDebtSnapshot,
  StoryDebtEntry,
  StoryReviewQueueEntry,
  TodayFiveSummary,
  WriterAgentTraceSnapshot,
  WriterOperation,
} from "../protocol";

interface CompanionPanelProps {
  mode: StoryMode;
  onApplyOperation?: (operation: WriterOperation, proposalId?: string) => Promise<ApplyOperationResult>;
}

interface ApplyOperationResult {
  applied: boolean;
  saved: boolean;
  revision?: string;
  savedContent?: string;
  chapterTitle?: string;
  error?: string;
}

type CompanionTone = "neutral" | "⚠️ 需要注意" | "📝 提个醒" | "✅ 一切正常";

function readinessText(tone: string | undefined): { text: string; color: string } {
  if (tone?.includes("需要注意")) {
    return { text: "需要注意", color: "text-warning" };
  }
  if (tone?.includes("提个醒")) {
    return { text: "可继续，有提示", color: "text-success" };
  }
  return { text: "可以继续写", color: "text-success" };
}

export const CompanionPanel: React.FC<CompanionPanelProps> = ({ mode, onApplyOperation }) => {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const currentChapterRevision = useAppStore((s) => s.currentChapterRevision);
  const agentMode = useAppStore((s) => s.agentMode);
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);
  const sprintProgress = useAppStore((s) => s.sprintProgress);
  const setSprintProgress = useAppStore((s) => s.setSprintProgress);

  const [status, setStatus] = useState<WriterAgentStatus | null>(null);
  const [ledger, setLedger] = useState<WriterAgentLedgerSnapshot | null>(null);
  const [storageDiagnostics, setStorageDiagnostics] = useState<ProjectStorageDiagnostics | null>(null);
  const [chapterBackups, setChapterBackups] = useState<FileBackupInfo[]>([]);
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [reviewQueue, setReviewQueue] = useState<StoryReviewQueueEntry[]>([]);
  const [storyDebt, setStoryDebt] = useState<StoryDebtSnapshot | null>(null);
  const [todayFiveSummary, setTodayFiveSummary] = useState<TodayFiveSummary | null>(null);
  const [trace, setTrace] = useState<WriterAgentTraceSnapshot | null>(null);
  const [activeTab, setActiveTab] = useState<"status" | "foundation" | "queue" | "promises" | "canon" | "decisions" | "audit">("status");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [operationError, setOperationError] = useState<string | null>(null);
  const [contractDraft, setContractDraft] = useState<StoryContractDraft>(() => emptyStoryContractDraft());
  const [missionDraft, setMissionDraft] = useState<ChapterMissionDraft>(() => emptyChapterMissionDraft());
  const [foundationSaveState, setFoundationSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [foundationDirty, setFoundationDirty] = useState(false);
  const [saveFeedback, setSaveFeedback] = useState<string | null>(null);
  const [showAllPromises, setShowAllPromises] = useState(false);
  const foundationChapterRef = useRef(currentChapter);

  const refreshStatus = useCallback(async () => {
    try {
      const [nextReviewQueue, nextTodayFiveSummary, nextTrace] = await Promise.all([
        invoke<StoryReviewQueueEntry[]>(Commands.getStoryReviewQueue),
        invoke<TodayFiveSummary>(Commands.getWriterAgentTodayFive),
        invoke<WriterAgentTraceSnapshot>(Commands.getWriterAgentTrace, { limit: 24 }),
      ]);
      invoke<SprintProgress | null>(Commands.getSupervisedSprintProgress)
        .then((progress) => setSprintProgress(progress))
        .catch(() => setSprintProgress(null));
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
      setReviewQueue(nextReviewQueue);
      setTodayFiveSummary(nextTodayFiveSummary);
      setTrace(nextTrace);
      if (mode !== "write") {
        const [nextStatus, nextLedger, nextProposals, nextStoryDebt] = await Promise.all([
          invoke<WriterAgentStatus>(Commands.getWriterAgentStatus),
          invoke<WriterAgentLedgerSnapshot>(Commands.getWriterAgentLedger),
          invoke<AgentProposal[]>(Commands.getWriterAgentPendingProposals),
          invoke<StoryDebtSnapshot>(Commands.getStoryDebtSnapshot),
        ]);
        setStatus(nextStatus);
        setLedger(nextLedger);
        setStoryDebt(nextStoryDebt);
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
      }
    } catch {
      // kernel not initialized yet
    }
  }, [currentChapter, foundationDirty, mode, setSprintProgress]);

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

    const result = await onApplyOperation?.(operation, proposalId);
    if (!result?.applied) {
      setOperationError(result?.error ?? "编辑器无法应用这次操作。");
      return false;
    }

    if (!result.saved) {
      setOperationError(result.error ?? "编辑器已应用这次修改，但保存失败，反馈不会写入记录。");
      return false;
    }

    return true;
  }, [onApplyOperation]);

  useEffect(() => {
    // Listen for new proposals from the kernel
    const fn = listen<AgentProposal>(Events.agentProposal, (event) => {
      setProposals((prev) => mergeProposal(prev, event.payload));
    });
    return () => { fn.then((f) => f()); };
  }, []);

  useEffect(() => {
    if (!saveFeedback) return;
    const id = setTimeout(() => setSaveFeedback(null), 4000);
    return () => clearTimeout(id);
  }, [saveFeedback]);

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
      if (mode !== "write") {
        const nextLedger = await invoke<WriterAgentLedgerSnapshot>(Commands.getWriterAgentLedger);
        setLedger(nextLedger);
        const nextStoryDebt = await invoke<StoryDebtSnapshot>(Commands.getStoryDebtSnapshot);
        setStoryDebt(nextStoryDebt);
      }
    } catch (e) {
      console.error("Proposal feedback failed:", e);
    }
  }, [mode, nowMs]);

  const handleFeedback = useCallback(async (proposalId: string, action: ProposalFeedback["action"]) => {
    await recordFeedback(proposalId, action);
  }, [recordFeedback]);

  const handleApplyProposal = useCallback(async (proposal: AgentProposal) => {
    setOperationError(null);
    const operation = primaryOperation(proposal);
    if (!operation) {
      await recordFeedback(proposal.id, "accepted", proposal.preview, "作者接受了无可执行操作的建议。");
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
          nowMs,
        ),
      });

      if (!result.success) {
        const message = result.error?.message ?? "内核拒绝了这次操作。";
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
  }, [applyApprovedOperation, currentChapterRevision, nowMs, recordFeedback]);

  const handleApplyQueueEntry = useCallback(async (entry: StoryReviewQueueEntry) => {
    setOperationError(null);
    const operation = queuePrimaryOperation(entry);
    if (!operation) {
      await recordFeedback(entry.proposalId, "accepted", entry.message, "作者接受了无可执行操作的队列项。");
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
          nowMs,
        ),
      });

      if (!result.success) {
        const message = result.error?.message ?? "内核拒绝了这次操作。";
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
  }, [applyApprovedOperation, currentChapterRevision, nowMs, recordFeedback]);

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
        approval: operationApproval("story_review_queue", feedbackReason, entry.proposalId, nowMs),
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "内核拒绝了这次操作。");
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
  }, [applyApprovedOperation, currentChapterRevision, nowMs, recordFeedback]);

  const handlePromiseLedgerOperation = useCallback(async (operation: WriterOperation) => {
    setOperationError(null);
    try {
      const result = await invoke<OperationResult>(Commands.approveWriterOperation, {
        operation,
        currentRevision: currentChapterRevision ?? "",
        approval: operationApproval(
          "promise_ledger",
          `Author updated promise ledger: ${operation.kind}`,
          undefined,
          nowMs,
        ),
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "无法更新这个伏笔。");
        return;
      }
      await refreshStatus();
    } catch (e) {
      setOperationError(String(e));
    }
  }, [currentChapterRevision, nowMs, refreshStatus]);

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
        approval: operationApproval("story_debt", feedbackReason, entry.relatedReviewIds[0], nowMs),
      });
      if (!result.success) {
        setOperationError(result.error?.message ?? "无法应用这个故事债务动作。");
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
  }, [applyApprovedOperation, currentChapterRevision, nowMs, recordFeedback, refreshStatus]);


  const handleIgnoreDebtEntry = useCallback(async (entry: StoryDebtEntry) => {
    const proposalId = entry.relatedReviewIds[0]?.replace(/^review_/, "");
    if (proposalId) {
      await recordFeedback(proposalId, "rejected", undefined, "已从故事债务摘要中忽略。");
    }
  }, [recordFeedback]);

  const handleSaveFoundation = useCallback(async () => {
    const saveStart = performance.now();
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
      const statusExplanationError = validateChapterMissionStatusExplanation(missionDraft);
      if (statusExplanationError) {
        setFoundationSaveState("error");
        setOperationError(statusExplanationError);
        return;
      }

      operations.push({
        kind: "chapter_mission.upsert",
        mission: {
          projectId,
          chapterTitle: currentChapter,
          mission: missionDraft.mission.trim(),
          mustInclude: missionDraft.mustInclude.trim(),
          mustNot: missionDraft.mustNot.trim(),
          expectedEnding: missionDraft.expectedEnding.trim(),
          status: missionDraft.status.trim() || "active",
          sourceRef: missionDraft.sourceRef.trim() || "author",
          blockedReason: missionDraft.blockedReason.trim(),
          retiredHistory: missionDraft.retiredHistory.trim(),
        },
      });
    }

    if (operations.length === 0) {
      setFoundationSaveState("error");
      setOperationError("保存前至少填写一个故事契约或本章任务字段。");
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
            undefined,
            nowMs,
          ),
        });
        if (!result.success) {
          throw new Error(result.error?.message ?? "内核拒绝了地基保存操作。");
        }
      }
      setFoundationDirty(false);
      setFoundationSaveState("saved");
      const elapsed = Math.round(performance.now() - saveStart);
      setSaveFeedback(`已保存 · ${todayFiveSummary?.items[3]?.detail || '线索已更新'} · <${elapsed}ms`);
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
    nowMs,
    refreshStatus,
    status?.projectId,
    todayFiveSummary,
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
  const riskOrder: Record<string, number> = { high: 0, medium: 1, low: 2 };
  const promiseStatusBoost = (promise: { core: boolean; promoted: boolean; blockedReason: string }) =>
    promise.core ? -3 : promise.blockedReason ? -2 : promise.promoted ? -1 : 0;
  const rankedPromises = [...(ledger?.openPromises ?? [])].sort(
    (a, b) =>
      promiseStatusBoost(a) - promiseStatusBoost(b)
      || (riskOrder[a.risk] ?? 3) - (riskOrder[b.risk] ?? 3)
      || b.priority - a.priority,
  );
  const secondBrainItems = todayFiveSummary?.items ?? [];
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
    <div className="flex h-full flex-col bg-bg-surface">
      {/* Header */}
      <div className="border-b border-border-subtle px-4 py-3">
        <div className="mb-2 flex items-center justify-between">
          <span className="font-display text-sm font-medium tracking-wide text-text-primary">
            {mode === "write" ? "写作助手" : mode === "review" ? "审稿队列" : "助手证据"}
          </span>
          <span className={`w-2 h-2 rounded-full ${
            isAgentThinking ? "bg-accent animate-pulse" : "bg-success"
          }`} />
        </div>
        <div className="mb-2 text-[10px] text-text-muted">
          {isAgentThinking ? "正在生成" : "待命"}
        </div>
        {todayFiveSummary && (() => {
          const guardTone = todayFiveSummary.items[0]?.tone;
          const readiness = readinessText(guardTone);
          return (
            <div className={`mb-2 text-xs ${readiness.color}`}>
              {readiness.text}
            </div>
          );
        })()}
        {status && mode !== "write" && (
          <div className="grid grid-cols-2 gap-2 text-xs text-text-muted">
            <div>
              <span className="block text-text-secondary">观察</span>
              <span className="font-mono">{status.observationCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">建议</span>
              <span className="font-mono">{status.proposalCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">未回收伏笔</span>
              <span className="font-mono text-accent">{status.openPromiseCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">反馈</span>
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
                当前助手模式为 {agentMode}。切到主动模式后才会给出环境建议。
              </div>
            )}
            {operationError && (
              <div className="p-2 rounded bg-danger/10 border border-danger/30 text-xs text-danger">
                {operationError}
              </div>
            )}
            {sprintProgress && (
              <div className="rounded border border-accent/20 bg-accent-subtle/20 p-2 text-xs">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-secondary">连续写作</span>
                  <span className="text-[10px] text-accent">{sprintProgress.status}</span>
                </div>
                <div className="text-text-primary">
                  {sprintProgress.chaptersCompleted}/
                  {sprintProgress.chaptersCompleted + sprintProgress.chaptersRemaining} 章
                </div>
                <div className="mt-1 text-[10px] text-text-muted">
                  检查点 {sprintProgress.checkpointCount} · 预算 {sprintProgress.spentBudgetMicros}
                  {sprintProgress.budgetCeilingMicros ? ` / ${sprintProgress.budgetCeilingMicros}` : ""}
                </div>
              </div>
            )}
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2 text-xs">
                <span className="font-medium text-text-secondary">正在守护的内容</span>
                {mode !== "write" && (
                  <span className="text-[10px] text-text-muted">
                    {storyDebt?.openCount ?? 0} 个待处理 · {ledger?.openPromises.length ?? 0} 个伏笔
                  </span>
                )}
              </div>
              <div className="grid grid-cols-1 gap-2 2xl:grid-cols-2">
                {secondBrainItems.map((item) => (
                  <div
                    key={item.slot}
                    className={`min-w-0 rounded border p-2 text-xs ${secondBrainToneClass(item.tone as CompanionTone)}`}
                  >
                    <div className="mb-1 text-[10px] uppercase tracking-wide text-text-muted">
                      {item.label}
                    </div>
                    <div className={`truncate font-medium ${secondBrainValueClass(item.tone as CompanionTone)}`} title={item.value}>
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
            {saveFeedback && (
              <div className="rounded border border-success/30 bg-success/10 p-2 text-xs text-success animate-pulse">
                {saveFeedback}
              </div>
            )}
            {mode !== "write" && mode === "explore" && (
              <div className={`rounded border p-2 text-xs ${secondBrainToneClass(contextBudgetTone(trace))}`}>
                {(() => {
                  const latestContextTrace = latestContextProposal(trace);
                  const latestContextBudget = latestContextTrace?.contextBudget;
                  return (
                    <>
                      <div className="mb-1 flex items-center justify-between gap-2">
                        <span className="text-[10px] uppercase tracking-wide text-text-muted">
                          <span className="sr-only">Evidence Trace</span>
                          证据轨迹
                        </span>
                        <span className="font-mono text-[10px] text-text-muted">
                          {latestContextTrace?.kind ?? "空闲"}
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
            {mode === "explore" && (
              <div className="text-xs text-text-muted">
                <div className="mb-2 text-text-secondary font-medium">当前场景</div>
                <div className="p-2 rounded bg-bg-raised border border-border-subtle">
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate">{currentChapter || "未载入章节"}</span>
                    {chapterBackups.length > 0 && (
                      <button
                        onClick={handleRestoreLatestChapterBackup}
                        className="shrink-0 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-[10px] text-text-secondary hover:border-accent/40 hover:text-accent"
                        title={chapterBackups[0].filename}
                      >
                        恢复最近版本
                      </button>
                    )}
                  </div>
                  {chapterBackups.length > 0 && (
                    <div className="mt-1 text-[10px] text-text-muted">
                      {chapterBackups.length} 个最近备份 · 最新 {formatBytes(chapterBackups[0].bytes)}
                    </div>
                  )}
                </div>
              </div>
            )}
            {mode !== "write" && mode === "explore" && storageDiagnostics && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2 text-xs">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">
                    <span className="sr-only">Project Storage</span>
                    项目存储
                  </span>
                  <span className={storageDiagnostics.healthy ? "text-success" : "text-danger"}>
                    {storageDiagnostics.healthy ? "正常" : "需要处理"}
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
            {mode !== "write" && visibleProposals.length > 0 && (
              <div>
                <div className="text-xs text-text-secondary font-medium mb-2">
                  待处理建议（{visibleProposals.length}）
                </div>
                {visibleProposals.slice(0, 5).map((p) => (
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
                            增强
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
                        {p.alternatives.length > 1 ? ` · ${p.alternatives.length} 个分支` : ""}
                      </div>
                    )}
                    <div className="flex gap-1">
                      <button
                        onClick={() => handleApplyProposal(p)}
                        className="px-2 py-1 text-[10px] rounded bg-accent-subtle text-accent border border-accent/40 hover:bg-accent/20"
                      >
                        应用
                      </button>
                      <button
                        onClick={() => handleFeedback(p.id, "rejected")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                      >
                        拒绝
                      </button>
                      <button
                        onClick={() => handleFeedback(p.id, "snoozed")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                      >
                        稍后
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
                  <div className="font-medium text-text-primary">故事契约</div>
                  <div className="text-[10px] text-text-muted">所有助手动作都必须遵守的全书约束。</div>
                </div>
                <span className="shrink-0 text-[10px] text-text-muted">
                  {ledger?.storyContract?.updatedAt ? "已保存" : "草稿"}
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
                  placeholder="作品标题"
                />
                <div className="grid grid-cols-2 gap-2">
                  <input
                    value={contractDraft.genre}
                    onChange={(event) => {
                      setFoundationDirty(true);
                      setContractDraft((prev) => ({ ...prev, genre: event.target.value }));
                    }}
                    className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                    placeholder="类型"
                  />
                  <input
                    value={contractDraft.targetReader}
                    onChange={(event) => {
                      setFoundationDirty(true);
                      setContractDraft((prev) => ({ ...prev, targetReader: event.target.value }));
                    }}
                    className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                    placeholder="目标读者"
                  />
                </div>
                <textarea
                  value={contractDraft.readerPromise}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, readerPromise: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="读者承诺"
                />
                <textarea
                  value={contractDraft.first30ChapterPromise}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, first30ChapterPromise: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="前 30 章承诺"
                />
                <textarea
                  value={contractDraft.mainConflict}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, mainConflict: event.target.value }));
                  }}
                  className="min-h-16 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="主冲突"
                />
                <textarea
                  value={contractDraft.structuralBoundary}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, structuralBoundary: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="边界 / 禁止动作"
                />
                <textarea
                  value={contractDraft.toneContract}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setContractDraft((prev) => ({ ...prev, toneContract: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="基调与风格底线"
                />
              </div>
            </div>

            <div className="rounded border border-border-subtle bg-bg-raised p-3">
              <div className="mb-2 flex items-center justify-between gap-2">
                <div>
                  <div className="font-medium text-text-primary">本章任务</div>
                  <div className="text-[10px] text-text-muted">{currentChapter || "未选择章节"}</div>
                </div>
                <select
                  value={missionDraft.status}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, status: event.target.value }));
                  }}
                  className="rounded border border-border-subtle bg-bg-deep px-2 py-1 text-[10px] text-text-secondary outline-none focus:border-accent"
                >
                  <option value="draft">草稿</option>
                  <option value="active">进行中</option>
                  <option value="needs_review">待复核</option>
                  <option value="completed">已完成</option>
                  <option value="drifted">已偏移</option>
                  <option value="blocked">受阻</option>
                  <option value="retired">已归档</option>
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
                  placeholder="这一章必须完成什么"
                />
                <textarea
                  value={missionDraft.mustInclude}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, mustInclude: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="必须包含"
                />
                <textarea
                  value={missionDraft.mustNot}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, mustNot: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="不能揭示或不能做"
                />
                <textarea
                  value={missionDraft.expectedEnding}
                  onChange={(event) => {
                    setFoundationDirty(true);
                    setMissionDraft((prev) => ({ ...prev, expectedEnding: event.target.value }));
                  }}
                  className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                  placeholder="预期结尾状态"
                />
                {missionDraft.status === "blocked" && (
                  <textarea
                    value={missionDraft.blockedReason}
                    onChange={(event) => {
                      setFoundationDirty(true);
                      setMissionDraft((prev) => ({ ...prev, blockedReason: event.target.value }));
                    }}
                    className="min-h-14 rounded border border-warning/30 bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-warning"
                    placeholder="为什么受阻？这会提供给助手。"
                  />
                )}
                {missionDraft.status === "retired" && (
                  <textarea
                    value={missionDraft.retiredHistory}
                    onChange={(event) => {
                      setFoundationDirty(true);
                      setMissionDraft((prev) => ({ ...prev, retiredHistory: event.target.value }));
                    }}
                    className="min-h-14 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-text-primary outline-none focus:border-accent"
                    placeholder="为什么归档？这会保留在历史里。"
                  />
                )}
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
                  ? "正在保存地基..."
                  : foundationSaveState === "saved"
                    ? "地基已保存"
                    : foundationSaveState === "error"
                      ? "地基保存失败"
                      : foundationDirty
                        ? "地基有未保存修改"
                        : "地基已同步"}
              </span>
              <button
                onClick={handleSaveFoundation}
                disabled={foundationSaveState === "saving"}
                className="rounded bg-accent px-3 py-1 text-xs text-bg-deep transition-colors hover:bg-accent/80 disabled:opacity-60"
              >
                保存地基
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
                  <span className="font-medium text-text-primary">故事债务</span>
                  <span className="text-[10px] text-text-muted">
                    {storyDebt.chapterTitle || currentChapter || "项目"}
                  </span>
                </div>
                <div className="mt-2 grid grid-cols-6 gap-1 text-center">
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-text-primary">{storyDebt.openCount}</div>
                    <div className="text-[10px] text-text-muted">待处理</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-danger">{storyDebt.contractCount}</div>
                    <div className="text-[10px] text-text-muted">契约</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-danger">{storyDebt.missionCount}</div>
                    <div className="text-[10px] text-text-muted">任务</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-danger">{storyDebt.canonRiskCount}</div>
                    <div className="text-[10px] text-text-muted">设定</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-accent">{storyDebt.promiseCount}</div>
                    <div className="text-[10px] text-text-muted">伏笔</div>
                  </div>
                  <div className="rounded bg-bg-deep p-1">
                    <div className="font-mono text-text-secondary">{storyDebt.pacingCount}</div>
                    <div className="text-[10px] text-text-muted">节奏</div>
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
                              更新设定
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
                              忽略
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
                暂无待处理审稿项。
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
                      {operation ? operationLabel(operation) : "应用"}
                    </button>
                    <button
                      onClick={() => handleFeedback(entry.proposalId, "rejected")}
                      className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                    >
                      忽略
                    </button>
                    <button
                      onClick={() => handleFeedback(entry.proposalId, "snoozed")}
                      className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                    >
                      稍后
                    </button>
                    {canonOperation && (
                      <button
                        onClick={() => handleApplyQueueOperation(entry, canonOperation, "Updated canon instead of changing text.")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-deep text-text-secondary border border-border-subtle hover:text-accent hover:border-accent/40"
                      >
                        更新设定
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
              <p className="text-text-muted">暂无待兑现伏笔。</p>
            )}
            {(showAllPromises ? rankedPromises : rankedPromises.slice(0, 3)).map((promise) => {
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
              const riskBadge =
                promise.core
                  ? "bg-danger/25 text-danger"
                  : promise.promoted
                    ? "bg-accent-subtle text-accent"
                    : promise.risk === "high"
                  ? "bg-danger/20 text-danger"
                  : promise.risk === "medium"
                    ? "bg-warning/20 text-warning"
                    : "bg-bg-deep text-text-muted";
              return (
                <div key={promise.id} className="rounded bg-bg-raised border border-border-subtle p-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-text-primary">{promise.title}</span>
                    <span className={`text-[10px] px-1 rounded ${riskBadge}`}>
                      {promise.core ? "core" : promise.promoted ? "promoted" : promise.blockedReason ? "blocked" : promise.risk}
                    </span>
                  </div>
                  <p className="mt-1 text-text-secondary">{promise.description}</p>
                  <div className="mt-2 text-[10px] text-text-muted">
                    {promise.introducedChapter || "unknown"} {"->"} {promise.expectedPayoff || "unset"}
                  </div>
                  {promise.blockedReason && (
                    <div className="mt-1 text-[10px] text-warning">
                      受阻：{promise.blockedReason}
                    </div>
                  )}
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
            {rankedPromises.length > 3 && (
              <button
                onClick={() => setShowAllPromises(!showAllPromises)}
                className="w-full py-1.5 text-[10px] text-text-muted hover:text-accent border border-dashed border-border-subtle rounded"
              >
                {showAllPromises
                  ? "只显示前 3 项"
                  : `显示全部 ${rankedPromises.length} 个伏笔`}
              </button>
            )}
          </div>
        )}

        {effectiveTab === "canon" && (
          <div className="space-y-2 text-xs">
            {(ledger?.canonEntities.length ?? 0) === 0 && (ledger?.canonRules.length ?? 0) === 0 && (
              <p className="text-text-muted">暂无设定实体或规则。</p>
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
              <p className="text-text-muted">暂无创作决策记录。</p>
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
              (trace?.postWriteDiagnostics.length ?? 0) === 0 &&
              (ledger?.memoryReliability.length ?? 0) === 0 &&
              (ledger?.memoryAudit.length ?? 0) === 0 &&
              (trace?.contextRecalls.length ?? ledger?.contextRecalls.length ?? 0) === 0 && (
              <p className="text-text-muted">暂无助手审计事件。</p>
            )}
            {(ledger?.memoryReliability.length ?? 0) > 0 && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">记忆可靠度</span>
                  <span className="text-[10px] text-text-muted">
                    {ledger?.memoryReliability.length ?? 0} slots
                  </span>
                </div>
                <div className="space-y-1.5">
                  {ledger?.memoryReliability.slice(0, 6).map((item) => (
                    <div
                      key={item.slot}
                      className={`rounded border p-2 ${secondBrainToneClass(memoryReliabilityTone(item.status))}`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-text-secondary" title={item.slot}>
                          {item.category} · {item.slot.split("|").slice(-2).join(" · ")}
                        </span>
                        <span className={`shrink-0 font-mono text-[10px] ${secondBrainValueClass(memoryReliabilityTone(item.status))}`}>
                          {memoryReliabilityPercent(item.reliability)}
                        </span>
                      </div>
                      <div className="mt-1 flex flex-wrap gap-1">
                        <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                          {item.status}
                        </span>
                        <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                          +{item.reinforcementCount} / -{item.correctionCount}
                        </span>
                        <span className="rounded bg-bg-surface px-1.5 py-0.5 text-[10px] text-text-muted">
                          delta {item.netConfidenceDelta.toFixed(2)}
                        </span>
                      </div>
                      {(item.lastSourceError || item.lastReason) && (
                        <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-text-secondary">
                          {item.lastSourceError || item.lastReason}
                        </p>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
            {(trace?.postWriteDiagnostics.length ?? 0) > 0 && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">写后诊断</span>
                  <span className="text-[10px] text-text-muted">
                    {trace?.postWriteDiagnostics.length ?? 0} reports
                  </span>
                </div>
                <div className="space-y-1.5">
                  {trace?.postWriteDiagnostics.slice(0, 4).map((report) => (
                    <div
                      key={`${report.observationId}-${report.createdAtMs}`}
                      className={`rounded border p-2 ${secondBrainToneClass(postWriteReportTone(report))}`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-text-secondary" title={postWriteReportLabel(report)}>
                          {postWriteReportLabel(report)}
                        </span>
                        <span className="shrink-0 font-mono text-[10px] text-text-muted">
                          {postWriteReportCounts(report)}
                        </span>
                      </div>
                      {report.diagnostics.slice(0, 3).map((diagnostic) => (
                        <div key={diagnostic.diagnosticId} className="mt-2 rounded bg-bg-deep border border-border-subtle p-1.5">
                          <div className="flex items-center justify-between gap-2">
                            <span className="truncate text-text-secondary" title={diagnostic.message}>
                              {diagnostic.category}
                            </span>
                            <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] ${diagnosticSeverityClass(diagnostic.severity)}`}>
                              {diagnostic.severity}
                            </span>
                          </div>
                          <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-text-muted">
                            {diagnostic.message}
                          </p>
                          {diagnostic.fixSuggestion && (
                            <p className="mt-1 line-clamp-2 text-[10px] leading-snug text-accent">
                              {diagnostic.fixSuggestion}
                            </p>
                          )}
                        </div>
                      ))}
                      {report.sourceRefs.length > 0 && (
                        <div className="mt-2 flex flex-wrap gap-1">
                          {report.sourceRefs.slice(0, 5).map((sourceRef) => (
                            <span key={`${report.observationId}-${sourceRef}`} className="rounded bg-bg-deep px-1.5 py-0.5 text-[10px] text-text-muted">
                              {sourceRef}
                            </span>
                          ))}
                        </div>
                      )}
                      {report.remediation[0] && (
                        <p className="mt-2 line-clamp-2 text-[10px] leading-snug text-text-secondary">
                          {report.remediation[0]}
                        </p>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
            {(trace?.taskPackets.length ?? 0) > 0 && (
              <div className="rounded bg-bg-raised border border-border-subtle p-2">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">触发原因</span>
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
                  <span className="font-medium text-text-primary">上下文召回</span>
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
                  <span className="font-medium text-text-primary">上下文轨迹</span>
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
                记忆审计
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
                  <p className="mt-1 text-text-secondary">原因：{entry.reason}</p>
                )}
              </div>
            ))}
          </div>
        )}
        <div className="mt-4 border-t border-border-subtle pt-3 text-[10px] text-text-muted text-center">
          本地检索引擎: &lt;5ms · 上下文组装: &lt;5ms · 状态快照: &lt;1ms
        </div>
      </div>
    </div>
  );
};
