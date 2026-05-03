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
    #[serde(default)]
    pub remediation: Vec<ToolExecutionRemediation>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionRemediation {
    pub code: String,
    pub message: String,
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
                remediation: remediation_for_missing_tool(tool_name),
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
                    remediation: remediation_for_permission_error(
                        &descriptor.name,
                        descriptor.requires_approval,
                        &reason,
                    ),
                    error: Some(reason),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        // Doom loop check
        let is_doom = self.doom_detector.is_doom_loop(tool_name, &args);

        let (output, error, mut remediation) =
            match self.handler.execute(tool_name, args.clone()).await {
                Ok(result) => (result, None, Vec::new()),
                Err(e) => (
                    serde_json::Value::Null,
                    Some(e.clone()),
                    remediation_for_handler_error(tool_name, &e),
                ),
            };

        let mut error_msg = error;
        if is_doom {
            error_msg = Some(format!(
                "DOOM LOOP DETECTED: tool '{}' called with same args 3+ times",
                tool_name
            ));
            remediation = vec![ToolExecutionRemediation {
                code: "tool_doom_loop".to_string(),
                message: "Stop retrying this identical tool call; change the arguments or return a blocked-tool result to the caller.".to_string(),
            }];
        }

        ToolExecution {
            tool_name: tool_name.to_string(),
            input: args,
            output,
            error: error_msg,
            remediation,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}

fn remediation_for_missing_tool(tool_name: &str) -> Vec<ToolExecutionRemediation> {
    vec![ToolExecutionRemediation {
        code: "tool_not_registered".to_string(),
        message: format!(
            "Check the task tool inventory before calling '{}', or register the tool before this run.",
            tool_name
        ),
    }]
}

fn remediation_for_permission_error(
    tool_name: &str,
    requires_approval: bool,
    reason: &str,
) -> Vec<ToolExecutionRemediation> {
    let lower = reason.to_ascii_lowercase();
    if requires_approval || lower.contains("approval") {
        return vec![ToolExecutionRemediation {
            code: "approval_required".to_string(),
            message: format!(
                "Surface an explicit approval request before retrying '{}', or choose a read-only/preview tool.",
                tool_name
            ),
        }];
    }
    if lower.contains("external access") {
        return vec![ToolExecutionRemediation {
            code: "external_access_denied".to_string(),
            message: format!(
                "Keep '{}' inside the workspace boundary, or request an external-access policy change before retrying.",
                tool_name
            ),
        }];
    }
    vec![ToolExecutionRemediation {
        code: "tool_denied".to_string(),
        message: format!(
            "Use the effective tool inventory to pick an allowed alternative to '{}'.",
            tool_name
        ),
    }]
}

fn remediation_for_handler_error(tool_name: &str, error: &str) -> Vec<ToolExecutionRemediation> {
    let lower = error.to_ascii_lowercase();
    let (code, message) = if lower.contains("unknown tool") || lower.contains("unknown agent") {
        (
            "unknown_agent_or_tool",
            format!(
                "Verify the external agent/tool name for '{}', refresh the registry, and retry only if it appears in the allowed inventory.",
                tool_name
            ),
        )
    } else if lower.contains("missing binary")
        || lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("could not find")
    {
        (
            "missing_binary_or_resource",
            format!(
                "Install or configure the binary/resource required by '{}', then run the tool again.",
                tool_name
            ),
        )
    } else if lower.contains("workspace")
        && (lower.contains("unavailable") || lower.contains("missing") || lower.contains("denied"))
    {
        (
            "workspace_unavailable",
            format!(
                "Recreate or select a valid workspace for '{}', then retry with a workspace-local path.",
                tool_name
            ),
        )
    } else {
        (
            "tool_handler_failed",
            format!(
                "Record the failure evidence for '{}' and either retry with narrower arguments or ask the caller for recovery input.",
                tool_name
            ),
        )
    };
    vec![ToolExecutionRemediation {
        code: code.to_string(),
        message,
    }]
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
        assert!(result
            .remediation
            .iter()
            .any(|item| item.code == "tool_not_registered"));
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
        assert!(result
            .remediation
            .iter()
            .any(|item| item.code == "approval_required"));
    }

    #[tokio::test]
    async fn executor_allows_registered_read_tool() {
        let mut executor = ToolExecutor::new(registry(), MockHandler);
        let result = executor
            .execute("read_tool", serde_json::json!({"id": 1}))
            .await;

        assert!(result.error.is_none());
        assert_eq!(result.output["tool"], "read_tool");
        assert!(result.remediation.is_empty());
    }
}
