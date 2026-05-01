import type { EditorEntityCard } from "../protocol";

interface EntityHoverState {
  card: EditorEntityCard;
  x: number;
  y: number;
}

interface EntityHoverCardProps {
  hover: EntityHoverState | null;
}

export default function EntityHoverCard({ hover }: EntityHoverCardProps) {
  if (!hover) return null;

  return (
    <div
      className="entity-hover-card"
      style={{
        left: hover.x,
        top: hover.y,
      }}
    >
      <div className="entity-hover-title">{hover.card.keyword}</div>
      <div className="entity-hover-chapter">{hover.card.chapter}</div>
      <div className="entity-hover-content">{hover.card.content}</div>
    </div>
  );
}
