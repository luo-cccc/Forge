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
type ConnectionState = "checking" | "connected" | "empty" | "saving" | "error";

interface SettingsViewProps {
  onConfigured?: () => void;
  mode?: "onboarding" | "panel";
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(ms: number): string {
  if (!ms) return "unknown";
  return new Date(ms).toLocaleString();
}

function keychainLabel(): string {
  if (navigator.platform.includes("Win")) return "Windows Credential Manager";
  if (navigator.platform.includes("Mac")) return "macOS Keychain";
  return "system secret storage";
}

export default function SettingsView({
  onConfigured,
  mode = "panel",
}: SettingsViewProps) {
  const [apiKey, setApiKey] = useState("");
  const [showSecret, setShowSecret] = useState(false);
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [connectionState, setConnectionState] = useState<ConnectionState>("checking");
  const [statusMessage, setStatusMessage] = useState("Checking local keychain...");
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
          ? `Connected. Key is stored in ${keychainLabel()}.`
          : "Paste an OpenAI-compatible API key to connect the model.",
      );
      return ok;
    } catch (e) {
      setHasKey(false);
      setConnectionState("error");
      setStatusMessage(`Could not read the local keychain: ${String(e)}`);
      return false;
    }
  }, []);

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
      setStatusMessage("Paste an API key first.");
      return;
    }

    setConnectionState("saving");
    setStatusMessage("Saving key to the local keychain...");
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
      setStatusMessage(`Save failed: ${String(e)}`);
    }
  }, [apiKey, hasKey, onConfigured, refreshConnection]);

  const handlePaste = useCallback(async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        setApiKey(text.trim());
        setStatusMessage("Key pasted. Click Connect to save it locally.");
      }
    } catch (e) {
      setConnectionState("error");
      setStatusMessage(`Clipboard access failed: ${String(e)}`);
    }
  }, []);

  const handleVerify = useCallback(async () => {
    setVerifying(true);
    try {
      const ok = await refreshConnection();
      setLogs(ok ? "A key is present in the OS keychain." : "No saved key found.");
    } finally {
      setVerifying(false);
    }
  }, [refreshConnection]);

  const handleExportLogs = useCallback(async () => {
    try {
      const path = await invoke<string>(Commands.exportDiagnosticLogs);
      setLogs(`Logs exported to: ${path}`);
    } catch (e) {
      setLogs(`Export failed: ${String(e)}`);
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
      setLogs(`Trajectory export failed: ${String(e)}`);
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
      setLogs(`Restore failed: ${String(e)}`);
    } finally {
      setRestoring(null);
    }
  }, [refreshBackups]);

  const statusClass =
    connectionState === "connected"
      ? "border-success/30 bg-success/10 text-success"
      : connectionState === "error"
        ? "border-danger/30 bg-danger/10 text-danger"
        : connectionState === "saving" || connectionState === "checking"
          ? "border-warning/30 bg-warning/10 text-warning"
          : "border-border-subtle bg-bg-deep text-text-muted";
  const primaryLabel =
    connectionState === "saving"
      ? "Connecting..."
      : hasKey && !apiKey.trim()
        ? mode === "onboarding" ? "Continue" : "Connected"
        : "Connect";

  return (
    <div className={`flex h-full flex-col overflow-y-auto ${mode === "onboarding" ? "" : "p-4"}`}>
      {mode === "panel" && (
        <div className="mb-4">
          <h2 className="font-display text-sm font-medium tracking-wide text-text-primary">
            Settings
          </h2>
          <p className="mt-1 text-xs text-text-muted">
            Model connection and local maintenance.
          </p>
        </div>
      )}

      <section className="rounded-xl border border-border-subtle bg-bg-raised/80 p-4 shadow-sm">
        <div className="mb-4 flex items-start justify-between gap-3">
          <div>
            <div className="text-sm font-medium text-text-primary">Connect your model</div>
            <p className="mt-1 text-xs leading-relaxed text-text-muted">
              Forge uses an OpenAI-compatible endpoint. The key stays on this device.
            </p>
          </div>
          <div className={`shrink-0 rounded-full border px-2.5 py-1 text-[10px] ${statusClass}`}>
            {connectionState === "connected"
              ? "Connected"
              : connectionState === "saving"
                ? "Saving"
                : connectionState === "checking"
                  ? "Checking"
                  : connectionState === "error"
                    ? "Action needed"
                    : "Not connected"}
          </div>
        </div>

        <div className="mb-3 grid grid-cols-2 gap-2">
          <div className="rounded-lg border border-accent/25 bg-accent-subtle p-3">
            <div className="text-xs font-medium text-text-primary">OpenRouter compatible</div>
            <div className="mt-1 text-[10px] text-text-muted">Default desktop runtime</div>
          </div>
          <div className="rounded-lg border border-border-subtle bg-bg-deep p-3">
            <div className="text-xs font-medium text-text-secondary">Stored locally</div>
            <div className="mt-1 text-[10px] text-text-muted">{keychainLabel()}</div>
          </div>
        </div>

        <label className="mb-1.5 block text-xs font-medium text-text-secondary">
          API key
        </label>
        <div className="flex gap-2">
          <div className="relative min-w-0 flex-1">
            <input
              type={showSecret ? "text" : "password"}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSave();
              }}
              placeholder={hasKey ? "Key stored. Paste a new key to replace it." : "sk-or-v1-..."}
              className="h-10 w-full rounded-lg border border-border-subtle bg-bg-deep px-3 pr-16 text-sm text-text-primary placeholder-text-muted outline-none transition-colors focus:border-accent/60"
            />
            <button
              type="button"
              onClick={() => setShowSecret((value) => !value)}
              className="absolute right-2 top-1/2 -translate-y-1/2 rounded px-2 py-1 text-[10px] text-text-muted hover:bg-bg-raised hover:text-text-primary"
            >
              {showSecret ? "Hide" : "Show"}
            </button>
          </div>
          <button
            type="button"
            onClick={handlePaste}
            className="forge-btn forge-btn-secondary h-10"
          >
            Paste
          </button>
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={handleSave}
            disabled={connectionState === "saving" || (hasKey === true && !apiKey.trim() && mode === "panel")}
            className="forge-btn forge-btn-primary h-9 px-4"
          >
            {primaryLabel}
          </button>
          <button
            type="button"
            onClick={handleVerify}
            disabled={verifying}
            className="forge-btn forge-btn-secondary h-9"
          >
            {verifying ? "Checking..." : "Check saved key"}
          </button>
        </div>

        <div className={`mt-3 rounded-lg border px-3 py-2 text-xs ${statusClass}`}>
          {statusMessage}
        </div>
      </section>

      {mode === "panel" && (
        <details
          className="mt-4 rounded-xl border border-border-subtle bg-bg-surface"
          open={advancedOpen}
          onToggle={(event) => setAdvancedOpen(event.currentTarget.open)}
        >
          <summary className="cursor-pointer select-none px-4 py-3 text-xs font-medium text-text-secondary hover:text-text-primary">
            Advanced maintenance
          </summary>
          <div className="space-y-4 border-t border-border-subtle p-4">
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={handleExportLogs}
                className="forge-btn forge-btn-secondary"
              >
                Export logs
              </button>
              <button
                type="button"
                onClick={() => handleExportTrajectory()}
                className="forge-btn forge-btn-secondary"
              >
                Export Forge trace
              </button>
              <button
                type="button"
                onClick={() => handleExportTrajectory("trace_viewer")}
                className="forge-btn forge-btn-secondary"
              >
                Export trace viewer JSONL
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
                    <div key={item.id} className="rounded-lg border border-border-subtle bg-bg-raised p-2">
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
                              <div
                                key={backup.id}
                                className="flex items-center justify-between gap-2 rounded-md bg-bg-deep px-2 py-1"
                              >
                                <div className="min-w-0">
                                  <div className="truncate text-[10px] text-text-secondary" title={backup.filename}>
                                    {formatTime(backup.modifiedAt)}
                                  </div>
                                  <div className="font-mono text-[10px] text-text-muted">
                                    {formatBytes(backup.bytes)}
                                  </div>
                                </div>
                                <button
                                  type="button"
                                  onClick={() => handleRestore(item, backup)}
                                  disabled={restoring === restoreKey}
                                  className="shrink-0 rounded-md border border-border-subtle px-2 py-1 text-[10px] text-text-secondary hover:border-accent/40 hover:text-accent disabled:opacity-50"
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
              <div className="rounded-lg border border-border-subtle bg-bg-deep p-2 font-mono text-xs text-text-muted">
                {logs}
              </div>
            )}
          </div>
        </details>
      )}
    </div>
  );
}
