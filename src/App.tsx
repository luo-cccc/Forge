import HarnessEcho from "./HarnessEcho";

function App() {
  return (
    <div className="min-h-screen bg-slate-900 text-white flex flex-col items-center justify-center gap-8">
      <h1 className="text-3xl font-bold tracking-tight">Agent Writer</h1>
      <HarnessEcho />
    </div>
  );
}

export default App;
