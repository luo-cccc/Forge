use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::permission::{PermissionDecision, PermissionMode, PermissionPolicy};
use crate::tool_registry::ToolRegistry;

/// Callback trait for tool handlers.
/// Implementations bridge to the application layer (Tauri storage, lorebook, etc.).
/// Ported from Claw Code's tool dispatch pattern.
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

/// Result of a single tool execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolExecution {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Tracks tool calls to detect doom loops.
/// Ported from OpenCode `processor.ts` doom loop detection (line 305-331).
#[derive(Debug, Clone, Default)]
pub struct DoomLoopDetector {
    call_history: HashMap<(String, u64), u32>,
}

impl DoomLoopDetector {
    fn hash_args(args: &serde_json::Value) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        args.to_string().hash(&mut h);
        h.finish()
    }

    /// Returns true if same tool + same args called 3+ consecutive times.
    pub fn is_doom_loop(&mut self, tool_name: &str, args: &serde_json::Value) -> bool {
        let key = (tool_name.to_string(), Self::hash_args(args));
        let count = self.call_history.entry(key).or_insert(0);
        *count += 1;
        *count >= 3
    }

    /// Reset all tracking. Call after a successful round with different output.
    pub fn reset(&mut self) {
        self.call_history.clear();
    }
}

/// The tool executor dispatches tool calls to registered handlers.
/// Generic over the handler implementation — matches Claw Code pattern.
pub struct ToolExecutor<H: ToolHandler> {
    pub registry: Arc<Mutex<ToolRegistry>>,
    pub handler: H,
    pub doom_detector: DoomLoopDetector,
    pub permission_policy: PermissionPolicy,
}

impl<H: ToolHandler> ToolExecutor<H> {
    pub fn new(registry: ToolRegistry, handler: H) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry)),
            handler,
            doom_detector: DoomLoopDetector::default(),
            permission_policy: PermissionPolicy::new(PermissionMode::WorkspaceWrite),
        }
    }

    pub fn with_permission_policy(mut self, policy: PermissionPolicy) -> Self {
        self.permission_policy = policy;
        self
    }

    /// Execute a tool and return structured result.
    pub async fn execute(&mut self, tool_name: &str, args: serde_json::Value) -> ToolExecution {
        let start = std::time::Instant::now();

        let descriptor = {
            let registry = self.registry.lock().await;
            registry.get(tool_name).cloned()
        };
        let Some(descriptor) = descriptor else {
            return ToolExecution {
                tool_name: tool_name.to_string(),
                input: args,
                output: serde_json::Value::Null,
                error: Some(format!("Tool '{}' is not registered", tool_name)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        };

        match self.permission_policy.authorize(
            &descriptor.name,
            descriptor.side_effect_level,
            descriptor.requires_approval,
        ) {
            PermissionDecision::Allow => {}
            PermissionDecision::Deny { reason } | PermissionDecision::Ask { reason } => {
                return ToolExecution {
                    tool_name: tool_name.to_string(),
                    input: args,
                    output: serde_json::Value::Null,
                    error: Some(reason),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        // Doom loop check
        let is_doom = self.doom_detector.is_doom_loop(tool_name, &args);

        let (output, error) = match self.handler.execute(tool_name, args.clone()).await {
            Ok(result) => (result, None),
            Err(e) => (serde_json::Value::Null, Some(e)),
        };

        let mut error_msg = error;
        if is_doom {
            error_msg = Some(format!(
                "DOOM LOOP DETECTED: tool '{}' called with same args 3+ times",
                tool_name
            ));
        }

        ToolExecution {
            tool_name: tool_name.to_string(),
            input: args,
            output,
            error: error_msg,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_registry::{ToolDescriptor, ToolSideEffectLevel, ToolStage};

    struct MockHandler;

    #[async_trait::async_trait]
    impl ToolHandler for MockHandler {
        async fn execute(
            &self,
            tool_name: &str,
            args: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"tool": tool_name, "args": args}))
        }
    }

    fn registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry
            .register(ToolDescriptor::new(
                "read_tool",
                "Read.",
                "none",
                "json",
                ToolSideEffectLevel::Read,
                false,
                100,
                0,
                ToolStage::Context,
            ))
            .unwrap();
        registry
            .register(ToolDescriptor::new(
                "write_tool",
                "Write.",
                "none",
                "json",
                ToolSideEffectLevel::Write,
                true,
                100,
                0,
                ToolStage::Execute,
            ))
            .unwrap();
        registry
    }

    #[test]
    fn test_doom_loop_detection() {
        let mut d = DoomLoopDetector::default();
        let args = serde_json::json!({"q": "test"});
        assert!(!d.is_doom_loop("search", &args));
        assert!(!d.is_doom_loop("search", &args));
        assert!(d.is_doom_loop("search", &args));
    }

    #[test]
    fn test_doom_loop_different_args_no_trigger() {
        let mut d = DoomLoopDetector::default();
        d.is_doom_loop("search", &serde_json::json!({"q": "a"}));
        d.is_doom_loop("search", &serde_json::json!({"q": "b"}));
        assert!(!d.is_doom_loop("search", &serde_json::json!({"q": "c"})));
    }

    #[test]
    fn test_doom_loop_reset() {
        let mut d = DoomLoopDetector::default();
        d.is_doom_loop("s", &serde_json::json!({}));
        d.is_doom_loop("s", &serde_json::json!({}));
        d.reset();
        assert!(!d.is_doom_loop("s", &serde_json::json!({})));
    }

    #[tokio::test]
    async fn executor_rejects_unregistered_tool() {
        let mut executor = ToolExecutor::new(registry(), MockHandler);
        let result = executor
            .execute("missing_tool", serde_json::json!({}))
            .await;

        assert!(result
            .error
            .as_deref()
            .is_some_and(|error| error.contains("not registered")));
    }

    #[tokio::test]
    async fn executor_blocks_approval_required_tool_before_handler() {
        let mut executor = ToolExecutor::new(registry(), MockHandler);
        let result = executor.execute("write_tool", serde_json::json!({})).await;

        assert!(result
            .error
            .as_deref()
            .is_some_and(|error| error.contains("requires explicit approval")));
        assert!(result.output.is_null());
    }

    #[tokio::test]
    async fn executor_allows_registered_read_tool() {
        let mut executor = ToolExecutor::new(registry(), MockHandler);
        let result = executor
            .execute("read_tool", serde_json::json!({"id": 1}))
            .await;

        assert!(result.error.is_none());
        assert_eq!(result.output["tool"], "read_tool");
    }
}
