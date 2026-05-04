import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Commands } from "../protocol";
import type {
  WriterAgentTraceSnapshot,
  WriterProposalTrace,
  ContextBudgetTrace,
  WriterContextSourceTrend,
  WriterInspectorTimeline,
  WriterPostWriteDiagnosticReport,
  WriterTimelineEvent,
  WriterTimelineEventKind,
} from "../protocol";

type InspectorFilter =
  | "all"
  | "failure"
  | "save_completed"
  | "subtask"
  | "task_receipt"
  | "task_artifact"
  | "run_event"
  | "task_packet"
  | "operation_lifecycle"
  | "context_recall"
  | "product_metrics";

const filterLabels: Record<InspectorFilter, string> = {
  all: "All",
  failure: "Failures",
  save_completed: "Saves",
  subtask: "Subtasks",
  task_receipt: "Receipts",
  task_artifact: "Artifacts",
  run_event: "Run Events",
  task_packet: "Packets",
  operation_lifecycle: "Lifecycle",
  context_recall: "Context",
  product_metrics: "Metrics",
};

const filterOrder: InspectorFilter[] = [
  "all",
  "failure",
  "save_completed",
  "subtask",
  "task_receipt",
  "task_artifact",
  "run_event",
  "task_packet",
  "operation_lifecycle",
  "context_recall",
  "product_metrics",
];

function formatRate(value: number | undefined): string {
  if (value === undefined || Number.isNaN(value)) return "0%";
  return `${Math.round(value * 100)}%`;
}

