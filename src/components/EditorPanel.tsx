import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Editor } from "@tiptap/core";
import { useEffect, useRef, useState } from "react";
import { useAppStore } from "../store";
import AIPreviewMark from "../extensions/AIPreviewMark";
import LorebookDrawer from "./LorebookDrawer";
import InlineCommandBubble from "./InlineCommandBubble";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

interface EditorPanelProps {
  onEditorReady?: (editor: Editor) => void;
  onSelectionUpdate?: (sel: SelectionState) => void;
}

interface StreamChunk {
  content: string;
}

const ACTION_RE = /<ACTION_(INSERT|REPLACE)>(.*?)<\/ACTION_\1>/gs;

export default function EditorPanel({
  onEditorReady,
  onSelectionUpdate,
}: EditorPanelProps) {
  const actionEpoch = useAppStore((s) => s.actionEpoch);
  const setIsInlineRequest = useAppStore((s) => s.setIsInlineRequest);
  const incrementActionEpoch = useAppStore((s) => s.incrementActionEpoch);
  const editor = useEditor({
    extensions: [StarterKit, AIPreviewMark],
    content: "<p>Start writing your novel here...</p>",
    editorProps: {
      attributes: {
        class:
          "prose prose-invert max-w-none h-full focus:outline-none px-8 py-6 text-text-primary leading-relaxed font-editor",
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

  useEffect(() => {
    if (actionEpoch && actionEpoch > 0) {
      setShowToast(true);
      const timer = setTimeout(() => setShowToast(false), 4000);
      return () => clearTimeout(timer);
    }
  }, [actionEpoch]);

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [bubbleVisible, setBubbleVisible] = useState(false);
  const [bubbleThinking, setBubbleThinking] = useState(false);
  const [inlineDone, setInlineDone] = useState(false);
  const [saveIndicator, setSaveIndicator] = useState<string | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const currentChapter = useAppStore((s) => s.currentChapter);

  // Debounced auto-save: 3s after typing stops
  useEffect(() => {
    if (!editor) return;
    const handler = () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(async () => {
        const content = editor.getHTML();
        try {
          await invoke("save_chapter", { title: currentChapter, content });
          const now = new Date();
          setSaveIndicator(
            `Saved at ${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}`,
          );
          setTimeout(() => setSaveIndicator(null), 3000);
        } catch (e) {
          console.error("Auto-save failed:", e);
        }
      }, 3000);
    };
    editor.on("update", handler);
    return () => {
      editor.off("update", handler);
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    };
  }, [editor, currentChapter]);

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

    setIsInlineRequest(true);
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
      setIsInlineRequest(false);
      setBubbleThinking(false);
      setBubbleVisible(false);
      setInlineDone(true);
      incrementActionEpoch();
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
      setIsInlineRequest(false);
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

  const btnActive = (active: boolean) =>
    active ? "bg-bg-raised text-accent" : "text-text-muted hover:text-text-primary";

  if (!editor) {
    return (
      <div className="flex items-center justify-center h-full text-text-muted">
        Loading editor...
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="px-4 py-2.5 border-b border-border-subtle text-sm text-text-secondary flex items-center justify-between relative">
        <span className="font-display tracking-wider text-xs">Editor</span>
        {saveIndicator && (
          <span className="text-[10px] text-text-muted tracking-wide ml-2">
            {saveIndicator}
          </span>
        )}
        {showToast && (
          <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 px-3 py-1.5 rounded-sm bg-success/20 border border-success text-success text-xs whitespace-nowrap">
            AI writing complete. Press Ctrl+Z / Cmd+Z to undo
          </div>
        )}
        <div className="flex gap-1">
          <button
            onClick={() => editor.chain().focus().toggleBold().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor transition-colors ${btnActive(editor.isActive("bold"))}`}
            title="Bold"
          >
            B
          </button>
          <button
            onClick={() => editor.chain().focus().toggleItalic().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor italic transition-colors ${btnActive(editor.isActive("italic"))}`}
            title="Italic"
          >
            I
          </button>
          <button
            onClick={() => editor.chain().focus().toggleStrike().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor line-through transition-colors ${btnActive(editor.isActive("strike"))}`}
            title="Strikethrough"
          >
            S
          </button>
          <span className="w-px h-4 bg-border-subtle mx-1 self-center" />
          <button
            onClick={() => setDrawerOpen(!drawerOpen)}
            className={`px-2.5 py-1 rounded-sm text-xs transition-colors ${
              drawerOpen ? "bg-bg-raised text-accent" : "text-text-muted hover:text-text-primary"
            }`}
            title="Lorebook"
          >
            Lorebook
          </button>
          <span className="w-px h-4 bg-border-subtle mx-1 self-center" />
          <button
            onClick={() => editor.commands.toggleHeading({ level: 2 })}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor transition-colors ${btnActive(editor.isActive("heading", { level: 2 }))}`}
            title="Heading"
          >
            H
          </button>
          <button
            onClick={() => editor.chain().focus().toggleBlockquote().run()}
            className={`px-2.5 py-1 rounded-sm text-xs font-editor transition-colors ${btnActive(editor.isActive("blockquote"))}`}
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
        {bubbleVisible && (
          <InlineCommandBubble
            editor={editor}
            onSubmit={handleBubbleSubmit}
            onDismiss={handleBubbleDismiss}
            isThinking={bubbleThinking}
            onStop={() => {}}
          />
        )}
        {inlineDone && (
          <div className="absolute bottom-4 right-4 flex items-center gap-2 bg-bg-raised border border-accent rounded-sm px-3 py-2 shadow-lg z-40">
            <span className="text-xs text-accent">AI Preview</span>
            <span className="w-px h-4 bg-border-subtle" />
            <button
              onClick={handleAccept}
              className="text-xs text-bg-deep bg-success hover:bg-success/80 px-2.5 py-0.5 rounded-sm transition-colors"
            >
              Accept (Tab)
            </button>
            <button
              onClick={handleReject}
              className="text-xs text-danger hover:text-danger/80 px-2.5 py-0.5 transition-colors"
            >
              Reject (Esc)
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
