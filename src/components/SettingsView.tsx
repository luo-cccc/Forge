import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Commands,
  Events,
  type BackupTarget,
  type FileBackupInfo,
  type ProjectFileRestored,
} from "../protocol";

const RECOVERY_TARGETS = [
  { id: "lorebook", label: "Lorebook", target: { kind: "lorebook" } },
  { id: "outline", label: "Outline", target: { kind: "outline" } },
  { id: "project_brain", label: "Project Brain", target: { kind: "project_brain" } },
] satisfies Array<{ id: ProjectFileRestored["kind"]; label: string; target: BackupTarget }>;

type BackupMap = Record<ProjectFileRestored["kind"], FileBackupInfo[]>;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(ms: number): string {
  if (!ms) return "unknown";
  return new Date(ms).toLocaleString();
}

export default function SettingsView() {
  const [apiKey, setApiKey] = useState("");
  const [saved, setSaved] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [logs, setLogs] = useState("");
  const [verifying, setVerifying] = useState(false);
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

  useEffect(() => {
    const check = async () => {
      try {
        const ok = await invoke<boolean>(Commands.checkApiKey, { provider: "openai" });
        setHasKey(ok);
      } catch (e) {
        console.error("Failed to check API key:", e);
      }
    };
    const loadBackups = async () => {
      try {
        const pairs = await Promise.all(
          RECOVERY_TARGETS.map(async (item) => {
            const rows = await invoke<FileBackupInfo[]>(Commands.listFileBackups, {
              target: item.target,
            });
            return [item.id, rows.slice(0, 5)] as const;
          }),
        );
        setBackups(Object.fromEntries(pairs) as BackupMap);
      } catch (e) {
        console.error("Failed to load backups:", e);
      }
    };
    void check();
    void loadBackups();
  }, []);

  const handleSave = useCallback(async () => {
    try {
      await invoke(Commands.setApiKey, { provider: "openai", key: apiKey });
      setApiKey("");
      setSaved(true);
      setHasKey(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (e) {
      setLogs(`Save failed: ${e}`);
    }
  }, [apiKey]);

  const handleVerify = useCallback(async () => {
    setVerifying(true);
    try {
      const ok = await invoke<boolean>(Commands.checkApiKey, { provider: "openai" });
      setHasKey(ok);
      setLogs(ok ? "Key is stored in the OS keychain." : "No key found");
    } catch (e) {
      setLogs(`Verify failed: ${e}`);
    } finally {
      setVerifying(false);
    }
  }, []);

  const handleExportLogs = useCallback(async () => {
    try {
      const path = await invoke<string>(Commands.exportDiagnosticLogs);
      setLogs(`Logs exported to: ${path}`);
    } catch (e) {
      setLogs(`Export failed: ${e}`);
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
          ? `Trace viewer trajectory exported to: ${path}`
          : `Forge trajectory exported to: ${path}`,
      );
    } catch (e) {
      setLogs(`Trajectory export failed: ${e}`);
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
      setLogs(`${item.label} restored from ${backup.filename}`);
      await refreshBackups();
    } catch (e) {
      setLogs(`Restore failed: ${e}`);
    } finally {
      setRestoring(null);
    }
  }, [refreshBackups]);

  return (
    <div className="flex flex-col h-full overflow-y-auto p-4 space-y-4">
      <h2 className="text-sm font-display text-text-primary tracking-wider">Settings</h2>

      <div className="space-y-2">
        <label className="text-xs text-text-secondary">API Key (OpenAI / OpenRouter)</label>
        <div className="flex gap-2">
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={hasKey ? "•••••••• (stored in OS keychain)" : "sk-or-v1-..."}
            className="flex-1 px-3 py-1.5 rounded-sm bg-bg-surface border border-border-subtle text-text-primary text-xs placeholder-text-muted focus:outline-none focus:border-accent"
          />
          <button
            onClick={handleSave}
            className="px-3 py-1.5 rounded-sm bg-accent hover:bg-accent/80 text-bg-deep text-xs transition-colors"
          >
            {saved ? "Saved!" : "Save"}
          </button>
        </div>
        <p className="text-[10px] text-text-muted">
          Stored securely in {navigator.platform.includes("Win") ? "Windows Credential Manager" : "macOS Keychain / Linux Secret Service"}.
        </p>
      </div>

      <div className="flex gap-2">
        <button
          onClick={handleVerify}
          disabled={verifying}
          className="text-xs px-3 py-1.5 rounded-sm bg-bg-raised border border-border-subtle text-text-secondary hover:text-text-primary transition-colors disabled:opacity-50"
        >
          {verifying ? "..." : "Verify Key"}
        </button>
        <button
          onClick={handleExportLogs}
          className="text-xs px-3 py-1.5 rounded-sm bg-bg-raised border border-border-subtle text-text-secondary hover:text-text-primary transition-colors flex items-center gap-1"
        >
          Export Diagnostic Logs
        </button>
        <button
          onClick={() => handleExportTrajectory()}
          className="text-xs px-3 py-1.5 rounded-sm bg-bg-raised border border-border-subtle text-text-secondary hover:text-text-primary transition-colors"
        >
          Export Forge Trace
        </button>
        <button
          onClick={() => handleExportTrajectory("trace_viewer")}
          className="text-xs px-3 py-1.5 rounded-sm bg-bg-raised border border-border-subtle text-text-secondary hover:text-text-primary transition-colors"
        >
          Export Trace Viewer JSONL
        </button>
      </div>

      <div className="space-y-2 border-t border-border-subtle pt-4">
        <div>
          <h3 className="text-xs font-medium text-text-primary">Recovery</h3>
          <p className="mt-1 text-[10px] text-text-muted">
            Restore project support files from bounded local backups.
          </p>
        </div>
        <div className="space-y-2">
          {RECOVERY_TARGETS.map((item) => {
            const rows = backups[item.id] ?? [];
            return (
              <div key={item.id} className="rounded-sm border border-border-subtle bg-bg-raised p-2">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <span className="text-xs text-text-secondary">{item.label}</span>
                  <span className="font-mono text-[10px] text-text-muted">{rows.length}</span>
                </div>
                {rows.length === 0 ? (
                  <div className="text-[10px] text-text-muted">No backups yet.</div>
                ) : (
                  <div className="space-y-1">
                    {rows.slice(0, 3).map((backup) => {
                      const restoreKey = `${item.id}:${backup.id}`;
                      return (
                        <div key={backup.id} className="flex items-center justify-between gap-2 rounded bg-bg-deep px-2 py-1">
                          <div className="min-w-0">
                            <div className="truncate text-[10px] text-text-secondary" title={backup.filename}>
                              {formatTime(backup.modifiedAt)}
                            </div>
                            <div className="font-mono text-[10px] text-text-muted">
                              {formatBytes(backup.bytes)}
                            </div>
                          </div>
                          <button
                            onClick={() => handleRestore(item, backup)}
                            disabled={restoring === restoreKey}
                            className="shrink-0 rounded-sm border border-border-subtle px-2 py-1 text-[10px] text-text-secondary hover:border-accent/40 hover:text-accent disabled:opacity-50"
                          >
                            {restoring === restoreKey ? "..." : "Restore"}
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
        <div className="text-xs text-text-muted font-mono bg-code-bg p-2 rounded-sm">{logs}</div>
      )}
    </div>
  );
}
