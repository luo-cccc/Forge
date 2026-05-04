const fs = require("fs");
const path = require("path");
const ts = require("typescript");

const repoRoot = path.join(__dirname, "..");
const moduleCache = new Map();
const fixedNow = 1_777_891_200_000;

const storeState = {
  currentChapter: "Chapter-7",
  currentChapterRevision: "rev-render",
  agentMode: "proactive",
  isAgentThinking: false,
};

const fixture = buildFixture();

const React = require("react");
const server = require("react-dom/server");
const originalHooks = {
  useState: React.useState,
  useEffect: React.useEffect,
  useRef: React.useRef,
  useCallback: React.useCallback,
};

let stateIndex = 0;
const stateOverrides = [
  fixture.status,
  fixture.ledger,
  fixture.storageDiagnostics,
  fixture.chapterBackups,
  fixture.proposals,
  fixture.reviewQueue,
  fixture.storyDebt,
  fixture.trace,
  "status",
  fixedNow,
];

try {
  React.useState = (initialValue) => {
    const value = stateIndex < stateOverrides.length
      ? stateOverrides[stateIndex]
      : typeof initialValue === "function"
        ? initialValue()
        : initialValue;
    stateIndex += 1;
    return [value, () => {}];
  };
  React.useEffect = () => {};
  React.useRef = (value) => ({ current: value });
  React.useCallback = (fn) => fn;

  const { CompanionPanel } = loadModule(path.join(repoRoot, "src", "components", "CompanionPanel.tsx"));
  const html = server.renderToStaticMarkup(
    React.createElement(CompanionPanel, {
      mode: "write",
      onApplyOperation: async () => ({ applied: true, saved: true }),
    }),
  );

  assertIncludes(html, "Writing Companion");
  assertIncludes(html, "What It Is Guarding");
  assertIncludes(html, "Urgent Story Guards");
  assertIncludes(html, "URGENT_PUBLIC_PREVIEW");

  const forbidden = [
    "Evidence Trace",
    "Project Storage",
    "Why It Spoke",
    "Context Trace",
    "Provider Budget",
    "Latest Receipt",
    "Proposal Context Budgets",
    "Context Pressure",
    "Review budget",
    "writer.save_completed",
    "task_receipt",
    "task_artifact",
    "operation_lifecycle",
    "RAW_RATIONALE_SENTINEL",
    "RAW_EVIDENCE_SENTINEL",
    "RAW_OPERATION_KIND_SENTINEL",
    "RAW_TASK_PACKET_OBJECTIVE",
    "RAW_CONTEXT_SOURCE_SENTINEL",
    "RAW_RUN_EVENT_SENTINEL",
    "RAW_LIFECYCLE_SENTINEL",
  ];

  const leaks = forbidden.filter((needle) => html.includes(needle));
  if (leaks.length > 0) {
    console.error("Write-mode Companion render leaked inspector/internal content:");
    for (const leak of leaks) {
      console.error(`- ${leak}`);
    }
    process.exit(1);
  }

  console.log("P2 render checks passed (write-mode DOM guard passing).");
} finally {
  React.useState = originalHooks.useState;
  React.useEffect = originalHooks.useEffect;
  React.useRef = originalHooks.useRef;
  React.useCallback = originalHooks.useCallback;
}

function assertIncludes(html, needle) {
  if (!html.includes(needle)) {
    console.error(`Write-mode Companion render is missing expected content: ${needle}`);
    process.exit(1);
  }
}

function loadModule(filePath) {
  const absolutePath = path.normalize(filePath);
  if (moduleCache.has(absolutePath)) {
    return moduleCache.get(absolutePath).exports;
  }

  const source = fs.readFileSync(absolutePath, "utf8");
  const output = ts.transpileModule(source, {
    fileName: absolutePath,
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX,
      esModuleInterop: true,
    },
  }).outputText;

  const mod = { exports: {} };
  moduleCache.set(absolutePath, mod);
  const localRequire = (request) => resolveRequire(request, absolutePath);
  const runner = new Function("require", "module", "exports", "__filename", "__dirname", output);
  runner(localRequire, mod, mod.exports, absolutePath, path.dirname(absolutePath));
  return mod.exports;
}

function resolveRequire(request, fromFile) {
  if (request === "react") return React;
  if (request === "@tauri-apps/api/core") return { invoke: mockInvoke };
  if (request === "@tauri-apps/api/event") return { listen: async () => () => {} };
  if (request === "../store" || request.endsWith("/store")) {
    return { useAppStore: (selector) => selector(storeState) };
  }
  if (request.startsWith(".")) {
    const base = path.resolve(path.dirname(fromFile), request);
    const resolved = resolveLocalModule(base);
    return loadModule(resolved);
  }
  return require(request);
}

