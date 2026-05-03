//! Isolated read-only research / diagnostic subtasks for Writer Agent.
//!
//! Subtasks may create evidence artifacts in their own workspace, but they do
//! not emit direct WriterOperations. Their output is evidence that the main
//! Writer Agent loop can later surface in proposals.

use agent_harness_core::{Intent, ToolFilter, ToolPolicyContract, ToolSideEffectLevel};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use super::operation::WriterOperation;
use super::proposal::EvidenceRef;
use super::task_receipt::{failure_bundle_from_tool_execution, WriterFailureEvidenceBundle};

const SUBTASK_ROOT_DIR: &str = "agent_subtasks";
const SUBTASK_ARTIFACT_DIR: &str = "artifacts";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriterSubtaskKind {
    Research,
    Diagnostic,
    Drafting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WriterSubtaskWorkspace {
    pub subtask_id: String,
    pub kind: WriterSubtaskKind,
    pub workspace_dir: String,
    pub artifact_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterSubtaskResult {
    pub subtask_id: String,
    pub kind: WriterSubtaskKind,
    pub objective: String,
    pub summary: String,
    pub evidence_refs: Vec<EvidenceRef>,
    pub artifact_refs: Vec<String>,
    pub blocked_operation_kinds: Vec<String>,
    pub created_at_ms: u64,
}

pub fn create_subtask_workspace(
    project_data_dir: &Path,
    kind: WriterSubtaskKind,
    subtask_id: &str,
) -> Result<WriterSubtaskWorkspace, String> {
    let workspace_dir = subtask_workspace_dir(project_data_dir, subtask_id)?;
    let artifact_dir = workspace_dir.join(SUBTASK_ARTIFACT_DIR);
    std::fs::create_dir_all(&artifact_dir).map_err(|e| {
        format!(
            "Failed to create Writer Agent subtask artifact dir '{}': {}",
            artifact_dir.display(),
            e
        )
    })?;
    Ok(WriterSubtaskWorkspace {
        subtask_id: normalized_subtask_id(subtask_id)?,
        kind,
        workspace_dir: workspace_dir.to_string_lossy().to_string(),
        artifact_dir: artifact_dir.to_string_lossy().to_string(),
    })
}

pub fn subtask_workspace_dir(project_data_dir: &Path, subtask_id: &str) -> Result<PathBuf, String> {
    Ok(project_data_dir
        .join(SUBTASK_ROOT_DIR)
        .join(normalized_subtask_id(subtask_id)?))
}

pub fn safe_subtask_artifact_path(
    project_data_dir: &Path,
    subtask_id: &str,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let artifact_dir =
        subtask_workspace_dir(project_data_dir, subtask_id)?.join(SUBTASK_ARTIFACT_DIR);
    let relative = safe_relative_path(relative_path, "subtask artifact")?;
    let joined = artifact_dir.join(relative);
    let root = artifact_dir
        .canonicalize()
        .unwrap_or_else(|_| artifact_dir.clone());
    let parent = joined.parent().unwrap_or(&artifact_dir);
    let stays_in_workspace = if parent.exists() {
        parent
            .canonicalize()
            .map(|canonical| canonical.starts_with(&root))
            .unwrap_or(false)
    } else {
        joined.starts_with(&artifact_dir)
    };
    if !stays_in_workspace {
        return Err(format!(
            "Subtask artifact path escapes isolated workspace: {}",
            relative_path
        ));
    }
    Ok(joined)
}

pub fn write_subtask_artifact(
    project_data_dir: &Path,
    subtask_id: &str,
    relative_path: &str,
    content: &str,
) -> Result<String, String> {
    let path = safe_subtask_artifact_path(project_data_dir, subtask_id, relative_path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create subtask artifact parent '{}': {}",
                parent.display(),
                e
            )
        })?;
    }
    crate::storage::atomic_write(&path, content)?;
    Ok(format!(
        "subtask:{}:artifact:{}",
        normalized_subtask_id(subtask_id)?,
        relative_path.replace('\\', "/")
    ))
}

pub fn tool_policy_for_subtask(kind: WriterSubtaskKind) -> ToolPolicyContract {
    match kind {
        WriterSubtaskKind::Research => ToolPolicyContract {
            max_side_effect_level: ToolSideEffectLevel::ProviderCall,
            allow_approval_required: false,
            required_tool_tags: vec!["project".to_string()],
        },
        WriterSubtaskKind::Diagnostic => ToolPolicyContract {
            max_side_effect_level: ToolSideEffectLevel::Read,
            allow_approval_required: false,
            required_tool_tags: vec!["project".to_string()],
        },
        WriterSubtaskKind::Drafting => ToolPolicyContract {
            max_side_effect_level: ToolSideEffectLevel::ProviderCall,
            allow_approval_required: false,
            required_tool_tags: vec!["generation".to_string(), "preview".to_string()],
        },
    }
}

