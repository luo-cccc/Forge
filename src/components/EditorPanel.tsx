import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import type { Editor } from "@tiptap/core";
import { useEffect, useState } from "react";
import LorebookDrawer from "./LorebookDrawer";
import InlineCommandBubble from "./InlineCommandBubble";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import AIPreviewMark from "../extensions/AIPreviewMark";

interface StreamChunk {
  content: string;
}

const ACTION_RE = /<ACTION_(INSERT|REPLACE)>(.*?)<\/ACTION_\1>/gs;

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

interface EditorPanelProps {
  onEditorReady?: (editor: Editor) => void;
  onSelectionUpdate?: (sel: SelectionState) => void;
  actionEpoch?: number;
  isInlineRequestRef: React.RefObject<boolean>;
}

export default function EditorPanel({
  onEditorReady,
  onSelectionUpdate,
  actionEpoch,
  isInlineRequestRef,
}: EditorPanelProps) {
  const editor = useEditor({
    extensions: [StarterKit, AIPreviewMark],
    content: "<p>Start writing your novel here...</p>",
    editorProps: {
      attributes: {
        class:
          "prose prose-invert prose-slate max-w-none h-full focus:outline-none px-6 py-4 text-slate-200 leading-relaxed",
      },
    },
  });

  useEffect(() => {
    if (editor && onEditorReady) {
      onEditorReady(editor);
    }
  }, [editor, onEditorReady]);

  useEffect(() => {
    if (!editor || !onSelectionUpdate) return;

    const handler = () => {
      const { from, to } = editor.state.selection;
      const text = editor.state.doc.textBetween(from, to, " ");
      onSelectionUpdate({ from, to, text });
    };

    editor.on("selectionUpdate", handler);
    return () => {
      editor.off("selectionUpdate", handler);
    };
  }, [editor, onSelectionUpdate]);

  const [showToast, setShowToast] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [bubbleVisible, setBubbleVisible] = useState(false);
  const [bubbleThinking, setBubbleThinking] = useState(false);
  const [inlineDone, setInlineDone] = useState(false);

  useEffect(() => {
    if (!editor) return;

    const handler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "k") {
        event.preventDefault();
        setBubbleVisible(true);
      }
      if (event.key === "Escape" && bubbleVisible) {
        setBubbleVisible(false);
      }
    };

    const view = editor.view;
    view.dom.addEventListener("keydown", handler);
    return () => view.dom.removeEventListener("keydown", handler);
  }, [editor, bubbleVisible]);

  const handleBubbleSubmit = async (command: string) => {
    if (!editor) return;
    const editorRef = { current: editor };

    isInlineRequestRef.current = true;
    setBubbleThinking(true);

    const fullText = editor.getText();

    let unlistenChunk: UnlistenFn;
    let unlistenEnd: UnlistenFn;
    let rawBuffer = "";

    unlistenChunk = await listen<StreamChunk>("agent-stream-chunk", (event) => {
      rawBuffer += event.payload.content;
      const ed = editorRef.current;
      if (!ed) return;

      let m: RegExpExecArray | null;
      const re = new RegExp(ACTION_RE.source, "gs");
      while ((m = re.exec(rawBuffer)) !== null) {
        const [, kind, content] = m;

        if (kind === "replace") {
          const sel = ed.state.selection;
          const rFrom = sel.from;
          const rTo = sel.to;
          if (rFrom < rTo) {
            ed.chain()
              .focus()
              .insertContentAt({ from: rFrom, to: rTo }, content)
              .setTextSelection({ from: rFrom, to: rFrom + content.length })
              .setMark("aiPreview")
              .setTextSelection(rFrom + content.length)
              .run();
          } else {
            const p = ed.state.selection.from;
            ed.chain()
              .focus()
              .insertContent(content)
              .setTextSelection({ from: p, to: p + content.length })
              .setMark("aiPreview")
              .setTextSelection(p + content.length)
              .run();
          }
        } else {
          const p = ed.state.selection.from;
          ed.chain()
            .focus()
            .insertContent(content)
            .setTextSelection({ from: p, to: p + content.length })
            .setMark("aiPreview")
            .setTextSelection(p + content.length)
            .run();
        }
      }
      rawBuffer = rawBuffer.replace(ACTION_RE, "");
    });

    unlistenEnd = await listen("agent-stream-end", () => {
      unlistenChunk();
      unlistenEnd();
      isInlineRequestRef.current = false;
      setBubbleThinking(false);
      setBubbleVisible(false);
      setInlineDone(true);
    });

    try {
      await invoke("ask_agent", {
        message: command,
        context: fullText,
        paragraph: "",
        selectedText: "",
      });
    } catch {
      unlistenChunk();
      unlistenEnd();
      isInlineRequestRef.current = false;
      setBubbleThinking(false);
      setBubbleVisible(false);
    }
  };

  const handleBubbleDismiss = () => {
    setBubbleVisible(false);
  };

  const handleAccept = () => {
    if (!editor) return;
    const rangesToClear: { from: number; to: number }[] = [];
    editor.state.doc.descendants((node, pos) => {
      if (node.marks?.some((m) => m.type.name === "aiPreview")) {
        rangesToClear.push({ from: pos, to: pos + node.nodeSize });
      }
    });
    editor.chain().focus();
    for (const r of rangesToClear) {
      editor
        .chain()
        .setTextSelection({ from: r.from, to: r.to })
        .unsetMark("aiPreview")
        .run();
    }
    setInlineDone(false);
  };

  const handleReject = () => {
    if (!editor) return;
    const rangesToDelete: { from: number; to: number }[] = [];
    editor.state.doc.descendants((node, pos) => {
      if (node.marks?.some((m) => m.type.name === "aiPreview")) {
        rangesToDelete.push({ from: pos, to: pos + node.nodeSize });
      }
    });
    for (let i = rangesToDelete.length - 1; i >= 0; i--) {
      const r = rangesToDelete[i];
      editor.chain().focus().deleteRange({ from: r.from, to: r.to }).run();
    }
    setInlineDone(false);
  };

  useEffect(() => {
    if (!editor || !inlineDone) return;

    const handler = (e: KeyboardEvent) => {
      if (e.key === "Tab") {
        e.preventDefault();
        handleAccept();
      } else if (e.key === "Escape") {
        e.preventDefault();
        handleReject();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editor, inlineDone]);

  useEffect(() => {
    if (actionEpoch && actionEpoch > 0) {
      setShowToast(true);
      const timer = setTimeout(() => setShowToast(false), 4000);
      return () => clearTimeout(timer);
    }
  }, [actionEpoch]);

  if (!editor) {
    return (
      <div className="flex items-center justify-center h-full text-slate-500">
        Loading editor...
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="px-4 py-3 border-b border-slate-700 text-sm text-slate-400 font-medium flex items-center justify-between relative">
        <span>Editor</span>
        {showToast && (
          <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 px-3 py-1.5 rounded-md bg-emerald-900/90 border border-emerald-700 text-emerald-200 text-xs whitespace-nowrap transition-opacity">
            AI writing complete. Press Ctrl+Z / Cmd+Z to undo
          </div>
        )}
        <div className="flex gap-1">
          <button
            onClick={() => editor.chain().focus().toggleBold().run()}
            className={`px-2 py-0.5 rounded text-xs transition-colors ${
              editor.isActive("bold")
                ? "bg-slate-700 text-white"
                : "text-slate-400 hover:text-white"
            }`}
            title="Bold"
          >
            B
          </button>
          <button
            onClick={() => editor.chain().focus().toggleItalic().run()}
            className={`px-2 py-0.5 rounded text-xs italic transition-colors ${
              editor.isActive("italic")
                ? "bg-slate-700 text-white"
                : "text-slate-400 hover:text-white"
            }`}
            title="Italic"
          >
            I
          </button>
          <button
            onClick={() => editor.chain().focus().toggleStrike().run()}
            className={`px-2 py-0.5 rounded text-xs line-through transition-colors ${
              editor.isActive("strike")
                ? "bg-slate-700 text-white"
                : "text-slate-400 hover:text-white"
            }`}
            title="Strikethrough"
          >
            S
          </button>
          <button
            onClick={() => setDrawerOpen(!drawerOpen)}
            className={`px-2 py-0.5 rounded text-xs transition-colors ${
              drawerOpen
                ? "bg-slate-700 text-white"
                : "text-slate-400 hover:text-white"
            }`}
            title="Lorebook"
          >
            📖
          </button>
          <span className="w-px bg-slate-600 mx-1" />
          <button
            onClick={() =>
              editor.commands.toggleHeading({ level: 2 })
            }
            className={`px-2 py-0.5 rounded text-xs transition-colors ${
              editor.isActive("heading", { level: 2 })
                ? "bg-slate-700 text-white"
                : "text-slate-400 hover:text-white"
            }`}
            title="Heading"
          >
            H
          </button>
          <button
            onClick={() => editor.chain().focus().toggleBlockquote().run()}
            className={`px-2 py-0.5 rounded text-xs transition-colors ${
              editor.isActive("blockquote")
                ? "bg-slate-700 text-white"
                : "text-slate-400 hover:text-white"
            }`}
            title="Blockquote"
          >
            &ldquo;
          </button>
        </div>
      </div>
      <LorebookDrawer
        isOpen={drawerOpen}
        onClose={() => setDrawerOpen(false)}
      />
      <div className="flex-1 overflow-y-auto relative">
        <EditorContent editor={editor} />
        {inlineDone && (
          <div className="absolute bottom-4 right-4 flex items-center gap-2 bg-slate-800 border border-emerald-700 rounded-lg px-3 py-2 shadow-lg z-40">
            <span className="text-xs text-emerald-300">AI Preview</span>
            <span className="w-px h-4 bg-slate-600" />
            <button
              onClick={handleAccept}
              className="text-xs text-white bg-emerald-600 hover:bg-emerald-500 px-2 py-0.5 rounded transition-colors"
            >
              Accept (Tab)
            </button>
            <button
              onClick={handleReject}
              className="text-xs text-red-400 hover:text-red-300 px-2 py-0.5 transition-colors"
            >
              Reject (Esc)
            </button>
          </div>
        )}
        {bubbleVisible && (
          <InlineCommandBubble
            editor={editor}
            onSubmit={handleBubbleSubmit}
            onDismiss={handleBubbleDismiss}
            isThinking={bubbleThinking}
            onStop={() => {
              // Stop is handled naturally by stream-end cleanup
            }}
          />
        )}
      </div>
    </div>
  );
}
