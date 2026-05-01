import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Commands, Events, type ProjectFileRestored } from "../protocol";

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
      const result = await invoke<LoreEntry[]>(Commands.getLorebook);
      setEntries(result);
    } catch (e) {
      console.error("Failed to load lorebook:", e);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      const timer = setTimeout(() => {
        void fetchEntries();
      }, 0);
      return () => clearTimeout(timer);
    }
  }, [isOpen, fetchEntries]);

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<ProjectFileRestored>).detail;
      if (detail?.kind === "lorebook") {
        void fetchEntries();
      }
    };
    window.addEventListener(Events.projectFileRestored, handler);
    return () => window.removeEventListener(Events.projectFileRestored, handler);
  }, [fetchEntries]);

  const handleAdd = async () => {
    const kw = keyword.trim();
    const ct = content.trim();
    if (!kw || !ct) return;

    try {
      const result = await invoke<LoreEntry[]>(Commands.saveLoreEntry, {
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
      const result = await invoke<LoreEntry[]>(Commands.deleteLoreEntry, { id });
      setEntries(result);
    } catch (e) {
      console.error("Failed to delete entry:", e);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="border-b border-border-subtle bg-bg-surface">
      <div className="px-4 py-3 border-b border-border-subtle text-xs text-text-secondary font-display tracking-wider flex items-center justify-between">
        <span>Lorebook</span>
        <button
          onClick={onClose}
          className="text-text-muted hover:text-text-primary transition-colors"
        >
          ✕
        </button>
      </div>

      <div className="p-4 space-y-3 max-h-80 overflow-y-auto">
        {entries.length === 0 && (
          <p className="text-text-muted text-xs">No entries yet. Add your first character or setting below.</p>
        )}
        {entries.map((entry) => (
          <div key={entry.id} className="bg-bg-raised rounded-sm p-3 text-sm">
            <div className="flex items-center justify-between mb-1">
              <span className="text-accent font-medium">{entry.keyword}</span>
              <button
                onClick={() => handleDelete(entry.id)}
                className="text-text-muted hover:text-danger text-xs transition-colors"
              >
                Delete
              </button>
            </div>
            <p className="text-text-secondary text-xs leading-relaxed">{entry.content}</p>
          </div>
        ))}

        <div className="pt-3 border-t border-border-subtle space-y-2">
          <input
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder="Keyword (e.g. 林墨)"
            className="w-full px-2.5 py-1.5 rounded-sm bg-bg-deep border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent"
          />
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="Description..."
            rows={3}
            className="w-full px-2.5 py-1.5 rounded-sm bg-bg-deep border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent resize-none"
          />
          <button
            onClick={handleAdd}
            className="w-full px-3 py-1.5 rounded-sm bg-accent hover:bg-accent/80 text-bg-deep text-xs transition-colors"
          >
            Add Entry
          </button>
        </div>
      </div>
    </div>
  );
}
