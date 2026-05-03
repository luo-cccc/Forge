//! Verifiable task receipts and failure evidence bundles for long Writer Agent work.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WriterTaskReceipt {
    pub task_id: String,
    pub task_kind: String,
    pub chapter: Option<String>,
    pub objective: String,
    pub required_evidence: Vec<String>,
    pub expected_artifacts: Vec<String>,
    pub must_not: Vec<String>,
    pub source_refs: Vec<String>,
    pub base_revision: Option<String>,
    pub created_at_ms: u64,
}

impl WriterTaskReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: impl Into<String>,
        task_kind: impl Into<String>,
        chapter: Option<String>,
        objective: impl Into<String>,
        required_evidence: Vec<String>,
        expected_artifacts: Vec<String>,
        must_not: Vec<String>,
        source_refs: Vec<String>,
        base_revision: Option<String>,
        created_at_ms: u64,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            task_kind: task_kind.into(),
            chapter,
            objective: objective.into(),
            required_evidence: normalize_strings(required_evidence),
            expected_artifacts: normalize_strings(expected_artifacts),
            must_not: normalize_strings(must_not),
            source_refs: normalize_strings(source_refs),
            base_revision,
            created_at_ms,
        }
    }

    pub fn source_has_evidence(&self, evidence: &str) -> bool {
        let evidence = evidence.trim();
        !evidence.is_empty()
            && self.source_refs.iter().any(|source| {
                source == evidence
                    || source
                        .split_once(':')
                        .map(|(source_type, _)| source_type == evidence)
                        .unwrap_or(false)
            })
    }

    pub fn validate_write_attempt(
        &self,
        task_id: &str,
        chapter: &str,
        base_revision: &str,
        artifact: &str,
    ) -> Vec<WriterTaskReceiptMismatch> {
        let mut mismatches = Vec::new();
        if self.task_id != task_id {
            mismatches.push(WriterTaskReceiptMismatch::new(
                "task_id",
                self.task_id.clone(),
                task_id.to_string(),
                self.task_id.clone(),
            ));
        }
        if self.task_kind != "ChapterGeneration" {
            mismatches.push(WriterTaskReceiptMismatch::new(
                "task_kind",
                "ChapterGeneration",
                self.task_kind.clone(),
                self.task_id.clone(),
            ));
        }
        if self.chapter.as_deref() != Some(chapter) {
            mismatches.push(WriterTaskReceiptMismatch::new(
                "chapter",
                self.chapter.clone().unwrap_or_default(),
                chapter.to_string(),
                self.task_id.clone(),
            ));
        }
        if self.base_revision.as_deref() != Some(base_revision) {
            mismatches.push(WriterTaskReceiptMismatch::new(
                "base_revision",
                self.base_revision.clone().unwrap_or_default(),
                base_revision.to_string(),
                self.task_id.clone(),
            ));
        }
        if !self
            .expected_artifacts
            .iter()
            .any(|expected| expected == artifact)
        {
            mismatches.push(WriterTaskReceiptMismatch::new(
                "expected_artifacts",
                artifact.to_string(),
                self.expected_artifacts.join(","),
                self.task_id.clone(),
            ));
        }
        for evidence in &self.required_evidence {
            if !self.source_has_evidence(evidence) {
                mismatches.push(WriterTaskReceiptMismatch::new(
                    "required_evidence",
                    evidence.clone(),
                    "missing".to_string(),
                    self.task_id.clone(),
                ));
            }
        }
        mismatches
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WriterTaskReceiptMismatch {
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub evidence_ref: String,
}

impl WriterTaskReceiptMismatch {
    pub fn new(
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
            evidence_ref: evidence_ref.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriterFailureCategory {
    ContextMissing,
    ToolDenied,
    ProviderFailed,
    ReceiptMismatch,
    SaveFailed,
    FeedbackBlocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WriterFailureEvidenceBundle {
    pub category: WriterFailureCategory,
    pub code: String,
    pub message: String,
    pub recoverable: bool,
    pub task_id: Option<String>,
    pub evidence_refs: Vec<String>,
    pub details: serde_json::Value,
    pub remediation: Vec<String>,
    pub created_at_ms: u64,
}

impl WriterFailureEvidenceBundle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        category: WriterFailureCategory,
        code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
        task_id: Option<String>,
        evidence_refs: Vec<String>,
        details: serde_json::Value,
        remediation: Vec<String>,
        created_at_ms: u64,
    ) -> Self {
        Self {
            category,
            code: code.into(),
            message: message.into(),
            recoverable,
            task_id,
            evidence_refs: normalize_strings(evidence_refs),
            details,
            remediation: normalize_strings(remediation),
            created_at_ms,
        }
    }
}

pub fn normalize_strings(values: Vec<String>) -> Vec<String> {
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

    #[test]
    fn receipt_validates_chapter_write_attempt() {
        let receipt = WriterTaskReceipt::new(
            "req-1",
            "ChapterGeneration",
            Some("Chapter-1".to_string()),
            "Draft Chapter-1.",
            vec!["instruction".to_string(), "target_beat".to_string()],
            vec!["saved_chapter".to_string()],
            vec!["overwrite_without_revision_match".to_string()],
            vec![
                "instruction:user".to_string(),
                "target_beat:Chapter-1".to_string(),
            ],
            Some("rev-1".to_string()),
            10,
        );

        assert!(receipt
            .validate_write_attempt("req-1", "Chapter-1", "rev-1", "saved_chapter")
            .is_empty());
        assert_eq!(
            receipt
                .validate_write_attempt("req-2", "Chapter-1", "rev-1", "saved_chapter")
                .first()
                .map(|mismatch| mismatch.field.as_str()),
            Some("task_id")
        );
    }
}
