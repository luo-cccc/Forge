import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Commands } from "./protocol";

export default function HarnessEcho() {
  const [input, setInput] = useState("");
  const [response, setResponse] = useState("");

  const handleInvoke = async () => {
    try {
      const result = await invoke<string>(Commands.harnessEcho, { message: input });
      setResponse(result);
    } catch (e) {
      setResponse(`Error: ${e}`);
    }
  };

  return (
    <div className="flex flex-col items-center gap-4">
      <div className="flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleInvoke()}
          placeholder="Enter a message..."
          className="px-3 py-2 rounded-md bg-slate-800 border border-slate-600 text-white placeholder-slate-400 focus:outline-none focus:border-blue-500 w-64"
        />
        <button
          onClick={handleInvoke}
          className="px-4 py-2 rounded-md bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors"
        >
          Send
        </button>
      </div>
      {response && (
        <p className="text-slate-300 font-mono text-sm">{response}</p>
      )}
    </div>
  );
}
