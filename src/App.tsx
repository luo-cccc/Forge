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
import {
  DEFAULT_FORGE_THEME,
  FORGE_THEME_STORAGE_KEY,
  isForgeTheme,
  type ForgeTheme,
} from "./uiPreferences";

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

async function checkConfiguredApiKey(): Promise<boolean> {
  try {
    return await invoke<boolean>(Commands.checkApiKey, { provider: "openai" });
  } catch (error) {
    console.error("API key check failed:", error);
    return false;
  }
}

function App() {
  const editorRef = useRef<Editor | null>(null);
  const selectionRef = useRef<SelectionState>({ from: 0, to: 0, text: "" });

  const storyMode = useAppStore((s) => s.storyMode);
  const setStoryMode = useAppStore((s) => s.setStoryMode);
  const currentChapter = useAppStore((s) => s.currentChapter);
  const setCurrentChapter = useAppStore((s) => s.setCurrentChapter);
  const setCurrentChapterRevision = useAppStore((s) => s.setCurrentChapterRevision);
  const isEditorDirty = useAppStore((s) => s.isEditorDirty);
  const setIsEditorDirty = useAppStore((s) => s.setIsEditorDirty);
  const isAgentThinking = useAppStore((s) => s.isAgentThinking);
  const setIsAgentThinking = useAppStore((s) => s.setIsAgentThinking);
  const [hasApiKey, setHasApiKey] = useState<boolean | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(true);
  const [companionCollapsed, setCompanionCollapsed] = useState(true);
  const [theme, setTheme] = useState<ForgeTheme>(() => {
    if (typeof window === "undefined") return DEFAULT_FORGE_THEME;
    const storedTheme = window.localStorage.getItem(FORGE_THEME_STORAGE_KEY);
    return isForgeTheme(storedTheme) ? storedTheme : DEFAULT_FORGE_THEME;
  });

  const handleSettingsConfigured = useCallback(() => {
    setHasApiKey(true);
    setShowSettings(false);
  }, []);

  const handleThemeChange = useCallback((nextTheme: ForgeTheme) => {
    setTheme(nextTheme);
    window.localStorage.setItem(FORGE_THEME_STORAGE_KEY, nextTheme);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    let cancelled = false;
    void checkConfiguredApiKey().then((ok) => {
      if (cancelled) return;
      setHasApiKey(ok);
      setShowSettings(!ok);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleEditorReady = useCallback(async (editor: Editor) => {
    editorRef.current = editor;
    try {
      await invoke(Commands.createChapter, { title: "Chapter-1" });
    } catch {
      // Already exists
    }
    try {
      const content = await invoke<string>(Commands.loadChapter, { title: "Chapter-1" });
      editor.commands.setContent(content || "<p>开始写作...</p>");
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
            editorRef.current.commands.setContent("<p>开始写作...</p>");
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
      return { applied: false, saved: false, error: "编辑器尚未准备好。" };
    }

    if ("chapter" in operation && operation.chapter !== currentChapter) {
      return {
        applied: false,
        saved: false,
        error: `操作目标是 ${operation.chapter}，但当前打开的是 ${currentChapter}。`,
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
        return { applied: false, saved: false, error: `暂不支持这种操作：${operation.kind}` };
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
      return { applied: true, saved: false, error: `保存失败：${String(e)}` };
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

  if (hasApiKey === null && !showSettings) {
    return (
      <div className="forge-overlay" data-theme={theme}>
        <div className="forge-boot-card">
          <div className="forge-brand-mark">F</div>
          <h1>正在打开 Forge</h1>
          <p>正在检查本机模型配置...</p>
        </div>
      </div>
    );
  }

  if (showSettings) {
    const isOnboarding = hasApiKey !== true;
    return (
      <div className="forge-overlay" data-theme={theme}>
        <div className={isOnboarding ? "forge-onboarding-shell" : "forge-settings-shell"}>
          <header className="forge-onboarding-header">
            <div>
              <div className="forge-brand-mark">F</div>
              <h1>{isOnboarding ? "连接 Forge" : "设置"}</h1>
              <p>
                {isOnboarding
                  ? "只需要配置一次模型密钥。Forge 会把密钥保存在本机，保存成功后直接进入写作界面。"
                  : "模型连接、诊断日志和本地恢复工具。"}
              </p>
            </div>
            {!isOnboarding && (
              <button
                className="forge-btn forge-btn-secondary"
                onClick={() => setShowSettings(false)}
              >
                关闭
              </button>
            )}
          </header>
          <SettingsView
            mode={isOnboarding ? "onboarding" : "panel"}
            onConfigured={handleSettingsConfigured}
            theme={theme}
            onThemeChange={handleThemeChange}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="forge-root" data-theme={theme}>
      <header className="forge-titlebar">
        <div className="forge-titlebar-left">
          <div className="forge-title-lockup">
            <span className="forge-brand-dot">F</span>
            <div>
              <strong>Forge</strong>
              <span>中文小说创作</span>
            </div>
          </div>
        </div>
        <div className="forge-titlebar-center">
          <span className="forge-titlebar-chapter">{currentChapter}</span>
          <span className={`forge-save-pill ${isEditorDirty ? "dirty" : ""}`}>
            {isEditorDirty ? "未保存" : "已保存"}
          </span>
        </div>
        <div className="forge-titlebar-actions">
          <button
            className="forge-btn forge-btn-primary"
            onClick={handleGenerate}
            disabled={isAgentThinking}
          >
            {isAgentThinking ? "续写中..." : "续写正文"}
          </button>
          <button
            className="forge-btn forge-btn-ghost"
            onClick={() => setShowSettings(true)}
          >
            设置
          </button>
        </div>
      </header>

      <div className="forge-shell">
        <nav className="forge-rail" aria-label="主要导航">
          <button
            className={`forge-rail-btn ${!sidebarCollapsed ? "active" : ""}`}
            onClick={() => setSidebarCollapsed((value) => !value)}
            aria-pressed={!sidebarCollapsed}
          >
            章节
          </button>
          {([
            ["write", "续写"],
            ["review", "审稿"],
            ["explore", "项目"],
            ["inspect", "检查"],
          ] as const).map(([mode, label]) => (
            <button
              key={mode}
              className={`forge-rail-btn ${!companionCollapsed && storyMode === mode ? "active" : ""}`}
              onClick={() => {
                setStoryMode(mode);
                setCompanionCollapsed((value) => (storyMode === mode ? !value : false));
              }}
              aria-pressed={!companionCollapsed && storyMode === mode}
            >
              {label}
            </button>
          ))}
        </nav>

        <aside className={`forge-sidebar forge-floating-panel ${sidebarCollapsed ? "collapsed" : ""}`}>
          <div className="forge-panel-header">
            <div>
              <span className="forge-panel-kicker">项目</span>
              <strong>章节与大纲</strong>
            </div>
            <button
              className="forge-btn forge-btn-ghost forge-btn-compact"
              onClick={() => setSidebarCollapsed(true)}
            >
              关闭
            </button>
          </div>
          <div className="forge-sidebar-body">
            <ProjectTree onSelectChapter={handleSelectChapter} editorRef={editorRef} onApplyFix={handleApplyFix} />
          </div>
        </aside>

        <main className="forge-main">
          <div className="forge-editor-area">
            <div className="forge-editor-body">
              <EditorPanel onEditorReady={handleEditorReady} onSelectionUpdate={handleSelectionUpdate} />
            </div>
          </div>
        </main>

        {!companionCollapsed && (
          <aside className="forge-companion forge-floating-panel">
            <div className="forge-panel-header">
              <div>
                <span className="forge-panel-kicker">助手</span>
                <strong>
                  {storyMode === "write"
                    ? "续写辅助"
                    : storyMode === "review"
                      ? "审稿队列"
                      : storyMode === "explore"
                        ? "项目问答"
                        : "运行检查"}
                </strong>
              </div>
              <button
                className="forge-btn forge-btn-ghost forge-btn-compact"
                onClick={() => setCompanionCollapsed(true)}
              >
                关闭
              </button>
            </div>
            <div className="forge-mode-row" aria-label="助手模式">
              {([
                ["write", "续写"],
                ["review", "审稿"],
                ["explore", "项目"],
                ["inspect", "检查"],
              ] as const).map(([mode, label]) => (
                <button
                  key={mode}
                  className={`forge-mode-btn ${storyMode === mode ? "active" : ""}`}
                  onClick={() => setStoryMode(mode)}
                >
                  {label}
                </button>
              ))}
            </div>
            <div className="forge-companion-body">
              {storyMode === "inspect"
                ? <WriterInspectorPanel getContext={getContext} />
                : storyMode === "explore"
                  ? <AgentPanel mode={storyMode} getContext={getContext} />
                  : <CompanionPanel mode={storyMode} onApplyOperation={handleApplyWriterOperation} />
              }
            </div>
          </aside>
        )}
      </div>
    </div>
  );
}
export default App;
