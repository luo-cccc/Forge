pub mod actions;
pub mod agent_loop;
pub mod config;
pub mod context_pack;
pub mod domain;
pub mod hermes_memory;
pub mod llm;
pub mod planner;
pub mod provider;
pub mod retry;
pub mod router;
pub mod run_trace;
pub mod tool_executor;
pub mod tool_registry;
pub mod vector_db;

pub use actions::{parse_actions, Action};
pub use agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopEvent};
pub use config::HarnessConfig;
pub use context_pack::{
    char_count, truncate_text_report, ContextBudgetReport, ContextPacker, ContextSourceReport,
    PackedContext,
};
pub use domain::{writing_domain_profile, AgentDomainProfile, ContextPriority, DomainCapability};
pub use hermes_memory::HermesDB;
pub use llm::LLMClient;
pub use router::{classify_intent, Intent};
pub use run_trace::{AgentRunEvent, AgentRunEventKind, AgentRunStatus, AgentRunTrace};
pub use tool_registry::{
    default_writing_tool_registry, ToolDescriptor, ToolFilter, ToolRegistry, ToolRegistryError,
    ToolSideEffectLevel, ToolStage,
};
pub use tool_executor::{DoomLoopDetector, ToolExecution, ToolExecutor, ToolHandler};
pub use vector_db::{chunk_text, cosine_similarity, extract_keywords, Chunk, VectorDB};

/// 通用文本截断 — 取最后 max_chars 字符，从词边界断开
pub fn truncate_context(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        return text;
    }
    let start = text.len().saturating_sub(max_chars);
    let slice = &text[start..];
    if let Some(idx) = slice.find(' ') {
        &slice[idx + 1..]
    } else {
        slice
    }
}
