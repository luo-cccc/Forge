import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  const [toast, setToast] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<OutlineNode[]>("get_outline");
      setNodes(result);
    } catch (e) {
      console.error("Failed to load outline:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    let unlisten: UnlistenFn;
    const setup = async () => {
      unlisten = await listen<BatchStatus>("batch-status", (event) => {
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
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
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
      const result = await invoke<OutlineNode[]>("save_outline_node", {
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
      const result = await invoke<OutlineNode[]>("delete_outline_node", {
        chapterTitle: title,
      });
      setNodes(result);
    } catch (e) {
      console.error("Failed to delete outline node:", e);
    }
  };

  const handleGenerate = async (node: OutlineNode) => {
    try {
      await invoke("batch_generate_chapter", {
        chapterTitle: node.chapter_title,
        summary: node.summary,
      });
    } catch (e) {
      console.error("Failed to start generation:", e);
    }
  };

  const statusBadge = (status: string) => {
    const colors: Record<string, string> = {
      empty: "bg-slate-700 text-slate-400",
      generated: "bg-emerald-900/60 text-emerald-300",
      polished: "bg-blue-900/60 text-blue-300",
    };
    return colors[status] || colors.empty;
  };

  return (
    <div className="flex flex-col h-full relative">
      {toast && (
        <div className="absolute top-1 left-1/2 -translate-x-1/2 z-50 px-3 py-1.5 rounded-md bg-emerald-900/95 border border-emerald-700 text-emerald-200 text-xs whitespace-nowrap shadow-lg">
          {toast}
        </div>
      )}
      <div className="flex-1 overflow-y-auto">
        {nodes.map((node) => {
          const isGen = generating.has(node.chapter_title);
          return (
            <div
              key={node.chapter_title}
              className="px-3 py-2 border-b border-slate-800"
            >
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs text-slate-300 font-medium">
                  {node.chapter_title}
                </span>
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => handleGenerate(node)}
                    disabled={isGen}
                    className="text-[10px] px-1.5 py-0.5 rounded bg-purple-700 hover:bg-purple-600 disabled:opacity-50 text-purple-200 transition-colors flex items-center gap-1"
                  >
                    {isGen ? (
                      <>
                        <span className="inline-block w-2 h-2 border border-purple-300 border-t-transparent rounded-full animate-spin" />
                        ...
                      </>
                    ) : (
                      "Generate"
                    )}
                  </button>
                  <button
                    onClick={() => handleDelete(node.chapter_title)}
                    className="text-slate-600 hover:text-red-400 text-xs"
                  >
                    ×
                  </button>
                </div>
              </div>
              <p className="text-xs text-slate-500 leading-relaxed line-clamp-2">
                {node.summary}
              </p>
              <span
                className={`inline-block mt-1 text-[10px] px-1.5 py-0.5 rounded ${statusBadge(node.status)}`}
              >
                {node.status}
              </span>
            </div>
          );
        })}
      </div>
      <div className="p-2 border-t border-slate-700 space-y-1">
        <input
          value={chapterTitle}
          onChange={(e) => setChapterTitle(e.target.value)}
          placeholder="Chapter title..."
          className="w-full px-2 py-1 rounded bg-slate-800 border border-slate-600 text-white text-xs placeholder-slate-500 focus:outline-none focus:border-blue-500"
        />
        <textarea
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
          placeholder="Summary / beat..."
          rows={2}
          className="w-full px-2 py-1 rounded bg-slate-800 border border-slate-600 text-white text-xs placeholder-slate-500 focus:outline-none focus:border-blue-500 resize-none"
        />
        <button
          onClick={handleSave}
          className="w-full px-2 py-1 rounded bg-blue-600 hover:bg-blue-500 text-white text-xs transition-colors"
        >
          Save Beat
        </button>
      </div>
    </div>
  );
}
