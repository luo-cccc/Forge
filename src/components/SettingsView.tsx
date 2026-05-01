import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Commands } from "../protocol";

export default function SettingsView() {
  const [apiKey, setApiKey] = useState("");
  const [saved, setSaved] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [logs, setLogs] = useState("");
  const [verifying, setVerifying] = useState(false);

  useEffect(() => {
    const check = async () => {
      try {
        const ok = await invoke<boolean>(Commands.checkApiKey, { provider: "openai" });
        setHasKey(ok);
      } catch (e) {
        console.error("Failed to check API key:", e);
      }
    };
    check();
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

  return (
    <div className="flex flex-col h-full p-4 space-y-4">
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
      </div>

      {logs && (
        <div className="text-xs text-text-muted font-mono bg-code-bg p-2 rounded-sm">{logs}</div>
      )}
    </div>
  );
}
