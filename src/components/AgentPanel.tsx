import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../store";

interface StreamChunk {
  content: string;
}

interface StreamEnd {
  reason: string;
}

interface Message {
  role: "user" | "agent";
  content: string;
}

interface SearchStatus {
  keyword: string;
  round: number;
}

interface AgentPanelProps {
  getContext: () => { full: string; paragraph: string; selected: string };
  onActionInsert: (text: string) => void;
  onActionReplace: (text: string) => void;
}

const ACTION_RE = /<ACTION_(INSERT|REPLACE)>(.*?)<\/ACTION_\1>/gs;

interface ParsedAction {
  kind: "insert" | "replace";
  content: string;
}

function extractActions(buffer: string): { actions: ParsedAction[]; cleanText: string } {
  const actions: ParsedAction[] = [];
  const cleanText = buffer.replace(ACTION_RE, (_, kind: string, content: string) => {
    actions.push({ kind: kind.toLowerCase() as "insert" | "replace", content });
    return "";
  });
  return { actions, cleanText };
}

export default function AgentPanel({
  getContext,
  onActionInsert,
  onActionReplace,
}: AgentPanelProps) {
  const isInlineRequest = useAppStore((s) => s.isInlineRequest);
  const setIsAgentThinking = useAppStore((s) => s.setIsAgentThinking);
  const incrementActionEpoch = useAppStore((s) => s.incrementActionEpoch);
  const [messages, setMessages] = useState<Message[]>([]);
  const [streaming, setStreaming] = useState("");
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [searchStatus, setSearchStatus] = useState<SearchStatus | null>(null);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [lastInput, setLastInput] = useState<string>("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const rawBufferRef = useRef("");

  useEffect(() => {
    let unlistenChunk: UnlistenFn;
    let unlistenEnd: UnlistenFn;
    let unlistenSearch: UnlistenFn;
    let unlistenError: UnlistenFn;

    const setup = async () => {
      unlistenChunk = await listen<StreamChunk>("agent-stream-chunk", (event) => {
        if (isInlineRequest) return;
        if (searchStatus) setSearchStatus(null);
        rawBufferRef.current += event.payload.content;

        const { actions, cleanText } = extractActions(rawBufferRef.current);
        for (const action of actions) {
          if (action.kind === "replace") {
            onActionReplace(action.content);
          } else {
            onActionInsert(action.content);
          }
        }
        rawBufferRef.current = cleanText;
        setStreaming(rawBufferRef.current);
      });

      unlistenSearch = await listen<SearchStatus>(
        "agent-search-status",
        (event) => {
          if (isInlineRequest) return;
          rawBufferRef.current = "";
          setSearchStatus(event.payload);
        },
      );

      unlistenError = await listen<{ message: string; source: string }>(
        "agent-error",
        (event) => {
          if (isInlineRequest) return;
          setIsStreaming(false);
          setIsAgentThinking(false);
          setStreaming("");
          setAgentError(event.payload.message);
        },
      );

      unlistenEnd = await listen<StreamEnd>("agent-stream-end", () => {
        if (isInlineRequest) return;
        const finalText = rawBufferRef.current.replace(ACTION_RE, "");
        rawBufferRef.current = "";

        if (finalText) {
          setMessages((prev) => [...prev, { role: "agent", content: finalText }]);
        }
        setStreaming("");
        setIsStreaming(false);
        setIsAgentThinking(false);
        setSearchStatus(null);
        incrementActionEpoch();
      });
    };

    setup();

    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenEnd) unlistenEnd();
      if (unlistenSearch) unlistenSearch();
      if (unlistenError) unlistenError();
    };
  }, [onActionInsert, onActionReplace, isInlineRequest, setIsAgentThinking, incrementActionEpoch]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, streaming]);

  const handleSubmit = useCallback(async () => {
    const text = input.trim();
    if (!text || isStreaming) return;

    setInput("");
    setLastInput(text);
    setAgentError(null);
    setMessages((prev) => [...prev, { role: "user", content: text }]);
    setIsStreaming(true);
    setIsAgentThinking(true);
    rawBufferRef.current = "";

    try {
      const { full, paragraph, selected } = getContext();
      await invoke("ask_agent", { message: text, context: full, paragraph, selectedText: selected });
    } catch (e) {
      setStreaming("");
      setIsStreaming(false);
      setIsAgentThinking(false);
      setMessages((prev) => [
        ...prev,
        { role: "agent", content: `Error: ${e}` },
      ]);
    }
  }, [input, isStreaming, getContext, setIsAgentThinking]);

  const handleRetry = useCallback(async () => {
    if (!lastInput) return;
    setAgentError(null);
    setIsStreaming(true);
    setIsAgentThinking(true);
    rawBufferRef.current = "";
    try {
      const { full, paragraph, selected } = getContext();
      await invoke("ask_agent", { message: lastInput, context: full, paragraph, selectedText: selected });
    } catch (e) {
      setStreaming("");
      setIsStreaming(false);
      setIsAgentThinking(false);
      setMessages((prev) => [...prev, { role: "agent", content: `Error: ${e}` }]);
    }
  }, [lastInput, getContext, setIsAgentThinking]);

  return (
    <div className="flex flex-col h-full border-l border-border-subtle">
      <div className="px-4 py-3 border-b border-border-subtle text-xs text-text-secondary font-display tracking-wider">
        Agent
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.length === 0 && !streaming && !searchStatus && (
          <p className="text-text-muted text-xs">Agent responses will appear here.</p>
        )}
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`text-sm max-w-[90%] rounded-sm px-3 py-2 whitespace-pre-wrap ${
              msg.role === "user"
                ? "bg-accent text-bg-deep ml-auto"
                : "bg-bg-raised text-text-primary"
            }`}
          >
            {msg.content}
          </div>
        ))}
        {agentError && (
          <div className="text-sm max-w-[90%] rounded-sm px-3 py-2 bg-danger/20 border border-danger text-danger whitespace-pre-wrap flex items-center gap-3">
            <span>{agentError}</span>
            <button
              onClick={handleRetry}
              className="text-xs px-2 py-0.5 rounded-sm bg-danger text-white hover:bg-danger/80 transition-colors flex-shrink-0"
            >
              Retry
            </button>
          </div>
        )}
        {searchStatus && (
          <div className="text-sm max-w-[90%] rounded-sm px-3 py-2 bg-accent-subtle border border-accent text-accent whitespace-pre-wrap">
            Searching lorebook: <span className="font-medium">{searchStatus.keyword}</span>...
          </div>
        )}
        {streaming && (
          <div className="text-sm max-w-[90%] rounded-sm px-3 py-2 bg-bg-raised text-text-primary whitespace-pre-wrap">
            {streaming}
            <span className="inline-block w-1.5 h-4 bg-accent ml-0.5 animate-pulse align-middle" />
          </div>
        )}
      </div>

      <div className="p-4 border-t border-border-subtle">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
          disabled={isStreaming}
          className="w-full px-3 py-2 rounded-sm bg-bg-deep border border-border-subtle text-text-primary placeholder-text-muted focus:outline-none focus:border-accent text-sm disabled:opacity-50"
          placeholder="Ask the agent..."
        />
      </div>
    </div>
  );
}
