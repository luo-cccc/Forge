import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  onActionsCompleted: () => void;
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
  onActionsCompleted,
}: AgentPanelProps) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [streaming, setStreaming] = useState("");
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [searchStatus, setSearchStatus] = useState<SearchStatus | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rawBufferRef = useRef("");

  useEffect(() => {
    let unlistenChunk: UnlistenFn;
    let unlistenEnd: UnlistenFn;
    let unlistenSearch: UnlistenFn;

    const setup = async () => {
      unlistenChunk = await listen<StreamChunk>("agent-stream-chunk", (event) => {
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
          rawBufferRef.current = "";
          setSearchStatus(event.payload);
        },
      );

      unlistenEnd = await listen<StreamEnd>("agent-stream-end", () => {
        // Flush remaining buffer
        const finalText = rawBufferRef.current.replace(ACTION_RE, "");
        rawBufferRef.current = "";

        if (finalText) {
          setMessages((prev) => [...prev, { role: "agent", content: finalText }]);
        }
        setStreaming("");
        setIsStreaming(false);
        setSearchStatus(null);
        onActionsCompleted();
      });
    };

    setup();

    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenEnd) unlistenEnd();
      if (unlistenSearch) unlistenSearch();
    };
  }, [onActionInsert, onActionReplace, onActionsCompleted]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, streaming]);

  const handleSubmit = useCallback(async () => {
    const text = input.trim();
    if (!text || isStreaming) return;

    setInput("");
    setMessages((prev) => [...prev, { role: "user", content: text }]);
    setIsStreaming(true);
    rawBufferRef.current = "";

    try {
      const { full, paragraph, selected } = getContext();
      await invoke("ask_agent", { message: text, context: full, paragraph, selectedText: selected });
    } catch (e) {
      setStreaming("");
      setIsStreaming(false);
      setMessages((prev) => [
        ...prev,
        { role: "agent", content: `Error: ${e}` },
      ]);
    }
  }, [input, isStreaming, getContext]);

  return (
    <div className="flex flex-col h-full border-l border-slate-700">
      <div className="px-4 py-3 border-b border-slate-700 text-sm text-slate-400 font-medium">
        Agent
      </div>

      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.length === 0 && !streaming && (
          <p className="text-slate-500 text-sm">Agent responses will appear here.</p>
        )}
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`text-sm max-w-[90%] rounded-lg px-3 py-2 whitespace-pre-wrap ${
              msg.role === "user"
                ? "bg-blue-600 text-white ml-auto"
                : "bg-slate-800 text-slate-200"
            }`}
          >
            {msg.content}
          </div>
        ))}
        {searchStatus && (
          <div className="text-sm max-w-[90%] rounded-lg px-3 py-2 bg-amber-900/50 border border-amber-700 text-amber-200 whitespace-pre-wrap">
            🔍 Searching lorebook: <span className="font-medium">{searchStatus.keyword}</span>...
          </div>
        )}
        {streaming && (
          <div className="text-sm max-w-[90%] rounded-lg px-3 py-2 bg-slate-800 text-slate-200 whitespace-pre-wrap">
            {streaming}
            <span className="inline-block w-1.5 h-4 bg-slate-400 ml-0.5 animate-pulse align-middle" />
          </div>
        )}
      </div>

      <div className="p-4 border-t border-slate-700">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
          disabled={isStreaming}
          className="w-full px-3 py-2 rounded-md bg-slate-800 border border-slate-600 text-white placeholder-slate-500 focus:outline-none focus:border-blue-500 text-sm disabled:opacity-50"
          placeholder="Ask the agent..."
        />
      </div>
    </div>
  );
}
