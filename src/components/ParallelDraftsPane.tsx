import type { ParallelDraft } from "../protocol";

interface ParallelDraftsPaneProps {
  drafts: ParallelDraft[];
  loading: boolean;
  error: string | null;
  onInsert: (text: string) => void;
  onClose: () => void;
}

function splitDraft(text: string): string[] {
  const paragraphs = text
    .split(/\n+/)
    .map((part) => part.trim())
    .filter(Boolean);
  if (paragraphs.length > 1) return paragraphs;
  return text
    .split(/(?<=[。！？!?])/)
    .map((part) => part.trim())
    .filter(Boolean);
}

export default function ParallelDraftsPane({
  drafts,
  loading,
  error,
  onInsert,
  onClose,
}: ParallelDraftsPaneProps) {
  return (
    <aside className="parallel-drafts-pane">
      <div className="parallel-drafts-header">
        <span>平行草稿</span>
        <button onClick={onClose} className="parallel-drafts-close" title="Close">
          x
        </button>
      </div>
      <div className="parallel-drafts-body">
        {loading && <div className="parallel-drafts-muted">Drafting...</div>}
        {error && <div className="parallel-drafts-error">{error}</div>}
        {!loading && !error && drafts.length === 0 && (
          <div className="parallel-drafts-muted">No drafts yet.</div>
        )}
        {drafts.map((draft) => (
          <section key={draft.id} className="parallel-draft">
            <div className="parallel-draft-label">{draft.label}</div>
            <div className="parallel-draft-text">
              {splitDraft(draft.text).map((part, index) => (
                <button
                  key={`${draft.id}-${index}`}
                  className="parallel-draft-segment"
                  onClick={() => onInsert(part)}
                  title="Insert this segment"
                >
                  {part}
                </button>
              ))}
            </div>
          </section>
        ))}
      </div>
    </aside>
  );
}
