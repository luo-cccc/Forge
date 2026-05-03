//! Memory candidate feedback and proposal slot helpers for WriterAgentKernel.

use std::collections::HashSet;

use super::feedback::ProposalFeedback;
use super::memory::{MemoryAuditSummary, WriterMemory};
use super::operation::{CanonEntityOp, PlotPromiseOp, WriterOperation};
use super::proposal::{AgentProposal, ProposalKind};

pub(crate) enum MemoryCandidate {
    Canon(CanonEntityOp),
    Promise(PlotPromiseOp),
}

pub(crate) fn proposal_slot_key(proposal: &AgentProposal) -> String {
    let target = proposal
        .target
        .as_ref()
        .map(|target| format!("{}:{}", target.from, target.to))
        .unwrap_or_else(|| "none".to_string());

    if proposal.kind == ProposalKind::Ghost {
        return format!("{}|{:?}|{}", proposal.observation_id, proposal.kind, target);
    }

    if let Some(memory_slot) = memory_operation_slot(proposal) {
        return memory_slot;
    }

    let evidence_key = proposal
        .evidence
        .first()
        .map(|evidence| format!("{:?}:{}", evidence.source, evidence.reference))
        .unwrap_or_default();
    let preview_key: String = proposal
        .preview
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect();

    format!(
        "{:?}|{}|{}|{}",
        proposal.kind, target, evidence_key, preview_key
    )
}

pub(crate) fn suppression_slot_key(proposal: &AgentProposal) -> String {
    let target = proposal
        .target
        .as_ref()
        .map(|target| format!("{}:{}", target.from, target.to))
        .unwrap_or_else(|| "none".to_string());
    let evidence_key = proposal
        .evidence
        .first()
        .map(|evidence| format!("{:?}:{}", evidence.source, evidence.reference))
        .unwrap_or_default();

    if proposal.kind == ProposalKind::Ghost {
        return ghost_suppression_slot_key(proposal);
    }

    if let Some(memory_slot) = memory_operation_slot(proposal) {
        return memory_slot;
    }

    format!(
        "{:?}|{}|{}|{}",
        proposal.kind,
        target,
        evidence_key,
        preview_fingerprint(&proposal.preview)
    )
}

fn ghost_suppression_slot_key(proposal: &AgentProposal) -> String {
    let chapter = proposal
        .operations
        .first()
        .and_then(|operation| match operation {
            WriterOperation::TextInsert { chapter, .. }
            | WriterOperation::TextReplace { chapter, .. }
            | WriterOperation::TextAnnotate { chapter, .. } => Some(chapter.as_str()),
            _ => None,
        })
        .unwrap_or("project");
    format!(
        "{:?}|{}|{}",
        proposal.kind,
        chapter,
        preview_fingerprint(&proposal.preview)
    )
}

fn memory_operation_slot(proposal: &AgentProposal) -> Option<String> {
    match proposal.operations.first()? {
        WriterOperation::CanonUpsertEntity { entity } => {
            Some(memory_candidate_slot_for_canon(entity))
        }
        WriterOperation::PromiseAdd { promise } => Some(memory_candidate_slot_for_promise(promise)),
        _ => None,
    }
}

fn memory_audit_title(proposal: &AgentProposal) -> String {
    match proposal.operations.first() {
        Some(WriterOperation::CanonUpsertEntity { entity }) => {
            format!("{} [{}]", entity.name, entity.kind)
        }
        Some(WriterOperation::PromiseAdd { promise }) => {
            format!("{} [{}]", promise.title, promise.kind)
        }
        _ => proposal.preview.clone(),
    }
}

pub(crate) fn record_memory_audit_event(
    memory: &WriterMemory,
    proposal: &AgentProposal,
    feedback: &ProposalFeedback,
) {
    if memory_operation_slot(proposal).is_none() {
        return;
    }
    let entry = MemoryAuditSummary {
        proposal_id: proposal.id.clone(),
        kind: format!("{:?}", proposal.kind),
        action: format!("{:?}", feedback.action),
        title: memory_audit_title(proposal),
        evidence: proposal
            .evidence
            .first()
            .map(|evidence| evidence.snippet.clone())
            .unwrap_or_default(),
        rationale: proposal.rationale.clone(),
        reason: feedback.reason.clone(),
        created_at: feedback.created_at,
    };
    memory.record_memory_audit(&entry).ok();
}

pub(crate) fn memory_candidate_slot_for_canon(entity: &CanonEntityOp) -> String {
    format!("memory|canon|{}|{}", entity.kind, entity.name)
}

pub(crate) fn memory_candidate_slot_for_promise(promise: &PlotPromiseOp) -> String {
    format!("memory|promise|{}|{}", promise.kind, promise.title)
}

fn memory_feedback_key(slot: &str) -> String {
    format!("memory_extract:{}", slot)
}

pub(crate) fn record_memory_candidate_feedback(
    memory: &WriterMemory,
    proposal: &AgentProposal,
    accepted: bool,
) {
    let Some(slot) = memory_operation_slot(proposal) else {
        return;
    };
    let value = if accepted { "accepted" } else { "rejected" };
    let _ = memory.upsert_style_preference(&memory_feedback_key(&slot), value, accepted);
}

pub(crate) struct MemoryExtractionFeedback {
    suppressed_slots: HashSet<String>,
    preferred_slots: HashSet<String>,
}