function resolveLocalModule(base) {
  const candidates = [
    base,
    `${base}.tsx`,
    `${base}.ts`,
    `${base}.jsx`,
    `${base}.js`,
    path.join(base, "index.tsx"),
    path.join(base, "index.ts"),
    path.join(base, "index.js"),
  ];
  const resolved = candidates.find((candidate) => fs.existsSync(candidate) && fs.statSync(candidate).isFile());
  if (!resolved) {
    throw new Error(`Cannot resolve local module: ${base}`);
  }
  return resolved;
}

function mockInvoke(command) {
  if (command === "get_writer_agent_status") return Promise.resolve(fixture.status);
  if (command === "get_writer_agent_ledger") return Promise.resolve(fixture.ledger);
  if (command === "get_writer_agent_pending_proposals") return Promise.resolve(fixture.proposals);
  if (command === "get_story_review_queue") return Promise.resolve(fixture.reviewQueue);
  if (command === "get_story_debt_snapshot") return Promise.resolve(fixture.storyDebt);
  if (command === "get_writer_agent_trace") return Promise.resolve(fixture.trace);
  if (command === "get_project_storage_diagnostics") return Promise.resolve(fixture.storageDiagnostics);
  if (command === "list_file_backups") return Promise.resolve(fixture.chapterBackups);
  return Promise.resolve(null);
}

