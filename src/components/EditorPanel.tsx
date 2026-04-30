export default function EditorPanel() {
  return (
    <div className="flex flex-col h-full">
      <div className="px-4 py-3 border-b border-slate-700 text-sm text-slate-400 font-medium">
        Editor
      </div>
      <textarea
        className="flex-1 w-full bg-transparent text-slate-200 p-4 resize-none focus:outline-none font-mono text-sm leading-relaxed"
        placeholder="Start writing your novel here..."
      />
    </div>
  );
}
