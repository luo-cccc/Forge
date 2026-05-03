import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import {
  Commands,
  Events,
  type AgentError,
  type AgentLoopEventPayload,
  type ChapterGenerationEvent,
  type ChapterGenerationStart,
  type ChainOfThoughtStep,
  type EditorEntityCard,
  type EditorHoverHint,
  type Epiphany,
  type FrontendChapterStateSnapshot,
  type GenerateChapterAutonomousPayload,
  type WriterProviderBudgetApproval,
  type WriterProviderBudgetReport,
  type StoryMode,
  type SearchStatus,
  type StoryboardMarker,
  type StreamChunk,
  type StreamEnd,
} from "../protocol";

interface Message {
  role: "user" | "agent";
  content: string;
}

interface AgentPanelProps {
  mode: StoryMode;
  getContext: () => { full: string; paragraph: string; selected: string; cursorPosition: number };
}

interface PendingChapterDraftRetry {
  payload: GenerateChapterAutonomousPayload;
  budget: WriterProviderBudgetReport;
}

function buildAskAgentContext(
  currentChapter: string,
  currentChapterRevision: string | null,
  isEditorDirty: boolean,
  full: string,
  cursorPosition: number,
) {
  return {
    chapterTitle: currentChapter,
    chapterRevision: currentChapterRevision ?? undefined,
    cursorPosition: Math.min(cursorPosition, Array.from(full).length),
    dirty: isEditorDirty,
  };
}

function detectChapterDraftRequest(text: string): number | null {
  const normalized = text.replace(/\s+/g, "");
  const match = normalized.match(/(?:帮我)?(?:写|生成|起草|撰写)第([0-9一二三四五六七八九十百]+)章(?:完整)?(?:初稿|草稿)?/);
  if (!match) return null;
  return parseChineseChapterNumber(match[1]);
}

function parseChineseChapterNumber(raw: string): number | null {
  if (/^\d+$/.test(raw)) return Number(raw);
  const digitMap: Record<string, number> = {
    零: 0,
    一: 1,
    二: 2,
    两: 2,
    三: 3,
    四: 4,
    五: 5,
    六: 6,
    七: 7,
    八: 8,
    九: 9,
  };
  if (raw === "十") return 10;
  if (raw.includes("十")) {
    const [tensRaw, onesRaw] = raw.split("十");
    const tens = tensRaw ? digitMap[tensRaw] : 1;
    const ones = onesRaw ? digitMap[onesRaw] : 0;
    if (tens === undefined || ones === undefined) return null;
    return tens * 10 + ones;
  }
  return digitMap[raw] ?? null;
}

function formatProviderCostMicros(value: number): string {
  return `$${(value / 1_000_000).toFixed(4)}`;
}

function providerBudgetFromError(
  error: ChapterGenerationEvent["error"],
): WriterProviderBudgetReport | null {
  const details = error?.evidence?.details;
  if (!details || typeof details !== "object" || !("providerBudget" in details)) return null;
  const budget = (details as { providerBudget?: unknown }).providerBudget;
  if (!budget || typeof budget !== "object") return null;
  const report = budget as Partial<WriterProviderBudgetReport>;
  if (
    typeof report.task !== "string" ||
    typeof report.model !== "string" ||
    typeof report.estimatedTotalTokens !== "number" ||
    typeof report.estimatedCostMicros !== "number"
  ) {
    return null;
  }
  return {
    task: report.task,
    model: report.model,
    estimatedInputTokens: report.estimatedInputTokens ?? 0,
    requestedOutputTokens: report.requestedOutputTokens ?? 0,
    estimatedTotalTokens: report.estimatedTotalTokens,
    estimatedCostMicros: report.estimatedCostMicros,
    maxTotalTokensWithoutApproval: report.maxTotalTokensWithoutApproval ?? 0,
    maxEstimatedCostMicrosWithoutApproval: report.maxEstimatedCostMicrosWithoutApproval ?? 0,
    decision: report.decision ?? "approval_required",
    approvalRequired: report.approvalRequired ?? true,
    reasons: Array.isArray(report.reasons) ? report.reasons : [],
    remediation: Array.isArray(report.remediation) ? report.remediation : [],
  };
}

