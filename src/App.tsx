<div className="forge-root">
      <nav className="forge-activity-bar">
        <button className="forge-activity-btn active" title="Chapters">📄</button>
        <button className="forge-activity-btn" title="Settings" onClick={() => setShowSettings(true)}>⚙</button>
      </nav>

      <aside className={`forge-sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        <div className="forge-sidebar-header">
          <span>Chapters</span>
          <button className="forge-btn-ghost" onClick={() => setSidebarCollapsed(!sidebarCollapsed)} style={{fontSize:10,padding:'0 4px'}}>◁</button>
        </div>
        <div className="forge-sidebar-body">
          <ProjectTree onSelectChapter={handleSelectChapter} editorRef={editorRef} onApplyFix={handleApplyFix} />
        </div>
      </aside>

      <div className={`forge-main ${sidebarCollapsed ? '' : 'sidebar-open'}`}>
        <div className="forge-editor-area">
          <div className="forge-tab-bar">
            <div className="forge-tab active">{currentChapter}</div>
            <button className="forge-btn-ghost" onClick={handleGenerate} style={{marginLeft:'auto',height:26,fontSize:'var(--text-xs)'}}>
              {isAgentThinking ? '⏳' : '+ Generate'}
            </button>
            <button className="forge-btn-ghost" onClick={() => setCompanionCollapsed(!companionCollapsed)} style={{height:26,fontSize:'var(--text-xs)'}}>
              {companionCollapsed ? '◁' : '▷'}
            </button>
          </div>
          <div className="forge-editor-body">
            <EditorPanel onEditorReady={handleEditorReady} onSelectionUpdate={handleSelectionUpdate} />
          </div>
        </div>

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