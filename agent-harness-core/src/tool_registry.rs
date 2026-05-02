use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::permission::{PermissionDecision, PermissionPolicy};
use crate::router::Intent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSideEffectLevel {
    None,
    Read,
    ProviderCall,
    Write,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStage {
    Observe,
    Plan,
    Context,
    Execute,
    Reflect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_type: String,
    pub output_type: String,
    pub side_effect_level: ToolSideEffectLevel,
    pub requires_approval: bool,
    pub timeout_ms: u64,
    pub context_cost_chars: usize,
    pub tags: Vec<String>,
    pub stage: ToolStage,
    pub source: String,
    pub supported_intents: Vec<Intent>,
    pub enabled_by_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

impl ToolDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        description: &str,
        input_type: &str,
        output_type: &str,
        side_effect_level: ToolSideEffectLevel,
        requires_approval: bool,
        timeout_ms: u64,
        context_cost_chars: usize,
        stage: ToolStage,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_type: input_type.to_string(),
            output_type: output_type.to_string(),
            side_effect_level,
            requires_approval,
            timeout_ms,
            context_cost_chars,
            tags: Vec::new(),
            stage,
            source: "core".to_string(),
            supported_intents: Vec::new(),
            enabled_by_default: true,
            input_schema: None,
        }
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|tag| tag.to_string()).collect();
        self
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_string();
        self
    }

    pub fn with_supported_intents(mut self, intents: &[Intent]) -> Self {
        self.supported_intents = intents.to_vec();
        self
    }

    pub fn with_input_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    pub fn disabled_by_default(mut self) -> Self {
        self.enabled_by_default = false;
        self
    }

    pub fn supports_intent(&self, intent: &Intent) -> bool {
        self.supported_intents.is_empty() || self.supported_intents.contains(intent)
    }

    /// Convert to OpenAI function calling schema.
    /// Returns None if input_schema is absent (not all tools are LLM-callable).
    pub fn to_openai_tool(&self) -> Option<serde_json::Value> {
        let schema = self.input_schema.as_ref()?;
        Some(serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": schema
            }
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolFilter {
    pub intent: Option<Intent>,
    pub include_requires_approval: bool,
    pub include_disabled: bool,
    pub max_side_effect_level: Option<ToolSideEffectLevel>,
    pub required_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveToolStatus {
    Allowed,
    Disabled,
    IntentMismatch,
    SideEffectTooHigh,
    MissingTag,
    ApprovalRequired,
    PermissionDenied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveToolEntry {
    pub descriptor: ToolDescriptor,
    pub allowed: bool,
    pub status: EffectiveToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveToolInventory {
    pub allowed: Vec<ToolDescriptor>,
    pub blocked: Vec<EffectiveToolEntry>,
}

impl EffectiveToolInventory {
    pub fn allowed_names(&self) -> Vec<String> {
        self.allowed.iter().map(|tool| tool.name.clone()).collect()
    }

    pub fn blocked_names(&self) -> Vec<String> {
        self.blocked
            .iter()
            .map(|entry| entry.descriptor.name.clone())
            .collect()
    }

    pub fn to_openai_tools(&self) -> Vec<serde_json::Value> {
        self.allowed
            .iter()
            .filter_map(|tool| tool.to_openai_tool())
            .collect()
    }

    pub fn openai_callable_allowed_count(&self) -> usize {
        self.allowed
            .iter()
            .filter(|tool| tool.input_schema.is_some())
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolRegistryError {
    DuplicateTool(String),
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<ToolDescriptor>,
    index: HashMap<String, usize>,
    generation: u64,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn register(&mut self, descriptor: ToolDescriptor) -> Result<(), ToolRegistryError> {
        if self.index.contains_key(&descriptor.name) {
            return Err(ToolRegistryError::DuplicateTool(descriptor.name));
        }
        self.index.insert(descriptor.name.clone(), self.tools.len());
        self.tools.push(descriptor);
        self.generation += 1;
        Ok(())
    }

    pub fn upsert(&mut self, descriptor: ToolDescriptor) {
        if let Some(index) = self.index.get(&descriptor.name).copied() {
            self.tools[index] = descriptor;
        } else {
            self.index.insert(descriptor.name.clone(), self.tools.len());
            self.tools.push(descriptor);
        }
        self.generation += 1;
    }

    pub fn get(&self, name: &str) -> Option<&ToolDescriptor> {
        self.index
            .get(name)
            .and_then(|index| self.tools.get(*index))
    }

    pub fn list(&self) -> Vec<ToolDescriptor> {
        self.tools.clone()
    }

    pub fn filter(&self, filter: &ToolFilter) -> Vec<ToolDescriptor> {
        self.tools
            .iter()
            .filter(|tool| filter.include_disabled || tool.enabled_by_default)
            .filter(|tool| filter.include_requires_approval || !tool.requires_approval)
            .filter(|tool| {
                filter
                    .max_side_effect_level
                    .map(|level| tool.side_effect_level <= level)
                    .unwrap_or(true)
            })
            .filter(|tool| {
                filter
                    .intent
                    .as_ref()
                    .map(|intent| tool.supports_intent(intent))
                    .unwrap_or(true)
            })
            .filter(|tool| {
                filter
                    .required_tags
                    .iter()
                    .all(|tag| tool.tags.iter().any(|candidate| candidate == tag))
            })
            .cloned()
            .collect()
    }

    pub fn effective_inventory(
        &self,
        filter: &ToolFilter,
        policy: &PermissionPolicy,
    ) -> EffectiveToolInventory {
        let mut inventory = EffectiveToolInventory::default();

        for tool in &self.tools {
            if let Some((status, reason)) = filter_block_reason(tool, filter) {
                inventory.blocked.push(EffectiveToolEntry {
                    descriptor: tool.clone(),
                    allowed: false,
                    status,
                    reason: Some(reason),
                });
                continue;
            }

            match policy.authorize(&tool.name, tool.side_effect_level, tool.requires_approval) {
                PermissionDecision::Allow => inventory.allowed.push(tool.clone()),
                PermissionDecision::Ask { reason } => inventory.blocked.push(EffectiveToolEntry {
                    descriptor: tool.clone(),
                    allowed: false,
                    status: EffectiveToolStatus::ApprovalRequired,
                    reason: Some(reason),
                }),
                PermissionDecision::Deny { reason } => inventory.blocked.push(EffectiveToolEntry {
                    descriptor: tool.clone(),
                    allowed: false,
                    status: EffectiveToolStatus::PermissionDenied,
                    reason: Some(reason),
                }),
            }
        }

        inventory
    }

    pub fn to_effective_openai_tools(
        &self,
        filter: &ToolFilter,
        policy: &PermissionPolicy,
    ) -> Vec<serde_json::Value> {
        self.effective_inventory(filter, policy).to_openai_tools()
    }

    /// Export all tools matching the filter as OpenAI function calling schemas.
    pub fn to_openai_tools(&self, filter: &ToolFilter) -> Vec<serde_json::Value> {
        self.filter(filter)
            .iter()
            .filter_map(|d| d.to_openai_tool())
            .collect()
    }
}

fn filter_block_reason(
    tool: &ToolDescriptor,
    filter: &ToolFilter,
) -> Option<(EffectiveToolStatus, String)> {
    if !filter.include_disabled && !tool.enabled_by_default {
        return Some((
            EffectiveToolStatus::Disabled,
            format!("Tool '{}' is disabled by default", tool.name),
        ));
    }

    if !filter.include_requires_approval && tool.requires_approval {
        return Some((
            EffectiveToolStatus::ApprovalRequired,
            format!(
                "Tool '{}' requires approval and the filter excludes approval tools",
                tool.name
            ),
        ));
    }

    if let Some(level) = filter.max_side_effect_level {
        if tool.side_effect_level > level {
            return Some((
                EffectiveToolStatus::SideEffectTooHigh,
                format!(
                    "Tool '{}' side effect {:?} exceeds max {:?}",
                    tool.name, tool.side_effect_level, level
                ),
            ));
        }
    }

    if let Some(intent) = filter.intent.as_ref() {
        if !tool.supports_intent(intent) {
            return Some((
                EffectiveToolStatus::IntentMismatch,
                format!("Tool '{}' does not support intent {:?}", tool.name, intent),
            ));
        }
    }

    for tag in &filter.required_tags {
        if !tool.tags.iter().any(|candidate| candidate == tag) {
            return Some((
                EffectiveToolStatus::MissingTag,
                format!("Tool '{}' is missing required tag '{}'", tool.name, tag),
            ));
        }
    }

    None
}

pub fn default_writing_tool_registry() -> ToolRegistry {
    use Intent::{AnalyzeText, Chat, ExecutePlan, GenerateContent, Linter, RetrieveKnowledge};
    use ToolSideEffectLevel::{None, ProviderCall, Read, Write};
    use ToolStage::{Context, Execute, Observe, Plan, Reflect};

    let mut registry = ToolRegistry::new();
    for tool in [
        ToolDescriptor::new(
            "classify_writing_intent",
            "Classify a user request into the writing agent's lightweight intent taxonomy.",
            "writer_request",
            "intent",
            None,
            false,
            100,
            200,
            Observe,
        )
        .with_tags(&["router", "writing"])
        .with_supported_intents(&[Chat, RetrieveKnowledge, AnalyzeText, GenerateContent, ExecutePlan, Linter]),
        ToolDescriptor::new(
            "load_current_chapter",
            "Load the active chapter text and revision metadata for context-aware writing.",
            "chapter_title",
            "chapter_text",
            Read,
            false,
            500,
            1_800,
            Context,
        )
        .with_tags(&["project", "chapter", "read"])
        .with_supported_intents(&[AnalyzeText, GenerateContent, ExecutePlan, Linter]),
        ToolDescriptor::new(
            "load_outline_node",
            "Load the outline summary and status for a target chapter.",
            "chapter_title",
            "outline_node",
            Read,
            false,
            500,
            800,
            Context,
        )
        .with_tags(&["project", "outline", "read"])
        .with_supported_intents(&[AnalyzeText, GenerateContent, ExecutePlan]),
        ToolDescriptor::new(
            "search_lorebook",
            "Search project lore entries before inventing details about named entities or rules.",
            "keywords",
            "lorebook_entries",
            Read,
            false,
            800,
            1_200,
            Context,
        )
        .with_tags(&["project", "lore", "read"])
        .with_supported_intents(&[RetrieveKnowledge, AnalyzeText, GenerateContent, ExecutePlan, Linter]),
        ToolDescriptor::new(
            "query_project_brain",
            "Run semantic retrieval over the embedded project brain.",
            "semantic_query",
            "rag_snippets",
            ProviderCall,
            false,
            2_500,
            1_500,
            Context,
        )
        .with_tags(&["project", "rag", "provider"])
        .with_supported_intents(&[RetrieveKnowledge, AnalyzeText, GenerateContent, ExecutePlan]),
        ToolDescriptor::new(
            "read_user_drift_profile",
            "Read learned user preferences and durable writing profile entries.",
            "none",
            "preference_entries",
            Read,
            false,
            500,
            800,
            Context,
        )
        .with_tags(&["memory", "preference", "read"])
        .with_supported_intents(&[Chat, AnalyzeText, GenerateContent, ExecutePlan]),
        ToolDescriptor::new(
            "load_domain_profile",
            "Load the active domain capability profile for tool selection, context priority, and quality gates.",
            "domain_id",
            "domain_profile",
            Read,
            false,
            700,
            1_200,
            Context,
        )
        .with_tags(&["domain", "capability", "read"])
        .with_supported_intents(&[Chat, AnalyzeText, GenerateContent, ExecutePlan, Linter]),
        ToolDescriptor::new(
            "pack_agent_context",
            "Assemble named context sources under a strict character budget with truncation reports.",
            "context_sources",
            "packed_context",
            None,
            false,
            100,
            0,
            Context,
        )
        .with_tags(&["context", "budget", "internal"])
        .with_supported_intents(&[Chat, RetrieveKnowledge, AnalyzeText, GenerateContent, ExecutePlan, Linter]),
        ToolDescriptor::new(
            "plan_chapter_task",
            "Break a writing request into observable plan steps before executing edits or generation.",
            "writer_goal",
            "execution_plan",
            None,
            false,
            300,
            600,
            Plan,
        )
        .with_tags(&["planner", "writing"])
        .with_supported_intents(&[AnalyzeText, GenerateContent, ExecutePlan]),
        ToolDescriptor::new(
            "generate_bounded_continuation",
            "Generate a short context-bounded continuation preview for proactive co-writing.",
            "agent_observation_context",
            "suggestion_preview",
            ProviderCall,
            false,
            6_000,
            2_400,
            Execute,
        )
        .with_tags(&["generation", "preview", "provider"])
        .with_supported_intents(&[GenerateContent]),
        ToolDescriptor::new(
            "generate_chapter_draft",
            "Generate and save a full chapter draft using project context and conflict checks.",
            "chapter_generation_payload",
            "saved_chapter",
            Write,
            true,
            120_000,
            12_000,
            Execute,
        )
        .with_tags(&["generation", "chapter", "write"])
        .with_supported_intents(&[GenerateContent, ExecutePlan]),
        ToolDescriptor::new(
            "record_run_trace",
            "Record agent run events for UI status, debugging, and future trajectory compression.",
            "run_event",
            "run_trace",
            None,
            false,
            50,
            0,
            Reflect,
        )
        .with_tags(&["trace", "observability", "internal"])
        .with_supported_intents(&[Chat, RetrieveKnowledge, AnalyzeText, GenerateContent, ExecutePlan, Linter]),
    ] {
        registry
            .register(tool)
            .expect("default writing tools are uniquely named");
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::{PermissionMode, PermissionPolicy};

    #[test]
    fn registry_filters_by_intent_and_approval() {
        let registry = default_writing_tool_registry();
        let tools = registry.filter(&ToolFilter {
            intent: Some(Intent::GenerateContent),
            include_requires_approval: false,
            include_disabled: false,
            max_side_effect_level: Some(ToolSideEffectLevel::ProviderCall),
            required_tags: Vec::new(),
        });

        assert!(tools
            .iter()
            .any(|tool| tool.name == "generate_bounded_continuation"));
        assert!(!tools
            .iter()
            .any(|tool| tool.name == "generate_chapter_draft"));
    }

    #[test]
    fn registry_generation_changes_on_upsert() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.generation(), 0);

        registry
            .register(ToolDescriptor::new(
                "read",
                "Read something.",
                "none",
                "text",
                ToolSideEffectLevel::Read,
                false,
                100,
                10,
                ToolStage::Context,
            ))
            .unwrap();
        assert_eq!(registry.generation(), 1);

        registry.upsert(ToolDescriptor::new(
            "read",
            "Read something else.",
            "none",
            "text",
            ToolSideEffectLevel::Read,
            false,
            100,
            10,
            ToolStage::Context,
        ));
        assert_eq!(registry.generation(), 2);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn effective_inventory_blocks_approval_required_write_tool() {
        let registry = default_writing_tool_registry();
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        let inventory = registry.effective_inventory(
            &ToolFilter {
                intent: Some(Intent::GenerateContent),
                include_requires_approval: true,
                include_disabled: false,
                max_side_effect_level: Some(ToolSideEffectLevel::Write),
                required_tags: Vec::new(),
            },
            &policy,
        );

        assert!(inventory
            .allowed
            .iter()
            .any(|tool| tool.name == "generate_bounded_continuation"));
        assert!(!inventory
            .allowed
            .iter()
            .any(|tool| tool.name == "generate_chapter_draft"));

        let blocked = inventory
            .blocked
            .iter()
            .find(|entry| entry.descriptor.name == "generate_chapter_draft")
            .expect("chapter draft tool should be present in blocked inventory");
        assert_eq!(blocked.status, EffectiveToolStatus::ApprovalRequired);
        assert!(blocked
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("requires explicit approval")));
    }

    #[test]
    fn effective_inventory_reports_filter_reasons() {
        let registry = default_writing_tool_registry();
        let policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        let inventory = registry.effective_inventory(
            &ToolFilter {
                intent: Some(Intent::Linter),
                include_requires_approval: true,
                include_disabled: false,
                max_side_effect_level: Some(ToolSideEffectLevel::Read),
                required_tags: Vec::new(),
            },
            &policy,
        );

        let blocked = inventory
            .blocked
            .iter()
            .find(|entry| entry.descriptor.name == "query_project_brain")
            .expect("provider-call tool should be blocked by side-effect ceiling");
        assert_eq!(blocked.status, EffectiveToolStatus::SideEffectTooHigh);
    }
}