function formatTime(tsMs: number): string {
  if (!tsMs) return "time n/a";
  return new Date(tsMs).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatCostMicros(value: number | undefined): string {
  if (value === undefined) return "-";
  return `$${(value / 1_000_000).toFixed(4)}`;
}

function formatDuration(value: number | null | undefined): string {
  if (value === undefined || value === null) return "n/a";
  if (value < 1000) return `${value}ms`;
  if (value < 60_000) return `${(value / 1000).toFixed(1)}s`;
  return `${Math.round(value / 60_000)}m`;
}

function eventToneClass(kind: WriterTimelineEventKind): string {
  if (kind === "failure") return "border-danger/40 bg-danger/10";
  if (kind === "subtask") return "border-accent/30 bg-accent-subtle/20";
  if (kind === "task_receipt") return "border-success/30 bg-success/10";
  if (kind === "task_artifact") return "border-success/30 bg-bg-raised";
  if (kind === "run_event") return "border-accent/30 bg-accent-subtle/20";
  if (kind === "product_metrics") return "border-success/30 bg-success/10";
  return "border-border-subtle bg-bg-raised";
}

function eventBadgeClass(kind: WriterTimelineEventKind): string {
  if (kind === "failure") return "bg-danger/20 text-danger";
  if (kind === "subtask") return "bg-accent-subtle text-accent";
  if (kind === "task_receipt") return "bg-success/10 text-success";
  if (kind === "task_artifact") return "bg-success/10 text-success";
  if (kind === "run_event") return "bg-accent-subtle text-accent";
  if (kind === "product_metrics") return "bg-success/10 text-success";
  return "bg-bg-deep text-text-muted";
}

function diagnosticToneClass(report: WriterPostWriteDiagnosticReport): string {
  if (report.errorCount > 0) return "border-danger/40 bg-danger/10";
  if (report.warningCount > 0) return "border-accent/30 bg-accent-subtle/20";
  return "border-success/30 bg-success/10";
}

function trendToneClass(trend: WriterContextSourceTrend): string {
  if (trend.droppedCount > 0) return "border-danger/40 bg-danger/10";
  if (trend.truncatedCount > 0) return "border-accent/30 bg-accent-subtle/20";
  return "border-border-subtle bg-bg-raised";
}

function detailRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function textField(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function numberField(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function stringArrayField(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0)
    : [];
}

function providerBudgetDetail(detail: unknown): {
  decision?: string;
  model?: string;
  estimatedTotalTokens?: number;
  estimatedCostMicros?: number;
  approvalRequired?: boolean;
  reasons: string[];
  remediation: string[];
} | null {
  const data = detailRecord(detail);
  if (!("providerBudget" in data) && !("estimatedTotalTokens" in data)) return null;
  return {
    decision: textField(data.decision),
    model: textField(data.model),
    estimatedTotalTokens: numberField(data.estimatedTotalTokens),
    estimatedCostMicros: numberField(data.estimatedCostMicros),
    approvalRequired: typeof data.approvalRequired === "boolean" ? data.approvalRequired : undefined,
    reasons: stringArrayField(data.reasons),
    remediation: stringArrayField(data.remediation),
  };
}

function failureDetail(detail: unknown): {
  code?: string;
  category?: string;
  recoverable?: boolean;
  remediation: string[];
  details: Record<string, unknown>;
} | null {
  const data = detailRecord(detail);
  if (!("code" in data) && !("category" in data) && !("remediation" in data)) return null;
  return {
    code: textField(data.code),
    category: textField(data.category),
    recoverable: typeof data.recoverable === "boolean" ? data.recoverable : undefined,
    remediation: stringArrayField(data.remediation),
    details: detailRecord(data.details),
  };
}

function recoveryActionsForFailure(failure: NonNullable<ReturnType<typeof failureDetail>>): Array<{
  label: string;
  filter: InspectorFilter;
}> {
  const code = (failure.code ?? "").toLowerCase();
  const category = (failure.category ?? "").toLowerCase();
  const remediation = failure.remediation.join(" ").toLowerCase();
  const detailsText = JSON.stringify(failure.details).toLowerCase();
  const actions: Array<{ label: string; filter: InspectorFilter }> = [];
  const add = (label: string, filter: InspectorFilter) => {
    if (!actions.some((action) => action.label === label && action.filter === filter)) {
      actions.push({ label, filter });
    }
  };

  if (code.includes("budget") || detailsText.includes("providerbudget")) {
    add("Review budget", "run_event");
  }
  if (code.includes("save") || category.includes("save")) {
    add("Review save", "save_completed");
  }
  if (code.includes("receipt") || category.includes("receipt")) {
    add("Review receipt", "task_receipt");
    add("Review task packet", "task_packet");
  }
  if (
    code.includes("tool") ||
    category.includes("tool") ||
    remediation.includes("tool") ||
    remediation.includes("provider")
  ) {
    add("Review run events", "run_event");
  }
  if (remediation.includes("context") || detailsText.includes("context")) {
    add("Review context", "context_recall");
  }
  add("Show failures", "failure");
  return actions.slice(0, 4);
}

function saveCompletedDetail(detail: unknown): {
  chapterTitle?: string;
  chapterRevision?: string;
  saveResult?: string;
  proposalId?: string;
  operationKind?: string;
  postWriteReportId?: string;
  diagnosticTotalCount?: number;
  diagnosticErrorCount?: number;
  diagnosticWarningCount?: number;
} | null {
  const data = detailRecord(detail);
  if (!("saveResult" in data) && !("postWriteReportId" in data)) return null;
  return {
    chapterTitle: textField(data.chapterTitle),
    chapterRevision: textField(data.chapterRevision),
    saveResult: textField(data.saveResult),
    proposalId: textField(data.proposalId),
    operationKind: textField(data.operationKind),
    postWriteReportId: textField(data.postWriteReportId),
    diagnosticTotalCount: numberField(data.diagnosticTotalCount),
    diagnosticErrorCount: numberField(data.diagnosticErrorCount),
    diagnosticWarningCount: numberField(data.diagnosticWarningCount),
  };
}

function taskReceiptDetail(detail: unknown): {
  taskKind?: string;
  chapter?: string;
  expectedArtifacts: string[];
  requiredEvidence: string[];
  mustNot: string[];
} | null {
  const data = detailRecord(detail);
  if (!("taskKind" in data) && !("expectedArtifacts" in data) && !("mustNot" in data)) return null;
  return {
    taskKind: textField(data.taskKind),
    chapter: textField(data.chapter),
    expectedArtifacts: stringArrayField(data.expectedArtifacts),
    requiredEvidence: stringArrayField(data.requiredEvidence),
    mustNot: stringArrayField(data.mustNot),
  };
}

function taskArtifactDetail(detail: unknown): {
  taskKind?: string;
  artifactKind?: string;
  chapter?: string;
  content?: string;
  contentCharCount?: number;
  contentTruncated?: boolean;
  requiredEvidence: string[];
} | null {
  const data = detailRecord(detail);
  if (!("artifactKind" in data) && !("contentCharCount" in data) && !("content" in data)) return null;
  return {
    taskKind: textField(data.taskKind),
    artifactKind: textField(data.artifactKind),
    chapter: textField(data.chapter),
    content: textField(data.content),
    contentCharCount: numberField(data.contentCharCount),
    contentTruncated: typeof data.contentTruncated === "boolean" ? data.contentTruncated : undefined,
    requiredEvidence: stringArrayField(data.requiredEvidence),
  };
}

function contextBudgetToneClass(report: ContextBudgetTrace): string {
  if (report.sourceReports.some((source) => source.provided === 0)) {
    return "border-danger/30 bg-bg-deep";
  }
  if (report.sourceReports.some((source) => source.truncated)) {
    return "border-accent/30 bg-bg-deep";
  }
  return "border-border-subtle bg-bg-deep";
}

function formatCompactNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}m`;
  if (value >= 10_000) return `${Math.round(value / 1_000)}k`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
  return String(value);
}

function trendCoverageRate(trend: WriterContextSourceTrend): number {
  if (trend.totalRequested <= 0) return 1;
  return trend.totalProvided / trend.totalRequested;
}

function contextPressureSummary(trends: WriterContextSourceTrend[]): {
  totalRequested: number;
  totalProvided: number;
  coverageRate: number;
  truncatedEvents: number;
  droppedEvents: number;
  truncatedSources: number;
  droppedSources: number;
  hottestSource?: WriterContextSourceTrend;
} {
  const summary = trends.reduce(
    (acc, trend) => {
      acc.totalRequested += trend.totalRequested;
      acc.totalProvided += trend.totalProvided;
      acc.truncatedEvents += trend.truncatedCount;
      acc.droppedEvents += trend.droppedCount;
      if (trend.truncatedCount > 0) acc.truncatedSources += 1;
      if (trend.droppedCount > 0) acc.droppedSources += 1;
      return acc;
    },
    {
      totalRequested: 0,
      totalProvided: 0,
      truncatedEvents: 0,
      droppedEvents: 0,
      truncatedSources: 0,
      droppedSources: 0,
    },
  );
  const hottestSource = [...trends].sort((left, right) => {
    const leftPressure = left.droppedCount * 4 + left.truncatedCount * 2 + (1 - trendCoverageRate(left));
    const rightPressure = right.droppedCount * 4 + right.truncatedCount * 2 + (1 - trendCoverageRate(right));
    return rightPressure - leftPressure || right.appearances - left.appearances || left.source.localeCompare(right.source);
  })[0];

  return {
    ...summary,
    coverageRate: summary.totalRequested > 0 ? summary.totalProvided / summary.totalRequested : 1,
    hottestSource,
  };
}

function eventSortValue(event: WriterTimelineEvent): number {
  return event.tsMs || 0;
}

function matchingFilter(event: WriterTimelineEvent, filter: InspectorFilter): boolean {
  if (filter === "all") return true;
  if (filter === "save_completed") {
    return event.kind === "run_event" && event.label === "writer.save_completed";
  }
  return event.kind === filter;
}

export const WriterInspectorPanel: React.FC = () => {
  const [timeline, setTimeline] = useState<WriterInspectorTimeline | null>(null);
  const [trace, setTrace] = useState<WriterAgentTraceSnapshot | null>(null);
  const [filter, setFilter] = useState<InspectorFilter>("all");
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number>(0);

  const refresh = useCallback(async () => {
    try {
      const [nextTimeline, nextTrace] = await Promise.all([
        invoke<WriterInspectorTimeline>(Commands.getWriterAgentInspectorTimeline, { limit: 120 }),
        invoke<WriterAgentTraceSnapshot>(Commands.getWriterAgentTrace, { limit: 80 }),
      ]);
      setTimeline(nextTimeline);
      setTrace(nextTrace);
      setLastUpdated(Date.now());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    const initial = setTimeout(refresh, 0);
    const interval = setInterval(refresh, 5000);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [refresh]);

  const events = useMemo(() => timeline?.events ?? [], [timeline?.events]);
  const filteredEvents = useMemo(
    () => events.filter((event) => matchingFilter(event, filter)),
    [events, filter],
  );
  const eventCounts = useMemo(() => {
    const counts = new Map<InspectorFilter, number>([["all", events.length]]);
    for (const event of events) {
      const key = event.kind as InspectorFilter;
      counts.set(key, (counts.get(key) ?? 0) + 1);
      if (event.kind === "run_event" && event.label === "writer.save_completed") {
        counts.set("save_completed", (counts.get("save_completed") ?? 0) + 1);
      }
    }
    return counts;
  }, [events]);
  const latestProviderBudget = events.find((event) =>
    event.kind === "run_event" && event.label === "writer.provider_budget"
  );
  const latestFailure = events.find((event) => event.kind === "failure");
  const latestSave = events.find((event) =>
    event.kind === "run_event" && event.label === "writer.save_completed"
  );
  const latestReceipt = events.find((event) => event.kind === "task_receipt");
  const latestArtifact = events.find((event) => event.kind === "task_artifact");
  const latestPostWrite = trace?.postWriteDiagnostics[0];
  const metrics = trace?.productMetrics;
  const trends = useMemo(() => trace?.contextSourceTrends ?? [], [trace?.contextSourceTrends]);
  const contextPressure = useMemo(() => contextPressureSummary(trends), [trends]);
  const providerBudget = providerBudgetDetail(latestProviderBudget?.detail);
  const latestFailureDetail = failureDetail(latestFailure?.detail);
  const latestSaveDetail = saveCompletedDetail(latestSave?.detail);
  const latestReceiptDetail = taskReceiptDetail(latestReceipt?.detail);
  const latestArtifactDetail = taskArtifactDetail(latestArtifact?.detail);
  const saveCompletedEvents = events.filter((event) =>
    event.kind === "run_event" && event.label === "writer.save_completed"
  );
  const metricsTrend = trace?.productMetricsTrend;
  const proposalById = useMemo(() => {
    const proposals = new Map<string, WriterProposalTrace>();
    for (const proposal of trace?.recentProposals ?? []) {
      proposals.set(proposal.id, proposal);
    }
    return proposals;
  }, [trace?.recentProposals]);

  return (
    <div className="flex h-full flex-col bg-bg-surface">
      <div className="border-b border-border-subtle px-4 py-3">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="font-display text-sm tracking-wider text-text-primary">
              Writer Inspector
            </div>
            <div className="mt-1 text-[10px] text-text-muted">
              Internal run timeline, failures, budgets, diagnostics, and context pressure.
            </div>
          </div>
          <button
            onClick={refresh}
            className="shrink-0 rounded border border-border-subtle bg-bg-deep px-2 py-1 text-xs text-text-secondary hover:border-accent/40 hover:text-accent"
          >
            Refresh
          </button>
        </div>
        <div className="mt-3 grid grid-cols-4 gap-2 text-xs">
          <div className="rounded border border-border-subtle bg-bg-deep p-2">
            <span className="block text-[10px] uppercase text-text-muted">Events</span>
            <span className="font-mono text-text-primary">{events.length}</span>
          </div>
          <div className="rounded border border-border-subtle bg-bg-deep p-2">
            <span className="block text-[10px] uppercase text-text-muted">Failures</span>
            <span className="font-mono text-danger">{eventCounts.get("failure") ?? 0}</span>
          </div>
          <div className="rounded border border-border-subtle bg-bg-deep p-2">
            <span className="block text-[10px] uppercase text-text-muted">Acceptance</span>
            <span className="font-mono text-success">
              {formatRate(metrics?.proposalAcceptanceRate)}
            </span>
          </div>
          <div className="rounded border border-border-subtle bg-bg-deep p-2">
            <span className="block text-[10px] uppercase text-text-muted">Updated</span>
            <span className="font-mono text-text-secondary">{formatTime(lastUpdated)}</span>
          </div>
        </div>
      </div>

      <div className="border-b border-border-subtle p-2">
        <div className="grid grid-cols-4 gap-1">
          {filterOrder.map((item) => (
            <button
              key={item}
              onClick={() => setFilter(item)}
              className={`rounded px-2 py-1 text-[11px] transition-colors ${
                filter === item
                  ? "bg-accent text-bg-deep"
                  : "bg-bg-deep text-text-muted hover:text-text-secondary"
              }`}
            >
              {filterLabels[item]} {item === "all" ? events.length : eventCounts.get(item) ?? 0}
            </button>
          ))}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[1.35fr_1fr] gap-0">
        <div className="min-h-0 overflow-y-auto border-r border-border-subtle p-3">
          {error && (
            <div className="mb-2 rounded border border-danger/40 bg-danger/10 p-2 text-xs text-danger">
              {error}
            </div>
          )}
          {filteredEvents.length === 0 && (
            <div className="rounded border border-border-subtle bg-bg-raised p-3 text-xs text-text-muted">
              No inspector events for this filter yet.
            </div>
          )}
          <div className="space-y-2">
            {[...filteredEvents]
              .sort((left, right) => eventSortValue(right) - eventSortValue(left))
              .map((event, index) => {
                const budget = providerBudgetDetail(event.detail);
                const failure = failureDetail(event.detail);
                const saveCompleted = saveCompletedDetail(event.detail);
                const taskReceipt = taskReceiptDetail(event.detail);
                const taskArtifact = taskArtifactDetail(event.detail);
                const proposalTrace = event.taskId ? proposalById.get(event.taskId) : undefined;
                const contextBudget = proposalTrace?.contextBudget;
                return (
                  <div
                    key={`${event.kind}-${event.taskId ?? "none"}-${event.tsMs}-${index}`}
                    className={`rounded border p-2 text-xs ${eventToneClass(event.kind)}`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="min-w-0 truncate font-medium text-text-primary" title={event.label}>
                        {event.label}
                      </span>
                      <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] ${eventBadgeClass(event.kind)}`}>
                        {event.kind}
                      </span>
                    </div>
                    <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-text-muted">
                      <span>{formatTime(event.tsMs)}</span>
                      {event.taskId && <span>task {event.taskId}</span>}
                      {event.sourceRefs.slice(0, 4).map((source) => (
                        <span key={`${event.label}-${event.tsMs}-${source}`} className="rounded bg-bg-deep px-1 py-0.5">
                          {source}
                        </span>
                      ))}
                    </div>
                    <p className="mt-2 line-clamp-3 text-text-secondary">{event.summary}</p>
                    {budget && (
                      <div className="mt-2 rounded border border-border-subtle bg-bg-deep p-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-text-secondary">
                            {budget.model ?? "provider"} · {budget.decision ?? "decision"}
                          </span>
                          <span className={budget.approvalRequired ? "text-danger" : "text-success"}>
                            {budget.approvalRequired ? "approval required" : "allowed"}
                          </span>
                        </div>
                        <div className="mt-1 font-mono text-[10px] text-text-muted">
                          {budget.estimatedTotalTokens ?? 0} tokens · {formatCostMicros(budget.estimatedCostMicros)}
                        </div>
                        {budget.reasons[0] && (
                          <p className="mt-1 line-clamp-2 text-[10px] text-accent">{budget.reasons[0]}</p>
                        )}
                      </div>
                    )}
                    {failure && (
                      <div className="mt-2 rounded border border-danger/30 bg-bg-deep p-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-danger">{failure.code ?? "failure"}</span>
                          <span className="text-[10px] text-text-muted">
                            {failure.category ?? "unknown"} · {failure.recoverable === false ? "blocked" : "recoverable"}
                          </span>
                        </div>
                        {failure.remediation[0] && (
                          <p className="mt-1 line-clamp-2 text-[10px] text-text-secondary">
                            {failure.remediation[0]}
                          </p>
                        )}
                        <div className="mt-2 flex flex-wrap gap-1">
                          {recoveryActionsForFailure(failure).map((action) => (
                            <button
                              key={`${event.taskId ?? event.tsMs}-${action.label}`}
                              onClick={() => setFilter(action.filter)}
                              className="rounded border border-danger/30 bg-danger/10 px-1.5 py-0.5 text-[10px] text-danger hover:bg-danger/20"
                            >
                              {action.label}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                    {saveCompleted && (
                      <div className="mt-2 rounded border border-success/30 bg-bg-deep p-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="truncate text-success">
                            {saveCompleted.saveResult ?? "save completed"}
                          </span>
                          <span className="text-[10px] text-text-muted">
                            {saveCompleted.diagnosticErrorCount ?? 0}e · {saveCompleted.diagnosticWarningCount ?? 0}w
                          </span>
                        </div>
                        <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-text-muted">
                          {saveCompleted.chapterTitle && <span>{saveCompleted.chapterTitle}</span>}
                          {saveCompleted.operationKind && <span>{saveCompleted.operationKind}</span>}
                          {saveCompleted.postWriteReportId && <span>report {saveCompleted.postWriteReportId}</span>}
                        </div>
                      </div>
                    )}
                    {taskReceipt && (
                      <div className="mt-2 rounded border border-success/30 bg-bg-deep p-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="truncate text-success">
                            {taskReceipt.taskKind ?? "task receipt"}
                          </span>
                          <span className="text-[10px] text-text-muted">
                            {taskReceipt.expectedArtifacts.length} artifacts · {taskReceipt.mustNot.length} guards
                          </span>
                        </div>
                        <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-text-muted">
                          {taskReceipt.chapter && <span>{taskReceipt.chapter}</span>}
                          {taskReceipt.requiredEvidence.slice(0, 4).map((source) => (
                            <span key={`${event.taskId}-${source}`} className="rounded bg-bg-surface px-1 py-0.5">
                              {source}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                    {taskArtifact && (
                      <div className="mt-2 rounded border border-success/30 bg-bg-deep p-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="truncate text-success">
                            {taskArtifact.taskKind ?? "task"} · {taskArtifact.artifactKind ?? "artifact"}
                          </span>
                          <span className="text-[10px] text-text-muted">
                            {taskArtifact.contentCharCount ?? 0} chars
                            {taskArtifact.contentTruncated ? " · truncated" : ""}
                          </span>
                        </div>
                        <p className="mt-1 line-clamp-3 text-[10px] text-text-secondary">
                          {taskArtifact.content}
                        </p>
                      </div>
                    )}
                    {contextBudget && (
                      <div className={`mt-2 rounded border p-2 ${contextBudgetToneClass(contextBudget)}`}>
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-text-secondary">Context Budget · {contextBudget.task}</span>
                          <span className="font-mono text-[10px] text-text-muted">
                            {contextBudget.used}/{contextBudget.totalBudget}
                          </span>
                        </div>
                        <div className="mt-1 space-y-1">
                          {contextBudget.sourceReports.slice(0, 5).map((source) => (
                            <div
                              key={`${event.taskId}-${source.source}-${source.reason}`}
                              className="rounded border border-border-subtle bg-bg-surface px-1.5 py-1 text-[10px]"
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="truncate text-text-secondary" title={source.source}>
                                  {source.source}
                                </span>
                                <span className={
                                  source.provided === 0
                                    ? "text-danger"
                                    : source.truncated
                                      ? "text-accent"
                                      : "text-text-muted"
                                }>
                                  {source.provided}/{source.requested}
                                </span>
                              </div>
                              <p className="mt-0.5 line-clamp-1 text-text-muted">
                                {source.truncationReason ?? source.reason}
                              </p>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
          </div>
        </div>

        <div className="min-h-0 overflow-y-auto p-3">
          <div className="space-y-3">
            <section className="rounded border border-border-subtle bg-bg-raised p-2 text-xs">
              <div className="mb-2 font-medium text-text-primary">Run Health</div>
              <div className="grid grid-cols-2 gap-2">
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Durable Save</span>
                  <span className="font-mono text-success">{formatRate(metrics?.durableSaveSuccessRate)}</span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Mission Complete</span>
                  <span className="font-mono text-text-primary">
                    {formatRate(metrics?.chapterMissionCompletionRate)}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Promise Recall</span>
                  <span className="font-mono text-text-primary">{formatRate(metrics?.promiseRecallHitRate)}</span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Ignored Repeat</span>
                  <span className="font-mono text-text-primary">
                    {formatRate(metrics?.ignoredRepeatedSuggestionRate)}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Ask To Ops</span>
                  <span className="font-mono text-text-primary">
                    {formatRate(metrics?.manualAskConvertedToOperationRate)}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Save Feedback</span>
                  <span className="font-mono text-text-primary">
                    {formatDuration(metrics?.averageSaveToFeedbackMs)}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Save Events</span>
                  <span className="font-mono text-text-primary">{saveCompletedEvents.length}</span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Trend Sessions</span>
                  <span className="font-mono text-text-primary">{metricsTrend?.sessionCount ?? 0}</span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Latency Delta</span>
                  <span className={
                    (metricsTrend?.saveToFeedbackDeltaMs ?? 0) > 0
                      ? "font-mono text-accent"
                      : "font-mono text-success"
                  }>
                    {formatDuration(metricsTrend?.saveToFeedbackDeltaMs)}
                  </span>
                </div>
              </div>
            </section>

            <section className="rounded border border-border-subtle bg-bg-raised p-2 text-xs">
              <div className="mb-2 flex items-center justify-between gap-2">
                <span className="font-medium text-text-primary">Session Trend</span>
                <span className="text-[10px] text-text-muted">
                  {metricsTrend?.sourceEventCount ?? 0} events
                </span>
              </div>
              <div className="grid grid-cols-3 gap-2">
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Recent</span>
                  <span className="font-mono text-text-primary">
                    {formatDuration(metricsTrend?.recentAverageSaveToFeedbackMs)}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Previous</span>
                  <span className="font-mono text-text-primary">
                    {formatDuration(metricsTrend?.previousAverageSaveToFeedbackMs)}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2">
                  <span className="block text-[10px] text-text-muted">Overall</span>
                  <span className="font-mono text-text-primary">
                    {formatDuration(metricsTrend?.overallAverageSaveToFeedbackMs)}
                  </span>
                </div>
              </div>
              <div className="mt-2 space-y-1.5">
                {(metricsTrend?.recentSessions ?? []).slice(0, 5).map((session) => (
                  <div key={session.sessionId} className="rounded border border-border-subtle bg-bg-deep p-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-text-secondary" title={session.sessionId}>
                        {session.sessionId}
                      </span>
                      <span className="font-mono text-[10px] text-text-muted">
                        {formatDuration(session.averageSaveToFeedbackMs)}
                      </span>
                    </div>
                    <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-text-muted">
                      <span>{formatTime(session.lastEventAt)}</span>
                      <span>{session.proposalCount} proposals</span>
                      <span>{session.feedbackCount} feedback</span>
                      <span>ask ops {formatRate(session.manualAskConvertedToOperationRate)}</span>
                      <span>accept {formatRate(session.proposalAcceptanceRate)}</span>
                      <span>save {formatRate(session.durableSaveSuccessRate)}</span>
                    </div>
                  </div>
                ))}
              </div>
              {(metricsTrend?.recentSessions ?? []).length === 0 && (
                <p className="mt-2 text-text-muted">No persisted session trend yet.</p>
              )}
            </section>

            {(latestProviderBudget || providerBudget) && (
              <section className="rounded border border-border-subtle bg-bg-raised p-2 text-xs">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Provider Budget</span>
                  <span className={providerBudget?.approvalRequired ? "text-danger" : "text-success"}>
                    {providerBudget?.decision ?? "unknown"}
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2 text-text-secondary">
                  <div className="font-mono text-[10px] text-text-muted">
                    {providerBudget?.estimatedTotalTokens ?? 0} tokens · {formatCostMicros(providerBudget?.estimatedCostMicros)}
                  </div>
                  {providerBudget?.reasons.map((reason) => (
                    <p key={reason} className="mt-1 text-[10px] text-accent">{reason}</p>
                  ))}
                  {providerBudget?.remediation.map((item) => (
                    <p key={item} className="mt-1 text-[10px] text-text-secondary">{item}</p>
                  ))}
                </div>
              </section>
            )}

            {latestFailure && (
              <section className="rounded border border-danger/40 bg-danger/10 p-2 text-xs">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <span className="font-medium text-danger">Latest Failure</span>
                  <button
                    onClick={() => setFilter("failure")}
                    className="rounded border border-danger/30 bg-bg-deep px-1.5 py-0.5 text-[10px] text-danger hover:bg-danger/10"
                  >
                    Open failures
                  </button>
                </div>
                <p className="line-clamp-3 text-text-secondary">{latestFailure.summary}</p>
                {latestFailureDetail && (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {recoveryActionsForFailure(latestFailureDetail).map((action) => (
                      <button
                        key={`latest-${action.label}`}
                        onClick={() => setFilter(action.filter)}
                        className="rounded border border-danger/30 bg-bg-deep px-1.5 py-0.5 text-[10px] text-danger hover:bg-danger/10"
                      >
                        {action.label}
                      </button>
                    ))}
                  </div>
                )}
              </section>
            )}

            {latestSave && (
              <section className="rounded border border-border-subtle bg-bg-raised p-2 text-xs">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Latest Save</span>
                  <span className="text-[10px] text-text-muted">
                    {formatDuration(metrics?.averageSaveToFeedbackMs)} feedback
                  </span>
                </div>
                <div className="rounded bg-bg-deep p-2 text-text-secondary">
                  <div className="flex items-center justify-between gap-2">
                    <span>{latestSaveDetail?.saveResult ?? latestSave.summary}</span>
                    <span className={
                      (latestSaveDetail?.diagnosticErrorCount ?? 0) > 0 ? "text-danger" : "text-success"
                    }>
                      {latestSaveDetail?.diagnosticTotalCount ?? 0} diagnostics
                    </span>
                  </div>
                  <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-text-muted">
                    {latestSaveDetail?.chapterTitle && <span>{latestSaveDetail.chapterTitle}</span>}
                    {latestSaveDetail?.operationKind && <span>{latestSaveDetail.operationKind}</span>}
                    {latestSaveDetail?.proposalId && <span>proposal {latestSaveDetail.proposalId}</span>}
                    {latestSaveDetail?.postWriteReportId && <span>report {latestSaveDetail.postWriteReportId}</span>}
                  </div>
                </div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {latestSave.sourceRefs.slice(0, 6).map((source) => (
                    <span key={source} className="rounded bg-bg-deep px-1.5 py-0.5 text-[10px] text-text-muted">
                      {source}
                    </span>
                  ))}
                </div>
              </section>
            )}

            {latestReceipt && (
              <section className="rounded border border-success/30 bg-success/10 p-2 text-xs">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Latest Receipt</span>
                  <button
                    onClick={() => setFilter("task_receipt")}
                    className="rounded border border-success/30 bg-bg-deep px-1.5 py-0.5 text-[10px] text-success hover:bg-success/10"
                  >
                    Open receipts
                  </button>
                </div>
                <p className="line-clamp-3 text-text-secondary">{latestReceipt.summary}</p>
                <div className="mt-2 flex flex-wrap gap-1 text-[10px] text-text-muted">
                  {latestReceiptDetail?.expectedArtifacts.slice(0, 4).map((artifact) => (
                    <span key={`artifact-${artifact}`} className="rounded bg-bg-deep px-1.5 py-0.5">
                      {artifact}
                    </span>
                  ))}
                  {latestReceiptDetail?.mustNot.slice(0, 4).map((rule) => (
                    <span key={`guard-${rule}`} className="rounded bg-bg-deep px-1.5 py-0.5 text-danger">
                      no {rule}
                    </span>
                  ))}
                </div>
              </section>
            )}

            {latestArtifact && (
              <section className="rounded border border-success/30 bg-bg-raised p-2 text-xs">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Latest Artifact</span>
                  <button
                    onClick={() => setFilter("task_artifact")}
                    className="rounded border border-success/30 bg-bg-deep px-1.5 py-0.5 text-[10px] text-success hover:bg-success/10"
                  >
                    Open artifacts
                  </button>
                </div>
                <div className="rounded bg-bg-deep p-2 text-text-secondary">
                  <div className="flex items-center justify-between gap-2">
                    <span>{latestArtifactDetail?.artifactKind ?? latestArtifact.label}</span>
                    <span className="font-mono text-[10px] text-text-muted">
                      {latestArtifactDetail?.contentCharCount ?? 0} chars
                    </span>
                  </div>
                  <p className="mt-1 line-clamp-4 text-[10px] text-text-muted">
                    {latestArtifactDetail?.content ?? latestArtifact.summary}
                  </p>
                </div>
                <div className="mt-2 flex flex-wrap gap-1 text-[10px] text-text-muted">
                  {latestArtifactDetail?.chapter && <span>{latestArtifactDetail.chapter}</span>}
                  {latestArtifactDetail?.requiredEvidence.slice(0, 4).map((source) => (
                    <span key={`artifact-source-${source}`} className="rounded bg-bg-deep px-1.5 py-0.5">
                      {source}
                    </span>
                  ))}
                </div>
              </section>
            )}

            {latestPostWrite && (
              <section className={`rounded border p-2 text-xs ${diagnosticToneClass(latestPostWrite)}`}>
                <div className="mb-2 flex items-center justify-between gap-2">
                  <span className="font-medium text-text-primary">Post-write Diagnostics</span>
                  <span className="font-mono text-[10px] text-text-muted">
                    {latestPostWrite.errorCount}e · {latestPostWrite.warningCount}w · {latestPostWrite.infoCount}i
                  </span>
                </div>
                {latestPostWrite.diagnostics.slice(0, 4).map((diagnostic) => (
                  <div key={diagnostic.diagnosticId} className="mt-1 rounded border border-border-subtle bg-bg-deep p-1.5">
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-text-secondary">{diagnostic.category}</span>
                      <span className="text-[10px] text-text-muted">{diagnostic.severity}</span>
                    </div>
                    <p className="mt-1 line-clamp-2 text-[10px] text-text-muted">{diagnostic.message}</p>
                  </div>
                ))}
                {latestPostWrite.remediation[0] && (
                  <p className="mt-2 line-clamp-2 text-[10px] text-text-secondary">
                    {latestPostWrite.remediation[0]}
                  </p>
                )}
              </section>
            )}

            <section className="rounded border border-border-subtle bg-bg-raised p-2 text-xs">
              <div className="mb-2 flex items-center justify-between gap-2">
                <span className="font-medium text-text-primary">Context Pressure</span>
                <span className="text-[10px] text-text-muted">{trends.length} sources</span>
              </div>
              {trends.length === 0 && (
                <p className="text-text-muted">No context trend data yet.</p>
              )}
              {trends.length > 0 && (
                <div className="mb-2 grid grid-cols-3 gap-1.5">
                  <div className="rounded border border-border-subtle bg-bg-deep p-1.5">
                    <div className="text-[9px] uppercase tracking-wide text-text-muted">coverage</div>
                    <div className="mt-0.5 font-mono text-[11px] text-text-primary">
                      {formatRate(contextPressure.coverageRate)}
                    </div>
                    <div className="mt-0.5 text-[9px] text-text-muted">
                      {formatCompactNumber(contextPressure.totalProvided)}/{formatCompactNumber(contextPressure.totalRequested)}
                    </div>
                  </div>
                  <div className="rounded border border-accent/30 bg-bg-deep p-1.5">
                    <div className="text-[9px] uppercase tracking-wide text-text-muted">truncated</div>
                    <div className="mt-0.5 font-mono text-[11px] text-accent">
                      {contextPressure.truncatedEvents}
                    </div>
                    <div className="mt-0.5 text-[9px] text-text-muted">
                      {contextPressure.truncatedSources} sources
                    </div>
                  </div>
                  <div className="rounded border border-danger/30 bg-bg-deep p-1.5">
                    <div className="text-[9px] uppercase tracking-wide text-text-muted">dropped</div>
                    <div className="mt-0.5 font-mono text-[11px] text-danger">
                      {contextPressure.droppedEvents}
                    </div>
                    <div className="mt-0.5 text-[9px] text-text-muted">
                      {contextPressure.droppedSources} sources
                    </div>
                  </div>
                </div>
              )}
              {contextPressure.hottestSource && (
                <div className="mb-2 rounded border border-border-subtle bg-bg-deep p-1.5 text-[10px]">
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-text-muted">pressure source</span>
                    <span className="truncate text-text-secondary" title={contextPressure.hottestSource.source}>
                      {contextPressure.hottestSource.source}
                    </span>
                  </div>
                  {(contextPressure.hottestSource.lastTruncationReason || contextPressure.hottestSource.lastReason) && (
                    <p className="mt-1 line-clamp-2 text-text-muted">
                      {contextPressure.hottestSource.lastTruncationReason ?? contextPressure.hottestSource.lastReason}
                    </p>
                  )}
                </div>
              )}
              <div className="space-y-1.5">
                {trends.slice(0, 8).map((trend) => (
                  <div key={trend.source} className={`rounded border p-2 ${trendToneClass(trend)}`}>
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate font-medium text-text-secondary" title={trend.source}>
                        {trend.source}
                      </span>
                      <span className="font-mono text-[10px] text-text-muted">
                        {formatRate(trendCoverageRate(trend))}
                      </span>
                    </div>
                    <div className="mt-1 h-1 overflow-hidden rounded bg-bg-deep">
                      <div
                        className={
                          trend.droppedCount > 0
                            ? "h-full bg-danger"
                            : trend.truncatedCount > 0
                              ? "h-full bg-accent"
                              : "h-full bg-success"
                        }
                        style={{ width: `${Math.max(4, Math.min(100, trendCoverageRate(trend) * 100))}%` }}
                      />
                    </div>
                    <div className="mt-1 flex flex-wrap gap-1 text-[10px] text-text-muted">
                      <span>seen {trend.appearances}</span>
                      <span>provided {trend.providedCount}</span>
                      <span>trunc {trend.truncatedCount}</span>
                      <span>drop {trend.droppedCount}</span>
                      <span>avg {formatCompactNumber(Math.round(trend.averageProvided))}</span>
                    </div>
                    {(trend.lastReason || trend.lastTruncationReason) && (
                      <p className="mt-1 line-clamp-2 text-[10px] text-text-secondary">
                        {trend.lastTruncationReason ?? trend.lastReason}
                      </p>
                    )}
                  </div>
                ))}
              </div>
            </section>

            <section className="rounded border border-border-subtle bg-bg-raised p-2 text-xs">
              <div className="mb-2 flex items-center justify-between gap-2">
                <span className="font-medium text-text-primary">Proposal Context Budgets</span>
                <span className="text-[10px] text-text-muted">{proposalById.size} proposals</span>
              </div>
              {(trace?.recentProposals ?? [])
                .filter((proposal) => proposal.contextBudget)
                .slice(0, 4)
                .map((proposal) => {
                  const budget = proposal.contextBudget;
                  if (!budget) return null;
                  const truncated = budget.sourceReports.filter((source) => source.truncated).length;
                  const dropped = budget.sourceReports.filter((source) => source.provided === 0).length;
                  return (
                    <div key={proposal.id} className={`mt-1.5 rounded border p-2 ${contextBudgetToneClass(budget)}`}>
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-text-secondary" title={proposal.previewSnippet}>
                          {proposal.kind}
                        </span>
                        <span className="font-mono text-[10px] text-text-muted">
                          {budget.used}/{budget.totalBudget}
                        </span>
                      </div>
                      <div className="mt-1 flex gap-1 text-[10px] text-text-muted">
                        <span>{budget.sourceReports.length} sources</span>
                        <span>trunc {truncated}</span>
                        <span>drop {dropped}</span>
                      </div>
                      {budget.sourceReports[0] && (
                        <p className="mt-1 line-clamp-1 text-[10px] text-text-secondary">
                          {budget.sourceReports[0].source}: {budget.sourceReports[0].provided}/{budget.sourceReports[0].requested}
                        </p>
                      )}
                    </div>
                  );
                })}
              {!(trace?.recentProposals ?? []).some((proposal) => proposal.contextBudget) && (
                <p className="text-text-muted">No proposal context budgets yet.</p>
              )}
            </section>
          </div>
        </div>
      </div>
    </div>
  );
};
