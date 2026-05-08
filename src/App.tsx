import { useRef, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "@tiptap/core";
import { useAppStore } from "./store";
import { Commands, Events, type ChapterRestored, type WriterOperation } from "./protocol";
import EditorPanel from "./components/EditorPanel";
import AgentPanel from "./components/AgentPanel";
import { CompanionPanel } from "./components/CompanionPanel";
import ProjectTree from "./components/ProjectTree";
import { WriterInspectorPanel } from "./components/WriterInspectorPanel";
import SettingsView from "./components/SettingsView";

interface SelectionState {
  from: number;
  to: number;
  text: string;
}

interface ApplyWriterOperationResult {
  applied: boolean;
  saved: boolean;
  revision?: string;
  savedContent?: string;
  chapterTitle?: string;
  error?: string;
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
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);
  const setIsAgentThinking = useAppStore((s) => s.setIsAgentThinking);
  const [hasApiKey, setHasApiKey] = useState<boolean | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [companionCollapsed, setCompanionCollapsed] = useState(false);

  useEffect(() => {
    invoke<boolean>(Commands.checkApiKey, { provider: "openai" })
      .then((v: unknown) => setHasApiKey(v as boolean))
      .catch(() => setHasApiKey(false));
  }, []);

  useEffect(() => {
    if (hasApiKey === false) setShowSettings(true);
  }, [hasApiKey]);

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

  const handleGenerate = useCallback(async () => {
    setIsAgentThinking(true);
    try {
      await invoke(Commands.generateChapterAutonomous, {
        title: currentChapter,
        content: editorRef.current?.getHTML() || "",
      });
    } catch (e) {
      console.error("Generation failed:", e);
    } finally {
      setIsAgentThinking(false);
    }
  }, [currentChapter, setIsAgentThinking]);

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
          setIsEditorDirty(true);
          return;
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

  const handleApplyWriterOperation = useCallback(async (
    operation: WriterOperation,
    proposalId?: string,
  ): Promise<ApplyWriterOperationResult> => {
    const editor = editorRef.current;
    if (!editor) {
      return { applied: false, saved: false, error: "Editor is not ready." };
    }

    if ("chapter" in operation && operation.chapter !== currentChapter) {
      return {
        applied: false,
        saved: false,
        error: `Operation targets ${operation.chapter}, but the open chapter is ${currentChapter}.`,
      };
    }

    switch (operation.kind) {
      case "text.insert": {
        const at = docPositionFromTextCharIndex(editor, operation.at);
        editor
          .chain()
          .focus()
          .insertContentAt(at, operation.text)
          .setTextSelection(at + operation.text.length)
          .run();
        break;
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
        break;
      }
      default:
        return { applied: false, saved: false, error: `Unsupported operation kind: ${operation.kind}` };
    }

    setIsEditorDirty(true);
    const content = editor.getHTML();
    try {
      const revision = await invoke<string>(Commands.saveChapter, { title: currentChapter, content });
      setCurrentChapterRevision(revision);
      setIsEditorDirty(false);
      await invoke(Commands.recordWriterOperationDurableSave, {
        proposalId,
        operation,
        saveResult: `editor_save:${revision}`,
        savedContent: content,
        chapterTitle: currentChapter,
        chapterRevision: revision,
      });
      return { applied: true, saved: true, revision, savedContent: content, chapterTitle: currentChapter };
    } catch (e) {
      setIsEditorDirty(true);
      await invoke(Commands.recordWriterOperationDurableSave, {
        proposalId,
        operation,
        saveResult: `editor_save_failed:${String(e)}`,
        savedContent: content,
        chapterTitle: currentChapter,
        chapterRevision: undefined,
      }).catch((error) => {
        console.error("Failed to record operation save failure:", error);
      });
      return { applied: true, saved: false, error: `Save failed: ${String(e)}` };
    }
  }, [currentChapter, setCurrentChapterRevision, setIsEditorDirty]);

  useEffect(() => {
    const handleRestored = async (event: Event) => {
      const detail = (event as CustomEvent<ChapterRestored>).detail;
      if (!detail?.title || detail.title !== currentChapter) return;
      try {
        const content = await invoke<string>(Commands.loadChapter, { title: detail.title });
        if (editorRef.current) {
          editorRef.current.commands.setContent(content || "<p></p>");
        }
        setCurrentChapterRevision(
          detail.revision
            ?? await invoke<string>(Commands.getChapterRevision, { title: detail.title }),
        );
        setIsEditorDirty(false);
      } catch (e) {
        console.error("Reload restored chapter failed:", e);
      }
    };

    window.addEventListener(Events.chapterRestored, handleRestored);
    return () => window.removeEventListener(Events.chapterRestored, handleRestored);
  }, [currentChapter, setCurrentChapterRevision, setIsEditorDirty]);

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

  if (showSettings) {
  if (showSettings) {
    return (
      <div className="forge-settings-overlay">
        <div className="forge-settings-panel">
          <div className="forge-settings-header">
            <h3 style={{fontWeight:600}}>Settings</h3>
            <button className="forge-btn forge-btn-ghost" onClick={() => setShowSettings(false)}>Close</button>
          </div>
          <div className="forge-settings-body">
            <SettingsView />
            <button className="forge-btn forge-btn-primary" style={{marginTop:'var(--space-4)'}} onClick={() => { setShowSettings(false); invoke(Commands.checkApiKey, { provider: "openai" }).then((v) => setHasApiKey(v as boolean)).catch(() => setHasApiKey(false)); }}>Done</button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="forge-root">
      {/* Toolbar */}
      <header className="forge-toolbar">
        <div className="forge-toolbar-left">
          <button className="forge-btn forge-btn-ghost" onClick={() => setSidebarCollapsed(!sidebarCollapsed)} style={{fontSize:14}}>{sidebarCollapsed ? '▶' : '◀'}</button>
          <span style={{fontWeight:600,fontSize:'var(--text-sm)'}}>Forge</span>
          <span className="forge-toolbar-divider" />
          <span className="truncate text-tertiary" style={{fontSize:'var(--text-xs)'}}>{currentChapter}</span>
        </div>
        <div className="forge-toolbar-right">
          <button className="forge-btn forge-btn-primary" onClick={handleGenerate} disabled={isAgentThinking}>
            {isAgentThinking ? 'Generating...' : 'Generate'}
          </button>
          <button className="forge-btn forge-btn-ghost" onClick={() => setCompanionCollapsed(!companionCollapsed)}>
            {companionCollapsed ? 'Panel' : 'Panel'}
          </button>
          <button className="forge-btn forge-btn-ghost" onClick={() => setShowSettings(true)}>Settings</button>
        </div>
      </header>

      {/* Body */}
      <div className="forge-body">
        {/* Sidebar */}
        <aside className={}>
          <div className="forge-sidebar-search">
            <input type="text" placeholder="Search chapters..." />
          </div>
          <div className="forge-sidebar-list">
            <ProjectTree onSelectChapter={handleSelectChapter} editorRef={editorRef} onApplyFix={handleApplyFix} />
          </div>
          <div className="forge-sidebar-footer">
            <div className="forge-sidebar-footer-avatar">F</div>
            <span className="truncate" style={{fontSize:'var(--text-xs)'}}>Local Project</span>
          </div>
        </aside>

        {/* Main Editor */}
        <main className="forge-main">
          <div className="forge-editor-area">
            <div className="forge-editor-inner">
              <EditorPanel onEditorReady={handleEditorReady} onSelectionUpdate={handleSelectionUpdate} />
            </div>
          </div>
        </main>

        {/* Companion */}
        <aside className={}>
          <div className="forge-mode-row">
            {(["write","review","explore","inspect"] as const).map(m => (
              <button key={m} className={} onClick={()=>setStoryMode(m)}>
                {m==="write"?"Write":m==="review"?"Review":m==="explore"?"Explore":"Inspect"}
              </button>
            ))}
          </div>
          {storyMode==="inspect"
            ? <WriterInspectorPanel getContext={getContext} />
            : <CompanionPanel mode={storyMode} onApplyOperation={handleApplyWriterOperation} />
          }
          {storyMode==="explore" && <AgentPanel mode={storyMode} getContext={getContext} />}
        </aside>
      </div>

      {/* Status Bar */}
      <footer className="forge-statusbar">
        <div style={{display:'flex',alignItems:'center',gap:'var(--space-2)'}}>
          <span className={} />
          <span>{isAgentThinking ? 'Generating...' : 'Ready'}</span>
        </div>
        <div style={{display:'flex',alignItems:'center',gap:'var(--space-3)'}}>
          <span>332 gates</span>
          <span>local &lt;5ms</span>
        </div>
      </footer>
    </div>
  );
export default App;
