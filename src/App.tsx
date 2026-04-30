import { useRef, useCallback } from "react";
import type { Editor } from "@tiptap/core";
import EditorPanel from "./components/EditorPanel";
import AgentPanel from "./components/AgentPanel";

function App() {
  const editorRef = useRef<Editor | null>(null);

  const handleEditorReady = useCallback((editor: Editor) => {
    editorRef.current = editor;
  }, []);

  const handleActionInsert = useCallback((text: string) => {
    const editor = editorRef.current;
    if (editor) {
      editor.commands.insertContent(text);
    }
  }, []);

  const getContext = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return { full: "", paragraph: "" };

    const full = editor.getText();

    const { from } = editor.state.selection;
    const $from = editor.state.doc.resolve(from);
    const start = $from.start();
    const end = $from.end();
    const paragraph = editor.state.doc.textBetween(start, end, " ");

    return { full, paragraph };
  }, []);

  return (
    <div className="h-screen bg-slate-900 text-white flex">
      <div className="w-2/3 h-full">
        <EditorPanel onEditorReady={handleEditorReady} />
      </div>
      <div className="w-1/3 h-full">
        <AgentPanel getContext={getContext} onActionInsert={handleActionInsert} />
      </div>
    </div>
  );
}

export default App;
