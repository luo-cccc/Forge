/// Headless Sandbox — 验证 agent-harness-core 独立于 Tiptap/Zustand
/// 只有输入框 + 输出控制台，测试核心引擎全链路
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  Commands,
  Events,
  type AgentError,
  type Epiphany,
  type SearchStatus,
  type StreamChunk,
  type StreamEnd,
} from "../protocol";

export default function SandboxView() {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState<string[]>(["Sandbox ready. Type a message and press Enter."]);
  const [streaming, setStreaming] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const outputRef = useRef<HTMLDivElement>(null);

  const log = useCallback((msg: string) => {
    setOutput((p) => [...p, `[${new Date().toLocaleTimeString()}] ${msg}`]);
  }, []);

  useEffect(() => {
    outputRef.current?.scrollTo(0, outputRef.current.scrollHeight);
  }, [output, streaming]);

  useEffect(() => {
    let unlistenChunk: UnlistenFn;
    let unlistenEnd: UnlistenFn;
    let unlistenSearch: UnlistenFn;
    let unlistenError: UnlistenFn;
    let unlistenEpiphany: UnlistenFn;

    const setup = async () => {
      unlistenChunk = await listen<StreamChunk>(Events.agentStreamChunk, (e) => {
        setStreaming((p) => p + e.payload.content);
      });

      unlistenSearch = await listen<SearchStatus>(Events.agentSearchStatus, (e) => {
        log(`🔍 Searching: ${e.payload.keyword} (round ${e.payload.round})`);
      });

      unlistenError = await listen<AgentError>(Events.agentError, (e) => {
        log(`❌ Error: ${e.payload.message}`);
        setIsLoading(false);
      });

      unlistenEpiphany = await listen<Epiphany>(Events.agentEpiphany, (e) => {
        log(`💡 Learned: [${e.payload.category}] ${e.payload.skill}`);
      });

      unlistenEnd = await listen<StreamEnd>(Events.agentStreamEnd, () => {
        setStreaming((p) => {
          if (p) log(`📝 Response: ${p.substring(0, 200)}...`);
          return "";
        });
        setIsLoading(false);
      });
    };
    setup();

    return () => {
      unlistenChunk?.();
      unlistenEnd?.();
      unlistenSearch?.();
      unlistenError?.();
      unlistenEpiphany?.();
    };
  }, [log]);

  const handleRAG = async () => {
    const q = input.trim();
    if (!q) return;
    setInput("");
    setIsLoading(true);
    log(`🔎 RAG query: ${q}`);
    try {
      await invoke(Commands.askProjectBrain, { query: q });
    } catch (e) {
      log(`❌ ${e}`);
      setIsLoading(false);
    }
  };

  const handleAsk = async () => {
    const msg = input.trim();
    if (!msg) return;
    setInput("");
    setIsLoading(true);
    log(`👤 ${msg}`);
    try {
      await invoke(Commands.askAgent, {
        message: msg,
        context: "",
        paragraph: "",
        selectedText: "",
        contextPayload: null,
      });
    } catch (e) {
      log(`❌ ${e}`);
      setIsLoading(false);
    }
  };

  const handleMemory = async () => {
    try {
      const data = await invoke<{ entities: unknown[]; chapters: unknown[] }>(Commands.getProjectGraphData);
      log(`📊 Graph: ${data.entities.length} entities, ${data.chapters.length} chapters`);
    } catch (e) {
      log(`❌ ${e}`);
    }
  };

  return (
    <div className="flex flex-col h-full bg-bg-deep text-text-primary">
      <div className="px-4 py-3 border-b border-border-subtle text-xs text-text-secondary font-display tracking-wider flex items-center gap-2">
        <span>🧪 Headless Sandbox</span>
        <span className="text-[10px] text-text-muted">— 验证 agent-harness-core 独立性</span>
      </div>

      <div
        ref={outputRef}
        className="flex-1 overflow-y-auto p-4 font-mono text-xs space-y-1 bg-code-bg"
      >
        {output.map((line, i) => (
          <div key={i} className="text-text-secondary leading-relaxed whitespace-pre-wrap">{line}</div>
        ))}
        {streaming && (
          <div className="text-text-primary whitespace-pre-wrap">
            {streaming}
            <span className="inline-block w-1.5 h-4 bg-accent animate-pulse align-middle" />
          </div>
        )}
      </div>

      <div className="p-3 border-t border-border-subtle flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleAsk();
            }
          }}
          disabled={isLoading}
          placeholder="Type a message (Enter=Ask, Shift+Enter=newline)..."
          className="flex-1 px-3 py-1.5 rounded-sm bg-bg-surface border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent disabled:opacity-50"
        />
        <button
          onClick={handleRAG}
          disabled={isLoading}
          className="px-3 py-1.5 rounded-sm bg-purple-500/20 border border-purple-500/40 text-purple-300 text-xs hover:bg-purple-500/30 transition-colors disabled:opacity-50"
        >
          RAG
        </button>
        <button
          onClick={handleMemory}
          disabled={isLoading}
          className="px-3 py-1.5 rounded-sm bg-bg-raised border border-border-subtle text-text-secondary text-xs hover:bg-bg-surface transition-colors disabled:opacity-50"
        >
          Graph
        </button>
      </div>
    </div>
  );
}
