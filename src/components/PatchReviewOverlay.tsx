import { useCallback, useEffect } from "react";
import { useAppStore } from "../store";
import type { TextPatch } from "../protocol";

export const PatchReviewOverlay: React.FC = () => {
  const activePatchSet = useAppStore((s) => s.activePatchSet);
  const patchStatuses = useAppStore((s) => s.patchStatuses);
  const acceptPatch = useAppStore((s) => s.acceptPatch);
  const rejectPatch = useAppStore((s) => s.rejectPatch);
  const acceptAllPatches = useAppStore((s) => s.acceptAllPatches);
  const rejectAllPatches = useAppStore((s) => s.rejectAllPatches);
  const clearPatches = useAppStore((s) => s.clearPatches);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!activePatchSet) return;
      const pending = activePatchSet.patches.filter((p) => patchStatuses[p.id] === "pending");

      if (e.key === "Tab") {
        e.preventDefault();
        if (pending.length > 0) acceptPatch(pending[0].id);
        if (pending.length <= 1) clearPatches();
      } else if (e.key === "Escape") {
        e.preventDefault();
        if (pending.length > 0) rejectPatch(pending[0].id);
        if (pending.length <= 1) clearPatches();
      } else if (e.key === "a" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        acceptAllPatches();
        clearPatches();
      }
    },
    [activePatchSet, patchStatuses, acceptPatch, rejectPatch, acceptAllPatches, clearPatches],
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  if (!activePatchSet) return null;

  const stats = {
    total: activePatchSet.patches.length,
    accepted: activePatchSet.patches.filter((p) => patchStatuses[p.id] === "accepted").length,
    rejected: activePatchSet.patches.filter((p) => patchStatuses[p.id] === "rejected").length,
    pending: activePatchSet.patches.filter((p) => patchStatuses[p.id] === "pending").length,
  };

  return (
    <div className="patch-review-overlay">
      <div className="patch-review-header">
        <span className="patch-review-title">AI 建议了 {stats.total} 处修改</span>
        <span className="patch-review-stats">
          已接受 {stats.accepted} · 已拒绝 {stats.rejected} · 待处理 {stats.pending}
        </span>
        <div className="patch-review-actions">
          <button onClick={acceptAllPatches} className="patch-btn-accept">全部接受</button>
          <button onClick={rejectAllPatches} className="patch-btn-reject">全部拒绝</button>
          <button onClick={clearPatches} className="patch-btn-dismiss">关闭</button>
        </div>
      </div>
      <div className="patch-review-list">
        {activePatchSet.patches.map((patch: TextPatch, i: number) => {
          const status = patchStatuses[patch.id];
          if (status === "accepted" || status === "rejected") return null;
          return (
            <div key={patch.id} className={`patch-item ${status}`}>
              <div className="patch-item-header">
                <span className="patch-item-index">#{i + 1}</span>
                <span className="patch-item-desc">{patch.description}</span>
                <span className={`patch-item-severity patch-sev-${patch.severity}`}>
                  {patch.severity}
                </span>
              </div>
              <div className="patch-item-diff">
                <div className="patch-old">
                  <span className="patch-label">-</span>
                  <span>
                    {patch.original.substring(0, 120)}
                    {patch.original.length > 120 ? "…" : ""}
                  </span>
                </div>
                <div className="patch-new">
                  <span className="patch-label">+</span>
                  <span>
                    {patch.replacement.substring(0, 120)}
                    {patch.replacement.length > 120 ? "…" : ""}
                  </span>
                </div>
              </div>
              <div className="patch-item-actions">
                <button onClick={() => acceptPatch(patch.id)} className="patch-btn-accept-small">
                  Tab 接受
                </button>
                <button onClick={() => rejectPatch(patch.id)} className="patch-btn-reject-small">
                  Esc 拒绝
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
