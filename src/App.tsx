import { useRef, useCallback, useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/core";
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
  const [actionEpoch, setActionEpoch] = useState(0);
  const isInlineRequestRef = useRef(false);

  const [currentChapter, setCurrentChapter] = useState("Chapter-1");

  useEffect(() => {
    const init = async () => {
      try {
        await invoke("create_chapter", { title: "Chapter-1" });
      } catch {
        // Already exists
      }
      try {
        const content = await invoke<string>("load_chapter", { title: "Chapter-1" });
        if (editorRef.current) {
          editorRef.current.commands.setContent(content || "<p>Start writing...</p>");
        }
      } catch {
        // Will retry when editor is ready
      }
    };
    init();
  }, []);

  const handleEditorReady = useCallback(async (editor: Editor) => {
    editorRef.current = editor;
    try {
      await invoke("create_chapter", { title: currentChapter });
    } catch {
      // Already exists
    }
    try {
      const content = await invoke<string>("load_chapter", { title: currentChapter });
      editor.commands.setContent(content || "<p>Start writing...</p>");
    } catch {
      // No content yet
    }
  }, [currentChapter]);

  const handleSelectionUpdate = useCallback((sel: SelectionState) => {
    selectionRef.current = sel;
  }, []);

  const handleSelectChapter = useCallback(
    async (title: string) => {
      if (title === currentChapter) return;

      // Auto-save current chapter
      const editor = editorRef.current;
      if (editor) {
        const content = editor.getHTML();
        try {
          await invoke("save_chapter", {
            title: currentChapter,
            content,
          });
        } catch (e) {
          console.error("Auto-save failed:", e);
        }
      }

      // Load new chapter
      try {
        const content = await invoke<string>("load_chapter", { title });
        if (editorRef.current) {
          editorRef.current.commands.setContent(content || "<p></p>");
        }
        setCurrentChapter(title);
      } catch (e) {
        console.error("Load chapter failed:", e);
        // Create it if not exists
        try {
          await invoke("create_chapter", { title });
          if (editorRef.current) {
            editorRef.current.commands.setContent("<p>Start writing...</p>");
          }
          setCurrentChapter(title);
        } catch (e2) {
          console.error("Create chapter failed:", e2);
        }
      }
    },
    [currentChapter],
  );

  const handleActionInsert = useCallback((text: string) => {
    const editor = editorRef.current;
    if (editor) {
      editor.commands.insertContent(text);
    }
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

  const handleActionsCompleted = useCallback(() => {
    setActionEpoch((e) => e + 1);
  }, []);

  const getContext = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return { full: "", paragraph: "", selected: "" };

    const full = editor.getText();

    const { from } = editor.state.selection;
    const $from = editor.state.doc.resolve(from);
    const start = $from.start();
    const end = $from.end();
    const paragraph = editor.state.doc.textBetween(start, end, " ");

    const selected = selectionRef.current.text;

    return { full, paragraph, selected };
  }, []);

  return (
    <div className="h-screen bg-bg-deep text-text-primary flex">
      <div className="w-48 h-full flex-shrink-0">
        <ProjectTree
          currentChapter={currentChapter}
          onSelectChapter={handleSelectChapter}
        />
      </div>
      <div className="flex-1 h-full min-w-0">
        <EditorPanel
          onEditorReady={handleEditorReady}
          onSelectionUpdate={handleSelectionUpdate}
          actionEpoch={actionEpoch}
          isInlineRequestRef={isInlineRequestRef}
        />
      </div>
      <div className="w-96 h-full flex-shrink-0">
        <AgentPanel
          getContext={getContext}
          onActionInsert={handleActionInsert}
          onActionReplace={handleActionReplace}
          onActionsCompleted={handleActionsCompleted}
          isInlineRequestRef={isInlineRequestRef}
        />
      </div>
    </div>
  );
}

export default App;
