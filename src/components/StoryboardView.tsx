import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Commands } from "../protocol";

interface GraphChapter {
  title: string;
  summary: string;
  status: string;
  word_count: number;
}

export default function StoryboardView() {
  const [chapters, setChapters] = useState<GraphChapter[]>([]);
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  const [analysis, setAnalysis] = useState<string | null>(null);
  const [analyzing, setAnalyzing] = useState(false);

  const load = useCallback(async () => {
    try {
      const data = await invoke<{ chapters: GraphChapter[] }>(Commands.getProjectGraphData);
      setChapters(data.chapters);
    } catch (e) {
      console.error("Failed to load chapters:", e);
    }
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      void load();
    }, 0);
    return () => clearTimeout(timer);
  }, [load]);

  const handleDragStart = (idx: number) => setDragIdx(idx);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  };

  const handleDrop = async (targetIdx: number) => {
    if (dragIdx === null || dragIdx === targetIdx) {
      setDragIdx(null);
      return;
    }
    const previous = chapters;
    const reordered = [...chapters];
    const [moved] = reordered.splice(dragIdx, 1);
    reordered.splice(targetIdx, 0, moved);
    setChapters(reordered);
    setDragIdx(null);

    try {
      await invoke(Commands.reorderOutlineNodes, {
        orderedTitles: reordered.map((chapter) => chapter.title),
      });
    } catch (e) {
      console.error("Failed to persist order:", e);
      setChapters(previous);
    }
  };

  const handleAnalyzePacing = async () => {
    setAnalyzing(true);
    setAnalysis(null);
    try {
      const summaries = chapters.map((c, i) => `${i + 1}. ${c.title}: ${c.summary} (${c.word_count} words)`).join("\n");
      const result = await invoke<string>(Commands.analyzePacing, { summaries });
      setAnalysis(result);
    } catch (e) {
      setAnalysis(`Analysis failed: ${e}`);
    } finally {
      setAnalyzing(false);
    }
  };

  const statusColors: Record<string, string> = {
    empty: "bg-bg-raised text-text-muted border-border-subtle",
    generated: "bg-success/20 text-success border-success/30",
    polished: "bg-accent/20 text-accent border-accent/30",
  };

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-border-subtle flex items-center justify-between">
        <span className="text-xs text-text-secondary font-display tracking-wider">Storyboard</span>
        <button
          onClick={handleAnalyzePacing}
          disabled={analyzing || chapters.length === 0}
          className="text-[10px] px-2 py-0.5 rounded-sm bg-accent/20 hover:bg-accent/30 disabled:opacity-50 text-accent transition-colors flex items-center gap-1"
        >
          {analyzing ? (
            <>
              <span className="inline-block w-2 h-2 border border-accent border-t-transparent rounded-full animate-spin" />
              Analyzing...
            </>
          ) : (
            "Analyze Pacing"
          )}
        </button>
      </div>

      {analysis && (
        <div className="px-3 py-2 border-b border-border-subtle text-xs text-text-primary max-h-48 overflow-y-auto whitespace-pre-wrap leading-relaxed">
          <div className="flex items-center justify-between mb-1">
            <span className="text-[10px] text-accent font-medium">Pacing Analysis</span>
            <button onClick={() => setAnalysis(null)} className="text-text-muted hover:text-text-primary text-[10px]">✕</button>
          </div>
          {analysis}
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-3 space-y-2">
        {chapters.map((ch, idx) => (
          <div
            key={ch.title}
            draggable
            onDragStart={() => handleDragStart(idx)}
            onDragOver={handleDragOver}
            onDrop={() => handleDrop(idx)}
            className={`rounded-sm px-3 py-2.5 border cursor-grab active:cursor-grabbing transition-colors ${
              dragIdx === idx
                ? "opacity-50 border-accent"
                : "bg-bg-raised border-border-subtle hover:border-border-active"
            }`}
          >
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs text-text-primary font-display">#{idx + 1} {ch.title}</span>
              <span className={`text-[9px] px-1.5 py-0.5 rounded-sm border ${statusColors[ch.status] || statusColors.empty}`}>
                {ch.status}
              </span>
            </div>
            {ch.summary && (
              <p className="text-[11px] text-text-secondary line-clamp-2 leading-relaxed">{ch.summary}</p>
            )}
            <div className="flex items-center gap-2 mt-1">
              <span className="text-[10px] text-text-muted">{ch.word_count} words</span>
              <div className="flex-1 h-1 bg-bg-deep rounded-full overflow-hidden">
                <div
                  className="h-full bg-accent/40 rounded-full"
                  style={{ width: `${Math.min(100, (ch.word_count / 5000) * 100)}%` }}
                />
              </div>
            </div>
          </div>
        ))}
        {chapters.length === 0 && (
          <p className="text-xs text-text-muted text-center py-8">No chapters yet.</p>
        )}
      </div>
    </div>
  );
}
