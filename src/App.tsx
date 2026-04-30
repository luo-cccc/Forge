import { useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/core";
import { useAppStore } from "./store";
import { Commands } from "./protocol";
import EditorPanel from "./components/EditorPanel";
import AgentPanel from "./components/AgentPanel";
import ProjectTree from "./components/ProjectTree";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

function App() {
  const editorRef = useRef<Editor | null>(null);
  const selectionRef = useRef<SelectionState>({ from: 0, to: 0, text: "" });

  const currentChapter = useAppStore((s) => s.currentChapter);
  const setCurrentChapter = useAppStore((s) => s.setCurrentChapter);

  const handleEditorReady = useCallback(async (editor: Editor) => {
    editorRef.current = editor;
    try {
      await invoke(Commands.createChapter, { title: "Chapter-1" });
    } catch {
      // Already exists
    }
    try {
      const content = await invoke<string>(Commands.loadChapter, { title: "Chapter-1" });
      editor.commands.setContent(content || "<p>Start writing...</p>");
    } catch {
      // No content yet
    }
  }, []);

  const handleSelectionUpdate = useCallback((sel: SelectionState) => {
    selectionRef.current = sel;
  }, []);

  const handleSelectChapter = useCallback(
    async (title: string) => {
      if (title === currentChapter) return;
      const editor = editorRef.current;
      if (editor) {
        const content = editor.getHTML();
        try {
          await invoke(Commands.saveChapter, { title: currentChapter, content });
        } catch (e) {
          console.error("Auto-save failed:", e);
        }
      }
      try {
        const content = await invoke<string>(Commands.loadChapter, { title });
        if (editorRef.current) {
          editorRef.current.commands.setContent(content || "<p></p>");
        }
        setCurrentChapter(title);
      } catch (e) {
        console.error("Load chapter failed:", e);
        try {
          await invoke(Commands.createChapter, { title });
          if (editorRef.current) {
            editorRef.current.commands.setContent("<p>Start writing...</p>");
          }
          setCurrentChapter(title);
        } catch (e2) {
          console.error("Create chapter failed:", e2);
        }
      }
    },
    [currentChapter, setCurrentChapter],
  );

  const handleActionInsert = useCallback((text: string) => {
    const editor = editorRef.current;
    if (editor) editor.commands.insertContent(text);
  }, []);

  const handleActionReplace = useCallback((text: string) => {
    const editor = editorRef.current;
    if (!editor) return;
    const { from, to } = selectionRef.current;
    if (from < to) {
      editor.commands.insertContentAt({ from, to }, text);
    } else {
      editor.commands.insertContent(text);
    }
  }, []);

  const handleApplyFix = useCallback((quote: string, suggestion: string) => {
    const editor = editorRef.current;
    if (!editor) return;
    const docText = editor.getText();
    const idx = docText.indexOf(quote);
    if (idx !== -1) {
      editor
        .chain()
        .focus()
        .setTextSelection({ from: idx, to: idx + quote.length })
        .deleteSelection()
        .insertContent(suggestion)
        .run();
    } else {
      editor.commands.insertContent(suggestion);
    }
  }, []);

  const getContext = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return { full: "", paragraph: "", selected: "" };
    const full = editor.getText();
    const { from } = editor.state.selection;
    const $from = editor.state.doc.resolve(from);
    const paragraph = editor.state.doc.textBetween($from.start(), $from.end(), " ");
    const selected = selectionRef.current.text;
    return { full, paragraph, selected };
  }, []);

  return (
    <div className="h-screen bg-bg-deep text-text-primary flex">
      <div className="w-48 h-full flex-shrink-0">
        <ProjectTree
          onSelectChapter={handleSelectChapter}
          editorRef={editorRef}
          onApplyFix={handleApplyFix}
        />
      </div>
      <div className="flex-1 h-full min-w-0">
        <EditorPanel
          onEditorReady={handleEditorReady}
          onSelectionUpdate={handleSelectionUpdate}
        />
      </div>
      <div className="w-96 h-full flex-shrink-0">
        <AgentPanel
          getContext={getContext}
          onActionInsert={handleActionInsert}
          onActionReplace={handleActionReplace}
        />
      </div>
    </div>
  );
}

export default App;
