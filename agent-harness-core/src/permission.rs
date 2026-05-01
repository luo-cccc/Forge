use crate::tool_registry::ToolSideEffectLevel;

/// Permission mode for the agent.
/// Ported from Claw Code `permissions.rs`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PermissionDecision {
    Allow,
    Deny { reason: String },
    Ask { reason: String },
}

#[derive(Debug, Clone)]
pub struct PermissionRule {
    pub pattern: String,
    pub action: PermissionDecision,
}

/// Permission policy — evaluates rules against tool invocations.
/// Pipeline: deny rules → mode escalation → approval → allow rules.
/// Ported from Claw Code `PermissionPolicy::authorize_with_context()`.
pub struct PermissionPolicy {
    pub mode: PermissionMode,
    pub rules: Vec<PermissionRule>,
    pub deny_rules: Vec<PermissionRule>,
}

impl PermissionPolicy {
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            mode,
            rules: Vec::new(),
            deny_rules: Vec::new(),
        }
    }

    pub fn authorize(
        &self,
        tool_name: &str,
        side_effect: ToolSideEffectLevel,
        requires_approval: bool,
    ) -> PermissionDecision {
        // 1. Check deny rules (always reject)
        for rule in &self.deny_rules {
            if tool_matches(tool_name, &rule.pattern) {
                if let PermissionDecision::Deny { ref reason } = rule.action {
                    return PermissionDecision::Deny {
                        reason: reason.clone(),
                    };
                }
            }
        }

        // 2. Mode-based escalation
        match self.mode {
            PermissionMode::ReadOnly => {
                if side_effect >= ToolSideEffectLevel::Write {
                    return PermissionDecision::Deny {
                        reason: format!(
                            "Tool '{}' requires write access but agent is in ReadOnly mode",
                            tool_name
                        ),
                    };
                }
                if side_effect >= ToolSideEffectLevel::ProviderCall {
                    return PermissionDecision::Ask {
                        reason: "External API call in ReadOnly mode".into(),
                    };
                }
            }
            PermissionMode::WorkspaceWrite => {
                if side_effect >= ToolSideEffectLevel::External {
                    return PermissionDecision::Deny {
                        reason: format!(
                            "Tool '{}' requires external access beyond workspace",
                            tool_name
                        ),
                    };
                }
            }
            PermissionMode::DangerFullAccess => {}
        }

        // 3. Explicit approval requirement
        if requires_approval {
            return PermissionDecision::Ask {
                reason: format!("Tool '{}' requires explicit approval", tool_name),
            };
        }

        // 4. Check allow rules
        for rule in &self.rules {
            if tool_matches(tool_name, &rule.pattern) {
                return rule.action.clone();
            }
        }

        PermissionDecision::Allow
    }
}

fn tool_matches(tool_name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return tool_name.starts_with(prefix);
    }
    tool_name == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readonly_denies_write() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly);
        let d = policy.authorize("generate_chapter_draft", ToolSideEffectLevel::Write, false);
        assert!(matches!(d, PermissionDecision::Deny { .. }));
    }

    #[test]
    fn test_readonly_allows_read() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly);
        let d = policy.authorize("load_current_chapter", ToolSideEffectLevel::Read, false);
        assert!(matches!(d, PermissionDecision::Allow));
    }

    #[test]
    fn test_approval_triggers_ask() {
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        let d = policy.authorize("generate_chapter_draft", ToolSideEffectLevel::Write, true);
        assert!(matches!(d, PermissionDecision::Ask { .. }));
    }

    #[test]
    fn test_deny_rule_overrides() {
        let mut policy = PermissionPolicy::new(PermissionMode::DangerFullAccess);
        policy.deny_rules.push(PermissionRule {
            pattern: "generate_*".into(),
            action: PermissionDecision::Deny {
                reason: "blocked".into(),
            },
        });
        let d = policy.authorize("generate_chapter_draft", ToolSideEffectLevel::Write, false);
        assert!(matches!(d, PermissionDecision::Deny { .. }));
    }

    #[test]
    fn test_tool_matches() {
        assert!(tool_matches("search_lorebook", "search_*"));
        assert!(tool_matches("load", "*"));
        assert!(!tool_matches("search", "load_*"));
    }
}
