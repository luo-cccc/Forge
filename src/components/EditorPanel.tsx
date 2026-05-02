import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Editor } from "@tiptap/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { useAppStore } from "../store";
import {
  Commands,
  Events,
  type AgentMode,
  type AgentObservation,
  type AgentObservationReason,
  type AgentSuggestion,
  type EditorEntityCard,
  type EditorGhostChunk,
  type EditorGhostEnd,
  type EditorSemanticLint,
  type EditorStatePayload,
  type InlineWriterOperationEvent,
  type ParallelDraft,
  type ParallelDraftPayload,
  type ProposalFeedback,
  type SemanticLintPayload,
  type WriterOperation,
} from "../protocol";
import CommentMark from "../extensions/CommentMark";
import GhostText, { ghostTextPluginKey } from "../extensions/GhostText";
import InlinePreview, {
  inlinePreviewPluginKey,
  type InlinePreviewState,
} from "../extensions/InlinePreview";
import SemanticLint from "../extensions/SemanticLint";
import EntityAnchor from "../extensions/EntityAnchor";
import LorebookDrawer from "./LorebookDrawer";
import InlineCommandBubble from "./InlineCommandBubble";
import AgentSuggestionOverlay from "./AgentSuggestionOverlay";
import { PatchReviewOverlay } from "./PatchReviewOverlay";
import { CoWriterStatusBar } from "./CoWriterStatusBar";
import PatchMark from "../extensions/PatchMark";
import EntityHoverCard from "./EntityHoverCard";
import ParallelDraftsPane from "./ParallelDraftsPane";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

interface EntityHoverState {
  card: EditorEntityCard;
  x: number;
  y: number;
}

interface EditorPanelProps {
  onEditorReady?: (editor: Editor) => void;
  onSelectionUpdate?: (sel: SelectionState) => void;
}

const OBSERVATION_DEBOUNCE_MS = 1100;
const OBSERVATION_WINDOW_CHARS = 1800;
const PARAGRAPH_BUDGET_CHARS = 900;
const EDITOR_TELEMETRY_DEBOUNCE_MS = 400;
const SEMANTIC_LINT_IDLE_MS = 10000;
const FIM_PREFIX_CHARS = 1000;
const FIM_SUFFIX_CHARS = 500;
const PARALLEL_DRAFT_PREFIX_CHARS = 3000;
const PARALLEL_DRAFT_SUFFIX_CHARS = 1000;

function limitChars(text: string, maxChars: number): string {
  const chars = Array.from(text);
  return chars.length > maxChars ? chars.slice(0, maxChars).join("") : text;
}

