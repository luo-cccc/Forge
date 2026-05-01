import { create } from "zustand";
import type {
  AgentMode,
  AgentObservation,
  AgentSuggestion,
} from "./protocol";

interface OutlineNode {
  chapter_title: string;
  summary: string;
  status: string;
}

interface AppState {
  currentChapter: string;
  setCurrentChapter: (title: string) => void;
  currentChapterRevision: string | null;
  setCurrentChapterRevision: (revision: string | null) => void;
  isEditorDirty: boolean;
  setIsEditorDirty: (v: boolean) => void;
  isAgentThinking: boolean;
  setIsAgentThinking: (v: boolean) => void;
  outlineData: OutlineNode[];
  setOutlineData: (data: OutlineNode[]) => void;
  isInlineRequest: boolean;
  setIsInlineRequest: (v: boolean) => void;
  actionEpoch: number;
  incrementActionEpoch: () => void;
  agentMode: AgentMode;
  setAgentMode: (mode: AgentMode) => void;
  latestObservation: AgentObservation | null;
  setLatestObservation: (observation: AgentObservation) => void;
  suggestionQueue: AgentSuggestion[];
  acceptedSuggestions: AgentSuggestion[];
  rejectedSuggestions: AgentSuggestion[];
  snoozedUntil: number | null;
  enqueueSuggestion: (suggestion: AgentSuggestion) => void;
  acceptSuggestion: (id: string) => AgentSuggestion | null;
  rejectSuggestion: (id: string) => AgentSuggestion | null;
  dismissSuggestion: (id: string) => void;
  snoozeSuggestions: (durationMs: number) => void;
  clearExpiredSnooze: (now: number) => void;
}

function isSameSuggestionAnchor(a: AgentSuggestion, b: AgentSuggestion): boolean {
  if (a.targetRange && b.targetRange) {
    return a.targetRange.from === b.targetRange.from && a.targetRange.to === b.targetRange.to;
  }
  return a.anchorPosition !== undefined && a.anchorPosition === b.anchorPosition;
}

export const useAppStore = create<AppState>((set) => ({
  currentChapter: "Chapter-1",
  setCurrentChapter: (title) => set({ currentChapter: title }),
  currentChapterRevision: null,
  setCurrentChapterRevision: (revision) => set({ currentChapterRevision: revision }),
  isEditorDirty: false,
  setIsEditorDirty: (v) => set({ isEditorDirty: v }),

  isAgentThinking: false,
  setIsAgentThinking: (v) => set({ isAgentThinking: v }),

  outlineData: [],
  setOutlineData: (data) => set({ outlineData: data }),

  isInlineRequest: false,
  setIsInlineRequest: (v) => set({ isInlineRequest: v }),

  actionEpoch: 0,
  incrementActionEpoch: () => set((s) => ({ actionEpoch: s.actionEpoch + 1 })),

  agentMode: "proactive",
  setAgentMode: (mode) => set({ agentMode: mode }),

  latestObservation: null,
  setLatestObservation: (observation) => set({ latestObservation: observation }),

  suggestionQueue: [],
  acceptedSuggestions: [],
  rejectedSuggestions: [],
  snoozedUntil: null,
  enqueueSuggestion: (suggestion) =>
    set((s) => {
      const filtered = s.suggestionQueue.filter(
        (existing) =>
          existing.id !== suggestion.id && !isSameSuggestionAnchor(existing, suggestion),
      );
      return { suggestionQueue: [suggestion, ...filtered].slice(0, 5) };
    }),
  acceptSuggestion: (id) => {
    let accepted: AgentSuggestion | null = null;
    set((s) => {
      accepted = s.suggestionQueue.find((suggestion) => suggestion.id === id) ?? null;
      if (!accepted) return {};
      return {
        suggestionQueue: s.suggestionQueue.filter((suggestion) => suggestion.id !== id),
        acceptedSuggestions: [accepted, ...s.acceptedSuggestions].slice(0, 20),
      };
    });
    return accepted;
  },
  rejectSuggestion: (id) => {
    let rejected: AgentSuggestion | null = null;
    set((s) => {
      rejected = s.suggestionQueue.find((suggestion) => suggestion.id === id) ?? null;
      if (!rejected) return {};
      return {
        suggestionQueue: s.suggestionQueue.filter((suggestion) => suggestion.id !== id),
        rejectedSuggestions: [rejected, ...s.rejectedSuggestions].slice(0, 50),
      };
    });
    return rejected;
  },
  dismissSuggestion: (id) =>
    set((s) => ({
      suggestionQueue: s.suggestionQueue.filter((suggestion) => suggestion.id !== id),
    })),
  snoozeSuggestions: (durationMs) =>
    set((s) => ({
      snoozedUntil: Date.now() + durationMs,
      suggestionQueue: [],
      rejectedSuggestions: [...s.suggestionQueue, ...s.rejectedSuggestions].slice(0, 50),
    })),
  clearExpiredSnooze: (now) =>
    set((s) => (s.snoozedUntil && s.snoozedUntil <= now ? { snoozedUntil: null } : {})),
}));
