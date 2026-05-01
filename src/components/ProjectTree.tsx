import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events, type ChapterGenerationEvent } from "../protocol";
import OutlinePanel from "./OutlinePanel";
import ScriptDoctorPanel from "./ScriptDoctorPanel";
import LoreGraphView from "./LoreGraphView";
import StoryboardView from "./StoryboardView";
import SandboxView from "./SandboxView";
import SettingsView from "./SettingsView";
import type { Editor } from "@tiptap/core";

interface ChapterInfo {
  title: string;
  filename: string;
}

interface ProjectTreeProps {
  onSelectChapter: (title: string) => void;
  editorRef: { current: Editor | null };
  onApplyFix: (quote: string, suggestion: string) => void;
}

export default function ProjectTree({ onSelectChapter, editorRef, onApplyFix }: ProjectTreeProps) {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const [chapters, setChapters] = useState<ChapterInfo[]>([]);
  const [newTitle, setNewTitle] = useState("");
  const [tab, setTab] = useState<"chapters" | "outline" | "doctor" | "graph" | "storyboard" | "sandbox" | "settings">("chapters");

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<ChapterInfo[]>(Commands.readProjectDir);
      setChapters(result);
    } catch (e) {
      console.error("Failed to read project dir:", e);
    }
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      void refresh();
    }, 0);
    return () => clearTimeout(timer);
  }, [refresh]);

  useEffect(() => {
    let unlisten: UnlistenFn;
    const setup = async () => {
      unlisten = await listen<ChapterGenerationEvent>(Events.chapterGeneration, (event) => {
        if (event.payload.phase === "chapter_generation_completed") {
          void refresh();
        }
      });
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [refresh]);

  const handleCreate = async () => {
    const title = newTitle.trim();
    if (!title) return;
    try {
      await invoke(Commands.createChapter, { title });
      setNewTitle("");
      await refresh();
    } catch (e) {
      console.error("Failed to create chapter:", e);
    }
  };

  return (
    <div className="flex flex-col h-full border-r border-border-subtle bg-bg-surface">
      <div className="flex border-b border-border-subtle">
        <button
          onClick={() => setTab("chapters")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "chapters"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          Chapters
        </button>
        <button
          onClick={() => setTab("outline")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "outline"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          Outline
        </button>
        <button
          onClick={() => setTab("doctor")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "doctor"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          Doctor
        </button>
        <button
          onClick={() => setTab("graph")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "graph"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          Graph
        </button>
        <button
          onClick={() => setTab("storyboard")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "storyboard"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          Board
        </button>
        <button
          onClick={() => setTab("sandbox")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "sandbox"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          🧪
        </button>
        <button
          onClick={() => setTab("settings")}
          className={`flex-1 py-2.5 text-xs transition-colors font-display tracking-wider ${
            tab === "settings"
              ? "bg-bg-deep text-accent border-b border-accent"
              : "text-text-muted hover:text-text-secondary"
          }`}
        >
          ⚙
        </button>
      </div>

      {tab === "chapters" ? (
        <>
          <div className="flex-1 overflow-y-auto">
            {chapters.map((ch) => (
              <button
                key={ch.filename}
                onClick={() => onSelectChapter(ch.title)}
                className={`w-full text-left px-4 py-2.5 text-xs transition-colors ${
                  currentChapter === ch.title
                    ? "bg-accent-subtle text-accent border-l-2 border-accent"
                    : "text-text-secondary hover:bg-bg-raised hover:text-text-primary border-l-2 border-transparent"
                }`}
              >
                <span className="mr-1">{currentChapter === ch.title ? "✓" : "□"}</span>
                {ch.title}
              </button>
            ))}
          </div>
          <div className="p-3 border-t border-border-subtle space-y-1.5">
            <input
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder="New chapter..."
              className="w-full px-2.5 py-1.5 rounded-sm bg-bg-deep border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent"
            />
            <button
              onClick={handleCreate}
              className="w-full px-2.5 py-1.5 rounded-sm bg-accent hover:bg-accent/90 text-bg-deep text-xs transition-colors"
            >
              + New Chapter
            </button>
          </div>
        </>
      ) : tab === "outline" ? (
        <OutlinePanel />
      ) : tab === "doctor" ? (
        <ScriptDoctorPanel editorRef={editorRef} onApplyFix={onApplyFix} />
      ) : tab === "graph" ? (
        <LoreGraphView />
      ) : tab === "storyboard" ? (
        <StoryboardView />
      ) : tab === "sandbox" ? (
        <SandboxView />
      ) : (
        <SettingsView />
      )}
    </div>
  );
}
