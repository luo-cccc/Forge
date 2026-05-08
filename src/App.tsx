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
      .then(setHasApiKey)
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
    return (
      <div className="forge-overlay">
        <div className="forge-overlay-inner">
          <h2 style={{fontSize:'var(--text-xl)',fontWeight:600,marginBottom:'var(--space-2)',color:'var(--fg-text-primary)'}}>Welcome to Forge</h2>
          <p style={{color:'var(--fg-text-secondary)',marginBottom:'var(--space-6)'}}>Connect a model to start writing. Your API key stays in the system keychain.</p>
          <SettingsView />
          <button className="forge-btn forge-btn-primary" style={{marginTop:'var(--space-4)'}} onClick={() => { setShowSettings(false); invoke(Commands.checkApiKey, { provider: "openai" }).then(setHasApiKey).catch(() => setHasApiKey(false)); }}>Done</button>
        </div>
      </div>
    );
  }

  return (
    <div className="forge-root">
      {/* Activity Bar */}
      <nav className="forge-activity-bar">
        <button className="forge-activity-btn active" title="Chapters">📄</button>
        <button className="forge-activity-btn" title="Settings" onClick={() => setShowSettings(true)}>⚙</button>
      </nav>

      {/* Sidebar */}
      <aside className={`forge-sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        <div className="forge-sidebar-header">
          <span>Chapters</span>
          <button className="forge-btn-ghost" onClick={() => setSidebarCollapsed(!sidebarCollapsed)} style={{fontSize:10,padding:'0 4px'}}>◁</button>
        </div>
        <div className="forge-sidebar-body">
          <ProjectTree onSelectChapter={handleSelectChapter} editorRef={editorRef} onApplyFix={handleApplyFix} />
        </div>
      </aside>

      {/* Main */}
      <div className={`forge-main ${sidebarCollapsed ? '' : 'sidebar-open'}`}>
        <div className="forge-editor-area">
          {/* Tab Bar */}
          <div className="forge-tab-bar">
            <div className="forge-tab active">{currentChapter}</div>
            <button className="forge-btn-ghost" onClick={handleGenerate} style={{marginLeft:'auto',height:26,fontSize:'var(--text-xs)'}}>
              {isAgentThinking ? '⏳' : '+ Generate'}
            </button>
            <button className="forge-btn-ghost" onClick={() => setCompanionCollapsed(!companionCollapsed)} style={{height:26,fontSize:'var(--text-xs)'}}>
              {companionCollapsed ? '◁' : '▷'}
            </button>
          </div>
          {/* Editor */}
          <div className="forge-editor-body">
            <EditorPanel onEditorReady={handleEditorReady} onSelectionUpdate={handleSelectionUpdate} />
          </div>
        </div>

        {/* Companion */}
        {!companionCollapsed && (
          <aside className="forge-companion">
            <div className="forge-mode-row">
              {(["write","review","explore","inspect"] as const).map(m => (
                <button key={m} className={`forge-mode-btn ${storyMode===m?'active':''}`} onClick={()=>setStoryMode(m)}>
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
        )}
      </div>

      {/* Status Bar */}
      <footer className="forge-statusbar">
        <div className="forge-statusbar-left">
          <span>{isAgentThinking ? 'Generating…' : 'Ready'}</span>
          <span>·</span>
          <span>332 gates</span>
        </div>
        <div className="forge-statusbar-right">
          <span>local &lt;5ms</span>
        </div>
      </footer>
    </div>
  );
export default App;
