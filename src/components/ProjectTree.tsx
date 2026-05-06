import { Suspense, lazy, useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events, type ChapterGenerationEvent, type SprintProgress, type VolumeSummary } from "../protocol";
import type { Editor } from "@tiptap/core";

const OutlinePanel = lazy(() => import("./OutlinePanel"));
const ScriptDoctorPanel = lazy(() => import("./ScriptDoctorPanel"));
const LoreGraphView = lazy(() => import("./LoreGraphView"));
const StoryboardView = lazy(() => import("./StoryboardView"));
const SandboxView = lazy(() => import("./SandboxView"));
const SettingsView = lazy(() => import("./SettingsView"));

interface ChapterInfo {
  title: string;
  filename: string;
}

interface ProjectTreeProps {
  onSelectChapter: (title: string) => void;
  editorRef: { current: Editor | null };
  onApplyFix: (quote: string, suggestion: string) => void;
}

function PanelFallback() {
  return (
    <div className="flex-1 flex items-center justify-center text-xs text-text-muted">
      Loading panel...
    </div>
  );
}

export default function ProjectTree({ onSelectChapter, editorRef, onApplyFix }: ProjectTreeProps) {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const activeVolumeId = useAppStore((s) => s.activeVolumeId);
  const setActiveVolumeId = useAppStore((s) => s.setActiveVolumeId);
  const volumeList = useAppStore((s) => s.volumeList);
  const setVolumeList = useAppStore((s) => s.setVolumeList);
  const sprintProgress = useAppStore((s) => s.sprintProgress);
  const setSprintProgress = useAppStore((s) => s.setSprintProgress);
  const [chapters, setChapters] = useState<ChapterInfo[]>([]);
  const [newTitle, setNewTitle] = useState("");
  const [tab, setTab] = useState<"chapters" | "outline" | "doctor" | "graph" | "storyboard" | "sandbox" | "settings">("chapters");

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<ChapterInfo[]>(Commands.readProjectDir);
      setChapters(result);
      const volumes = await invoke<VolumeSummary[]>(Commands.listVolumes).catch(() => []);
      setVolumeList(
        volumes.map((volume) => ({
          id: volume.id,
          title: volume.title,
          startChapter: volume.startChapter,
          endChapter: volume.endChapter,
          status: volume.status,
        })),
      );
      const progress = await invoke<SprintProgress | null>(Commands.getSupervisedSprintProgress).catch(() => null);
      setSprintProgress(progress);
    } catch (e) {
      console.error("Failed to read project dir:", e);
    }
  }, [setSprintProgress, setVolumeList]);

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

  const activeVolume = activeVolumeId
    ? volumeList.find((volume) => volume.id === activeVolumeId)
    : null;
  const visibleChapters = chapters.filter((chapter) => {
    if (!activeVolume) return true;
    const digits = chapter.title.replace(/\D+/g, "");
    const chapterNumber = digits ? Number(digits) : NaN;
    if (!Number.isFinite(chapterNumber)) return true;
    return (
      chapterNumber >= activeVolume.startChapter &&
      chapterNumber <= activeVolume.endChapter
    );
  });

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
      {(activeVolumeId || sprintProgress) && (
        <div className="px-3 py-2 border-b border-border-subtle bg-bg-deep text-[10px] text-text-muted space-y-1">
          {activeVolumeId && <div>Volume filter: {activeVolumeId}</div>}
          {sprintProgress && (
            <div>
              Sprint {sprintProgress.status} · {sprintProgress.chaptersCompleted}/
              {sprintProgress.chaptersCompleted + sprintProgress.chaptersRemaining}
            </div>
          )}
        </div>
      )}

      {tab === "chapters" ? (
        <>
          <div className="flex-1 overflow-y-auto">
            {visibleChapters.map((ch) => (
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
            <div className="flex gap-1">
              <button
                onClick={() => setActiveVolumeId(null)}
                className={`px-2 py-1 rounded-sm text-[10px] border ${activeVolumeId === null ? "border-accent text-accent" : "border-border-subtle text-text-muted"}`}
              >
                All
              </button>
              {volumeList.slice(0, 4).map((volume) => (
                <button
                  key={volume.id}
                  onClick={() => setActiveVolumeId(volume.id)}
                  className={`px-2 py-1 rounded-sm text-[10px] border ${activeVolumeId === volume.id ? "border-accent text-accent" : "border-border-subtle text-text-muted"}`}
                >
                  {volume.title}
                </button>
              ))}
            </div>
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
      ) : (
        <Suspense fallback={<PanelFallback />}>
          {tab === "outline" ? (
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
        </Suspense>
      )}
    </div>
  );
}
