import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/core";

interface ReviewItem {
  quote: string;
  type: string;
  issue: string;
  suggestion: string;
}

interface ScriptDoctorPanelProps {
  editorRef: { current: Editor | null };
  onApplyFix: (quote: string, suggestion: string) => void;
}

function fuzzyMatchQuote(docText: string, quote: string): { from: number; to: number } | null {
  // 1. Exact match
  const exact = docText.indexOf(quote);
  if (exact !== -1) return { from: exact, to: exact + quote.length };

  // 2. Match first 10 + last 10 characters
  const head = quote.substring(0, Math.min(10, quote.length));
  const tail = quote.substring(Math.max(0, quote.length - 10));
  const headIdx = docText.indexOf(head);
  if (headIdx !== -1) {
    const tailIdx = docText.indexOf(tail, headIdx + head.length);
    if (tailIdx !== -1) {
      return { from: headIdx, to: tailIdx + tail.length };
    }
  }

  // 3. Paragraph-level match: find best matching paragraph
  const paragraphs = docText.split(/\n\n+/);
  let bestScore = 0;
  let bestIdx = -1;
  let bestOffset = 0;

  for (const para of paragraphs) {
    if (para.trim().length < 5) continue;
    // Count matching bigrams
    const bigrams = new Set<string>();
    for (let i = 0; i < quote.length - 1; i++) bigrams.add(quote.substring(i, i + 2));
    let matches = 0;
    for (let i = 0; i < para.length - 1; i++) {
      if (bigrams.has(para.substring(i, i + 2))) matches++;
    }
    const score = matches / Math.max(para.length - 1, 1);
    if (score > bestScore) {
      bestScore = score;
      bestIdx = docText.indexOf(para);
      bestOffset = para.length;
    }
  }

  if (bestScore > 0.15 && bestIdx !== -1) {
    return { from: bestIdx, to: bestIdx + bestOffset };
  }

  return null;
}

export default function ScriptDoctorPanel({ editorRef, onApplyFix }: ScriptDoctorPanelProps) {
  const [reviews, setReviews] = useState<(ReviewItem & { id: string })[]>([]);
  const [loading, setLoading] = useState(false);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleAnalyze = useCallback(async () => {
    const editor = editorRef.current;
    if (!editor) return;
    const content = editor.getText();
    if (!content || content.length < 50) {
      setError("Chapter is too short to analyze.");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ReviewItem[]>("analyze_chapter", { content });
      const docText = editor.getText();

      const withIds = result
        .map((r) => ({
          ...r,
          id: crypto.randomUUID(),
          match: fuzzyMatchQuote(docText, r.quote),
        }))
        .filter((r) => r.match !== null);

      // Apply CommentMarks to matched ranges
      for (const item of withIds) {
        const m = item.match!;
        editor
          .chain()
          .setTextSelection({ from: m.from, to: m.to })
          .setMark("comment", { commentId: item.id })
          .run();
      }

      setReviews(withIds);
    } catch (e) {
      setError(`Analysis failed: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [editorRef]);

  const handleCardClick = useCallback(
    (id: string) => {
      setActiveId(id);
      const editor = editorRef.current;
      if (!editor) return;

      // Find the marked range and scroll to it
      let found = false;
      editor.state.doc.descendants((node, pos) => {
        if (found) return;
        const marks = node.marks?.filter((m) => m.type.name === "comment");
        if (marks?.some((m) => m.attrs.commentId === id)) {
          editor.commands.setTextSelection({ from: pos, to: pos + node.nodeSize });
          // Scroll the editor view
          const dom = editor.view.domAtPos(pos);
          if (dom.node) {
            (dom.node as HTMLElement).scrollIntoView?.({ behavior: "smooth", block: "center" });
          }
          found = true;
        }
      });
    },
    [editorRef],
  );

  const typeColors: Record<string, string> = {
    logic: "bg-yellow-500/20 text-yellow-300 border-yellow-500/40",
    ooc: "bg-red-500/20 text-red-300 border-red-500/40",
    pacing: "bg-blue-500/20 text-blue-300 border-blue-500/40",
    prose: "bg-purple-500/20 text-purple-300 border-purple-500/40",
  };

  return (
    <div className="flex flex-col h-full">
      <div className="p-3 border-b border-border-subtle">
        <button
          onClick={handleAnalyze}
          disabled={loading}
          className="w-full px-3 py-2 rounded-sm bg-accent hover:bg-accent/80 disabled:opacity-50 text-bg-deep text-xs transition-colors flex items-center justify-center gap-2"
        >
          {loading ? (
            <>
              <span className="inline-block w-3 h-3 border border-bg-deep border-t-transparent rounded-full animate-spin" />
              Analyzing...
            </>
          ) : (
            "Analyze Chapter"
          )}
        </button>
      </div>

      {error && (
        <div className="px-3 py-2 text-xs text-danger border-b border-border-subtle">{error}</div>
      )}

      <div className="flex-1 overflow-y-auto">
        {reviews.map((review) => (
          <div
            key={review.id}
            onClick={() => handleCardClick(review.id)}
            className={`px-3 py-2.5 border-b border-border-subtle cursor-pointer transition-colors hover:bg-bg-raised ${
              activeId === review.id ? "bg-bg-raised border-l-2 border-l-accent" : ""
            }`}
          >
            <div className="flex items-center gap-1.5 mb-1">
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded-sm border ${typeColors[review.type] || typeColors.prose}`}
              >
                {review.type}
              </span>
              <span className="text-[10px] text-text-muted italic line-clamp-1">
                &ldquo;{review.quote.substring(0, 40)}...&rdquo;
              </span>
            </div>
            <p className="text-xs text-text-secondary leading-relaxed mb-1">{review.issue}</p>
            <p className="text-xs text-accent leading-relaxed">{review.suggestion}</p>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onApplyFix(review.quote, review.suggestion);
              }}
              className="mt-1.5 text-[10px] px-2 py-0.5 rounded-sm bg-accent/20 hover:bg-accent/40 text-accent transition-colors"
            >
              Apply Fix
            </button>
          </div>
        ))}
        {!loading && reviews.length === 0 && !error && (
          <p className="p-4 text-xs text-text-muted text-center">
            Click "Analyze Chapter" to get script doctor feedback.
          </p>
        )}
      </div>
    </div>
  );
}