function buildFixture() {
  const operation = {
    kind: "text.replace",
    chapter: "Chapter-7",
    from: 1,
    to: 4,
    text: "URGENT_PUBLIC_PREVIEW",
    revision: "rev-render",
  };
  const evidence = {
    source: "chapter_mission",
    reference: "Chapter-7",
    snippet: "RAW_EVIDENCE_SENTINEL",
  };
  const proposal = {
    id: "proposal-render-1",
    observationId: "observation-render-1",
    kind: "continuity_warning",
    priority: "urgent",
    target: { from: 1, to: 4 },
    preview: "URGENT_PUBLIC_PREVIEW",
    operations: [operation],
    rationale: "RAW_RATIONALE_SENTINEL",
    evidence: [evidence],
    risks: ["RAW_RISK_SENTINEL"],
    alternatives: [],
    confidence: 0.94,
    expiresAt: fixedNow + 60_000,
  };

  const productMetrics = {
    proposalCount: 4,
    feedbackCount: 2,
    acceptedCount: 1,
    rejectedCount: 1,
    editedCount: 0,
    snoozedCount: 0,
    explainedCount: 0,
    ignoredCount: 0,
    positiveFeedbackCount: 1,
    negativeFeedbackCount: 1,
    proposalAcceptanceRate: 0.5,
    ignoredRepeatedSuggestionRate: 0,
    manualAskConvertedToOperationRate: 1,
    promiseRecallHitRate: 1,
    canonFalsePositiveRate: 0,
    chapterMissionCompletionRate: 0.75,
    durableSaveSuccessRate: 1,
    averageSaveToFeedbackMs: 120,
  };

  return {
    status: {
      projectId: "project-render",
      sessionId: "session-render",
      activeChapter: "Chapter-7",
      observationCount: 7,
      proposalCount: 4,
      openPromiseCount: 1,
      pendingProposals: 1,
      totalFeedbackEvents: 2,
    },
    ledger: {
      storyContract: {
        projectId: "project-render",
        title: "Render Fixture",
        genre: "Mystery",
        targetReader: "Long-form fiction reader",
        readerPromise: "A fair clue trail",
        first30ChapterPromise: "Resolve the first betrayal",
        mainConflict: "Truth versus loyalty",
        structuralBoundary: "No sudden reveal without setup",
        toneContract: "Tense but clear",
        updatedAt: "2026-05-04T00:00:00Z",
        quality: "strong",
        qualityGaps: [],
      },
      activeChapterMission: {
        id: 7,
        projectId: "project-render",
        chapterTitle: "Chapter-7",
        mission: "Protect the clue handoff",
        mustInclude: "The old key",
        mustNot: "Reveal the culprit",
        expectedEnding: "The clue changes hands",
        status: "active",
        sourceRef: "mission:chapter-7",
        updatedAt: "2026-05-04T00:00:00Z",
        blockedReason: "",
        retiredHistory: "",
      },
      chapterMissions: [],
      recentChapterResults: [{
        id: 1,
        projectId: "project-render",
        chapterTitle: "Chapter-6",
        chapterRevision: "rev-6",
        summary: "The key changed hands.",
        stateChanges: [],
        characterProgress: [],
        newConflicts: [],
        newClues: ["old key"],
        promiseUpdates: [],
        canonUpdates: [],
        sourceRef: "chapter:6",
        createdAt: fixedNow - 5_000,
      }],
      nextBeat: {
        chapterTitle: "Chapter-7",
        goal: "Make the clue cost something",
        carryovers: ["old key"],
        blockers: [],
        sourceRefs: ["chapter:6"],
      },
      canonEntities: [],
      canonRules: [{
        rule: "The culprit cannot be named before Chapter-10.",
        category: "reveal_boundary",
        priority: 10,
        status: "active",
      }],
      openPromises: [{
        id: 1,
        kind: "mystery_clue",
        title: "Old key",
        description: "The old key opens the tower ledger.",
        introducedChapter: "Chapter-3",
        lastSeenChapter: "Chapter-6",
        lastSeenRef: "chapter:6",
        expectedPayoff: "Chapter-9",
        priority: 8,
        risk: "high",
      }],
      recentDecisions: [],
      memoryAudit: [],
      memoryReliability: [],
      contextRecalls: [],
    },
    storageDiagnostics: {
      projectId: "project-render",
      projectName: "Render Fixture",
      appDataDir: "RAW_STORAGE_APP_DIR",
      projectDataDir: "RAW_STORAGE_PROJECT_DIR",
      checkedAt: fixedNow,
      healthy: true,
      files: [],
      databases: [],
    },
    chapterBackups: [{
      id: "backup-render",
      filename: "RAW_BACKUP_FILENAME",
      path: "RAW_BACKUP_PATH",
      bytes: 512,
      modifiedAt: fixedNow,
    }],
    proposals: [proposal],
    reviewQueue: [{
      id: "queue-render",
      proposalId: "proposal-render-1",
      category: "continuity_warning",
      severity: "warning",
      title: "RAW_QUEUE_TITLE",
      message: "RAW_QUEUE_MESSAGE",
      evidence: [evidence],
      operations: [operation],
      status: "pending",
      createdAt: fixedNow,
      expiresAt: fixedNow + 60_000,
    }],
    storyDebt: {
      chapterTitle: "Chapter-7",
      total: 0,
      openCount: 0,
      contractCount: 0,
      missionCount: 0,
      canonRiskCount: 0,
      promiseCount: 0,
      pacingCount: 0,
      entries: [],
    },
    trace: {
      recentObservations: [],
      taskPackets: [{
        id: "task-render",
        observationId: "observation-render-1",
        task: "ManualRequest",
        objective: "RAW_TASK_PACKET_OBJECTIVE",
        scope: "RAW_TASK_PACKET_SCOPE",
        intent: "RAW_TASK_PACKET_INTENT",
        requiredContextCount: 5,
        beliefCount: 4,
        successCriteriaCount: 3,
        maxSideEffectLevel: "ReadOnly",
        foundationComplete: true,
      }],
      recentProposals: [{
        id: "proposal-render-1",
        observationId: "observation-render-1",
        kind: "continuity_warning",
        priority: "urgent",
        state: "proposed",
        confidence: 0.94,
        previewSnippet: "RAW_PROPOSAL_TRACE_SENTINEL",
        evidence: [evidence],
        contextBudget: {
          task: "ManualRequest",
          used: 250,
          totalBudget: 500,
          wasted: 0,
          sourceReports: [{
            source: "RAW_CONTEXT_SOURCE_SENTINEL",
            requested: 100,
            provided: 50,
            truncated: true,
            reason: "RAW_CONTEXT_REASON_SENTINEL",
            truncationReason: "RAW_CONTEXT_TRUNCATION_SENTINEL",
          }],
        },
      }],
      recentFeedback: [],
      operationLifecycle: [{
        proposalId: "proposal-render-1",
        operationKind: "RAW_OPERATION_KIND_SENTINEL",
        sourceTask: "RAW_LIFECYCLE_SENTINEL",
        approvalSource: "RAW_APPROVAL_SOURCE_SENTINEL",
        affectedScope: "RAW_AFFECTED_SCOPE_SENTINEL",
        state: "operation_lifecycle",
        saveResult: "RAW_SAVE_RESULT_SENTINEL",
        feedbackResult: "RAW_FEEDBACK_RESULT_SENTINEL",
        createdAt: fixedNow,
      }],
      runEvents: [{
        seq: 1,
        tsMs: fixedNow,
        projectId: "project-render",
        sessionId: "session-render",
        taskId: "task-render",
        eventType: "writer.save_completed",
        sourceRefs: ["RAW_RUN_EVENT_SOURCE_SENTINEL"],
        data: { raw: "RAW_RUN_EVENT_SENTINEL" },
      }],
      postWriteDiagnostics: [],
      contextSourceTrends: [],
      contextRecalls: [],
      productMetrics,
      productMetricsTrend: {
        sourceEventCount: 4,
        sessionCount: 1,
        overallAverageSaveToFeedbackMs: 120,
        recentAverageSaveToFeedbackMs: 120,
        previousAverageSaveToFeedbackMs: null,
        saveToFeedbackDeltaMs: null,
        overallContextCoverageRate: 0.5,
        recentContextCoverageRate: 0.5,
        previousContextCoverageRate: 0,
        contextCoverageDelta: null,
        recentSessions: [],
      },
    },
  };
}
