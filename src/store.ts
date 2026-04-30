import { create } from "zustand";

interface OutlineNode {
  chapter_title: string;
  summary: string;
  status: string;
}

interface AppState {
  currentChapter: string;
  setCurrentChapter: (title: string) => void;
  isAgentThinking: boolean;
  setIsAgentThinking: (v: boolean) => void;
  outlineData: OutlineNode[];
  setOutlineData: (data: OutlineNode[]) => void;
  isInlineRequest: boolean;
  setIsInlineRequest: (v: boolean) => void;
  actionEpoch: number;
  incrementActionEpoch: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  currentChapter: "Chapter-1",
  setCurrentChapter: (title) => set({ currentChapter: title }),

  isAgentThinking: false,
  setIsAgentThinking: (v) => set({ isAgentThinking: v }),

  outlineData: [],
  setOutlineData: (data) => set({ outlineData: data }),

  isInlineRequest: false,
  setIsInlineRequest: (v) => set({ isInlineRequest: v }),

  actionEpoch: 0,
  incrementActionEpoch: () => set((s) => ({ actionEpoch: s.actionEpoch + 1 })),
}));
