import { useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/core";
import { useAppStore } from "./store";
import { Commands, type WriterOperation } from "./protocol";
import EditorPanel from "./components/EditorPanel";
import AgentPanel from "./components/AgentPanel";
import { CompanionPanel } from "./components/CompanionPanel";
import ProjectTree from "./components/ProjectTree";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

function docPositionFromTextCharIndex(editor: Editor, targetCharIndex: number): number {
  const target = Math.max(0, Math.min(targetCharIndex, Array.from(editor.getText()).length));
  let low = 0;
  let high = editor.state.doc.content.size;
  let position = 0;

  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const charsBefore = Array.from(editor.state.doc.textBetween(0, mid, "\n")).length;
    if (charsBefore <= target) {
      position = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }

  return Math.max(0, Math.min(position, editor.state.doc.content.size));
}

function App() {
  const editorRef = useRef<Editor | null>(null);
  const selectionRef = useRef<SelectionState>({ from: 0, to: 0, text: "" });

  const storyMode = useAppStore((s) => s.storyMode);
  const setStoryMode = useAppStore((s) => s.setStoryMode);
  const currentChapter = useAppStore((s) => s.currentChapter);
  const setCurrentChapter = useAppStore((s) => s.setCurrentChapter);
  const setCurrentChapterRevision = useAppStore((s) => s.setCurrentChapterRevision);
  const setIsEditorDirty = useAppStore((s) => s.setIsEditorDirty);

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
      const revision = await invoke<string>(Commands.getChapterRevision, { title: "Chapter-1" });
      setCurrentChapterRevision(revision);
      setIsEditorDirty(false);
    } catch {
      // No content yet
    }
  }, [setCurrentChapterRevision, setIsEditorDirty]);

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
          const revision = await invoke<string>(Commands.saveChapter, { title: currentChapter, content });
          setCurrentChapterRevision(revision);
          setIsEditorDirty(false);
        } catch (e) {
          console.error("Auto-save failed:", e);
        }
      }
      try {
        const content = await invoke<string>(Commands.loadChapter, { title });
        if (editorRef.current) {
          editorRef.current.commands.setContent(content || "<p></p>");
        }
        const revision = await invoke<string>(Commands.getChapterRevision, { title });
        setCurrentChapterRevision(revision);
        setIsEditorDirty(false);
        setCurrentChapter(title);
      } catch (e) {
        console.error("Load chapter failed:", e);
        try {
          await invoke(Commands.createChapter, { title });
          if (editorRef.current) {
            editorRef.current.commands.setContent("<p>Start writing...</p>");
          }
          const revision = await invoke<string>(Commands.getChapterRevision, { title });
          setCurrentChapterRevision(revision);
          setIsEditorDirty(false);
          setCurrentChapter(title);
        } catch (e2) {
          console.error("Create chapter failed:", e2);
        }
      }
    },
    [currentChapter, setCurrentChapter, setCurrentChapterRevision, setIsEditorDirty],
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

  const handleApplyWriterOperation = useCallback((operation: WriterOperation): boolean => {
    const editor = editorRef.current;
    if (!editor) return false;

    switch (operation.kind) {
      case "text.insert": {
        const at = docPositionFromTextCharIndex(editor, operation.at);
        editor
          .chain()
          .focus()
          .insertContentAt(at, operation.text)
          .setTextSelection(at + operation.text.length)
          .run();
        setIsEditorDirty(true);
        return true;
      }
      case "text.replace": {
        const from = docPositionFromTextCharIndex(editor, operation.from);
        const to = docPositionFromTextCharIndex(editor, operation.to);
        editor
          .chain()
          .focus()
          .insertContentAt({ from, to }, operation.text)
          .setTextSelection(from + operation.text.length)
          .run();
        setIsEditorDirty(true);
        return true;
      }
      default:
        return false;
    }
  }, [setIsEditorDirty]);

  const getContext = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return { full: "", paragraph: "", selected: "", cursorPosition: 0 };
    const full = editor.getText();
    const { from } = editor.state.selection;
    const $from = editor.state.doc.resolve(from);
    const paragraph = editor.state.doc.textBetween($from.start(), $from.end(), " ");
    const selected = selectionRef.current.text;
    const cursorPosition = Array.from(editor.state.doc.textBetween(0, from, "\n")).length;
    return { full, paragraph, selected, cursorPosition };
  }, []);

  const rightRailWidth =
    storyMode === "write" ? "w-72" : storyMode === "review" ? "w-[28rem]" : "w-[32rem]";
  const companionHeight =
    storyMode === "explore" ? "h-[36%]" : storyMode === "review" ? "h-full" : "h-full";

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
      <div className={`${rightRailWidth} h-full flex-shrink-0 border-l border-border-subtle flex flex-col min-h-0`}>
        <div className="border-b border-border-subtle px-3 py-2">
          <div className="grid grid-cols-3 gap-1 rounded bg-bg-deep border border-border-subtle p-1">
            {(["write", "review", "explore"] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => setStoryMode(mode)}
                className={`px-2 py-1 text-xs rounded-sm transition-colors ${
                  storyMode === mode
                    ? "bg-accent text-bg-deep"
                    : "text-text-muted hover:text-text-secondary"
                }`}
              >
                {mode === "write" ? "Write" : mode === "review" ? "Review" : "Explore"}
              </button>
            ))}
          </div>
        </div>
        <div className={`${companionHeight} min-h-0 ${storyMode === "explore" ? "border-b border-border-subtle" : ""}`}>
          <CompanionPanel
            mode={storyMode}
            onApplyOperation={handleApplyWriterOperation}
          />
        </div>
        {storyMode === "explore" && (
          <div className="flex-1 min-h-0">
          <AgentPanel
            mode={storyMode}
            getContext={getContext}
            onActionInsert={handleActionInsert}
            onActionReplace={handleActionReplace}
          />
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
