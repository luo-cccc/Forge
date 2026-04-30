import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import type { Editor } from "@tiptap/core";
import { useEffect, useState } from "react";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

interface EditorPanelProps {
  onEditorReady?: (editor: Editor) => void;
  onSelectionUpdate?: (sel: SelectionState) => void;
  actionEpoch?: number;
}

export default function EditorPanel({
  onEditorReady,
  onSelectionUpdate,
  actionEpoch,
}: EditorPanelProps) {
  const editor = useEditor({
    extensions: [StarterKit],
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
      <div className="flex-1 overflow-y-auto">
        <EditorContent editor={editor} />
      </div>
    </div>
  );
}
