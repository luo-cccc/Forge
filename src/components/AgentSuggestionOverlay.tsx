import type { AgentSuggestion } from "../protocol";

interface AgentSuggestionOverlayProps {
  suggestion: AgentSuggestion;
  onAccept: (suggestion: AgentSuggestion) => void;
  onReject: (suggestion: AgentSuggestion) => void;
  onSnooze: () => void;
}

function kindLabel(kind: AgentSuggestion["kind"]): string {
  switch (kind) {
    case "continue":
      return "Continue";
    case "revise":
      return "Revise";
    case "continuity":
      return "Continuity";
    case "lore":
      return "Lore";
    case "structure":
      return "Structure";
    case "question":
      return "Question";
  }
}

export default function AgentSuggestionOverlay({
  suggestion,
  onAccept,
  onReject,
  onSnooze,
}: AgentSuggestionOverlayProps) {
  const sourceText =
    suggestion.sourceSummaries.length > 0
      ? suggestion.sourceSummaries
          .slice(0, 2)
          .map((source) => `${source.label}: ${source.summary}`)
          .join(" | ")
      : "Local editor observation";

  return (
    <div className="absolute right-5 bottom-5 z-50 w-[min(28rem,calc(100%-2.5rem))] rounded-sm border border-accent/50 bg-bg-raised shadow-xl">
      <div className="flex items-center justify-between gap-3 border-b border-border-subtle px-3 py-2">
        <div className="min-w-0">
          <div className="text-[10px] uppercase tracking-wider text-accent">
            Agent suggestion · {kindLabel(suggestion.kind)}
          </div>
          <div className="mt-0.5 truncate text-[11px] text-text-muted">
            {suggestion.reason}
          </div>
        </div>
        <div className="text-[10px] text-text-muted">
          {Math.round(suggestion.confidence * 100)}%
        </div>
      </div>
      <div className="space-y-2 px-3 py-2.5">
        <div className="max-h-28 overflow-y-auto whitespace-pre-wrap text-sm leading-relaxed text-text-primary">
          {suggestion.previewText}
        </div>
        <div className="line-clamp-2 text-[11px] text-text-muted">
          {sourceText}
        </div>
      </div>
      <div className="flex items-center justify-end gap-2 border-t border-border-subtle px-3 py-2">
        <button
          onClick={() => onSnooze()}
          className="px-2.5 py-1 text-xs text-text-muted transition-colors hover:text-text-primary"
        >
          Snooze
        </button>
        <button
          onClick={() => onReject(suggestion)}
          className="px-2.5 py-1 text-xs text-danger transition-colors hover:text-danger/80"
        >
          Reject
        </button>
        <button
          onClick={() => onAccept(suggestion)}
          className="rounded-sm bg-success px-2.5 py-1 text-xs text-bg-deep transition-colors hover:bg-success/80"
        >
          Accept
        </button>
      </div>
    </div>
  );
}