pub fn tool_filter_for_subtask(kind: WriterSubtaskKind) -> ToolFilter {
    let policy = tool_policy_for_subtask(kind);
    ToolFilter {
        intent: match kind {
            WriterSubtaskKind::Research => None,
            WriterSubtaskKind::Diagnostic => Some(Intent::AnalyzeText),
            WriterSubtaskKind::Drafting => Some(Intent::GenerateContent),
        },
        include_requires_approval: policy.allow_approval_required,
        include_disabled: false,
        max_side_effect_level: Some(policy.max_side_effect_level),
        required_tags: policy.required_tool_tags,
    }
}

pub fn build_evidence_only_subtask_result(
    kind: WriterSubtaskKind,
    subtask_id: &str,
    objective: &str,
    summary: &str,
    evidence_refs: Vec<EvidenceRef>,
    artifact_refs: Vec<String>,
    attempted_operations: &[WriterOperation],
    created_at_ms: u64,
) -> Result<WriterSubtaskResult, String> {
    Ok(WriterSubtaskResult {
        subtask_id: normalized_subtask_id(subtask_id)?,
        kind,
        objective: objective.trim().to_string(),
        summary: summary.trim().to_string(),
        evidence_refs,
        artifact_refs: normalize_strings(artifact_refs),
        blocked_operation_kinds: denied_subtask_operations(kind, attempted_operations),
        created_at_ms,
    })
}

pub fn validate_evidence_only_subtask_result(result: &WriterSubtaskResult) -> Vec<String> {
    let mut errors = Vec::new();
    if normalized_subtask_id(&result.subtask_id).is_err() {
        errors.push("subtask id is invalid".to_string());
    }
    if result.objective.trim().is_empty() {
        errors.push("subtask objective is empty".to_string());
    }
    if result.summary.trim().is_empty() {
        errors.push("subtask summary is empty".to_string());
    }
    if result.evidence_refs.is_empty() && result.artifact_refs.is_empty() {
        errors.push("subtask result has no evidence refs or artifact refs".to_string());
    }
    for evidence in &result.evidence_refs {
        if evidence.reference.trim().is_empty() || evidence.snippet.trim().is_empty() {
            errors.push("subtask evidence ref is missing reference or snippet".to_string());
        }
    }
    for artifact in &result.artifact_refs {
        let expected_prefix = format!("subtask:{}:artifact:", result.subtask_id);
        if !artifact.starts_with(&expected_prefix) {
            errors.push(format!(
                "subtask artifact ref is outside the isolated workspace: {}",
                artifact
            ));
        }
    }
    errors
}

pub fn failure_bundle_from_subtask_tool_execution(
    kind: WriterSubtaskKind,
    subtask_id: &str,
    objective: &str,
    execution: &agent_harness_core::ToolExecution,
    artifact_refs: Vec<String>,
    created_at_ms: u64,
) -> Result<Option<WriterFailureEvidenceBundle>, String> {
    let subtask_id = normalized_subtask_id(subtask_id)?;
    let objective = objective.trim();
    if objective.is_empty() {
        return Err("Writer Agent subtask objective is empty".to_string());
    }
    let artifact_refs = normalize_strings(artifact_refs);
    for artifact in &artifact_refs {
        let expected_prefix = format!("subtask:{}:artifact:", subtask_id);
        if !artifact.starts_with(&expected_prefix) {
            return Err(format!(
                "subtask artifact ref is outside the isolated workspace: {}",
                artifact
            ));
        }
    }

    let Some(mut bundle) =
        failure_bundle_from_tool_execution(Some(&subtask_id), execution, created_at_ms)
    else {
        return Ok(None);
    };
    let kind_label = subtask_kind_label(kind);
    let tool_details = bundle.details.clone();
    let error = execution.error.as_deref().unwrap_or("unknown tool error");
    bundle.task_id = Some(subtask_id.clone());
    bundle.message = format!(
        "{} subtask '{}' failed while running tool '{}': {}",
        kind_label, subtask_id, execution.tool_name, error
    );
    bundle.evidence_refs = normalize_strings(
        bundle
            .evidence_refs
            .into_iter()
            .chain([
                format!("subtask:{}", subtask_id),
                format!("subtask:{}:kind:{}", subtask_id, kind_label),
            ])
            .chain(artifact_refs.iter().cloned())
            .collect(),
    );
    bundle.details = serde_json::json!({
        "subtaskId": subtask_id,
        "kind": kind_label,
        "objective": objective,
        "artifactRefs": artifact_refs,
        "toolExecution": tool_details,
    });
    bundle.remediation = normalize_strings(
        bundle
            .remediation
            .into_iter()
            .chain([format!(
                "subtask_{}_failure: Keep this subtask evidence-only; inspect isolated artifacts and retry only after the tool/provider configuration or query scope changes.",
                kind_label
            )])
            .collect(),
    );
    Ok(Some(bundle))
}

pub fn denied_subtask_operations(
    _kind: WriterSubtaskKind,
    attempted_operations: &[WriterOperation],
) -> Vec<String> {
    normalize_strings(
        attempted_operations
            .iter()
            .map(|operation| subtask_operation_kind_label(operation).to_string())
            .collect(),
    )
}

