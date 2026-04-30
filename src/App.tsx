import { useRef, useCallback, useState } from "react";
import type { Editor } from "@tiptap/core";
import EditorPanel from "./components/EditorPanel";
import AgentPanel from "./components/AgentPanel";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

function App() {
  const editorRef = useRef<Editor | null>(null);
  const selectionRef = useRef<SelectionState>({ from: 0, to: 0, text: "" });

  const handleEditorReady = useCallback((editor: Editor) => {
    editorRef.current = editor;
  }, []);

  const handleSelectionUpdate = useCallback((sel: SelectionState) => {
    selectionRef.current = sel;
  }, []);

  const handleActionInsert = useCallback((text: string) => {
    const editor = editorRef.current;
    if (editor) {
      editor.commands.insertContent(text);
    }
  }, []);

  const handleActionReplace = useCallback(
    (text: string) => {
      const editor = editorRef.current;
      if (!editor) return;

      const { from, to } = selectionRef.current;
      if (from < to) {
        editor.commands.insertContentAt({ from, to }, text);
      } else {
        editor.commands.insertContent(text);
      }
    },
    [],
  );

  const [actionEpoch, setActionEpoch] = useState(0);

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
    <div className="h-screen bg-slate-900 text-white flex">
      <div className="w-2/3 h-full">
        <EditorPanel
          onEditorReady={handleEditorReady}
          onSelectionUpdate={handleSelectionUpdate}
          actionEpoch={actionEpoch}
        />
      </div>
      <div className="w-1/3 h-full">
        <AgentPanel
          getContext={getContext}
          onActionInsert={handleActionInsert}
          onActionReplace={handleActionReplace}
          onActionsCompleted={handleActionsCompleted}
        />
      </div>
    </div>
  );
}

export default App;
