import type {
  AgentProposal,
  PlotPromiseSummary,
  StoryDebtEntry,
  StoryDebtSnapshot,
  WriterAgentLedgerSnapshot,
  WriterAgentTraceSnapshot,
  WriterPostWriteDiagnosticReport,
  WriterProposalTrace,
} from "../protocol";


export function postWriteReportTone(report: WriterPostWriteDiagnosticReport): SecondBrainTone {
  if (report.errorCount > 0) return "danger";
  if (report.warningCount > 0) return "accent";
  return "success";
}

export function memoryReliabilityTone(status: string): SecondBrainTone {
  if (status === "needs_review") return "danger";
  if (status === "trusted") return "success";
  return "accent";
}

export function memoryReliabilityPercent(value: number): string {
  return `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
}

export function postWriteReportLabel(report: WriterPostWriteDiagnosticReport): string {
  const chapter = report.chapterTitle ?? "saved text";
  const revision = report.chapterRevision ? ` · ${report.chapterRevision}` : "";
  return `${chapter}${revision}`;
}

export function postWriteReportCounts(report: WriterPostWriteDiagnosticReport): string {
  return `${report.errorCount} errors · ${report.warningCount} warnings · ${report.infoCount} info`;
}

type SecondBrainTone = "neutral" | "accent" | "danger" | "success";

interface SecondBrainItem {
  label: string;
  value: string;
  detail?: string;
  tone: SecondBrainTone;
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

export function secondBrainToneClass(tone: SecondBrainTone): string {
  if (tone === "danger") return "border-danger/40 bg-danger/10";
  if (tone === "accent") return "border-accent/30 bg-accent-subtle/20";
  if (tone === "success") return "border-success/30 bg-success/10";
  return "border-border-subtle bg-bg-raised";
}

export function secondBrainValueClass(tone: SecondBrainTone): string {
  if (tone === "danger") return "text-danger";
  if (tone === "accent") return "text-accent";
  if (tone === "success") return "text-success";
  return "text-text-primary";
}

export function latestContextProposal(trace: WriterAgentTraceSnapshot | null): WriterProposalTrace | undefined {
  return trace?.recentProposals.find((proposal) => proposal.contextBudget);
}

export function contextBudgetTone(trace: WriterAgentTraceSnapshot | null): SecondBrainTone {
  const budget = latestContextProposal(trace)?.contextBudget;
  if (!budget) return "neutral";
  if (budget.used > budget.totalBudget) return "danger";
  if (budget.sourceReports.some((source) => source.truncated)) return "accent";
  return "success";
}

export function formatContextBudgetValue(proposal: WriterProposalTrace | undefined): string {
  const budget = proposal?.contextBudget;
  if (!budget) return "No trace yet";
  return `${budget.used}/${budget.totalBudget} chars`;
}

export function formatContextBudgetDetail(proposal: WriterProposalTrace | undefined): string {
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
  if (status === "draft") return "Draft";
  if (status === "completed") return "Done";
  if (status === "drifted") return "Drift";
  if (status === "needs_review") return "Review";
  if (status === "active" || status === "in_progress") return "Active";
  if (status === "blocked") return "Blocked";
  if (status === "retired") return "Retired";
  return status || "Draft";
}

function missionStatusTone(status: string | undefined): SecondBrainTone {
  if (status === "completed") return "success";
  if (status === "drifted" || status === "needs_review" || status === "blocked") return "danger";
  if (status === "active" || status === "in_progress") return "accent";
  return "accent";
}

export function sourceBudgetClass(truncated: boolean): string {
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

export function buildSecondBrainItems(
  ledger: WriterAgentLedgerSnapshot | null,
  storyDebt: StoryDebtSnapshot | null,
  proposals: AgentProposal[],
  currentChapter: string,
  trace: WriterAgentTraceSnapshot | null,
  isAgentThinking: boolean,
  rankedPromises: PlotPromiseSummary[],
): SecondBrainItem[] {
  const contractDebt = firstDebt(storyDebt, ["story_contract"]);
  const missionDebt = firstDebt(storyDebt, ["chapter_mission"]);
  const canonRisk = firstDebt(storyDebt, ["canon_risk", "timeline_risk"]);
  const promiseDebt = firstDebt(storyDebt, ["promise"]);
  const pacingDebt = firstDebt(storyDebt, ["pacing"]);
  const decision = scopedDecision(ledger, currentChapter);
  const arcProposal = proposalForArc(proposals);
  const openPromise = rankedPromises[0];
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

  const highRiskCount = rankedPromises.filter((p) => p.risk === "high").length;
  const promiseValue = openPromise
    ? compactLine(openPromise.title, `Open promise · ${openPromise.risk}`, 72)
    : promiseDebt
      ? compactLine(promiseDebt.title, "Open promise", 72)
      : "No open promise";
  const promiseDetail = openPromise
    ? compactLine(
        openPromise.expectedPayoff
          ? `${openPromise.description} -> ${openPromise.expectedPayoff}`
          : openPromise.description,
        highRiskCount > 1 ? `${highRiskCount} high-risk promises open` : "Payoff not set",
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

  const contractQuality = storyContract?.quality ?? "missing";
  const contractGaps = storyContract?.qualityGaps ?? [];
  const contractValue = contractDebt
    ? compactLine(contractDebt.title, "Story contract guard", 72)
    : hasStoryContract
    ? compactLine(
        `${storyContract?.readerPromise || storyContract?.mainConflict || storyContract?.genre || "未填写"}`,
        `Story contract · ${contractQuality}`,
        72,
      )
    : "No story contract";
  const contractDetail = contractDebt
    ? compactLine(contractDebt.message, "Review book-level boundary before continuing")
    : hasStoryContract
    ? contractGaps.length > 0
      ? compactLine(contractGaps.join("; "), "Fill these gaps for stronger agent grounding")
      : compactLine("All key fields are set — the agent has strong book-level grounding.", "")
    : "Set the book-level promise so the agent can judge local choices against the whole novel.";
  const missionCalibration = proposals.find(
    (p) => p.kind === "chapter_mission" && p.rationale.includes("mission calibration"),
  );
  const missionValue = missionDebt
    ? compactLine(missionDebt.title, "Chapter mission guard", 72)
    : chapterMission
    ? compactLine(
        `${missionStatusLabel(chapterMission.status)} · ${chapterMission.mission || chapterMission.expectedEnding}`,
        missionCalibration ? "Mission needs review — save triggered a status change" : "Current chapter mission",
        72,
      )
    : currentChapter
      ? "No chapter mission"
      : "No chapter loaded";
  const missionDetail = missionDebt
    ? compactLine(missionDebt.message, "Resolve or update this chapter mission")
    : missionCalibration && chapterMission
    ? compactLine(
        `Save suggests: ${missionCalibration.preview}. Review in queue to accept or reject.`,
        "",
      )
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
      tone: missionDebt
        ? "danger"
        : missionCalibration
          ? "danger"
          : chapterMission
            ? missionStatusTone(chapterMission.status)
            : "accent",
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