pub fn subtask_operation_kind_label(operation: &WriterOperation) -> &'static str {
    match operation {
        WriterOperation::TextInsert { .. } => "text.insert",
        WriterOperation::TextReplace { .. } => "text.replace",
        WriterOperation::TextAnnotate { .. } => "text.annotate",
        WriterOperation::CanonUpsertEntity { .. } => "canon.upsert_entity",
        WriterOperation::CanonUpdateAttribute { .. } => "canon.update_attribute",
        WriterOperation::CanonUpsertRule { .. } => "canon.upsert_rule",
        WriterOperation::PromiseAdd { .. } => "promise.add",
        WriterOperation::PromiseResolve { .. } => "promise.resolve",
        WriterOperation::PromiseDefer { .. } => "promise.defer",
        WriterOperation::PromiseAbandon { .. } => "promise.abandon",
        WriterOperation::StyleUpdatePreference { .. } => "style.update_preference",
        WriterOperation::StoryContractUpsert { .. } => "story_contract.upsert",
        WriterOperation::ChapterMissionUpsert { .. } => "chapter_mission.upsert",
        WriterOperation::OutlineUpdate { .. } => "outline.update",
    }
}

fn subtask_kind_label(kind: WriterSubtaskKind) -> &'static str {
    match kind {
        WriterSubtaskKind::Research => "research",
        WriterSubtaskKind::Diagnostic => "diagnostic",
        WriterSubtaskKind::Drafting => "drafting",
    }
}

fn normalized_subtask_id(subtask_id: &str) -> Result<String, String> {
    let id = subtask_id.trim();
    if id.is_empty() || id.len() > 96 {
        return Err("Writer Agent subtask id must be 1-96 chars".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("Invalid Writer Agent subtask id: {}", subtask_id));
    }
    Ok(id.to_string())
}

fn safe_relative_path(relative_path: &str, label: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative_path);
    if relative_path.trim().is_empty() || path.is_absolute() {
        return Err(format!(
            "{} path must be relative: {}",
            label, relative_path
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return Err(format!(
            "{} path must stay inside the isolated workspace: {}",
            label, relative_path
        ));
    }
    Ok(path.to_path_buf())
}

fn normalize_strings(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || normalized.iter().any(|existing| existing == value) {
            continue;
        }
        normalized.push(value.to_string());
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer_agent::proposal::{EvidenceRef, EvidenceSource};

    #[test]
    fn subtask_artifact_path_rejects_escape() {
        let root = std::env::temp_dir().join("forge-subtask-path-test");
        assert!(safe_subtask_artifact_path(&root, "research_1", "notes/evidence.json").is_ok());
        assert!(safe_subtask_artifact_path(&root, "../bad", "notes/evidence.json").is_err());
        assert!(safe_subtask_artifact_path(&root, "research_1", "../secret.md").is_err());
        assert!(safe_subtask_artifact_path(&root, "research_1", "notes/../../secret.md").is_err());
    }

    #[test]
    fn evidence_result_blocks_operations() {
        let result = build_evidence_only_subtask_result(
            WriterSubtaskKind::Diagnostic,
            "diag-1",
            "Check mission drift.",
            "No drift found.",
            vec![EvidenceRef {
                source: EvidenceSource::ChapterText,
                reference: "Chapter-1".to_string(),
                snippet: "林墨停在旧门前。".to_string(),
            }],
            vec!["subtask:diag-1:artifact:evidence.json".to_string()],
            &[WriterOperation::TextReplace {
                chapter: "Chapter-1".to_string(),
                from: 0,
                to: 2,
                text: "改写".to_string(),
                revision: "rev-1".to_string(),
            }],
            1,
        )
        .unwrap();

        assert!(validate_evidence_only_subtask_result(&result).is_empty());
        assert_eq!(result.blocked_operation_kinds, vec!["text.replace"]);
    }

    #[test]
    fn subtask_tool_failure_preserves_subtask_evidence() {
        let execution = agent_harness_core::ToolExecution {
            tool_name: "query_project_brain".to_string(),
            input: serde_json::json!({ "query": "ring crack" }),
            output: serde_json::Value::Null,
            error: Some("missing binary for external research adapter".to_string()),
            remediation: vec![agent_harness_core::ToolExecutionRemediation {
                code: "missing_binary_or_resource".to_string(),
                message: "Install the research adapter before retrying.".to_string(),
            }],
            duration_ms: 18,
        };
        let bundle = failure_bundle_from_subtask_tool_execution(
            WriterSubtaskKind::Research,
            "research-3",
            "Find public evidence for the ring crack.",
            &execution,
            vec!["subtask:research-3:artifact:evidence/search.json".to_string()],
            30,
        )
        .unwrap()
        .unwrap();

        assert_eq!(bundle.task_id.as_deref(), Some("research-3"));
        assert!(bundle.message.contains("research subtask"));
        assert!(bundle
            .evidence_refs
            .iter()
            .any(|reference| reference == "subtask:research-3"));
        assert_eq!(bundle.details["kind"], "research");
        assert!(bundle.details["toolExecution"]["remediation"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
        assert!(bundle
            .remediation
            .iter()
            .any(|item| item.contains("subtask_research_failure")));
    }
}