function approvalFromBudget(budget: WriterProviderBudgetReport): WriterProviderBudgetApproval {
  return {
    task: budget.task,
    model: budget.model,
    approvedTotalTokens: budget.estimatedTotalTokens,
    approvedCostMicros: budget.estimatedCostMicros,
    approvedAtMs: Date.now(),
    source: "explore_chapter_generation",
  };
}

export default function AgentPanel({
  mode,
  getContext,
}: AgentPanelProps) {
  const isInlineRequest = useAppStore((s) => s.isInlineRequest);
  const currentChapter = useAppStore((s) => s.currentChapter);
  const currentChapterRevision = useAppStore((s) => s.currentChapterRevision);
  const isEditorDirty = useAppStore((s) => s.isEditorDirty);
  const agentMode = useAppStore((s) => s.agentMode);
  const setAgentMode = useAppStore((s) => s.setAgentMode);
  const latestObservation = useAppStore((s) => s.latestObservation);
  const suggestionQueue = useAppStore((s) => s.suggestionQueue);
  const companionNotes = useAppStore((s) => s.companionNotes);
  const entityCards = useAppStore((s) => s.entityCards);
  const hoverHints = useAppStore((s) => s.hoverHints);
  const storyboardMarkers = useAppStore((s) => s.storyboardMarkers);
  const addEntityCard = useAppStore((s) => s.addEntityCard);
  const addHoverHint = useAppStore((s) => s.addHoverHint);
  const addStoryboardMarker = useAppStore((s) => s.addStoryboardMarker);
  const snoozedUntil = useAppStore((s) => s.snoozedUntil);
  const setIsAgentThinking = useAppStore((s) => s.setIsAgentThinking);
  const incrementActionEpoch = useAppStore((s) => s.incrementActionEpoch);
  const [messages, setMessages] = useState<Message[]>([]);
  const [streaming, setStreaming] = useState("");
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [searchStatus, setSearchStatus] = useState<SearchStatus | null>(null);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [lastInput, setLastInput] = useState<string>("");
  const [brainMode, setBrainMode] = useState(false);
  const [epiphanies, setEpiphanies] = useState<Epiphany[]>([]);
  const [cotSteps, setCotSteps] = useState<ChainOfThoughtStep[]>([]);
  const [chapterEvents, setChapterEvents] = useState<ChapterGenerationEvent[]>([]);
  const [pendingChapterDraftRetry, setPendingChapterDraftRetry] =
    useState<PendingChapterDraftRetry | null>(null);
  const activeChapterRequestRef = useRef<string | null>(null);
  const activeChapterPayloadRef = useRef<GenerateChapterAutonomousPayload | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rawBufferRef = useRef("");

  useEffect(() => {
    let unlistenChunk: UnlistenFn;
    let unlistenEnd: UnlistenFn;
    let unlistenSearch: UnlistenFn;
    let unlistenError: UnlistenFn;
    let unlistenEpiphany: UnlistenFn;
    let unlistenCot: UnlistenFn;
    let unlistenChapter: UnlistenFn;
    let unlistenLoop: UnlistenFn;
    let unlistenEntity: UnlistenFn;
    let unlistenHover: UnlistenFn;
    let unlistenMarker: UnlistenFn;

    const setup = async () => {
      // Listen for new agent-loop-event (replaces legacy chunk/error/end events)
      unlistenLoop = await listen<AgentLoopEventPayload>("agent-loop-event", (event) => {
        const p = event.payload;
        switch (p.kind) {
          case "text_chunk":
            if (!isInlineRequest) {
              rawBufferRef.current += p.content ?? "";
              setStreaming((prev) => prev + (p.content ?? ""));
            }
            break;
          case "error":
            setAgentError(p.message ?? "Agent error");
            setIsStreaming(false);
            setIsAgentThinking(false);
            break;
          case "complete":
            setIsStreaming(false);
            setIsAgentThinking(false);
            setSearchStatus(null);
            if (rawBufferRef.current) {
              const clean = rawBufferRef.current;
              rawBufferRef.current = "";
              if (clean) {
                setMessages((prev) => [...prev, { role: "agent", content: clean }]);
              }
              setStreaming("");
            }
            break;
          case "compaction":
            // Context was compacted — transparent to user
            break;
        }
      });
      unlistenChunk = await listen<StreamChunk>(Events.agentStreamChunk, (event) => {
        if (isInlineRequest) return;
        setSearchStatus((prev) => (prev ? null : prev));
        rawBufferRef.current += event.payload.content;
        setStreaming(rawBufferRef.current);
      });

      unlistenSearch = await listen<SearchStatus>(
        Events.agentSearchStatus,
        (event) => {
          if (isInlineRequest) return;
          rawBufferRef.current = "";
          setSearchStatus(event.payload);
        },
      );

      unlistenEpiphany = await listen<Epiphany>(
        Events.agentEpiphany,
        (event) => {
          setEpiphanies((prev) => [
            { id: event.payload.id, skill: event.payload.skill, category: event.payload.category },
            ...prev.slice(0, 9),
          ]);
        },
      );

      unlistenCot = await listen<ChainOfThoughtStep>(Events.agentChainOfThought, (event) => {
        setCotSteps((prev) => {
          const existing = prev.findIndex((s) => s.step === event.payload.step);
          if (existing !== -1) {
            const next = [...prev];
            next[existing] = event.payload;
            return next;
          }
          return [...prev, event.payload];
        });
      });

      unlistenChapter = await listen<ChapterGenerationEvent>(
        Events.chapterGeneration,
        (event) => {
          const activeRequestId = activeChapterRequestRef.current;
          if (activeRequestId && event.payload.requestId !== activeRequestId) return;

          setChapterEvents((prev) => [...prev, event.payload].slice(-8));

          if (
            event.payload.phase === "chapter_generation_completed" ||
            event.payload.phase === "chapter_generation_conflict" ||
            event.payload.phase === "chapter_generation_failed"
          ) {
            setIsStreaming(false);
            setIsAgentThinking(false);
            activeChapterRequestRef.current = null;
            if (event.payload.phase === "chapter_generation_failed") {
              const budget = providerBudgetFromError(event.payload.error);
              const retryPayload = activeChapterPayloadRef.current;
              activeChapterPayloadRef.current = null;
              if (
                event.payload.error?.code === "PROVIDER_BUDGET_APPROVAL_REQUIRED" &&
                budget &&
                retryPayload
              ) {
                setPendingChapterDraftRetry({
                  payload: retryPayload,
                  budget,
                });
              }
            } else {
              activeChapterPayloadRef.current = null;
              setPendingChapterDraftRetry(null);
            }
            const finalText =
              event.payload.phase === "chapter_generation_completed"
                ? `已完成：${event.payload.saved?.chapterTitle ?? event.payload.targetChapterTitle ?? "章节"} 初稿已保存。`
                : event.payload.phase === "chapter_generation_conflict"
                  ? `保存冲突：${event.payload.conflict?.reason ?? event.payload.message}`
                  : `生成失败：${event.payload.error?.message ?? event.payload.message}`;
            setMessages((prev) => [...prev, { role: "agent", content: finalText }]);
          }
        },
      );

      unlistenEntity = await listen<EditorEntityCard>(Events.editorEntityCard, (event) => {
        addEntityCard(event.payload);
      });
      unlistenHover = await listen<EditorHoverHint>(Events.editorHoverHint, (event) => {
        addHoverHint(event.payload);
      });
      unlistenMarker = await listen<StoryboardMarker>(Events.storyboardMarker, (event) => {
        addStoryboardMarker(event.payload);
      });

      unlistenError = await listen<AgentError>(
        Events.agentError,
        (event) => {
          if (isInlineRequest) return;
          setIsStreaming(false);
          setIsAgentThinking(false);
          setStreaming("");
          setAgentError(event.payload.message);
        },
      );

      unlistenEnd = await listen<StreamEnd>(Events.agentStreamEnd, () => {
        if (isInlineRequest) return;
        const finalText = rawBufferRef.current;
        rawBufferRef.current = "";

        if (finalText) {
          setMessages((prev) => [...prev, { role: "agent", content: finalText }]);
        }
        setStreaming("");
        setIsStreaming(false);
        setIsAgentThinking(false);
        setSearchStatus(null);
        incrementActionEpoch();
      });
    };

    setup();

    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenEnd) unlistenEnd();
      if (unlistenSearch) unlistenSearch();
      if (unlistenError) unlistenError();
      if (unlistenEpiphany) unlistenEpiphany();
      if (unlistenLoop) unlistenLoop();
      if (unlistenCot) unlistenCot();
      if (unlistenChapter) unlistenChapter();
      if (unlistenEntity) unlistenEntity();
      if (unlistenHover) unlistenHover();
      if (unlistenMarker) unlistenMarker();
    };
  }, [
    isInlineRequest,
    setIsAgentThinking,
    incrementActionEpoch,
    addEntityCard,
    addHoverHint,
    addStoryboardMarker,
  ]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, streaming]);

  const handleSubmit = useCallback(async () => {
    const text = input.trim();
    if (!text || isStreaming) return;

    setInput("");
    setLastInput(text);
    setAgentError(null);
    setMessages((prev) => [...prev, { role: "user", content: text }]);
    setIsStreaming(true);
    setIsAgentThinking(true);
    rawBufferRef.current = "";
    activeChapterPayloadRef.current = null;
    setPendingChapterDraftRetry(null);
    setChapterEvents([]);

    try {
      const { full, paragraph, selected, cursorPosition } = getContext();
      const chapterNumber = mode === "explore" && !brainMode ? detectChapterDraftRequest(text) : null;
      if (chapterNumber) {
        const frontendState: FrontendChapterStateSnapshot = {
          openChapterTitle: currentChapter,
          openChapterRevision: currentChapterRevision ?? undefined,
          dirty: isEditorDirty,
        };
        const payload: GenerateChapterAutonomousPayload = {
          targetChapterNumber: chapterNumber,
          userInstruction: text,
          frontendState,
          saveMode: "replace_if_clean",
        };
        activeChapterPayloadRef.current = payload;
        const result = await invoke<ChapterGenerationStart>(
          Commands.generateChapterAutonomous,
          { payload },
        );
        activeChapterRequestRef.current = result.requestId;
        activeChapterPayloadRef.current = { ...payload, requestId: result.requestId };
      } else if (brainMode) {
        await invoke(Commands.askProjectBrain, { query: text });
      } else {
        await invoke(Commands.askAgent, {
          message: text,
          context: full,
          paragraph,
          selectedText: selected,
          contextPayload: buildAskAgentContext(
            currentChapter,
            currentChapterRevision,
            isEditorDirty,
            full,
            cursorPosition,
          ),
        });
      }
    } catch (e) {
      setStreaming("");
      setIsStreaming(false);
      setIsAgentThinking(false);
      setMessages((prev) => [
        ...prev,
        { role: "agent", content: `Error: ${e}` },
      ]);
    }
  }, [
    input,
    isStreaming,
    getContext,
    setIsAgentThinking,
    brainMode,
    mode,
    currentChapter,
    currentChapterRevision,
    isEditorDirty,
  ]);

  const handleRetry = useCallback(async () => {
    if (!lastInput) return;
    setAgentError(null);
    setIsStreaming(true);
    setIsAgentThinking(true);
    rawBufferRef.current = "";
    try {
      if (brainMode) {
        await invoke(Commands.askProjectBrain, { query: lastInput });
      } else {
        const { full, paragraph, selected, cursorPosition } = getContext();
        await invoke(Commands.askAgent, {
          message: lastInput,
          context: full,
          paragraph,
          selectedText: selected,
          contextPayload: buildAskAgentContext(
            currentChapter,
            currentChapterRevision,
            isEditorDirty,
            full,
            cursorPosition,
          ),
        });
      }
    } catch (e) {
      setStreaming("");
      setIsStreaming(false);
      setIsAgentThinking(false);
      setMessages((prev) => [...prev, { role: "agent", content: `Error: ${e}` }]);
    }
  }, [
    lastInput,
    getContext,
    setIsAgentThinking,
    brainMode,
    currentChapter,
    currentChapterRevision,
    isEditorDirty,
  ]);

  const handleApproveProviderBudget = useCallback(async () => {
    if (!pendingChapterDraftRetry || isStreaming) return;
    const payload: GenerateChapterAutonomousPayload = {
      ...pendingChapterDraftRetry.payload,
      requestId: undefined,
      providerBudgetApproval: approvalFromBudget(pendingChapterDraftRetry.budget),
    };
    setAgentError(null);
    setPendingChapterDraftRetry(null);
    setChapterEvents([]);
    setIsStreaming(true);
    setIsAgentThinking(true);
    rawBufferRef.current = "";
    activeChapterPayloadRef.current = payload;
    try {
      const result = await invoke<ChapterGenerationStart>(
        Commands.generateChapterAutonomous,
        { payload },
      );
      activeChapterRequestRef.current = result.requestId;
      activeChapterPayloadRef.current = { ...payload, requestId: result.requestId };
      setMessages((prev) => [
        ...prev,
        {
          role: "agent",
          content: `Provider budget approved for ${pendingChapterDraftRetry.budget.estimatedTotalTokens} tokens; retrying chapter generation.`,
        },
      ]);
    } catch (e) {
      setStreaming("");
      setIsStreaming(false);
      setIsAgentThinking(false);
      setMessages((prev) => [...prev, { role: "agent", content: `Error: ${e}` }]);
    }
  }, [isStreaming, pendingChapterDraftRetry, setIsAgentThinking]);

  return (
    <div className="flex flex-col h-full border-l border-border-subtle">
      <div className="px-4 py-3 border-b border-border-subtle text-xs text-text-secondary font-display tracking-wider flex items-center justify-between">
        <span>{brainMode ? "Project Brain" : "Explore Lab"}</span>
        <div className="flex items-center gap-1">
          {(["off", "passive", "proactive"] as const).map((mode) => (
            <button
              key={mode}
              onClick={() => setAgentMode(mode)}
              className={`text-[10px] px-2 py-0.5 rounded-sm transition-colors ${
                agentMode === mode
                  ? "bg-accent text-bg-deep"
                  : "bg-bg-raised text-text-muted border border-border-subtle"
              }`}
            >
              {mode}
            </button>
          ))}
          <button
            onClick={() => setBrainMode(!brainMode)}
            className={`text-[10px] px-2 py-0.5 rounded-sm transition-colors ${
              brainMode
                ? "bg-purple-500/20 text-purple-300 border border-purple-500/40"
                : "bg-bg-raised text-text-muted border border-border-subtle"
            }`}
          >
            {brainMode ? "Brain" : "Draft Lab"}
          </button>
        </div>
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-3">
        <div className="rounded-sm border border-border-subtle bg-bg-raised px-3 py-2 text-xs text-text-muted space-y-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-text-secondary">伴生面板</span>
            <span className={agentMode === "proactive" ? "text-accent" : "text-text-muted"}>
              {agentMode}
            </span>
          </div>
          {latestObservation ? (
            <div className="space-y-1">
              <div className="truncate">
                Last observation: {latestObservation.reason} · idle {latestObservation.idleMs}ms
              </div>
              <div className="truncate">
                Paragraph: {latestObservation.currentParagraph || "empty"}
              </div>
            </div>
          ) : (
            <div>No editor observation yet.</div>
          )}
          <div>
            Suggestions queued: {suggestionQueue.length}
            {snoozedUntil && snoozedUntil > Date.now()
              ? ` · snoozed until ${new Date(snoozedUntil).toLocaleTimeString()}`
              : ""}
          </div>
        </div>
        {(entityCards.length > 0 || companionNotes.length > 0 || storyboardMarkers.length > 0) && (
          <div className="rounded-sm border border-border-subtle bg-bg-surface px-3 py-2 text-xs space-y-3">
            {entityCards[0] && (
              <div className="space-y-1">
                <div className="text-accent">设定卡 · {entityCards[0].keyword}</div>
                <div className="text-text-secondary line-clamp-4">{entityCards[0].content}</div>
              </div>
            )}
            {companionNotes.length > 0 && (
              <div className="space-y-1">
                <div className="text-text-secondary">待处理便签</div>
                {companionNotes.slice(0, 3).map((note, index) => (
                  <div key={`${note}-${index}`} className="text-text-muted">
                    {note}
                  </div>
                ))}
              </div>
            )}
            {storyboardMarkers.length > 0 && (
              <div className="space-y-1">
                <div className="text-text-secondary">节奏标记</div>
                {storyboardMarkers.slice(0, 2).map((marker, index) => (
                  <div key={`${marker.chapter}-${index}`} className="text-text-muted">
                    {marker.chapter}: {marker.message}
                  </div>
                ))}
              </div>
            )}
            {hoverHints.length > 0 && (
              <div className="text-text-muted">
                最近提示：{hoverHints[0].message}
              </div>
            )}
          </div>
        )}
        {messages.length === 0 && !streaming && !searchStatus && (
          <div className="rounded-sm border border-border-subtle bg-bg-raised px-3 py-2 text-xs text-text-muted">
            Explore mode is for deliberate drafting and project questions. Chapter draft requests stay here instead of the writing surface.
          </div>
        )}
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`text-sm max-w-[90%] rounded-sm px-3 py-2 whitespace-pre-wrap ${
              msg.role === "user"
                ? brainMode
                  ? "bg-purple-500/80 text-white ml-auto"
                  : "bg-accent text-bg-deep ml-auto"
                : "bg-bg-raised text-text-primary"
            }`}
          >
            {msg.content}
          </div>
        ))}
        {epiphanies.length > 0 && (
          <div className="space-y-1.5">
            {epiphanies.map((ep) => (
              <div
                key={ep.id}
                className="text-xs max-w-[90%] rounded-sm px-3 py-2 bg-purple-500/10 border border-purple-500/30 text-purple-200 flex items-center gap-2 animate-pulse"
              >
                <span className="text-sm">💡</span>
                <span className="flex-1">
                  <span className="text-purple-300 font-medium">Learned: </span>
                  {ep.skill}
                </span>
                <span className="text-[10px] text-purple-400 px-1.5 py-0.5 rounded-sm bg-purple-500/15">
                  {ep.category}
                </span>
              </div>
            ))}
          </div>
        )}
        {cotSteps.length > 0 && (
          <div className="text-xs max-w-[90%] rounded-sm px-3 py-2 bg-bg-raised border border-border-subtle space-y-1.5">
            {cotSteps.map((s) => (
              <div key={s.step} className="flex items-center gap-2">
                <span className={`text-[10px] ${s.status === "done" ? "text-success" : s.status === "running" ? "text-accent animate-pulse" : "text-text-muted"}`}>
                  {s.status === "done" ? "✓" : s.status === "running" ? "◉" : "○"}
                </span>
                <span className={`flex-1 ${s.status === "done" ? "text-text-secondary" : "text-text-primary"}`}>
                  {s.description}
                </span>
                {s.step > 0 && s.total > 1 && (
                  <span className="text-[10px] text-text-muted">{s.step}/{s.total}</span>
                )}
              </div>
            ))}
          </div>
        )}
        {chapterEvents.length > 0 && (
          <div className="text-xs max-w-[90%] rounded-sm px-3 py-2 bg-bg-raised border border-border-subtle space-y-2">
            <div className="h-1 rounded-sm bg-bg-deep overflow-hidden">
              <div
                className="h-full bg-accent transition-all"
                style={{ width: `${chapterEvents.at(-1)?.progress ?? 0}%` }}
              />
            </div>
            {chapterEvents.map((event, i) => (
              <div key={`${event.requestId}-${event.phase}-${i}`} className="flex items-start gap-2">
                <span
                  className={`mt-0.5 text-[10px] ${
                    event.status === "done"
                      ? "text-success"
                      : event.status === "error" || event.status === "conflict"
                        ? "text-danger"
                        : "text-accent animate-pulse"
                  }`}
                >
                  {event.status === "done" ? "✓" : event.status === "error" || event.status === "conflict" ? "!" : "◉"}
                </span>
                <span className="flex-1 text-text-secondary">
                  {event.message}
                  {event.budget && (
                    <span className="block text-[10px] text-text-muted">
                      {event.budget.sourceCount} sources · {event.budget.includedChars}/{event.budget.maxChars} chars
                    </span>
                  )}
                </span>
              </div>
            ))}
          </div>
        )}
        {pendingChapterDraftRetry && (
          <div className="text-xs max-w-[90%] rounded-sm px-3 py-2 bg-accent-subtle/30 border border-accent/30 space-y-2">
            <div className="flex items-center justify-between gap-2">
              <span className="font-medium text-accent">Provider budget approval</span>
              <span className="font-mono text-[10px] text-text-muted">
                {pendingChapterDraftRetry.budget.decision}
              </span>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div className="rounded bg-bg-deep p-2">
                <span className="block text-[10px] text-text-muted">Tokens</span>
                <span className="font-mono text-text-primary">
                  {pendingChapterDraftRetry.budget.estimatedTotalTokens}
                </span>
              </div>
              <div className="rounded bg-bg-deep p-2">
                <span className="block text-[10px] text-text-muted">Estimated Cost</span>
                <span className="font-mono text-text-primary">
                  {formatProviderCostMicros(pendingChapterDraftRetry.budget.estimatedCostMicros)}
                </span>
              </div>
            </div>
            <p className="line-clamp-2 text-text-secondary">
              {pendingChapterDraftRetry.budget.reasons[0] ??
                "This provider call needs explicit approval before retrying."}
            </p>
            {pendingChapterDraftRetry.budget.remediation[0] && (
              <p className="line-clamp-2 text-[10px] text-text-muted">
                {pendingChapterDraftRetry.budget.remediation[0]}
              </p>
            )}
            <div className="flex gap-2">
              <button
                onClick={handleApproveProviderBudget}
                disabled={isStreaming}
                className="rounded-sm bg-accent px-2 py-1 text-[11px] text-bg-deep disabled:opacity-50"
              >
                Approve and Retry
              </button>
              <button
                onClick={() => setPendingChapterDraftRetry(null)}
                className="rounded-sm border border-border-subtle bg-bg-deep px-2 py-1 text-[11px] text-text-muted hover:text-text-secondary"
              >
                Dismiss
              </button>
            </div>
          </div>
        )}
        {agentError && (
          <div className="text-sm max-w-[90%] rounded-sm px-3 py-2 bg-danger/20 border border-danger text-danger whitespace-pre-wrap flex items-center gap-3">
            <span>{agentError}</span>
            <button
              onClick={handleRetry}
              className="text-xs px-2 py-0.5 rounded-sm bg-danger text-white hover:bg-danger/80 transition-colors flex-shrink-0"
            >
              Retry
            </button>
          </div>
        )}
        {searchStatus && (
          <div className={`text-sm max-w-[90%] rounded-sm px-3 py-2 border whitespace-pre-wrap ${
            brainMode
              ? "bg-purple-500/20 border-purple-500/40 text-purple-300"
              : "bg-accent-subtle border border-accent text-accent"
          }`}>
            Searching lorebook: <span className="font-medium">{searchStatus.keyword}</span>...
          </div>
        )}
        {streaming && (
          <div className="text-sm max-w-[90%] rounded-sm px-3 py-2 bg-bg-raised text-text-primary whitespace-pre-wrap">
            {streaming}
            <span className={`inline-block w-1.5 h-4 ml-0.5 animate-pulse align-middle ${brainMode ? "bg-purple-400" : "bg-accent"}`} />
          </div>
        )}
      </div>

      <div className="p-4 border-t border-border-subtle">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
          disabled={isStreaming}
          className="w-full px-3 py-2 rounded-sm bg-bg-deep border border-border-subtle text-text-primary placeholder-text-muted focus:outline-none focus:border-accent text-sm disabled:opacity-50"
          placeholder={brainMode ? "Ask Project Brain..." : "Explore a draft, branch, or chapter..."}
        />
      </div>
    </div>
  );
}
