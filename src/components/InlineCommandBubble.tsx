import { useState, useRef, useEffect, useCallback } from "react";
import type { Editor } from "@tiptap/core";

interface InlineCommandBubbleProps {
  editor: Editor;
  onSubmit: (command: string) => void;
  onDismiss: () => void;
  isThinking?: boolean;
  onStop?: () => void;
}

export default function InlineCommandBubble({
  editor,
  onSubmit,
  onDismiss,
  isThinking,
  onStop,
}: InlineCommandBubbleProps) {
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  const calcPosition = useCallback(() => {
    const { from } = editor.state.selection;
    const coords = editor.view.coordsAtPos(from);
    const editorEl = editor.view.dom.closest(".relative") as HTMLElement | null;
    const editorRect = editorEl?.getBoundingClientRect() ?? {
      left: 0,
      top: 0,
    };
    setPos({
      x: coords.left - editorRect.left,
      y: coords.bottom - editorRect.top + 4,
    });
  }, [editor]);

  useEffect(() => {
    calcPosition();
    inputRef.current?.focus();
  }, [calcPosition]);

  const handleSubmit = () => {
    const text = input.trim();
    if (!text) return;
    onSubmit(text);
    setInput("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSubmit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onDismiss();
    }
  };

  return (
    <div
      className="absolute z-50"
      style={{ left: pos.x, top: pos.y }}
    >
      <div className="bg-slate-800 border border-slate-600 rounded-lg shadow-2xl px-1 py-1 flex items-center gap-1">
        {isThinking ? (
          <>
            <span className="px-2 py-1 text-sm text-slate-300 flex items-center gap-2">
              <span className="inline-block w-2 h-2 bg-amber-400 rounded-full animate-pulse" />
              Thinking...
            </span>
            {onStop && (
              <button
                onClick={onStop}
                className="px-2 py-0.5 text-xs text-red-400 hover:text-red-300 transition-colors"
              >
                Stop
              </button>
            )}
          </>
        ) : (
          <>
            <input
              ref={inputRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              onBlur={onDismiss}
              placeholder="Tell AI what to do..."
              className="w-64 px-2 py-1 bg-transparent text-white text-sm placeholder-slate-500 focus:outline-none"
            />
            <span className="text-xs text-slate-500 pr-1.5 whitespace-nowrap">
              Enter ↵
            </span>
          </>
        )}
      </div>
    </div>
  );
}
