import EditorPanel from "./components/EditorPanel";
import AgentPanel from "./components/AgentPanel";

function App() {
  return (
    <div className="h-screen bg-slate-900 text-white flex">
      <div className="w-2/3 h-full">
        <EditorPanel />
      </div>
      <div className="w-1/3 h-full">
        <AgentPanel />
      </div>
    </div>
  );
}

export default App;
