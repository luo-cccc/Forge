import { Suspense, lazy, useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events, type ChapterGenerationEvent, type SprintProgress, type VolumeSummary } from "../protocol";
import type { Editor } from "@tiptap/core";

const OutlinePanel = lazy(() => import("./OutlinePanel"));

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
      正在载入...
    </div>
  );
}

export default function ProjectTree({ onSelectChapter }: ProjectTreeProps) {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const activeVolumeId = useAppStore((s) => s.activeVolumeId);
  const setActiveVolumeId = useAppStore((s) => s.setActiveVolumeId);
  const volumeList = useAppStore((s) => s.volumeList);
  const setVolumeList = useAppStore((s) => s.setVolumeList);
  const sprintProgress = useAppStore((s) => s.sprintProgress);
  const setSprintProgress = useAppStore((s) => s.setSprintProgress);
  const [chapters, setChapters] = useState<ChapterInfo[]>([]);
  const [newTitle, setNewTitle] = useState("");
  const [tab, setTab] = useState<"chapters" | "outline">("chapters");

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
    <div className="forge-project-panel">
      <div className="forge-project-tabs">
        {([
          ["chapters", "章节"],
          ["outline", "大纲"],
        ] as const).map(([nextTab, label]) => (
          <button
            key={nextTab}
            onClick={() => setTab(nextTab)}
            className={`forge-project-tab ${tab === nextTab ? "active" : ""}`}
          >
            {label}
          </button>
        ))}
      </div>
      {(activeVolumeId || sprintProgress) && (
        <div className="forge-project-context">
          {activeVolumeId && <div>卷筛选：{activeVolumeId}</div>}
          {sprintProgress && (
            <div>
              连续写作 {sprintProgress.status} · {sprintProgress.chaptersCompleted}/
              {sprintProgress.chaptersCompleted + sprintProgress.chaptersRemaining}
            </div>
          )}
        </div>
      )}

      {tab === "chapters" ? (
        <>
          <div className="forge-chapter-list">
            <div className="forge-panel-section-label">章节列表</div>
            {visibleChapters.map((ch, index) => (
              <button
                key={ch.filename}
                onClick={() => onSelectChapter(ch.title)}
                className={`forge-chapter-row ${currentChapter === ch.title ? "active" : ""}`}
              >
                <span>{String(index + 1).padStart(2, "0")}</span>
                <strong>{ch.title}</strong>
              </button>
            ))}
          </div>
          <div className="forge-chapter-create">
            <div className="forge-panel-section-label">项目管理</div>
            <div className="forge-volume-filter">
              <button
                onClick={() => setActiveVolumeId(null)}
                className={`forge-chip ${activeVolumeId === null ? "active" : ""}`}
              >
                全部
              </button>
              {volumeList.slice(0, 4).map((volume) => (
                <button
                  key={volume.id}
                  onClick={() => setActiveVolumeId(volume.id)}
                  className={`forge-chip ${activeVolumeId === volume.id ? "active" : ""}`}
                >
                  {volume.title}
                </button>
              ))}
            </div>
            <input
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder="输入新章节名"
              className="forge-input"
            />
            <button
              onClick={handleCreate}
              className="forge-btn forge-btn-secondary forge-btn-wide"
            >
              新建章节
            </button>
          </div>
        </>
      ) : (
        <Suspense fallback={<PanelFallback />}>
          {tab === "outline" ? (
            <OutlinePanel />
          ) : null}
        </Suspense>
      )}
    </div>
  );
}
