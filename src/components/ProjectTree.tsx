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
    <div className="flex flex-1 items-center justify-center text-xs text-text-muted">
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
    <div className="flex h-full flex-col bg-bg-surface">
      <div className="grid grid-cols-3 gap-1 border-b border-border-subtle p-2">
        {([
          ["chapters", "Chapters"],
          ["outline", "Outline"],
          ["doctor", "Doctor"],
          ["graph", "Graph"],
          ["storyboard", "Board"],
          ["sandbox", "Sandbox"],
          ["settings", "Settings"],
        ] as const).map(([nextTab, label]) => (
          <button
            key={nextTab}
            onClick={() => setTab(nextTab)}
            className={`rounded-md px-2 py-1.5 text-left text-[11px] transition-colors ${
              tab === nextTab
                ? "bg-bg-raised text-text-primary shadow-sm"
                : "text-text-muted hover:bg-bg-raised/60 hover:text-text-secondary"
            }`}
          >
            {label}
          </button>
        ))}
        <span className="hidden" />
      </div>
      {(activeVolumeId || sprintProgress) && (
        <div className="space-y-1 border-b border-border-subtle bg-bg-deep px-3 py-2 text-[10px] text-text-muted">
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
                className={`w-full border-l-2 px-4 py-2.5 text-left text-xs transition-colors ${
                  currentChapter === ch.title
                    ? "border-accent bg-accent-subtle text-text-primary"
                    : "border-transparent text-text-secondary hover:bg-bg-raised hover:text-text-primary"
                }`}
              >
                <span className="mr-2 font-mono text-[10px] text-text-muted">
                  {String(visibleChapters.indexOf(ch) + 1).padStart(2, "0")}
                </span>
                {ch.title}
              </button>
            ))}
          </div>
          <div className="space-y-2 border-t border-border-subtle p-3">
            <div className="flex gap-1">
              <button
                onClick={() => setActiveVolumeId(null)}
                className={`rounded-md border px-2 py-1 text-[10px] ${activeVolumeId === null ? "border-accent text-text-primary" : "border-border-subtle text-text-muted"}`}
              >
                All
              </button>
              {volumeList.slice(0, 4).map((volume) => (
                <button
                  key={volume.id}
                  onClick={() => setActiveVolumeId(volume.id)}
                  className={`rounded-md border px-2 py-1 text-[10px] ${activeVolumeId === volume.id ? "border-accent text-text-primary" : "border-border-subtle text-text-muted"}`}
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
              className="w-full rounded-md border border-border-subtle bg-bg-deep px-2.5 py-1.5 text-xs text-text-primary placeholder-text-muted focus:border-accent focus:outline-none"
            />
            <button
              onClick={handleCreate}
              className="w-full rounded-md bg-accent px-2.5 py-1.5 text-xs font-medium text-bg-deep transition-colors hover:bg-accent/90"
            >
              New Chapter
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
