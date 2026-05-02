//! Operation execution — typed operation dispatch.
//! Extracted from kernel.rs.

use super::kernel_helpers::{
    normalize_chapter_mission_status, validate_chapter_mission_summary,
    validate_story_contract_summary,
};
use super::memory::{ChapterMissionSummary, StoryContractSummary, WriterMemory};
use super::operation::{
    execute_text_operation, CanonEntityOp, OperationResult, PlotPromiseOp, WriterOperation,
};

pub(crate) fn execute_writer_operation(
    memory: &mut WriterMemory,
    active_chapter: &Option<String>,
    operation: WriterOperation,
    current_content: &str,
    current_revision: &str,
) -> Result<OperationResult, String> {
    let result: Result<OperationResult, String> = match &operation {
        WriterOperation::TextInsert { .. } | WriterOperation::TextReplace { .. } => {
            match execute_text_operation(&operation, current_content, current_revision) {
                Ok((_new_content, new_revision)) => Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: Some(new_revision),
                }),
                Err(e) => Ok(OperationResult {
                    success: false,
                    operation,
                    error: Some(e),
                    revision_after: None,
                }),
            }
        }
        WriterOperation::TextAnnotate {
            chapter,
            from,
            to,
            message,
            severity,
        } => {
            let source = format!("text:{}:{}-{}", chapter, from, to);
            memory
                .record_decision(
                    chapter,
                    &format!("Annotation: {:?}", severity),
                    "annotated_text",
                    &[],
                    message,
                    &[source],
                )
                .map_err(|e| format!("annotation: {}", e))?;
            Ok(OperationResult {
                success: true,
                operation,
                error: None,
                revision_after: None,
            })
        }
        WriterOperation::CanonUpsertEntity { entity } => {
            memory
                .upsert_canon_entity(
                    &entity.kind,
                    &entity.name,
                    &entity.aliases,
                    &entity.summary,
                    &entity.attributes,
                    entity.confidence,
                )
                .map_err(|e| format!("canon: {}", e))?;
            Ok(OperationResult {
                success: true,
                operation,
                error: None,
                revision_after: None,
            })
        }
        WriterOperation::CanonUpdateAttribute {
            entity,
            attribute,
            value,
            confidence,
        } => {
            let rationale = format!(
                "Author confirmed canon update: {}.{} = {}",
                entity, attribute, value
            );
            memory
                .update_canon_attribute(entity, attribute, value, *confidence)
                .map_err(|e| format!("canon: {}", e))?;
            memory
                .record_decision(
                    active_chapter.as_deref().unwrap_or("project"),
                    &format!("Canon update: {}", entity),
                    "updated_canon",
                    &[],
                    &rationale,
                    &[format!("canon:{}:{}", entity, attribute)],
                )
                .ok();
            Ok(OperationResult {
                success: true,
                operation,
                error: None,
                revision_after: None,
            })
        }
        WriterOperation::CanonUpsertRule { rule } => {
            memory
                .upsert_canon_rule(
                    &rule.rule,
                    &rule.category,
                    rule.priority,
                    "writer_operation",
                )
                .map_err(|e| format!("canon rule: {}", e))?;
            memory
                .record_decision(
                    active_chapter.as_deref().unwrap_or("project"),
                    &format!("Canon rule: {}", rule.category),
                    "upserted_canon_rule",
                    &[],
                    &rule.rule,
                    &[format!("canon_rule:{}", rule.category)],
                )
                .ok();
            Ok(OperationResult {
                success: true,
                operation,
                error: None,
                revision_after: None,
            })
        }
        WriterOperation::PromiseAdd { promise } => {
            memory
                .add_promise_with_entities(
                    &promise.kind,
                    &promise.title,
                    &promise.description,
                    &promise.introduced_chapter,
                    &promise.expected_payoff,
                    promise.priority,
                    &promise.related_entities,
                )
                .map_err(|e| format!("promise: {}", e))?;
            Ok(OperationResult {
                success: true,
                operation,
                error: None,
                revision_after: None,
            })
        }
        WriterOperation::PromiseResolve {
            promise_id,
            chapter,
        } => {
            let id = promise_id
                .parse::<i64>()
                .map_err(|_| format!("promise: invalid promise id '{}'", promise_id))?;
            let resolved = memory
                .resolve_promise(id, chapter)
                .map_err(|e| format!("promise: {}", e))?;
            Ok(OperationResult {
                success: resolved,
                operation,
                error: if resolved {
                    None
                } else {
                    Some(super::operation::OperationError::invalid(
                        "Promise is already resolved or does not exist",
                    ))
                },
                revision_after: None,
            })
        }
        WriterOperation::PromiseDefer {
            promise_id,
            chapter,
            expected_payoff,
        } => {
            let id = promise_id
                .parse::<i64>()
                .map_err(|_| format!("promise: invalid promise id '{}'", promise_id))?;
            let deferred = memory
                .defer_promise(id, expected_payoff)
                .map_err(|e| format!("promise: {}", e))?;
            if deferred {
                memory
                    .record_decision(
                        chapter,
                        &format!("Defer promise {}", promise_id),
                        "deferred_promise",
                        &[],
                        &format!(
                            "Author deferred promise {} to {}",
                            promise_id, expected_payoff
                        ),
                        &[format!("promise:{}", promise_id)],
                    )
                    .ok();
            }
            Ok(OperationResult {
                success: deferred,
                operation,
                error: if deferred {
                    None
                } else {
                    Some(super::operation::OperationError::invalid(
                        "Promise is already closed or does not exist",
                    ))
                },
                revision_after: None,
            })
        }
        WriterOperation::PromiseAbandon {
            promise_id,
            chapter,
            reason,
        } => {
            let id = promise_id
                .parse::<i64>()
                .map_err(|_| format!("promise: invalid promise id '{}'", promise_id))?;
            let abandoned = memory
                .abandon_promise(id)
                .map_err(|e| format!("promise: {}", e))?;
            if abandoned {
                memory
                    .record_decision(
                        chapter,
                        &format!("Abandon promise {}", promise_id),
                        "abandoned_promise",
                        &["resolve".to_string(), "defer".to_string()],
                        reason,
                        &[format!("promise:{}", promise_id)],
                    )
                    .ok();
            }
            Ok(OperationResult {
                success: abandoned,
                operation,
                error: if abandoned {
                    None
                } else {
                    Some(super::operation::OperationError::invalid(
                        "Promise is already closed or does not exist",
                    ))
                },
                revision_after: None,
            })
        }
        WriterOperation::StyleUpdatePreference { key, value } => {
            memory
                .upsert_style_preference(key, value, true)
                .map_err(|e| format!("style preference: {}", e))?;
            memory
                .record_decision(
                    active_chapter.as_deref().unwrap_or("project"),
                    &format!("Style preference: {}", key),
                    "updated_style_preference",
                    &[],
                    value,
                    &[format!("style:{}", key)],
                )
                .ok();
            Ok(OperationResult {
                success: true,
                operation,
                error: None,
                revision_after: None,
            })
        }
        WriterOperation::StoryContractUpsert { contract } => {
            let summary = StoryContractSummary {
                project_id: contract.project_id.clone(),
                title: contract.title.clone(),
                genre: contract.genre.clone(),
                target_reader: contract.target_reader.clone(),
                reader_promise: contract.reader_promise.clone(),
                first_30_chapter_promise: contract.first_30_chapter_promise.clone(),
                main_conflict: contract.main_conflict.clone(),
                structural_boundary: contract.structural_boundary.clone(),
                tone_contract: contract.tone_contract.clone(),
                updated_at: String::new(),
            };
            if let Some(error) = validate_story_contract_summary(&summary) {
                Ok(OperationResult {
                    success: false,
                    operation,
                    error: Some(super::operation::OperationError::invalid(&error)),
                    revision_after: None,
                })
            } else {
                memory
                    .upsert_story_contract(&summary)
                    .map_err(|e| format!("story contract: {}", e))?;
                memory
                    .record_decision(
                        "project",
                        "Story contract",
                        "updated_story_contract",
                        &[],
                        &summary.render_for_context(),
                        &[format!("story_contract:{}", summary.project_id)],
                    )
                    .ok();
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
        }
        WriterOperation::ChapterMissionUpsert { mission } => {
            let normalized_status = normalize_chapter_mission_status(&mission.status);
            let summary = ChapterMissionSummary {
                id: 0,
                project_id: mission.project_id.clone(),
                chapter_title: mission.chapter_title.clone(),
                mission: mission.mission.clone(),
                must_include: mission.must_include.clone(),
                must_not: mission.must_not.clone(),
                expected_ending: mission.expected_ending.clone(),
                status: normalized_status,
                source_ref: mission.source_ref.clone(),
                updated_at: String::new(),
            };
            if let Some(error) = validate_chapter_mission_summary(&summary) {
                Ok(OperationResult {
                    success: false,
                    operation,
                    error: Some(super::operation::OperationError::invalid(&error)),
                    revision_after: None,
                })
            } else {
                memory
                    .upsert_chapter_mission(&summary)
                    .map_err(|e| format!("chapter mission: {}", e))?;
                memory
                    .record_decision(
                        &summary.chapter_title,
                        "Chapter mission",
                        "updated_chapter_mission",
                        &[],
                        &summary.render_for_context(),
                        &[format!(
                            "chapter_mission:{}:{}",
                            summary.project_id, summary.chapter_title
                        )],
                    )
                    .ok();
                Ok(OperationResult {
                    success: true,
                    operation,
                    error: None,
                    revision_after: None,
                })
            }
        }
        WriterOperation::OutlineUpdate { .. } => Ok(OperationResult {
            success: false,
            operation,
            error: Some(super::operation::OperationError::invalid(
                "outline.update requires project storage runtime",
            )),
            revision_after: None,
        }),
    };
    result
}
