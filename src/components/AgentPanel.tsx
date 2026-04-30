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

interface AgentPanelProps {
  getContext: () => { full: string; paragraph: string };
  onActionInsert: (text: string) => void;
}

const ACTION_RE = /<ACTION_INSERT>(.*?)<\/ACTION_INSERT>/gs;

function extractActions(buffer: string): { actions: string[]; cleanText: string } {
  const actions: string[] = [];
  const cleanText = buffer.replace(ACTION_RE, (_, content) => {
    actions.push(content);
    return "";
  });
  return { actions, cleanText };
}

export default function AgentPanel({ getContext, onActionInsert }: AgentPanelProps) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [streaming, setStreaming] = useState("");
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rawBufferRef = useRef("");

  useEffect(() => {
    let unlistenChunk: UnlistenFn;
    let unlistenEnd: UnlistenFn;

    const setup = async () => {
      unlistenChunk = await listen<StreamChunk>("agent-stream-chunk", (event) => {
        rawBufferRef.current += event.payload.content;

        const { actions, cleanText } = extractActions(rawBufferRef.current);
        for (const action of actions) {
          onActionInsert(action);
        }
        rawBufferRef.current = cleanText;
        setStreaming(rawBufferRef.current);
      });

      unlistenEnd = await listen<StreamEnd>("agent-stream-end", () => {
        // Flush remaining buffer
        const finalText = rawBufferRef.current.replace(ACTION_RE, "");
        rawBufferRef.current = "";

        if (finalText) {
          setMessages((prev) => [...prev, { role: "agent", content: finalText }]);
        }
        setStreaming("");
        setIsStreaming(false);
      });
    };

    setup();

    return () => {
      if (unlistenChunk) unlistenChunk();
      if (unlistenEnd) unlistenEnd();
    };
  }, [onActionInsert]);

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
      const { full, paragraph } = getContext();
      await invoke("ask_agent", { message: text, context: full, paragraph });
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
