import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import { Commands, Events, type ChapterGenerationEvent, type ProjectFileRestored, type WriterAgentLedgerSnapshot } from "../protocol";

interface OutlineNode {
  chapter_title: string;
  summary: string;
  status: string;
}

interface BatchStatus {
  chapter_title: string;
  status: string;
  error: string;
}

export default function OutlinePanel() {
  const [nodes, setNodes] = useState<OutlineNode[]>([]);
  const [chapterTitle, setChapterTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [generating, setGenerating] = useState<Set<string>>(new Set());
  const [missions, setMissions] = useState<Record<string, string>>({});
  const [toast, setToast] = useState<string | null>(null);
  const currentChapter = useAppStore((s) => s.currentChapter);
  const currentChapterRevision = useAppStore((s) => s.currentChapterRevision);
  const isEditorDirty = useAppStore((s) => s.isEditorDirty);

  const refresh = useCallback(async () => {
    try {
      const [outline, ledger] = await Promise.all([
        invoke<OutlineNode[]>(Commands.getOutline),
        invoke<WriterAgentLedgerSnapshot>(Commands.getWriterAgentLedger).catch(() => null),
      ]);
      setNodes(outline);
      if (ledger?.chapterMissions) {
        const map: Record<string, string> = {};
        for (const m of ledger.chapterMissions) {
          map[m.chapterTitle] = m.status;
        }
        setMissions(map);
      }
    } catch (e) {
      console.error("Failed to load outline:", e);
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
    let unlistenChapter: UnlistenFn;
    const setup = async () => {
      unlisten = await listen<BatchStatus>(Events.batchStatus, (event) => {
        const { chapter_title, status, error } = event.payload;
        if (status === "generating") {
          setGenerating((prev) => new Set(prev).add(chapter_title));
        } else {
          setGenerating((prev) => {
            const next = new Set(prev);
            next.delete(chapter_title);
            return next;
          });
          if (status === "complete") {
            setToast(`${chapter_title} generated successfully`);
          } else {
            setToast(`Error: ${chapter_title} - ${error}`);
          }
          refresh();
        }
      });
      unlistenChapter = await listen<ChapterGenerationEvent>(Events.chapterGeneration, (event) => {
        if (event.payload.phase === "chapter_generation_completed") {
          setToast(`${event.payload.saved?.chapterTitle ?? "Chapter"} drafted successfully`);
          refresh();
        }
      });
    };
    setup();
    return () => {
      if (unlisten) unlisten();
      if (unlistenChapter) unlistenChapter();
    };
  }, [refresh]);

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<ProjectFileRestored>).detail;
      if (detail?.kind === "outline") {
        void refresh();
      }
    };
    window.addEventListener(Events.projectFileRestored, handler);
    return () => window.removeEventListener(Events.projectFileRestored, handler);
  }, [refresh]);

  useEffect(() => {
    if (toast) {
      const t = setTimeout(() => setToast(null), 5000);
      return () => clearTimeout(t);
    }
  }, [toast]);

  const handleSave = async () => {
    const title = chapterTitle.trim();
    const sum = summary.trim();
    if (!title || !sum) return;
    try {
      const result = await invoke<OutlineNode[]>(Commands.saveOutlineNode, {
        chapterTitle: title,
        summary: sum,
      });
      setNodes(result);
      setChapterTitle("");
      setSummary("");
    } catch (e) {
      console.error("Failed to save outline node:", e);
    }
  };

  const handleDelete = async (title: string) => {
    try {
      const result = await invoke<OutlineNode[]>(Commands.deleteOutlineNode, {
        chapterTitle: title,
      });
      setNodes(result);
    } catch (e) {
      console.error("Failed to delete outline node:", e);
    }
  };

  const handleGenerate = async (node: OutlineNode) => {
    try {
      await invoke(Commands.batchGenerateChapter, {
        chapterTitle: node.chapter_title,
        summary: node.summary,
        frontendState: {
          openChapterTitle: currentChapter,
          openChapterRevision: currentChapterRevision ?? undefined,
          dirty: isEditorDirty,
        },
      });
    } catch (e) {
      console.error("Failed to start generation:", e);
    }
  };

  const statusColors: Record<string, string> = {
    empty: "bg-bg-raised text-text-muted",
    generated: "bg-success/20 text-success border border-success/30",
    drafted: "bg-success/20 text-success border border-success/30",
    polished: "bg-accent-subtle text-accent border border-accent/30",
  };

  const missionColors: Record<string, string> = {
    draft: "bg-bg-raised text-text-muted",
    active: "bg-accent/20 text-accent border border-accent/30",
    completed: "bg-success/20 text-success border border-success/30",
    drifted: "bg-danger/20 text-danger border border-danger/30",
    blocked: "bg-warning/20 text-warning border border-warning/30",
    needs_review: "bg-warning/20 text-warning border border-warning/30",
    retired: "bg-bg-raised text-text-muted border border-border-subtle",
  };

  return (
    <div className="flex flex-col h-full relative">
      {toast && (
        <div className="absolute top-1 left-1/2 -translate-x-1/2 z-50 px-3 py-1.5 rounded-sm bg-success/20 border border-success text-success text-xs whitespace-nowrap">
          {toast}
        </div>
      )}
      <div className="flex-1 overflow-y-auto">
        {nodes.map((node) => {
          const isGen = generating.has(node.chapter_title);
          return (
            <div
              key={node.chapter_title}
              className="px-3 py-2.5 border-b border-border-subtle"
            >
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs text-text-primary font-display tracking-wider">
                  {node.chapter_title}
                </span>
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => handleGenerate(node)}
                    disabled={isGen}
                    className="text-[10px] px-1.5 py-0.5 rounded-sm bg-accent hover:bg-accent/80 disabled:opacity-50 text-bg-deep transition-colors flex items-center gap-1"
                  >
                    {isGen ? (
                      <>
                        <span className="inline-block w-2 h-2 border border-bg-deep border-t-transparent rounded-full animate-spin" />
                        ...
                      </>
                    ) : (
                      "Generate"
                    )}
                  </button>
                  <button
                    onClick={() => handleDelete(node.chapter_title)}
                    className="text-text-muted hover:text-danger text-xs transition-colors"
                  >
                    ×
                  </button>
                </div>
              </div>
              <p className="text-xs text-text-secondary leading-relaxed line-clamp-2">
                {node.summary}
              </p>
              <span
                className={`inline-block mt-1.5 text-[10px] px-1.5 py-0.5 rounded-sm ${statusColors[node.status] || statusColors.empty}`}
              >
                {node.status}
              </span>
              {missions[node.chapter_title] && (
                <span
                  className={`inline-block mt-1.5 ml-1 text-[10px] px-1.5 py-0.5 rounded-sm ${missionColors[missions[node.chapter_title]] || missionColors.draft}`}
                >
                  {missions[node.chapter_title].replaceAll("_", " ")}
                </span>
              )}
            </div>
          );
        })}
      </div>
      <div className="p-3 border-t border-border-subtle space-y-1.5">
        <input
          value={chapterTitle}
          onChange={(e) => setChapterTitle(e.target.value)}
          placeholder="Chapter title..."
          className="w-full px-2.5 py-1.5 rounded-sm bg-bg-deep border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent"
        />
        <textarea
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
          placeholder="Summary / beat..."
          rows={2}
          className="w-full px-2.5 py-1.5 rounded-sm bg-bg-deep border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent resize-none"
        />
        <button
          onClick={handleSave}
          className="w-full px-2.5 py-1.5 rounded-sm bg-accent hover:bg-accent/80 text-bg-deep text-xs transition-colors"
        >
          Save Beat
        </button>
      </div>
    </div>
  );
}
