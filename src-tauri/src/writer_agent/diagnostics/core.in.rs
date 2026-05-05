
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub id: String,
    pub severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub message: String,
    pub entity_name: Option<String>,
    pub from: usize,
    pub to: usize,
    pub evidence: Vec<DiagnosticEvidence>,
    pub fix_suggestion: Option<String>,
    pub operations: Vec<WriterOperation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiagnosticCategory {
    CanonConflict,
    UnresolvedPromise,
    StoryContractViolation,
    ChapterMissionViolation,
    TimelineIssue,
    CharacterVoiceInconsistency,
    PacingNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEvidence {
    pub source: String,
    pub reference: String,
    pub snippet: String,
}

pub struct DiagnosticsEngine;

impl DiagnosticsEngine {
    pub fn new() -> Self {
        Self
    }

    /// Run all diagnostics on a paragraph within a chapter context.
    pub fn diagnose(
        &self,
        paragraph: &str,
        paragraph_offset: usize,
        chapter_id: &str,
        project_id: &str,
        memory: &WriterMemory,
    ) -> Vec<DiagnosticResult> {
        let mut results = Vec::new();
        let mut counter = 0u32;

        let mut next_id = || {
            counter += 1;
            format!("diag_{}_{}", chapter_id, counter)
        };

        // 1. Entity conflict + timeline state checks.
        let entities = extract_entities(paragraph, memory);
        for entity in &entities {
            let canonical_entity = memory
                .resolve_canon_entity_name(entity)
                .ok()
                .flatten()
                .unwrap_or_else(|| entity.clone());
            if let Ok(facts) = memory.get_canon_facts_for_entity(entity) {
                for (key, canon_value) in &facts {
                    if let Some(mentioned_value) = detect_attribute_value(paragraph, entity, key) {
                        if !attribute_values_compatible(key, &mentioned_value, canon_value) {
                            let pos = paragraph
                                .find(&mentioned_value)
                                .map(|p| paragraph_offset + byte_to_char_index(paragraph, p))
                                .unwrap_or(paragraph_offset);
                            let to = pos + mentioned_value.chars().count();
                            results.push(DiagnosticResult {
                                id: next_id(),
                                severity: DiagnosticSeverity::Error,
                                category: DiagnosticCategory::CanonConflict,
                                message: format!(
                                    "{}: canon记录 {}={}，但文中出现 {}",
                                    canonical_entity, key, canon_value, mentioned_value
                                ),
                                entity_name: Some(canonical_entity.clone()),
                                from: pos,
                                to,
                                evidence: vec![DiagnosticEvidence {
                                    source: "canon".into(),
                                    reference: canonical_entity.clone(),
                                    snippet: format!("{} = {}", key, canon_value),
                                }],
                                fix_suggestion: Some(format!(
                                    "将 {} 改为 {}",
                                    mentioned_value, canon_value
                                )),
                                operations: canon_conflict_operations(
                                    chapter_id,
                                    pos,
                                    to,
                                    canon_value,
                                    &mentioned_value,
                                    &canonical_entity,
                                    key,
                                ),
                            });
                        }
                    }

                    if let Some(issue) = detect_timeline_issue(
                        paragraph,
                        paragraph_offset,
                        entity,
                        &canonical_entity,
                        key,
                        canon_value,
                    ) {
                        results.push(DiagnosticResult {
                            id: next_id(),
                            severity: DiagnosticSeverity::Warning,
                            category: DiagnosticCategory::TimelineIssue,
                            message: issue.message,
                            entity_name: Some(canonical_entity.clone()),
                            from: issue.from,
                            to: issue.to,
                            evidence: vec![DiagnosticEvidence {
                                source: "canon".into(),
                                reference: canonical_entity.clone(),
                                snippet: format!("{} = {}", key, canon_value),
                            }],
                            fix_suggestion: issue.fix_suggestion,
                            operations: Vec::new(),
                        });
                    }
                }
            }
        }

        // 2. Book-level contract checks.
        if let Ok(Some(contract)) = memory.get_story_contract(project_id) {
            for issue in detect_story_contract_violations(paragraph, paragraph_offset, &contract) {
                results.push(DiagnosticResult {
                    id: next_id(),
                    severity: issue.severity,
                    category: DiagnosticCategory::StoryContractViolation,
                    message: issue.message,
                    entity_name: None,
                    from: issue.from,
                    to: issue.to,
                    evidence: vec![DiagnosticEvidence {
                        source: "story_contract".into(),
                        reference: issue.reference,
                        snippet: issue.snippet,
                    }],
                    fix_suggestion: Some(issue.fix_suggestion),
                    operations: vec![WriterOperation::TextAnnotate {
                        chapter: chapter_id.to_string(),
                        from: issue.from,
                        to: issue.to,
                        message: issue.annotation,
                        severity: issue.annotation_severity,
                    }],
                });
            }
        }

        // 3. Chapter mission guard checks.
        if let Ok(Some(mission)) = memory.get_chapter_mission(project_id, chapter_id) {
            for issue in detect_chapter_mission_violations(paragraph, paragraph_offset, &mission) {
                results.push(DiagnosticResult {
                    id: next_id(),
                    severity: issue.severity,
                    category: DiagnosticCategory::ChapterMissionViolation,
                    message: issue.message,
                    entity_name: None,
                    from: issue.from,
                    to: issue.to,
                    evidence: vec![DiagnosticEvidence {
                        source: "chapter_mission".into(),
                        reference: issue.reference,
                        snippet: issue.snippet,
                    }],
                    fix_suggestion: Some(issue.fix_suggestion),
                    operations: vec![WriterOperation::TextAnnotate {
                        chapter: chapter_id.to_string(),
                        from: issue.from,
                        to: issue.to,
                        message: issue.annotation,
                        severity: issue.annotation_severity,
                    }],
                });
            }
        }

        // 4. Open promises for this chapter.
        if let Ok(promises) = memory.get_open_promise_summaries() {
            for promise in &promises {
                if !is_later_chapter(chapter_id, &promise.introduced_chapter) {
                    continue;
                }

                let mention = match_promise(paragraph, promise);
                if mention.is_match {
                    results.push(DiagnosticResult {
                        id: next_id(),
                        severity: DiagnosticSeverity::Info,
                        category: DiagnosticCategory::UnresolvedPromise,
                        message: format!(
                            "伏笔回收机会: {} ({}引入)",
                            promise.title, promise.introduced_chapter
                        ),
                        entity_name: None,
                        from: paragraph_offset + mention.from.unwrap_or(0),
                        to: paragraph_offset
                            + mention
                                .to
                                .unwrap_or_else(|| paragraph.chars().count())
                                .max(mention.from.unwrap_or(0) + 1),
                        evidence: vec![DiagnosticEvidence {
                            source: "promise".into(),
                            reference: promise.title.clone(),
                            snippet: promise.description.clone(),
                        }],
                        fix_suggestion: Some(format!(
                            "确认这里是否要回收伏笔：{}",
                            promise.expected_payoff
                        )),
                        operations: promise_decision_operations(promise, chapter_id),
                    });
                    continue;
                }

                if is_stale_promise(
                    chapter_id,
                    &promise.introduced_chapter,
                    &promise.expected_payoff,
                ) {
                    results.push(DiagnosticResult {
                        id: next_id(),
                        severity: DiagnosticSeverity::Warning,
                        category: DiagnosticCategory::UnresolvedPromise,
                        message: format!(
                            "伏笔仍未回收: {} ({}引入，预期{})",
                            promise.title,
                            promise.introduced_chapter,
                            if promise.expected_payoff.trim().is_empty() {
                                "后续回收"
                            } else {
                                promise.expected_payoff.as_str()
                            }
                        ),
                        entity_name: None,
                        from: paragraph_offset,
                        to: paragraph_offset + paragraph.chars().count().min(40),
                        evidence: vec![DiagnosticEvidence {
                            source: "promise".into(),
                            reference: promise.title.clone(),
                            snippet: promise.description.clone(),
                        }],
                        fix_suggestion: Some("决定回收、延后，或标记为废弃。".into()),
                        operations: promise_decision_operations(promise, chapter_id),
                    });
                }
            }
        }

        // 5. Pacing check (paragraph length).
        if paragraph.chars().count() > 2000 {
            results.push(DiagnosticResult {
                id: next_id(),
                severity: DiagnosticSeverity::Warning,
                category: DiagnosticCategory::PacingNote,
                message: "段落较长(>2000字)，考虑拆分或检查节奏".into(),
                entity_name: None,
                from: paragraph_offset,
                to: paragraph_offset + 10,
                evidence: vec![],
                fix_suggestion: Some("在对话或动作处拆分段落".into()),
                operations: Vec::new(),
            });
        }

        results
    }
}

