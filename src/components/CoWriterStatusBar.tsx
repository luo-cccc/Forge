import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../store";
import type { AgentLoopEventPayload } from "../protocol";

export const CoWriterStatusBar: React.FC = () => {
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);
  const [contextUsage, setContextUsage] = useState(0);
  const [activeTool, setActiveTool] = useState<string | null>(null);

  useEffect(() => {
    const fns: (() => void)[] = [];
    listen<AgentLoopEventPayload>("agent-loop-event", (e) => {
      const p = e.payload;
      switch (p.kind) {
        case "tool_call_start":
          setActiveTool(p.tool ?? null);
          break;
        case "tool_call_end":
          setActiveTool(null);
          break;
        case "compaction":
          if (p.before_tokens && p.after_tokens) {
            setContextUsage(Math.round((p.after_tokens / (p.before_tokens || 1)) * 100));
          }
          break;
        case "complete":
          setActiveTool(null);
          break;
      }
    }).then((fn) => fns.push(fn));
    return () => fns.forEach((fn) => fn());
  }, []);

  return (
    <div className="cowriter-status-bar">
      <div className="status-left">
        {isAgentThinking ? (
          <span className="status-thinking">
            {activeTool ? `\u{1F527} ${activeTool}` : "\u{1F4AD} 思考中..."}
          </span>
        ) : (
          <span className="status-idle">{'✓'} 就绪</span>
        )}
      </div>
      <div className="status-center">
        <div className="context-bar">
          <div className="context-fill" style={{ width: `${Math.min(contextUsage, 100)}%` }} />
        </div>
        <span className="context-label">上下文 {contextUsage}%</span>
      </div>
      <div className="status-right">
        <span className="status-model">DeepSeek V4</span>
      </div>
    </div>
  );
};