function makeObservationId(): string {
  return `obs-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function makeEditorRequestId(): string {
  return `fim-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function makeSemanticLintRequestId(): string {
  return `lint-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function textCharPosition(editor: Editor, pos: number): number {
  return Array.from(editor.state.doc.textBetween(0, pos, "\n")).length;
}

function docPositionFromTextCharIndex(editor: Editor, targetCharIndex: number): number {
  const target = Math.max(0, Math.min(targetCharIndex, Array.from(editor.getText()).length));
  let low = 0;
  let high = editor.state.doc.content.size;
  let position = 0;

  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const charsBefore = Array.from(editor.state.doc.textBetween(0, mid, "\n")).length;
    if (charsBefore <= target) {
      position = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }

  return Math.max(0, Math.min(position, editor.state.doc.content.size));
}

function previewInlineWriterOperation(
  editor: Editor,
  operation: WriterOperation,
  proposalId: string,
): boolean {
  if (operation.kind === "text.insert") {
    const at = docPositionFromTextCharIndex(editor, operation.at);
    editor.commands.setInlinePreview({
      id: `${proposalId}-${Date.now()}`,
      proposalId,
      kind: operation.kind,
      from: at,
      to: at,
      text: operation.text,
      chapter: operation.chapter,
      revision: operation.revision,
    });
    return true;
  }

  if (operation.kind === "text.replace") {
    const from = docPositionFromTextCharIndex(editor, operation.from);
    const to = docPositionFromTextCharIndex(editor, operation.to);
    editor.commands.setInlinePreview({
      id: `${proposalId}-${Date.now()}`,
      proposalId,
      kind: operation.kind,
      from,
      to,
      text: operation.text,
      chapter: operation.chapter,
      revision: operation.revision,
    });
    return true;
  }

  return false;
}

function sliceAroundCursor(editor: Editor): EditorStatePayload {
  const { from } = editor.state.selection;
  const $from = editor.state.doc.resolve(from);
  const docStart = Math.max(0, from - FIM_PREFIX_CHARS);
  const docEnd = Math.min(editor.state.doc.content.size, from + FIM_SUFFIX_CHARS);
  const prefix = editor.state.doc.textBetween(docStart, from, "\n");
  const suffix = editor.state.doc.textBetween(from, docEnd, "\n");
  const paragraph = editor.state.doc.textBetween($from.start(), $from.end(), " ");

  return {
    requestId: makeEditorRequestId(),
    prefix,
    suffix,
    cursorPosition: from,
    textCursorPosition: textCharPosition(editor, from),
    paragraph,
  };
}

function buildSemanticLintPayload(editor: Editor, chapterTitle: string): SemanticLintPayload {
  const { from } = editor.state.selection;
  const $from = editor.state.doc.resolve(from);

  return {
    requestId: makeSemanticLintRequestId(),
    paragraph: editor.state.doc.textBetween($from.start(), $from.end(), " "),
    paragraphFrom: $from.start(),
    cursorPosition: from,
    chapterTitle,
  };
}

function buildParallelDraftPayload(editor: Editor, chapterTitle: string): ParallelDraftPayload {
  const { from, to } = editor.state.selection;
  const $from = editor.state.doc.resolve(from);
  const docStart = Math.max(0, from - PARALLEL_DRAFT_PREFIX_CHARS);
  const docEnd = Math.min(editor.state.doc.content.size, to + PARALLEL_DRAFT_SUFFIX_CHARS);

  return {
    prefix: editor.state.doc.textBetween(docStart, from, "\n"),
    suffix: editor.state.doc.textBetween(to, docEnd, "\n"),
    paragraph: editor.state.doc.textBetween($from.start(), $from.end(), " "),
    selectedText: from < to ? editor.state.doc.textBetween(from, to, " ") : "",
    chapterTitle,
  };
}

function buildObservation(
  editor: Editor,
  mode: AgentMode,
  reason: AgentObservationReason,
  chapterTitle: string,
  chapterRevision: string | null,
  dirty: boolean,
  snoozedUntil: number | null,
  lastEditAt: number,
): AgentObservation {
  const now = Date.now();
  const { from, to } = editor.state.selection;
  const selectedText = editor.state.doc.textBetween(from, to, " ");
  const $from = editor.state.doc.resolve(from);
  const paragraph = editor.state.doc.textBetween($from.start(), $from.end(), " ");
  const fullText = editor.getText();
  const cursorInText = textCharPosition(editor, from);
  const halfWindow = Math.floor(OBSERVATION_WINDOW_CHARS / 2);
  const fullChars = Array.from(fullText);
  const start = Math.max(0, cursorInText - halfWindow);
  const end = Math.min(fullChars.length, cursorInText + halfWindow);

  return {
    id: makeObservationId(),
    mode,
    reason,
    createdAt: now,
    chapterTitle,
    chapterRevision: chapterRevision ?? undefined,
    dirty,
    cursorPosition: cursorInText,
    selection:
      from < to
        ? {
            from: textCharPosition(editor, from),
            to: textCharPosition(editor, to),
            text: limitChars(selectedText, PARAGRAPH_BUDGET_CHARS),
          }
        : undefined,
    currentParagraph: limitChars(paragraph, PARAGRAPH_BUDGET_CHARS),
    nearbyText: limitChars(fullChars.slice(start, end).join(""), OBSERVATION_WINDOW_CHARS),
    recentEditSummary:
      reason === "selection_change"
        ? "Selection changed after user pause."
        : `Editor paused after local changes in ${chapterTitle}.`,
    idleMs: Math.max(0, now - lastEditAt),
    snoozedUntil: snoozedUntil ?? undefined,
    outlineChapterTitle: chapterTitle,
  };
}

type ActiveGhostState = NonNullable<ReturnType<typeof ghostTextPluginKey.getState>>;

async function recordGhostAcceptance(ghost: ActiveGhostState) {
  if (!ghost.proposalId) return;
  const candidate = ghost.candidates[ghost.activeIndex];
  const text = candidate?.text ?? ghost.text;
  const branch = candidate?.label ? `Accepted inline ghost branch: ${candidate.label}` : "Accepted inline ghost.";
  const feedback: ProposalFeedback = {
    proposalId: ghost.proposalId,
    action: "accepted",
    finalText: text,
    reason: branch,
    createdAt: Date.now(),
  };
  await invoke(Commands.applyProposalFeedback, { feedback });
}

async function recordImplicitGhostRejection(ghost: ActiveGhostState | null | undefined) {
  if (!ghost?.proposalId) return;
  await invoke<boolean>(Commands.recordImplicitGhostRejection, {
    proposalId: ghost.proposalId,
    createdAt: Date.now(),
  });
}

export default function EditorPanel({
  onEditorReady,
  onSelectionUpdate,
}: EditorPanelProps) {
  const actionEpoch = useAppStore((s) => s.actionEpoch);
  const setIsInlineRequest = useAppStore((s) => s.setIsInlineRequest);
  const incrementActionEpoch = useAppStore((s) => s.incrementActionEpoch);
  const agentMode = useAppStore((s) => s.agentMode);
  const latestSuggestion = useAppStore((s) => s.suggestionQueue[0] ?? null);
  const enqueueSuggestion = useAppStore((s) => s.enqueueSuggestion);
  const acceptSuggestion = useAppStore((s) => s.acceptSuggestion);
  const rejectSuggestion = useAppStore((s) => s.rejectSuggestion);
  const snoozeSuggestions = useAppStore((s) => s.snoozeSuggestions);
  const setLatestObservation = useAppStore((s) => s.setLatestObservation);
  const snoozedUntil = useAppStore((s) => s.snoozedUntil);
  const clearExpiredSnooze = useAppStore((s) => s.clearExpiredSnooze);
  const entityCards = useAppStore((s) => s.entityCards);
  const [showToast, setShowToast] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [bubbleVisible, setBubbleVisible] = useState(false);
  const [bubbleThinking, setBubbleThinking] = useState(false);
  const [inlinePreview, setInlinePreview] = useState<InlinePreviewState | null>(null);
  const [saveIndicator, setSaveIndicator] = useState<string | null>(null);
  const [entityHover, setEntityHover] = useState<EntityHoverState | null>(null);
  const [parallelDraftsOpen, setParallelDraftsOpen] = useState(false);
  const [parallelDraftsLoading, setParallelDraftsLoading] = useState(false);
  const [parallelDraftsError, setParallelDraftsError] = useState<string | null>(null);
  const [parallelDrafts, setParallelDrafts] = useState<ParallelDraft[]>([]);
  const editor = useEditor({
    extensions: [
      StarterKit,
      CommentMark,
      GhostText,
      InlinePreview,
      SemanticLint,
      EntityAnchor,
      PatchMark,
    ],
    content: "<p>Start writing your novel here...</p>",
    editorProps: {
      attributes: {
        class:
          "prose prose-invert max-w-none h-full focus:outline-none px-8 py-6 text-text-primary leading-relaxed font-editor",
      },
    },
  });

  useEffect(() => {
    if (editor && onEditorReady) {
      onEditorReady(editor);
    }
  }, [editor, onEditorReady]);

  useEffect(() => {
    if (!editor) return;
    editor.commands.setEntityAnchors(entityCards);
  }, [editor, entityCards]);

  useEffect(() => {
    if (!editor) return;
    const root = editor.view.dom;
    const handleMouseMove = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      const anchor = target?.closest<HTMLElement>(".entity-anchor");
      if (!anchor) {
        setEntityHover(null);
        return;
      }

      const keyword = anchor.dataset.entityKeyword;
      const card = entityCards.find((entry) => entry.keyword === keyword);
      if (!card) {
        setEntityHover(null);
        return;
      }

      setEntityHover({
        card,
        x: Math.min(event.clientX + 14, window.innerWidth - 340),
        y: Math.min(event.clientY + 16, window.innerHeight - 220),
      });
    };
    const handleMouseLeave = () => setEntityHover(null);

    root.addEventListener("mousemove", handleMouseMove);
    root.addEventListener("mouseleave", handleMouseLeave);
    return () => {
      root.removeEventListener("mousemove", handleMouseMove);
      root.removeEventListener("mouseleave", handleMouseLeave);
    };
  }, [editor, entityCards]);

  useEffect(() => {
    if (actionEpoch && actionEpoch > 0) {
      const showTimer = setTimeout(() => setShowToast(true), 0);
      const hideTimer = setTimeout(() => setShowToast(false), 4000);
      return () => {
        clearTimeout(showTimer);
        clearTimeout(hideTimer);
      };
    }
  }, [actionEpoch]);

  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const currentChapter = useAppStore((s) => s.currentChapter);
  const currentChapterRevision = useAppStore((s) => s.currentChapterRevision);
  const isEditorDirty = useAppStore((s) => s.isEditorDirty);
  const setIsEditorDirty = useAppStore((s) => s.setIsEditorDirty);
  const setCurrentChapterRevision = useAppStore((s) => s.setCurrentChapterRevision);
  const observationTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const telemetryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const semanticLintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeGhostRequestIdRef = useRef<string | null>(null);
  const activeSemanticLintRequestIdRef = useRef<string | null>(null);
  const currentChapterRef = useRef(currentChapter);
  const currentChapterRevisionRef = useRef<string | null>(currentChapterRevision);
  const editSequenceRef = useRef(0);
  const lastEditAtRef = useRef(0);
  const lastObservationKeyRef = useRef("");
  const lastSemanticLintKeyRef = useRef("");

  useEffect(() => {
    lastEditAtRef.current = Date.now();
  }, []);

  useEffect(() => {
    currentChapterRef.current = currentChapter;
  }, [currentChapter]);

  useEffect(() => {
    currentChapterRevisionRef.current = currentChapterRevision;
  }, [currentChapterRevision]);

  useEffect(() => {
    if (agentMode === "off" || !editor) return;

    let unlistenSuggestion: (() => void) | undefined;
    const setup = async () => {
      unlistenSuggestion = await listen<AgentSuggestion>(Events.agentSuggestion, (event) => {
        enqueueSuggestion(event.payload);
      });
    };
    setup();
    return () => {
      if (unlistenSuggestion) unlistenSuggestion();
    };
  }, [agentMode, editor, enqueueSuggestion]);

  const submitObservation = useCallback(
    async (reason: AgentObservationReason) => {
      if (!editor) return;
      const now = Date.now();
      clearExpiredSnooze(now);
      const observation = buildObservation(
        editor,
        agentMode,
        reason,
        currentChapter,
        currentChapterRevision,
        isEditorDirty,
        snoozedUntil,
        lastEditAtRef.current,
      );
      const key = [
        observation.mode,
        observation.reason,
        observation.chapterTitle,
        observation.cursorPosition,
        observation.selection?.text ?? "",
        observation.currentParagraph,
        observation.snoozedUntil ?? "",
      ].join("|");
      if (key === lastObservationKeyRef.current) return;
      lastObservationKeyRef.current = key;
      setLatestObservation(observation);
      try {
        await invoke(Commands.agentObserve, { observation });
      } catch (e) {
        console.error("Agent observation failed:", e);
      }
    },
    [
      agentMode,
      clearExpiredSnooze,
      currentChapter,
      currentChapterRevision,
      editor,
      isEditorDirty,
      setLatestObservation,
      snoozedUntil,
    ],
  );

  const scheduleObservation = useCallback(
    (reason: AgentObservationReason) => {
      if (agentMode === "off") return;
      if (observationTimerRef.current) clearTimeout(observationTimerRef.current);
      observationTimerRef.current = setTimeout(() => {
        void submitObservation(reason);
      }, OBSERVATION_DEBOUNCE_MS);
    },
    [agentMode, submitObservation],
  );

  const abortGhostRequest = useCallback((requestId?: string | null, recordImplicit = false) => {
    if (telemetryTimerRef.current) {
      clearTimeout(telemetryTimerRef.current);
      telemetryTimerRef.current = null;
    }

    const activeRequestId = requestId ?? activeGhostRequestIdRef.current;
    const activeGhost = recordImplicit && editor ? ghostTextPluginKey.getState(editor.state) : null;
    editor?.commands.clearGhostText();
    if (activeGhost) {
      void recordImplicitGhostRejection(activeGhost).catch((e) => {
        console.error("Failed to record implicit ghost rejection:", e);
      });
    }
    if (!activeRequestId) return;
    activeGhostRequestIdRef.current = null;
    void invoke(Commands.abortEditorPrediction, { requestId: activeRequestId }).catch((e) => {
      console.error("Failed to abort ghost completion:", e);
    });
  }, [editor]);

  const scheduleEditorTelemetry = useCallback(() => {
    if (!editor || agentMode === "off") return;
    if (telemetryTimerRef.current) clearTimeout(telemetryTimerRef.current);
    telemetryTimerRef.current = setTimeout(() => {
      if (editor.state.selection.from !== editor.state.selection.to) return;

      const payload = {
        ...sliceAroundCursor(editor),
        chapterTitle: currentChapter,
        chapterRevision: currentChapterRevision ?? undefined,
        editorDirty: isEditorDirty,
      };
      activeGhostRequestIdRef.current = payload.requestId;

      void invoke(Commands.reportEditorState, { payload }).catch((e) => {
        if (activeGhostRequestIdRef.current === payload.requestId) {
          activeGhostRequestIdRef.current = null;
        }
        console.error("Editor telemetry failed:", e);
      });
    }, EDITOR_TELEMETRY_DEBOUNCE_MS);
  }, [agentMode, currentChapter, currentChapterRevision, editor, isEditorDirty]);

  const scheduleSemanticLint = useCallback(() => {
    if (!editor || agentMode === "off") return;
    if (semanticLintTimerRef.current) clearTimeout(semanticLintTimerRef.current);

    const selectionAtSchedule = editor.state.selection.from;
    const paragraphAtSchedule = (() => {
      const $from = editor.state.doc.resolve(selectionAtSchedule);
      return editor.state.doc.textBetween($from.start(), $from.end(), " ");
    })();

    semanticLintTimerRef.current = setTimeout(() => {
      if (!editor || editor.state.selection.from !== selectionAtSchedule) return;
      const payload = buildSemanticLintPayload(editor, currentChapter);
      const key = [
        payload.chapterTitle ?? "",
        payload.cursorPosition,
        payload.paragraphFrom,
        payload.paragraph,
      ].join("|");

      if (key === lastSemanticLintKeyRef.current || payload.paragraph !== paragraphAtSchedule) {
        return;
      }

      lastSemanticLintKeyRef.current = key;
      activeSemanticLintRequestIdRef.current = payload.requestId;
      void invoke(Commands.reportSemanticLintState, { payload }).catch((e) => {
        if (activeSemanticLintRequestIdRef.current === payload.requestId) {
          activeSemanticLintRequestIdRef.current = null;
        }
        console.error("Semantic lint failed:", e);
      });
    }, SEMANTIC_LINT_IDLE_MS);
  }, [agentMode, currentChapter, editor]);

  useEffect(() => {
    if (!editor || agentMode === "off") return;

    let unlistenChunk: (() => void) | undefined;
    let unlistenEnd: (() => void) | undefined;

    const setup = async () => {
      unlistenChunk = await listen<EditorGhostChunk>(Events.editorGhostChunk, (event) => {
        const chunk = event.payload;
        const selection = editor.state.selection;
        if (
          chunk.requestId !== activeGhostRequestIdRef.current ||
          chunk.cursorPosition !== selection.from ||
          selection.from !== selection.to
        ) {
          return;
        }

        const currentGhost = ghostTextPluginKey.getState(editor.state);
        if (!currentGhost) {
          const candidates =
            chunk.candidates && chunk.candidates.length > 0
              ? chunk.candidates
              : [{ id: "a", label: "A", text: chunk.content }];
          editor.commands.setGhostText({
            requestId: chunk.requestId,
            proposalId: chunk.proposalId,
            position: chunk.cursorPosition,
            text: chunk.content,
            intent: chunk.intent,
            candidates,
            activeIndex: 0,
          });
          return;
        }

        if (chunk.replace) {
          const candidates =
            chunk.candidates && chunk.candidates.length > 0
              ? chunk.candidates
              : [{ id: "a", label: "A", text: chunk.content }];
          editor.commands.setGhostText({
            requestId: chunk.requestId,
            proposalId: chunk.proposalId,
            position: chunk.cursorPosition,
            text: chunk.content,
            intent: chunk.intent,
            candidates,
            activeIndex: 0,
          });
          return;
        }

        editor.commands.appendGhostText(chunk.requestId, chunk.cursorPosition, chunk.content);
      });

      unlistenEnd = await listen<EditorGhostEnd>(Events.editorGhostEnd, (event) => {
        const end = event.payload;
        if (end.requestId !== activeGhostRequestIdRef.current) return;
        activeGhostRequestIdRef.current = null;
        if (end.reason !== "complete") {
          editor.commands.clearGhostText();
        }
      });
    };

    setup();
    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenEnd) unlistenEnd();
    };
  }, [agentMode, editor]);

  useEffect(() => {
    if (!editor || agentMode === "off") return;

    let unlistenLint: (() => void) | undefined;
    const setup = async () => {
      unlistenLint = await listen<EditorSemanticLint>(Events.editorSemanticLint, (event) => {
        const lint = event.payload;
        const selection = editor.state.selection;
        const isAmbientLint = lint.requestId.startsWith("ambient-lint-");
        if (
          !isAmbientLint &&
          (lint.requestId !== activeSemanticLintRequestIdRef.current ||
            lint.cursorPosition !== selection.from)
        ) {
          return;
        }

        if (!isAmbientLint) {
          activeSemanticLintRequestIdRef.current = null;
        }
        editor.commands.setSemanticLint(lint);
      });
    };

    setup();
    return () => {
      if (unlistenLint) unlistenLint();
    };
  }, [agentMode, editor]);

  useEffect(() => {
    if (!editor || !onSelectionUpdate) return;
    const handler = () => {
      const { from, to } = editor.state.selection;
      const text = editor.state.doc.textBetween(from, to, " ");
      onSelectionUpdate({ from, to, text });
      abortGhostRequest();
      scheduleEditorTelemetry();
      scheduleSemanticLint();
      if (from < to) {
        scheduleObservation("selection_change");
      }
    };
    editor.on("selectionUpdate", handler);
    return () => {
      editor.off("selectionUpdate", handler);
    };
  }, [
    abortGhostRequest,
    editor,
    onSelectionUpdate,
    scheduleEditorTelemetry,
    scheduleObservation,
    scheduleSemanticLint,
  ]);

  // Debounced auto-save: 3s after typing stops
  useEffect(() => {
    if (!editor) return;
    const handler = () => {
      const editSequence = editSequenceRef.current + 1;
      editSequenceRef.current = editSequence;
      lastEditAtRef.current = Date.now();
      setIsEditorDirty(true);
      abortGhostRequest(undefined, true);
      if (!inlinePreviewPluginKey.getState(editor.state)) {
        setInlinePreview(null);
      }
      editor.commands.clearSemanticLint();
      scheduleEditorTelemetry();
      scheduleSemanticLint();
      scheduleObservation("user_typed");
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(async () => {
        const content = editor.getHTML();
        const chapterAtSave = currentChapterRef.current;
        const sequenceAtSave = editSequence;
        try {
          const revision = await invoke<string>(Commands.saveChapter, { title: chapterAtSave, content });
          const isLatestSave =
            editSequenceRef.current === sequenceAtSave &&
            currentChapterRef.current === chapterAtSave &&
            editor.getHTML() === content;
          if (!isLatestSave) return;

          setCurrentChapterRevision(revision);
          currentChapterRevisionRef.current = revision;
          setIsEditorDirty(false);
          const now = new Date();
          setSaveIndicator(
            `Saved at ${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}`,
          );
          setTimeout(() => setSaveIndicator(null), 3000);
        } catch (e) {
          console.error("Auto-save failed:", e);
        }
      }, 3000);
    };
    editor.on("update", handler);
    return () => {
      editor.off("update", handler);
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      if (observationTimerRef.current) clearTimeout(observationTimerRef.current);
      if (telemetryTimerRef.current) clearTimeout(telemetryTimerRef.current);
      if (semanticLintTimerRef.current) clearTimeout(semanticLintTimerRef.current);
      abortGhostRequest();
    };
  }, [
    abortGhostRequest,
    currentChapter,
    editor,
    scheduleEditorTelemetry,
    scheduleObservation,
    scheduleSemanticLint,
    setCurrentChapterRevision,
    setIsEditorDirty,
  ]);

  useEffect(() => {
    if (!editor || agentMode === "off") return;
    scheduleObservation("chapter_switch");
    scheduleSemanticLint();
  }, [agentMode, currentChapter, editor, scheduleObservation, scheduleSemanticLint]);

  useEffect(() => {
    if (!editor) return;
    const handler = (event: KeyboardEvent) => {
      const activeGhost = ghostTextPluginKey.getState(editor.state);
      if (event.key === "Tab" && activeGhost?.text) {
        event.preventDefault();
        const accepted = editor.commands.acceptGhostText();
        if (accepted) {
          activeGhostRequestIdRef.current = null;
          incrementActionEpoch();
          void recordGhostAcceptance(activeGhost).catch((e) => {
            console.error("Failed to record ghost feedback:", e);
          });
        }
        return;
      }

      if (event.key === "ArrowRight" && activeGhost?.text) {
        event.preventDefault();
        editor.commands.nextGhostCandidate();
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key === "k") {
        event.preventDefault();
        setBubbleVisible(true);
      }
      if (event.key === "Escape" && bubbleVisible) {
        setBubbleVisible(false);
      }
    };
    const view = editor.view;
    view.dom.addEventListener("keydown", handler);
    return () => view.dom.removeEventListener("keydown", handler);
  }, [editor, bubbleVisible, incrementActionEpoch]);

  const handleBubbleSubmit = async (command: string) => {
    if (!editor) return;
    const editorRef = { current: editor };
    const requestId = `inline-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const requestChapter = currentChapter;
    const requestRevision = currentChapterRevision ?? "";

    setIsInlineRequest(true);
    setBubbleThinking(true);

    const fullText = editor.getText();
    const { from, to } = editor.state.selection;
    const selectedText = from < to ? editor.state.doc.textBetween(from, to, " ") : "";
    const $from = editor.state.doc.resolve(from);
    const paragraph = editor.state.doc.textBetween($from.start(), $from.end(), " ");

    const cleanup = () => {
      setIsInlineRequest(false);
      setBubbleThinking(false);
      setBubbleVisible(false);
    };

    const unlistenInline = await listen<InlineWriterOperationEvent>(
      Events.inlineWriterOperation,
      (event) => {
        if (event.payload.requestId !== requestId) return;
        unlistenInline();
        void (async () => {
          try {
            const operation = event.payload.operation;
            const liveChapter = currentChapterRef.current;
            const liveRevision = currentChapterRevisionRef.current ?? "";
            if (
              !("chapter" in operation) ||
              operation.chapter !== requestChapter ||
              operation.chapter !== liveChapter ||
              liveRevision !== requestRevision
            ) {
              cleanup();
              console.error("Inline writer operation discarded because chapter context changed.");
              return;
            }

            const result = await invoke<{
              success: boolean;
              error?: { code: string; message: string };
            }>(Commands.approveWriterOperation, {
              operation,
              currentRevision: liveRevision,
            });
            cleanup();
            if (!result.success) {
              console.error(
                "Inline writer operation rejected:",
                result.error?.message ?? "Operation rejected",
              );
              return;
            }
            if (currentChapterRef.current !== requestChapter) {
              console.error("Inline writer operation discarded before apply because chapter changed.");
              return;
            }

            const applied = previewInlineWriterOperation(
              editorRef.current,
              operation,
              event.payload.proposal.id,
            );
            if (applied) {
              const preview = inlinePreviewPluginKey.getState(editorRef.current.state);
              setInlinePreview(preview ?? null);
              incrementActionEpoch();
            }
          } catch (e) {
            cleanup();
            console.error("Inline writer operation approval failed:", e);
          }
        })();
      },
    );

    const unlistenError = await listen<{ message: string }>(Events.agentError, () => {
      unlistenInline();
      unlistenError();
      cleanup();
    });

    try {
      await invoke(Commands.askAgent, {
        message: command,
        context: fullText,
        paragraph,
        selectedText,
        contextPayload: {
          mode: "inline_operation",
          requestId,
          chapterTitle: currentChapter,
          chapterRevision: currentChapterRevision ?? undefined,
          cursorPosition: textCharPosition(editor, editor.state.selection.from),
          dirty: isEditorDirty,
        },
      });
      unlistenError();
    } catch {
      unlistenInline();
      unlistenError();
      cleanup();
    }
  };

  const handleBubbleDismiss = () => {
    setBubbleVisible(false);
  };

  const handleAccept = useCallback(() => {
    if (!editor) return;
    const preview = inlinePreviewPluginKey.getState(editor.state);
    if (!preview) {
      setInlinePreview(null);
      return;
    }
    if (preview.chapter !== currentChapterRef.current) {
      editor.commands.clearInlinePreview();
      setInlinePreview(null);
      console.error("Inline preview discarded because chapter changed.");
      return;
    }

    const applied = editor.commands.applyInlinePreview();
    if (!applied) return;
    setInlinePreview(null);
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }

    const content = editor.getHTML();
    const chapterAtSave = currentChapterRef.current;
    const sequenceAtSave = editSequenceRef.current;
    void (async () => {
      try {
        const revision = await invoke<string>(Commands.saveChapter, { title: chapterAtSave, content });
        const isStillCurrent =
          editSequenceRef.current === sequenceAtSave &&
          currentChapterRef.current === chapterAtSave &&
          editor.getHTML() === content;
        if (isStillCurrent) {
          setCurrentChapterRevision(revision);
          currentChapterRevisionRef.current = revision;
          setIsEditorDirty(false);
        }
      } catch (e) {
        setIsEditorDirty(true);
        console.error("Failed to save accepted inline operation:", e);
        return;
      }

      if (!preview.proposalId) return;
      const feedback: ProposalFeedback = {
        proposalId: preview.proposalId,
        action: "accepted",
        finalText: preview.text,
        reason: "Accepted inline typed operation preview after save.",
        createdAt: Date.now(),
      };
      try {
        await invoke(Commands.applyProposalFeedback, { feedback });
      } catch (e) {
        console.error("Failed to record inline operation feedback:", e);
      }
    })();
  }, [editor, setCurrentChapterRevision, setIsEditorDirty]);

  const handleReject = useCallback(() => {
    if (!editor) return;
    const preview = inlinePreviewPluginKey.getState(editor.state);
    editor.commands.clearInlinePreview();
    setInlinePreview(null);
    if (preview?.proposalId) {
      const feedback: ProposalFeedback = {
        proposalId: preview.proposalId,
        action: "rejected",
        reason: "Rejected inline typed operation preview.",
        createdAt: Date.now(),
      };
      void invoke(Commands.applyProposalFeedback, { feedback }).catch((e) => {
        console.error("Failed to record inline operation rejection:", e);
      });
    }
  }, [editor]);

  const handleAcceptSuggestion = useCallback(
    (suggestion: AgentSuggestion) => {
      if (!editor) return;
      const accepted = acceptSuggestion(suggestion.id);
      if (!accepted) return;
      const insertAt = accepted.targetRange ?? {
        from: accepted.anchorPosition ?? editor.state.selection.from,
        to: accepted.anchorPosition ?? editor.state.selection.from,
      };
      editor
        .chain()
        .focus()
        .insertContentAt({ from: insertAt.from, to: insertAt.to }, accepted.previewText)
        .run();
      incrementActionEpoch();
    },
    [acceptSuggestion, editor, incrementActionEpoch],
  );

  const handleRejectSuggestion = useCallback(
    (suggestion: AgentSuggestion) => {
      rejectSuggestion(suggestion.id);
    },
    [rejectSuggestion],
  );

  const handleSnoozeSuggestions = useCallback(() => {
    snoozeSuggestions(5 * 60 * 1000);
  }, [snoozeSuggestions]);

  const handleGenerateParallelDrafts = useCallback(async () => {
    if (!editor || parallelDraftsLoading) return;
    setParallelDraftsOpen(true);
    setParallelDraftsLoading(true);
    setParallelDraftsError(null);
    try {
      if (!isTauri()) {
        throw new Error("Parallel drafts require the Tauri desktop runtime and an API key.");
      }
      const payload = buildParallelDraftPayload(editor, currentChapter);
      const drafts = await invoke<ParallelDraft[]>(Commands.generateParallelDrafts, { payload });
      setParallelDrafts(drafts);
    } catch (e) {
      setParallelDraftsError(String(e));
    } finally {
      setParallelDraftsLoading(false);
    }
  }, [currentChapter, editor, parallelDraftsLoading]);

  const handleInsertParallelDraft = useCallback(
    (text: string) => {
      if (!editor) return;
      const insertAt = editor.state.selection.from;
      editor.chain().focus().insertContent(text).setTextSelection(insertAt + text.length).run();
      incrementActionEpoch();
    },
    [editor, incrementActionEpoch],
  );

  useEffect(() => {
    if (!editor || !inlinePreview) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Tab") {
        e.preventDefault();
        handleAccept();
      } else if (e.key === "Escape") {
        e.preventDefault();
        handleReject();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editor, inlinePreview, handleAccept, handleReject]);

  const btnActive = (active: boolean) =>
    active ? "bg-bg-raised text-accent" : "text-text-muted hover:text-text-primary";

  if (!editor) {
    return (
      <div className="flex items-center justify-center h-full text-text-muted">
        Loading editor...
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="px-4 py-2.5 border-b border-border-subtle text-sm text-text-secondary flex items-center justify-between relative">
        <span className="font-display tracking-wider text-xs">Editor</span>
        {saveIndicator && (
          <span className="text-[10px] text-text-muted tracking-wide ml-2">
            {saveIndicator}
          </span>
        )}
        {showToast && (
          <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 px-3 py-1.5 rounded-sm bg-success/20 border border-success text-success text-xs whitespace-nowrap">
            AI writing complete. Press Ctrl+Z / Cmd+Z to undo
          </div>
        )}
        <div className="flex gap-1">
          <button
            onClick={() => editor.chain().focus().toggleBold().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor transition-colors ${btnActive(editor.isActive("bold"))}`}
            title="Bold"
          >
            B
          </button>
          <button
            onClick={() => editor.chain().focus().toggleItalic().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor italic transition-colors ${btnActive(editor.isActive("italic"))}`}
            title="Italic"
          >
            I
          </button>
          <button
            onClick={() => editor.chain().focus().toggleStrike().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor line-through transition-colors ${btnActive(editor.isActive("strike"))}`}
            title="Strikethrough"
          >
            S
          </button>
          <span className="w-px h-4 bg-border-subtle mx-1 self-center" />
          <button
            onClick={() => setDrawerOpen(!drawerOpen)}
            className={`px-2.5 py-1 rounded-sm text-xs transition-colors ${
              drawerOpen ? "bg-bg-raised text-accent" : "text-text-muted hover:text-text-primary"
            }`}
            title="Lorebook"
          >
            Lorebook
          </button>
          <button
            onClick={handleGenerateParallelDrafts}
            className={`px-2.5 py-1 rounded-sm text-xs transition-colors ${
              parallelDraftsOpen
                ? "bg-bg-raised text-accent"
                : "text-text-muted hover:text-text-primary"
            }`}
            title="Parallel drafts"
          >
            Drafts
          </button>
          <span className="w-px h-4 bg-border-subtle mx-1 self-center" />
          <button
            onClick={() => editor.commands.toggleHeading({ level: 2 })}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor transition-colors ${btnActive(editor.isActive("heading", { level: 2 }))}`}
            title="Heading"
          >
            H
          </button>
          <button
            onClick={() => editor.chain().focus().toggleBlockquote().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor transition-colors ${btnActive(editor.isActive("blockquote"))}`}
            title="Blockquote"
          >
            &ldquo;
          </button>
        </div>
      </div>

      <LorebookDrawer
        isOpen={drawerOpen}
        onClose={() => setDrawerOpen(false)}
      />
      <div className="flex-1 min-h-0 relative flex">
        <div className="flex-1 min-w-0 flex flex-col relative">
          <div className="flex-1 overflow-y-auto relative">
            <EditorContent editor={editor} />
          </div>
          <CoWriterStatusBar />
        </div>
        {parallelDraftsOpen && (
          <ParallelDraftsPane
            drafts={parallelDrafts}
            loading={parallelDraftsLoading}
            error={parallelDraftsError}
            onInsert={handleInsertParallelDraft}
            onClose={() => setParallelDraftsOpen(false)}
          />
        )}
        {bubbleVisible && (
          <InlineCommandBubble
            editor={editor}
            onSubmit={handleBubbleSubmit}
            onDismiss={handleBubbleDismiss}
            isThinking={bubbleThinking}
            onStop={() => {}}
          />
        )}
        {inlinePreview && (
          <div className="absolute bottom-4 right-4 flex items-center gap-2 bg-bg-raised border border-accent rounded-sm px-3 py-2 shadow-lg z-40">
            <span className="text-xs text-accent">AI Preview</span>
            <span className="w-px h-4 bg-border-subtle" />
            <button
              onClick={handleAccept}
              className="text-xs text-bg-deep bg-success hover:bg-success/80 px-2.5 py-0.5 rounded-sm transition-colors"
            >
              Accept (Tab)
            </button>
            <button
              onClick={handleReject}
              className="text-xs text-danger hover:text-danger/80 px-2.5 py-0.5 transition-colors"
            >
              Reject (Esc)
            </button>
          </div>
        )}
        {latestSuggestion && (
          <AgentSuggestionOverlay
            suggestion={latestSuggestion}
            onAccept={handleAcceptSuggestion}
            onReject={handleRejectSuggestion}
            onSnooze={handleSnoozeSuggestions}
          />
        )}
        <PatchReviewOverlay />
        <EntityHoverCard hover={entityHover} />
      </div>
    </div>
  );
}
