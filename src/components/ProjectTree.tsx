import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import OutlinePanel from "./OutlinePanel";

interface ChapterInfo {
  title: string;
  filename: string;
}

interface ProjectTreeProps {
  currentChapter: string;
  onSelectChapter: (title: string) => void;
}

export default function ProjectTree({
  currentChapter,
  onSelectChapter,
}: ProjectTreeProps) {
  const [chapters, setChapters] = useState<ChapterInfo[]>([]);
  const [newTitle, setNewTitle] = useState("");
  const [tab, setTab] = useState<"chapters" | "outline">("chapters");

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<ChapterInfo[]>("read_project_dir");
      setChapters(result);
    } catch (e) {
      console.error("Failed to read project dir:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleCreate = async () => {
    const title = newTitle.trim();
    if (!title) return;
    try {
      await invoke("create_chapter", { title });
      setNewTitle("");
      await refresh();
    } catch (e) {
      console.error("Failed to create chapter:", e);
    }
  };

  return (
    <div className="flex flex-col h-full border-r border-slate-700 bg-slate-900">
      <div className="flex border-b border-slate-700">
        <button
          onClick={() => setTab("chapters")}
          className={`flex-1 py-2 text-xs font-medium transition-colors ${
            tab === "chapters"
              ? "bg-slate-800 text-blue-300 border-b border-blue-500"
              : "text-slate-500 hover:text-slate-300"
          }`}
        >
          Chapters
        </button>
        <button
          onClick={() => setTab("outline")}
          className={`flex-1 py-2 text-xs font-medium transition-colors ${
            tab === "outline"
              ? "bg-slate-800 text-blue-300 border-b border-blue-500"
              : "text-slate-500 hover:text-slate-300"
          }`}
        >
          Outline
        </button>
      </div>

      {tab === "chapters" ? (
        <>
          <div className="flex-1 overflow-y-auto">
            {chapters.map((ch) => (
              <button
                key={ch.filename}
                onClick={() => onSelectChapter(ch.title)}
                className={`w-full text-left px-3 py-2 text-xs transition-colors ${
                  currentChapter === ch.title
                    ? "bg-blue-900/40 text-blue-300 border-l-2 border-blue-500"
                    : "text-slate-400 hover:bg-slate-800 hover:text-slate-200 border-l-2 border-transparent"
                }`}
              >
                {ch.title}
              </button>
            ))}
          </div>
          <div className="p-2 border-t border-slate-700 space-y-1">
            <input
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder="New chapter..."
              className="w-full px-2 py-1 rounded bg-slate-800 border border-slate-600 text-white text-xs placeholder-slate-500 focus:outline-none focus:border-blue-500"
            />
            <button
              onClick={handleCreate}
              className="w-full px-2 py-1 rounded bg-blue-600 hover:bg-blue-500 text-white text-xs transition-colors"
            >
              + New Chapter
            </button>
          </div>
        </>
      ) : (
        <OutlinePanel />
      )}
    </div>
  );
}
