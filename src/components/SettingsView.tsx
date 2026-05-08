import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Commands,
  Events,
  type BackupTarget,
  type FileBackupInfo,
  type ProjectFileRestored,
} from "../protocol";
import { FORGE_THEMES, type ForgeTheme } from "../uiPreferences";

const RECOVERY_TARGETS = [
  { id: "lorebook", label: "设定库", target: { kind: "lorebook" } },
  { id: "outline", label: "大纲", target: { kind: "outline" } },
  { id: "project_brain", label: "项目记忆", target: { kind: "project_brain" } },
] satisfies Array<{ id: ProjectFileRestored["kind"]; label: string; target: BackupTarget }>;

type BackupMap = Record<ProjectFileRestored["kind"], FileBackupInfo[]>;
type ConnectionState = "checking" | "connected" | "empty" | "saving" | "error";

interface SettingsViewProps {
  onConfigured?: () => void;
  mode?: "onboarding" | "panel";
  theme: ForgeTheme;
  onThemeChange: (theme: ForgeTheme) => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(ms: number): string {
  if (!ms) return "未知";
  return new Date(ms).toLocaleString();
}

function keychainLabel(): string {
  if (navigator.platform.includes("Win")) return "Windows 凭据管理器";
  if (navigator.platform.includes("Mac")) return "macOS 钥匙串";
  return "系统密钥存储";
}

export default function SettingsView({
  onConfigured,
  mode = "panel",
  theme,
  onThemeChange,
}: SettingsViewProps) {
  const [apiKey, setApiKey] = useState("");
  const [showSecret, setShowSecret] = useState(false);
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [connectionState, setConnectionState] = useState<ConnectionState>("checking");
  const [statusMessage, setStatusMessage] = useState("正在检查本机密钥...");
  const [logs, setLogs] = useState("");
  const [verifying, setVerifying] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [backups, setBackups] = useState<BackupMap>({
    lorebook: [],
    outline: [],
    project_brain: [],
  });
  const [restoring, setRestoring] = useState<string | null>(null);

  const refreshBackups = useCallback(async () => {
    const pairs = await Promise.all(
      RECOVERY_TARGETS.map(async (item) => {
        const rows = await invoke<FileBackupInfo[]>(Commands.listFileBackups, {
          target: item.target,
        });
        return [item.id, rows.slice(0, 5)] as const;
      }),
    );
    setBackups(Object.fromEntries(pairs) as BackupMap);
  }, []);

  const refreshConnection = useCallback(async () => {
    try {
      const ok = await invoke<boolean>(Commands.checkApiKey, { provider: "openai" });
      setHasKey(ok);
      setConnectionState(ok ? "connected" : "empty");
      setStatusMessage(
        ok
          ? `已连接。密钥保存在${keychainLabel()}。`
          : "粘贴 OpenAI 或 OpenRouter 兼容密钥后即可开始写作。",
      );
      if (ok && mode === "onboarding") {
        onConfigured?.();
      }
      return ok;
    } catch (e) {
      setHasKey(false);
      setConnectionState("error");
      setStatusMessage(`无法读取本机密钥：${String(e)}`);
      return false;
    }
  }, [mode, onConfigured]);

  useEffect(() => {
    const connectionTimer = setTimeout(() => {
      void refreshConnection();
    }, 0);
    const backupsTimer = setTimeout(() => {
      void refreshBackups().catch((e) => {
        console.error("Failed to load backups:", e);
      });
    }, 0);
    return () => {
      clearTimeout(connectionTimer);
      clearTimeout(backupsTimer);
    };
  }, [refreshBackups, refreshConnection]);

  const handleSave = useCallback(async () => {
    if (!apiKey.trim()) {
      if (hasKey) {
        onConfigured?.();
        return;
      }
      setConnectionState("error");
      setStatusMessage("请先粘贴 API key。");
      return;
    }

    setConnectionState("saving");
    setStatusMessage("正在保存到本机密钥存储...");
    try {
      await invoke(Commands.setApiKey, { provider: "openai", key: apiKey.trim() });
      setApiKey("");
      setShowSecret(false);
      const ok = await refreshConnection();
      if (ok) {
        onConfigured?.();
      }
    } catch (e) {
      setConnectionState("error");
      setStatusMessage(`保存失败：${String(e)}`);
    }
  }, [apiKey, hasKey, onConfigured, refreshConnection]);

  const handlePaste = useCallback(async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        setApiKey(text.trim());
        setStatusMessage("已粘贴密钥，点击保存后进入写作。");
      }
    } catch (e) {
      setConnectionState("error");
      setStatusMessage(`无法读取剪贴板：${String(e)}`);
    }
  }, []);

  const handleVerify = useCallback(async () => {
    setVerifying(true);
    try {
      const ok = await refreshConnection();
      setLogs(ok ? "已在系统密钥存储中找到密钥。" : "未找到已保存密钥。");
    } finally {
      setVerifying(false);
    }
  }, [refreshConnection]);

  const handleExportLogs = useCallback(async () => {
    try {
      const path = await invoke<string>(Commands.exportDiagnosticLogs);
      setLogs(`日志已导出到：${path}`);
    } catch (e) {
      setLogs(`导出失败：${String(e)}`);
    }
  }, []);

  const handleExportTrajectory = useCallback(async (format?: "trace_viewer") => {
    try {
      const path = await invoke<string>(Commands.exportWriterAgentTrajectory, {
        limit: 200,
        ...(format ? { format } : {}),
      });
      setLogs(
        format === "trace_viewer"
          ? `Trace Viewer 轨迹已导出到：${path}`
          : `Forge 轨迹已导出到：${path}`,
      );
    } catch (e) {
      setLogs(`轨迹导出失败：${String(e)}`);
    }
  }, []);

  const handleRestore = useCallback(async (
    item: (typeof RECOVERY_TARGETS)[number],
    backup: FileBackupInfo,
  ) => {
    const restoreKey = `${item.id}:${backup.id}`;
    setRestoring(restoreKey);
    try {
      await invoke(Commands.restoreFileBackup, {
        target: item.target,
        backupId: backup.id,
      });
      window.dispatchEvent(new CustomEvent<ProjectFileRestored>(Events.projectFileRestored, {
        detail: { kind: item.id },
      }));
      setLogs(`${item.label}已从 ${backup.filename} 恢复`);
      await refreshBackups();
    } catch (e) {
      setLogs(`恢复失败：${String(e)}`);
    } finally {
      setRestoring(null);
    }
  }, [refreshBackups]);

  const statusTone =
    connectionState === "connected"
      ? "success"
      : connectionState === "error"
        ? "danger"
        : connectionState === "saving" || connectionState === "checking"
          ? "warning"
          : "";
  const primaryLabel =
    connectionState === "saving"
      ? "正在保存..."
      : hasKey && !apiKey.trim()
        ? mode === "onboarding" ? "进入写作" : "已连接"
        : "保存并进入写作";

  return (
    <div className={`forge-settings-view ${mode === "onboarding" ? "onboarding" : "panel"}`}>
      {mode === "panel" && (
        <div>
          <h2 className="forge-settings-title">设置</h2>
          <p className="forge-settings-subtitle">模型连接与本机维护。</p>
        </div>
      )}

      <section className="forge-settings-card">
        <div className="forge-settings-card-header">
          <div>
            <div className="forge-settings-card-title">模型密钥</div>
            <p className="forge-settings-card-description">
              Forge 使用 OpenAI 兼容接口，密钥只保存在这台电脑上。
            </p>
          </div>
          <div className={`forge-status-badge ${statusTone}`}>
            {connectionState === "connected"
              ? "已连接"
              : connectionState === "saving"
                ? "保存中"
                : connectionState === "checking"
                  ? "检查中"
                  : connectionState === "error"
                    ? "需要处理"
                    : "未连接"}
          </div>
        </div>

        <div className="forge-provider-summary">
          <div>
            <div className="forge-card-title">兼容 OpenAI / OpenRouter</div>
            <div className="forge-card-description">粘贴密钥即可连接当前模型运行时。</div>
          </div>
          <div>
            <div className="forge-card-title">本机保存</div>
            <div className="forge-card-description">{keychainLabel()}</div>
          </div>
        </div>

        <label className="forge-label">
          密钥
        </label>
        <div className="forge-secret-row">
          <div className="forge-secret-input-wrap">
            <input
              type={showSecret ? "text" : "password"}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSave();
              }}
              placeholder={hasKey ? "已保存密钥。粘贴新密钥可替换。" : "sk-or-v1-..."}
              className="forge-field"
            />
            <button
              type="button"
              onClick={() => setShowSecret((value) => !value)}
              className="forge-secret-toggle"
            >
              {showSecret ? "隐藏" : "显示"}
            </button>
          </div>
          <button type="button" onClick={handlePaste} className="forge-btn forge-btn-secondary forge-btn-large">
            粘贴
          </button>
        </div>

        <div className="forge-connect-actions">
          <button
            type="button"
            onClick={handleSave}
            disabled={connectionState === "saving" || (hasKey === true && !apiKey.trim() && mode === "panel")}
            className="forge-btn forge-btn-primary forge-btn-large"
          >
            {primaryLabel}
          </button>
          {mode === "onboarding" && (
            <span className="forge-inline-help">
              粘贴后按 Enter 也可以保存。
            </span>
          )}
        </div>

        <div className={`forge-status-message ${statusTone}`}>
          {statusMessage}
        </div>
      </section>

      <section className="forge-settings-card">
        <div className="forge-settings-card-header">
          <div>
            <div className="forge-settings-card-title">界面外观</div>
            <p className="forge-settings-card-description">
              主题会立即应用，并保存在这台电脑上。
            </p>
          </div>
        </div>

        <div className="forge-theme-grid" role="group" aria-label="主题">
          {FORGE_THEMES.map((item) => (
            <button
              key={item.value}
              type="button"
              className={`forge-theme-option ${theme === item.value ? "active" : ""}`}
              onClick={() => onThemeChange(item.value)}
              aria-pressed={theme === item.value}
            >
              <span className={`forge-theme-swatch ${item.value}`} aria-hidden="true" />
              <span>
                <strong>{item.label}</strong>
                <small>{item.description}</small>
              </span>
            </button>
          ))}
        </div>
      </section>

      {(mode === "panel" || hasKey) && (
        <details
          className="forge-settings-details"
          open={advancedOpen}
          onToggle={(event) => setAdvancedOpen(event.currentTarget.open)}
        >
          <summary>高级维护</summary>
          <div className="forge-settings-details-body">
            <div className="forge-maintenance-actions">
              <button
                type="button"
                onClick={handleVerify}
                disabled={verifying}
                className="forge-btn forge-btn-secondary"
              >
                {verifying ? "检查中..." : "检查已保存密钥"}
              </button>
              <button
                type="button"
                onClick={handleExportLogs}
                className="forge-btn forge-btn-secondary"
              >
                导出日志
              </button>
              <button
                type="button"
                onClick={() => handleExportTrajectory()}
                className="forge-btn forge-btn-secondary"
              >
                导出 Forge 轨迹
              </button>
              <button
                type="button"
                onClick={() => handleExportTrajectory("trace_viewer")}
                className="forge-btn forge-btn-secondary"
              >
                导出 Trace Viewer JSONL
              </button>
            </div>

            <div>
              <div>
                <h3 className="forge-card-title">恢复</h3>
                <p className="forge-card-description">
                  从本机备份恢复项目辅助文件。
                </p>
              </div>
              <div className="forge-recovery-grid">
                {RECOVERY_TARGETS.map((item) => {
                  const rows = backups[item.id] ?? [];
                  return (
                    <div key={item.id} className="forge-recovery-card">
                      <div className="forge-recovery-card-header">
                        <span>{item.label}</span>
                        <span className="text-mono forge-muted">{rows.length}</span>
                      </div>
                      {rows.length === 0 ? (
                        <div className="forge-muted">暂无备份。</div>
                      ) : (
                        <div className="forge-backup-list">
                          {rows.slice(0, 3).map((backup) => {
                            const restoreKey = `${item.id}:${backup.id}`;
                            return (
                              <div
                                key={backup.id}
                                className="forge-backup-row"
                              >
                                <div className="truncate">
                                  <div className="truncate" title={backup.filename}>
                                    {formatTime(backup.modifiedAt)}
                                  </div>
                                  <div className="text-mono forge-muted">
                                    {formatBytes(backup.bytes)}
                                  </div>
                                </div>
                                <button
                                  type="button"
                                  onClick={() => handleRestore(item, backup)}
                                  disabled={restoring === restoreKey}
                                  className="forge-btn forge-btn-ghost forge-btn-compact"
                                >
                                  {restoring === restoreKey ? "..." : "恢复"}
                                </button>
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            {logs && (
              <div className="forge-log-box">
                {logs}
              </div>
            )}
          </div>
        </details>
      )}
    </div>
  );
}