impl MemoryExtractionFeedback {
    pub(crate) fn from_memory(memory: &WriterMemory) -> Self {
        let mut suppressed_slots = HashSet::new();
        let mut preferred_slots = HashSet::new();
        for preference in memory.list_style_preferences(200).unwrap_or_default() {
            let Some(slot) = preference.key.strip_prefix("memory_extract:") else {
                continue;
            };
            if preference.rejected_count >= 1
                && preference.rejected_count >= preference.accepted_count
            {
                suppressed_slots.insert(slot.to_string());
            } else if preference.accepted_count > preference.rejected_count {
                preferred_slots.insert(slot.to_string());
            }
        }
        Self {
            suppressed_slots,
            preferred_slots,
        }
    }

    pub(crate) fn is_suppressed(&self, slot: &str) -> bool {
        self.suppressed_slots.contains(slot)
    }

    pub(crate) fn is_preferred(&self, slot: &str) -> bool {
        self.preferred_slots.contains(slot)
    }

    pub(crate) fn apply_to_candidate(&self, candidate: MemoryCandidate) -> Option<MemoryCandidate> {
        match candidate {
            MemoryCandidate::Canon(mut entity) => {
                let slot = memory_candidate_slot_for_canon(&entity);
                if self.is_suppressed(&slot) {
                    return None;
                }
                if self.is_preferred(&slot) {
                    entity.confidence = (entity.confidence + 0.08).min(0.95);
                }
                Some(MemoryCandidate::Canon(entity))
            }
            MemoryCandidate::Promise(mut promise) => {
                let slot = memory_candidate_slot_for_promise(&promise);
                if self.is_suppressed(&slot) {
                    return None;
                }
                if self.is_preferred(&slot) {
                    promise.priority = (promise.priority + 1).min(10);
                }
                Some(MemoryCandidate::Promise(promise))
            }
        }
    }
}

fn preview_fingerprint(preview: &str) -> String {
    preview
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::feedback::FeedbackAction;
    use super::super::proposal::{EvidenceRef, EvidenceSource, ProposalPriority};
    use super::*;

    fn canon_entity(name: &str) -> CanonEntityOp {
        CanonEntityOp {
            kind: "character".to_string(),
            name: name.to_string(),
            aliases: vec![],
            summary: "summary".to_string(),
            attributes: serde_json::json!({}),
            confidence: 0.62,
        }
    }

    fn promise(title: &str) -> PlotPromiseOp {
        PlotPromiseOp {
            kind: "mystery_clue".to_string(),
            title: title.to_string(),
            description: "description".to_string(),
            introduced_chapter: "Chapter-1".to_string(),
            expected_payoff: "later".to_string(),
            priority: 3,
            related_entities: vec![],
        }
    }

    fn proposal(
        kind: ProposalKind,
        preview: &str,
        operations: Vec<WriterOperation>,
    ) -> AgentProposal {
        AgentProposal {
            id: "proposal-id".to_string(),
            observation_id: "obs-1".to_string(),
            kind,
            priority: ProposalPriority::Ambient,
            target: None,
            preview: preview.to_string(),
            operations,
            rationale: "rationale".to_string(),
            evidence: vec![EvidenceRef {
                source: EvidenceSource::ChapterText,
                reference: "Chapter-1".to_string(),
                snippet: "snippet".to_string(),
            }],
            risks: vec![],
            alternatives: vec![],
            confidence: 0.7,
            expires_at: None,
        }
    }

    #[test]
    fn memory_candidate_slots_are_stable() {
        assert_eq!(
            memory_candidate_slot_for_canon(&canon_entity("沈照")),
            "memory|canon|character|沈照"
        );
        assert_eq!(
            memory_candidate_slot_for_promise(&promise("玉佩")),
            "memory|promise|mystery_clue|玉佩"
        );
    }

    #[test]
    fn proposal_slot_uses_memory_slot_for_memory_writes() {
        let proposal = proposal(
            ProposalKind::CanonUpdate,
            "沉淀设定",
            vec![WriterOperation::CanonUpsertEntity {
                entity: canon_entity("沈照"),
            }],
        );

        assert_eq!(proposal_slot_key(&proposal), "memory|canon|character|沈照");
        assert_eq!(
            suppression_slot_key(&proposal),
            "memory|canon|character|沈照"
        );
    }

    #[test]
    fn ghost_suppression_keys_include_chapter_and_preview_fingerprint() {
        let proposal = proposal(
            ProposalKind::Ghost,
            "  第一行   第二行   第三行  ",
            vec![WriterOperation::TextInsert {
                chapter: "Chapter-1".to_string(),
                at: 3,
                text: "text".to_string(),
                revision: "rev".to_string(),
            }],
        );

        assert_eq!(
            suppression_slot_key(&proposal),
            "Ghost|Chapter-1|第一行 第二行 第三行"
        );
    }

    #[test]
    fn memory_audit_ignores_non_memory_proposals() {
        let proposal = proposal(
            ProposalKind::Ghost,
            "preview",
            vec![WriterOperation::TextInsert {
                chapter: "Chapter-1".to_string(),
                at: 3,
                text: "text".to_string(),
                revision: "rev".to_string(),
            }],
        );
        let feedback = ProposalFeedback {
            proposal_id: proposal.id.clone(),
            action: FeedbackAction::Accepted,
            reason: None,
            final_text: None,
            created_at: 1,
        };

        assert!(memory_operation_slot(&proposal).is_none());
        assert_eq!(feedback.proposal_id, "proposal-id");
    }
}
