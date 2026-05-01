use agent_harness_core::tool_executor::ToolHandler;
use tauri::AppHandle;

/// Real tool handler that bridges agent-harness-core tools to the Tauri app's storage layer.
pub struct TauriToolBridge {
    pub app: AppHandle,
}

#[async_trait::async_trait]
impl ToolHandler for TauriToolBridge {
    async fn execute(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match tool_name {
            "load_current_chapter" => {
                let chapter = args.get("chapter").and_then(|v| v.as_str()).unwrap_or("");
                let content = crate::storage::load_chapter(&self.app, chapter.to_string())
                    .map_err(|e| format!("load_current_chapter: {}", e))?;
                Ok(serde_json::json!({"content": content, "chapter": chapter}))
            }
            "search_lorebook" => {
                let keyword = args.get("keyword").and_then(|v| v.as_str()).unwrap_or("");
                let entries = crate::storage::load_lorebook(&self.app)
                    .map_err(|e| format!("search_lorebook: {}", e))?;
                let matches: Vec<serde_json::Value> = entries
                    .iter()
                    .filter(|e| {
                        e.keyword.to_lowercase().contains(&keyword.to_lowercase())
                            || keyword.to_lowercase().contains(&e.keyword.to_lowercase())
                    })
                    .map(|e| serde_json::json!({"keyword": e.keyword, "content": e.content}))
                    .collect();
                Ok(serde_json::json!({"matches": matches}))
            }
            "load_outline_node" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let nodes = crate::storage::load_outline(&self.app)
                    .map_err(|e| format!("load_outline_node: {}", e))?;
                let node = nodes
                    .iter()
                    .find(|n| n.chapter_title == id)
                    .map(|n| {
                        serde_json::json!({
                            "chapter_title": n.chapter_title,
                            "summary": n.summary,
                            "status": n.status,
                        })
                    })
                    .unwrap_or(serde_json::json!({"error": "not found"}));
                Ok(node)
            }
            "query_project_brain" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let api_key =
                    crate::resolve_api_key().ok_or_else(|| "No API key configured".to_string())?;
                let settings = crate::llm_runtime::settings(api_key);
                let mut result_text = String::new();
                crate::brain_service::answer_query(&self.app, &settings, query, |content| {
                    result_text.push_str(&content);
                    Ok(crate::llm_runtime::StreamControl::Continue)
                })
                .await
                .map_err(|e| format!("query_project_brain: {}", e))?;
                Ok(serde_json::json!({"answer": result_text}))
            }
            "generate_bounded_continuation" => {
                let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                let api_key =
                    crate::resolve_api_key().ok_or_else(|| "No API key configured".to_string())?;
                let settings = crate::llm_runtime::settings(api_key);
                let messages = vec![serde_json::json!({"role": "user", "content": prompt})];
                let result = crate::llm_runtime::chat_text(&settings, messages, false, 120)
                    .await
                    .map_err(|e| format!("generate_bounded_continuation: {}", e))?;
                Ok(serde_json::json!({"text": result}))
            }
            "generate_chapter_draft" => {
                Err("generate_chapter_draft requires explicit approval.".into())
            }
            // Lightweight tools — no real I/O needed
            "read_user_drift_profile"
            | "load_domain_profile"
            | "pack_agent_context"
            | "plan_chapter_task"
            | "classify_writing_intent"
            | "record_run_trace" => Ok(serde_json::json!({"status": "ok", "tool": tool_name})),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
