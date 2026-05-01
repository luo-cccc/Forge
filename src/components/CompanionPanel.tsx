import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import type { WriterAgentStatus, AgentProposal, ProposalFeedback } from "../protocol";

export const CompanionPanel: React.FC = () => {
  const currentChapter = useAppStore((s) => s.currentChapter);
  const agentMode = useAppStore((s) => s.agentMode);
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);

  const [status, setStatus] = useState<WriterAgentStatus | null>(null);
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [activeTab, setActiveTab] = useState<"status" | "promises" | "canon" | "decisions">("status");

  const refreshStatus = useCallback(async () => {
    try {
      const s = await invoke<WriterAgentStatus>("get_writer_agent_status");
      setStatus(s);
    } catch {
      // kernel not initialized yet
    }
  }, []);

  useEffect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 5000);
    return () => clearInterval(interval);
  }, [refreshStatus]);

  useEffect(() => {
    // Listen for new proposals from the kernel
    const fn = listen<AgentProposal>("agent-proposal", (event) => {
      setProposals((prev) => [event.payload, ...prev].slice(0, 20));
    });
    return () => { fn.then((f) => f()); };
  }, []);

  const handleFeedback = async (proposalId: string, action: ProposalFeedback["action"]) => {
    const feedback: ProposalFeedback = {
      proposalId,
      action,
      createdAt: Date.now(),
    };
    // TODO: invoke apply_feedback command when added to Tauri
    setProposals((prev) => prev.filter((p) => p.id !== proposalId));
    refreshStatus();
  };

  const pendingProposals = proposals.filter((p) => {
    const age = Date.now() - (p.expiresAt ?? 0);
    return p.expiresAt === undefined || p.expiresAt === 0 || age < p.expiresAt;
  });

  return (
    <div className="flex flex-col h-full bg-bg-surface">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border-subtle">
        <div className="flex items-center justify-between mb-2">
          <span className="font-display text-sm tracking-wider text-text-primary">
            Companion
          </span>
          <span className={`w-2 h-2 rounded-full ${
            isAgentThinking ? "bg-accent animate-pulse" : "bg-success"
          }`} />
        </div>
        {status && (
          <div className="grid grid-cols-2 gap-2 text-xs text-text-muted">
            <div>
              <span className="block text-text-secondary">Observations</span>
              <span className="font-mono">{status.observationCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">Proposals</span>
              <span className="font-mono">{status.proposalCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">Open Promises</span>
              <span className="font-mono text-accent">{status.openPromiseCount}</span>
            </div>
            <div>
              <span className="block text-text-secondary">Feedback</span>
              <span className="font-mono">{status.totalFeedbackEvents}</span>
            </div>
          </div>
        )}
      </div>

      {/* Tabs */}
      <div className="flex border-b border-border-subtle">
        {(["status", "promises", "canon", "decisions"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`flex-1 py-2 text-xs tracking-wide transition-colors ${
              activeTab === tab
                ? "text-accent border-b border-accent"
                : "text-text-muted hover:text-text-secondary"
            }`}
          >
            {tab === "status" ? "状态" : tab === "promises" ? "伏笔" : tab === "canon" ? "设定" : "决策"}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-3 space-y-2">
        {activeTab === "status" && (
          <div className="space-y-3">
            {agentMode !== "proactive" && (
              <div className="p-3 rounded bg-accent-subtle/30 border border-accent/20 text-xs text-text-secondary">
                Agent is in {agentMode} mode. Switch to Proactive for ambient suggestions.
              </div>
            )}
            <div className="text-xs text-text-muted">
              <div className="mb-2 text-text-secondary font-medium">Active Scene</div>
              <div className="p-2 rounded bg-bg-raised border border-border-subtle">
                {currentChapter || "No chapter loaded"}
              </div>
            </div>
            {pendingProposals.length > 0 && (
              <div>
                <div className="text-xs text-text-secondary font-medium mb-2">
                  Pending Proposals ({pendingProposals.length})
                </div>
                {pendingProposals.slice(0, 5).map((p) => (
                  <div key={p.id} className={`p-2 rounded border mb-1 text-xs ${
                    p.priority === "urgent" ? "border-danger/40 bg-danger/10" :
                    p.priority === "normal" ? "border-accent/30 bg-accent-subtle/20" :
                    "border-border-subtle bg-bg-raised"
                  }`}>
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-text-primary font-medium">{p.kind}</span>
                      <span className={`px-1.5 py-0.5 rounded text-[10px] ${
                        p.priority === "urgent" ? "bg-danger/20 text-danger" :
                        p.priority === "normal" ? "bg-accent-subtle text-accent" :
                        "bg-bg-raised text-text-muted"
                      }`}>{p.priority}</span>
                    </div>
                    <p className="text-text-muted mb-2">{p.preview}</p>
                    {p.rationale && (
                      <p className="text-text-secondary italic mb-1">{p.rationale}</p>
                    )}
                    {p.evidence.length > 0 && (
                      <div className="mb-2 space-y-1">
                        {p.evidence.map((e, i) => (
                          <div key={i} className="p-1.5 rounded bg-bg-deep border border-border-subtle">
                            <span className="text-[10px] text-text-muted">{e.source}: </span>
                            <span className="text-[10px] text-text-secondary">{e.snippet}</span>
                          </div>
                        ))}
                      </div>
                    )}
                    <div className="flex gap-1">
                      <button
                        onClick={() => handleFeedback(p.id, "accepted")}
                        className="px-2 py-1 text-[10px] rounded bg-accent-subtle text-accent border border-accent/40 hover:bg-accent/20"
                      >
                        Accept
                      </button>
                      <button
                        onClick={() => handleFeedback(p.id, "rejected")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                      >
                        Reject
                      </button>
                      <button
                        onClick={() => handleFeedback(p.id, "snoozed")}
                        className="px-2 py-1 text-[10px] rounded bg-bg-raised text-text-muted border border-border-subtle hover:bg-bg-surface"
                      >
                        Snooze
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "promises" && (
          <div className="text-xs text-text-muted">
            <p>Open plot promises will appear here as the story develops.</p>
            <p className="mt-2">Add promises via the kernel API or canon ledger.</p>
          </div>
        )}

        {activeTab === "canon" && (
          <div className="text-xs text-text-muted">
            <p>Canon risks and entity information will appear here.</p>
            <p className="mt-2">The Canon Engine monitors for contradictions as you write.</p>
          </div>
        )}

        {activeTab === "decisions" && (
          <div className="text-xs text-text-muted">
            <p>Creative decisions are recorded when you accept or reject proposals.</p>
            <p className="mt-2">This builds a durable record of why the story took each path.</p>
          </div>
        )}
      </div>
    </div>
  );
};
