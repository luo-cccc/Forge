import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface LoreEntry {
  id: string;
  keyword: string;
  content: string;
}

interface LorebookDrawerProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function LorebookDrawer({ isOpen, onClose }: LorebookDrawerProps) {
  const [entries, setEntries] = useState<LoreEntry[]>([]);
  const [keyword, setKeyword] = useState("");
  const [content, setContent] = useState("");

  const fetchEntries = useCallback(async () => {
    try {
      const result = await invoke<LoreEntry[]>("get_lorebook");
      setEntries(result);
    } catch (e) {
      console.error("Failed to load lorebook:", e);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      fetchEntries();
    }
  }, [isOpen, fetchEntries]);

  const handleAdd = async () => {
    const kw = keyword.trim();
    const ct = content.trim();
    if (!kw || !ct) return;

    try {
      const result = await invoke<LoreEntry[]>("save_lore_entry", {
        keyword: kw,
        content: ct,
      });
      setEntries(result);
      setKeyword("");
      setContent("");
    } catch (e) {
      console.error("Failed to save entry:", e);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      const result = await invoke<LoreEntry[]>("delete_lore_entry", { id });
      setEntries(result);
    } catch (e) {
      console.error("Failed to delete entry:", e);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="border-b border-slate-700 bg-slate-850">
      <div className="px-4 py-3 border-b border-slate-700 text-sm text-slate-400 font-medium flex items-center justify-between">
        <span>Lorebook (设定集)</span>
        <button
          onClick={onClose}
          className="text-slate-500 hover:text-white transition-colors"
        >
          ✕
        </button>
      </div>

      <div className="p-4 space-y-3 max-h-80 overflow-y-auto">
        {entries.length === 0 && (
          <p className="text-slate-500 text-xs">No entries yet. Add your first character or setting below.</p>
        )}
        {entries.map((entry) => (
          <div
            key={entry.id}
            className="bg-slate-800 rounded-md p-3 text-sm"
          >
            <div className="flex items-center justify-between mb-1">
              <span className="text-blue-400 font-medium">{entry.keyword}</span>
              <button
                onClick={() => handleDelete(entry.id)}
                className="text-slate-500 hover:text-red-400 text-xs transition-colors"
              >
                Delete
              </button>
            </div>
            <p className="text-slate-300 text-xs leading-relaxed">{entry.content}</p>
          </div>
        ))}

        <div className="pt-3 border-t border-slate-700 space-y-2">
          <input
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder="Keyword (e.g. 林墨)"
            className="w-full px-2 py-1.5 rounded bg-slate-800 border border-slate-600 text-white text-xs placeholder-slate-500 focus:outline-none focus:border-blue-500"
          />
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="Description..."
            rows={3}
            className="w-full px-2 py-1.5 rounded bg-slate-800 border border-slate-600 text-white text-xs placeholder-slate-500 focus:outline-none focus:border-blue-500 resize-none"
          />
          <button
            onClick={handleAdd}
            className="w-full px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium transition-colors"
          >
            Add Entry
          </button>
        </div>
      </div>
    </div>
  );
}
