//! DiagnosticsEngine — ambient canon + promise checking for story continuity.
//! Runs on paragraph completion (3s idle) or chapter save to detect:
//! - Entity/attribute conflicts (weapon, location, relationship)
//! - Unresolved plot promises in current chapter scope
//! - Timeline inconsistencies

use super::memory::{ChapterMissionSummary, StoryContractSummary, WriterMemory};
use super::operation::{AnnotationSeverity, WriterOperation};
use serde::{Deserialize, Serialize};

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

/// Simple entity name extraction from Chinese text.
/// Finds capitalized/known names and key nouns.
fn extract_entities(paragraph: &str, memory: &WriterMemory) -> Vec<String> {
    let mut entities = Vec::new();
    if let Ok(known_names) = memory.get_canon_entity_names() {
        for name in known_names {
            if !name.trim().is_empty() && paragraph.contains(&name) && !entities.contains(&name) {
                entities.push(name);
            }
        }
    }

    // Find 2-3 char sequences that look like names (Chinese names are typically 2-3 chars)
    let chars: Vec<char> = paragraph.chars().collect();
    let mut i = 0;
    while i + 1 < chars.len() {
        // Look for patterns like "XX拔出" or "XX的"
        if i + 2 < chars.len() {
            let slice: String = chars[i..i + 2].iter().collect();
            // Check if followed by action verb or particle
            if i + 2 < chars.len() {
                let next = chars[i + 2];
                if matches!(next, '拔' | '握' | '拿' | '举' | '的' | '说' | '走' | '看')
                    && !entities.contains(&slice) {
                        entities.push(slice);
                    }
            }
        }
        i += 1;
    }
    entities
}

fn byte_to_char_index(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

/// Detect a specific attribute value mentioned near an entity.
fn detect_attribute_value(paragraph: &str, entity: &str, attribute: &str) -> Option<String> {
    match attribute {
        "weapon" => {
            let weapons = [
                "匕首",
                "长剑",
                "短剑",
                "寒影刀",
                "剑",
                "刀",
                "枪",
                "弓",
                "棍",
                "鞭",
                "斧",
                "戟",
                "锤",
            ];
            if let Some(pos) = paragraph.find(entity) {
                let after: String = paragraph[pos + entity.len()..].chars().take(30).collect();
                for w in &weapons {
                    if after.contains(w) {
                        return Some(w.to_string());
                    }
                }
            }
            None
        }
        "location" => {
            let locations = ["破庙", "宫殿", "山洞", "客栈", "城", "山林", "河边"];
            for loc in &locations {
                if paragraph.contains(loc) {
                    return Some(loc.to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn attribute_values_compatible(attribute: &str, mentioned_value: &str, canon_value: &str) -> bool {
    let mentioned = mentioned_value.trim();
    let canon = canon_value.trim();
    if mentioned == canon {
        return true;
    }

    match attribute {
        "weapon" => {
            canon.contains(mentioned)
                || mentioned.contains(canon)
                || weapon_family(canon) == weapon_family(mentioned)
        }
        _ => false,
    }
}

fn canon_conflict_operations(
    chapter_id: &str,
    from: usize,
    to: usize,
    canon_value: &str,
    mentioned_value: &str,
    entity: &str,
    attribute: &str,
) -> Vec<WriterOperation> {
    let replacement = canon_value.trim();
    if replacement.is_empty() {
        return vec![WriterOperation::TextAnnotate {
            chapter: chapter_id.to_string(),
            from,
            to,
            message: format!(
                "{} 的 {} 与 canon 冲突：文中 {}",
                entity, attribute, mentioned_value
            ),
            severity: AnnotationSeverity::Error,
        }];
    }

    vec![
        WriterOperation::TextReplace {
            chapter: chapter_id.to_string(),
            from,
            to,
            text: replacement.to_string(),
            revision: "missing".to_string(),
        },
        WriterOperation::CanonUpdateAttribute {
            entity: entity.to_string(),
            attribute: attribute.to_string(),
            value: mentioned_value.to_string(),
            confidence: 0.82,
        },
    ]
}

fn weapon_family(value: &str) -> &str {
    for family in ["剑", "刀", "枪", "弓", "匕首", "棍", "鞭", "斧", "戟", "锤"] {
        if value.contains(family) {
            return family;
        }
    }
    value
}

struct TimelineIssue {
    message: String,
    from: usize,
    to: usize,
    fix_suggestion: Option<String>,
}

fn detect_timeline_issue(
    paragraph: &str,
    paragraph_offset: usize,
    entity_mention: &str,
    canonical_entity: &str,
    attribute: &str,
    canon_value: &str,
) -> Option<TimelineIssue> {
    if !is_state_attribute(attribute) || !paragraph.contains(entity_mention) {
        return None;
    }
    if looks_like_flashback_or_nonliteral(paragraph) {
        return None;
    }

    let span = entity_context_after(paragraph, entity_mention, 36);
    let entity_pos = paragraph.find(entity_mention).unwrap_or(0);
    let from = paragraph_offset + byte_to_char_index(paragraph, entity_pos);
    let to = from + entity_mention.chars().count();

    if value_contains_any(canon_value, DEAD_STATE_CUES)
        && text_contains_any(&span, LIVING_ACTION_CUES)
    {
        return Some(TimelineIssue {
            message: format!(
                "时间线疑点: canon记录{}已死亡，但当前段落让其执行行动或说话",
                canonical_entity
            ),
            from,
            to,
            fix_suggestion: Some("确认这是回忆、幻象、尸体描写，还是需要调整出场人物。".into()),
        });
    }

    if value_contains_any(canon_value, UNAVAILABLE_STATE_CUES)
        && text_contains_any(&span, PRESENT_ACTION_CUES)
    {
        return Some(TimelineIssue {
            message: format!(
                "时间线疑点: canon记录{}当前不在场，但当前段落让其出现在现场",
                canonical_entity
            ),
            from,
            to,
            fix_suggestion: Some("补一笔其返回原因，或替换为当前在场角色。".into()),
        });
    }

    None
}

fn is_state_attribute(attribute: &str) -> bool {
    matches!(
        attribute,
        "status" | "state" | "life_state" | "current_state" | "availability"
    )
}

const DEAD_STATE_CUES: &[&str] = &["死亡", "已死", "死了", "阵亡", "dead", "deceased"];
const UNAVAILABLE_STATE_CUES: &[&str] = &["离开", "离队", "远在", "不在", "失踪", "被关", "囚禁"];
const LIVING_ACTION_CUES: &[&str] = &[
    "说道", "说", "笑", "哭", "走", "跑", "站", "看", "呼吸", "握", "拔", "推门", "点头", "摇头",
    "伸手", "醒来",
];
const PRESENT_ACTION_CUES: &[&str] = &[
    "走进",
    "推门而入",
    "出现",
    "站在",
    "回到",
    "坐在",
    "就在",
    "等在",
    "开口",
    "说道",
];

fn looks_like_flashback_or_nonliteral(text: &str) -> bool {
    text_contains_any(
        text,
        &[
            "回忆", "想起", "梦", "梦里", "幻觉", "幻象", "尸体", "遗体", "墓", "画像", "信里",
            "传闻", "灵魂",
        ],
    )
}

fn entity_context_after(text: &str, entity: &str, max_chars: usize) -> String {
    let Some(pos) = text.find(entity) else {
        return String::new();
    };
    text[pos + entity.len()..].chars().take(max_chars).collect()
}

fn value_contains_any(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_lowercase();
    needles
        .iter()
        .any(|needle| lower.contains(&needle.to_lowercase()))
}

fn text_contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

struct StoryContractIssue {
    severity: DiagnosticSeverity,
    message: String,
    from: usize,
    to: usize,
    reference: String,
    snippet: String,
    fix_suggestion: String,
    annotation: String,
    annotation_severity: AnnotationSeverity,
}

struct ChapterMissionIssue {
    severity: DiagnosticSeverity,
    message: String,
    from: usize,
    to: usize,
    reference: String,
    snippet: String,
    fix_suggestion: String,
    annotation: String,
    annotation_severity: AnnotationSeverity,
}

fn detect_story_contract_violations(
    paragraph: &str,
    paragraph_offset: usize,
    contract: &StoryContractSummary,
) -> Vec<StoryContractIssue> {
    let mut issues = Vec::new();

    if let Some(issue) = detect_structural_boundary_violation(
        paragraph,
        paragraph_offset,
        &contract.structural_boundary,
    ) {
        issues.push(issue);
    }

    issues
}

fn detect_chapter_mission_violations(
    paragraph: &str,
    paragraph_offset: usize,
    mission: &ChapterMissionSummary,
) -> Vec<ChapterMissionIssue> {
    let mut issues = Vec::new();

    if let Some(issue) = detect_mission_must_not_violation(
        paragraph,
        paragraph_offset,
        &mission.chapter_title,
        &mission.must_not,
    ) {
        issues.push(issue);
    }

    issues
}

fn detect_mission_must_not_violation(
    paragraph: &str,
    paragraph_offset: usize,
    chapter_title: &str,
    must_not: &str,
) -> Option<ChapterMissionIssue> {
    let must_not = must_not.trim();
    if must_not.is_empty() {
        return None;
    }

    let terms = mission_guard_terms(must_not);
    let matched = terms
        .iter()
        .find_map(|term| match_mission_guard_term(paragraph, term))?;
    if looks_negated_or_deferred_before(paragraph, matched.0) {
        return None;
    }
    let from = paragraph_offset + matched.0;
    let to = paragraph_offset + matched.1.max(matched.0 + 1);

    Some(ChapterMissionIssue {
        severity: DiagnosticSeverity::Error,
        message: format!(
            "章节任务违例: {} 的禁止事项被触碰「{}」",
            chapter_title, must_not
        ),
        from,
        to,
        reference: format!("{}:must_not", chapter_title),
        snippet: must_not.to_string(),
        fix_suggestion: "保留悬念、换成误导或旁证，或先更新本章任务再继续。".to_string(),
        annotation: format!("疑似违反本章禁止事项：{}", must_not),
        annotation_severity: AnnotationSeverity::Error,
    })
}

fn mission_guard_terms(must_not: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for term in meaningful_terms(must_not) {
        if is_mission_guard_stop_term(&term) {
            continue;
        }
        push_term(&mut terms, &term);
    }

    let chars = must_not.chars().collect::<Vec<_>>();
    for pair in chars.windows(2) {
        let term = pair.iter().collect::<String>();
        if term.chars().all(is_term_char)
            && !is_mission_guard_stop_term(&term)
            && !is_stop_term(&term)
        {
            push_term(&mut terms, &term);
        }
    }

    terms
}

fn match_mission_guard_term(paragraph: &str, term: &str) -> Option<(usize, usize)> {
    if let Some(byte_pos) = paragraph.find(term) {
        let from = byte_to_char_index(paragraph, byte_pos);
        return Some((from, from + term.chars().count()));
    }

    if term.chars().count() == 2 {
        let chars = term.chars().collect::<Vec<_>>();
        if chars.len() == 2 && paragraph.contains(chars[0]) && paragraph.contains(chars[1]) {
            let first = paragraph
                .find(chars[0])
                .map(|pos| byte_to_char_index(paragraph, pos))?;
            let second = paragraph
                .find(chars[1])
                .map(|pos| byte_to_char_index(paragraph, pos))?;
            let from = first.min(second);
            let to = first.max(second) + 1;
            return Some((from, to));
        }
    }

    None
}

fn is_mission_guard_stop_term(term: &str) -> bool {
    const STOP_TERMS: &[&str] = &[
        "不得", "不要", "不能", "禁止", "不许", "避免", "提前", "泄露", "揭露", "揭示", "揭开",
        "本章", "事项",
    ];
    STOP_TERMS.iter().any(|stop| term.contains(stop))
}

fn detect_structural_boundary_violation(
    paragraph: &str,
    paragraph_offset: usize,
    boundary: &str,
) -> Option<StoryContractIssue> {
    let boundary = boundary.trim();
    if boundary.is_empty() || !text_contains_any(boundary, CONTRACT_FORBID_CUES) {
        return None;
    }
    if !text_contains_any(paragraph, CONTRACT_REVEAL_CUES) {
        return None;
    }

    let terms = contract_boundary_terms(boundary);
    let matched = terms
        .iter()
        .find_map(|term| match_contract_term(paragraph, term))?;
    if looks_negated_or_deferred_before(paragraph, matched.0) {
        return None;
    }
    let from = paragraph_offset + matched.0;
    let to = paragraph_offset + matched.1.max(matched.0 + 1);

    Some(StoryContractIssue {
        severity: DiagnosticSeverity::Error,
        message: format!("书级合同违例: 当前段落疑似触碰禁区「{}」", boundary),
        from,
        to,
        reference: "structural_boundary".to_string(),
        snippet: boundary.to_string(),
        fix_suggestion: "延后揭示、改成误导线索，或先更新书级合同后再写正文。".to_string(),
        annotation: format!("疑似违反书级禁区：{}", boundary),
        annotation_severity: AnnotationSeverity::Error,
    })
}

const CONTRACT_FORBID_CUES: &[&str] = &["不得", "不要", "不能", "禁止", "不许", "避免", "禁"];
const CONTRACT_REVEAL_CUES: &[&str] = &[
    "真相", "来源", "身份", "秘密", "揭开", "揭露", "揭示", "说出", "坦白", "承认", "原来", "其实",
    "就是", "来自",
];

fn contract_boundary_terms(boundary: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for term in meaningful_terms(boundary) {
        if is_contract_boundary_stop_term(&term) {
            continue;
        }
        push_term(&mut terms, &term);
    }

    let chars = boundary.chars().collect::<Vec<_>>();
    for pair in chars.windows(2) {
        let term = pair.iter().collect::<String>();
        if term.chars().all(is_term_char)
            && !is_contract_boundary_stop_term(&term)
            && !is_stop_term(&term)
        {
            push_term(&mut terms, &term);
        }
    }

    terms
}

fn match_contract_term(paragraph: &str, term: &str) -> Option<(usize, usize)> {
    if let Some(byte_pos) = paragraph.find(term) {
        let from = byte_to_char_index(paragraph, byte_pos);
        return Some((from, from + term.chars().count()));
    }

    if term.chars().count() == 2 {
        let chars = term.chars().collect::<Vec<_>>();
        if chars.len() == 2 && paragraph.contains(chars[0]) && paragraph.contains(chars[1]) {
            let first = paragraph
                .find(chars[0])
                .map(|pos| byte_to_char_index(paragraph, pos))?;
            let second = paragraph
                .find(chars[1])
                .map(|pos| byte_to_char_index(paragraph, pos))?;
            let from = first.min(second);
            let to = first.max(second) + 1;
            return Some((from, to));
        }
    }

    None
}

fn is_contract_boundary_stop_term(term: &str) -> bool {
    const STOP_TERMS: &[&str] = &[
        "不得", "不要", "不能", "禁止", "不许", "避免", "提前", "泄露", "揭露", "揭示", "揭开",
        "真相", "来源", "身份", "秘密",
    ];
    STOP_TERMS.iter().any(|stop| term.contains(stop))
}

fn looks_negated_or_deferred_before(text: &str, match_from: usize) -> bool {
    let chars = text.chars().collect::<Vec<_>>();
    let start = match_from.saturating_sub(8);
    let context = chars[start..match_from.min(chars.len())]
        .iter()
        .collect::<String>();
    text_contains_any(
        &context,
        &[
            "没有",
            "并未",
            "未曾",
            "尚未",
            "还没",
            "不会",
            "不能",
            "不该",
            "不肯",
            "拒绝",
            "暂不",
            "避免",
            "没有真正",
            "并没有",
        ],
    )
}

#[derive(Default)]
struct PromiseMatch {
    is_match: bool,
    from: Option<usize>,
    to: Option<usize>,
}

fn match_promise(paragraph: &str, promise: &super::memory::PlotPromiseSummary) -> PromiseMatch {
    let terms = promise_terms(
        &promise.title,
        &promise.description,
        &promise.expected_payoff,
    );
    let mut score = 0usize;
    let mut first_span = None;

    for (index, term) in terms.iter().enumerate() {
        if let Some(byte_pos) = paragraph.find(term) {
            let from = byte_to_char_index(paragraph, byte_pos);
            let to = from + term.chars().count();
            first_span.get_or_insert((from, to));
            score += if index == 0 { 3 } else { 1 };
        }
    }

    let (from, to) = first_span.unwrap_or((0, paragraph.chars().count()));
    PromiseMatch {
        is_match: score >= 2,
        from: Some(from),
        to: Some(to),
    }
}

fn promise_terms(title: &str, description: &str, expected_payoff: &str) -> Vec<String> {
    let mut terms = Vec::new();
    push_term(&mut terms, title);
    for alias in promise_aliases(title) {
        push_term(&mut terms, alias);
    }

    for text in [description, expected_payoff] {
        for term in meaningful_terms(text) {
            push_term(&mut terms, &term);
        }
    }

    terms
}

fn push_term(terms: &mut Vec<String>, term: &str) {
    let normalized = term.trim();
    if normalized.chars().count() < 2 || is_stop_term(normalized) {
        return;
    }
    if !terms.iter().any(|existing| existing == normalized) {
        terms.push(normalized.to_string());
    }
}

fn promise_aliases(title: &str) -> Vec<&'static str> {
    let mut aliases = Vec::new();
    if title.contains("玉佩") {
        aliases.extend(["玉坠", "玉牌", "那枚玉", "那块玉"]);
    }
    if title.contains("密道") {
        aliases.extend(["暗道", "地道", "暗门"]);
    }
    if title.contains("钥匙") {
        aliases.extend(["钥匙串", "铜钥匙"]);
    }
    aliases
}

fn meaningful_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    for window in 2..=3 {
        if chars.len() < window {
            continue;
        }
        for slice in chars.windows(window) {
            let term: String = slice.iter().collect();
            if term.chars().all(is_term_char) && !is_stop_term(&term) {
                push_term(&mut terms, &term);
            }
        }
    }
    terms
}

fn is_term_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn is_stop_term(term: &str) -> bool {
    const STOP_TERMS: &[&str] = &[
        "需要", "交代", "下落", "伏笔", "回收", "揭示", "说明", "后续", "预期", "Chapter", "章节",
        "当前", "引入", "拿走", "留下", "发现", "必须", "为何", "什么", "没有",
    ];
    STOP_TERMS.iter().any(|stop| term.contains(stop))
}

fn is_later_chapter(current: &str, introduced: &str) -> bool {
    match (chapter_number(current), chapter_number(introduced)) {
        (Some(current), Some(introduced)) => current > introduced,
        _ => false,
    }
}

fn is_stale_promise(current: &str, introduced: &str, expected_payoff: &str) -> bool {
    if let (Some(current_number), Some(payoff_number)) =
        (chapter_number(current), chapter_number(expected_payoff))
    {
        return current_number >= payoff_number;
    }

    match (chapter_number(current), chapter_number(introduced)) {
        (Some(current_number), Some(introduced_number)) => current_number - introduced_number >= 3,
        _ => false,
    }
}

fn promise_decision_operations(
    promise: &super::memory::PlotPromiseSummary,
    chapter_id: &str,
) -> Vec<WriterOperation> {
    vec![
        WriterOperation::PromiseResolve {
            promise_id: promise.id.to_string(),
            chapter: chapter_id.to_string(),
        },
        WriterOperation::PromiseDefer {
            promise_id: promise.id.to_string(),
            chapter: chapter_id.to_string(),
            expected_payoff: next_chapter_label(chapter_id),
        },
        WriterOperation::PromiseAbandon {
            promise_id: promise.id.to_string(),
            chapter: chapter_id.to_string(),
            reason: format!(
                "Author decided '{}' no longer needs payoff in the current story shape.",
                promise.title
            ),
        },
    ]
}

fn next_chapter_label(chapter_id: &str) -> String {
    chapter_number(chapter_id)
        .map(|number| format!("Chapter-{}", number + 1))
        .unwrap_or_else(|| "later chapter".to_string())
}

fn chapter_number(chapter: &str) -> Option<i64> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in chapter.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            numbers.push(current.parse::<i64>().ok()?);
            current.clear();
        }
    }
    if !current.is_empty() {
        numbers.push(current.parse::<i64>().ok()?);
    }
    numbers.last().copied()
}

impl Default for DiagnosticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::memory::WriterMemory;
    use super::*;

    fn test_memory() -> WriterMemory {
        WriterMemory::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_extract_entities_action() {
        let m = test_memory();
        let entities = extract_entities("林墨拔出一把长剑", &m);
        assert!(entities.contains(&"林墨".to_string()));
    }

    #[test]
    fn test_detect_weapon_value() {
        let val = detect_attribute_value("林墨拔出一把长剑指向天空", "林墨", "weapon");
        assert_eq!(val, Some("长剑".to_string()));
    }

    #[test]
    fn test_diagnose_weapon_conflict() {
        let m = test_memory();
        m.upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.9,
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose("林墨拔出一把长剑", 10, "ch3", "default", &m);
        let conflict = results
            .iter()
            .find(|result| matches!(result.category, DiagnosticCategory::CanonConflict))
            .unwrap();
        assert_eq!(conflict.from, 16);
        assert_eq!(conflict.to, 18);
    }

    #[test]
    fn test_diagnose_accepts_weapon_family_match() {
        let m = test_memory();
        m.upsert_canon_entity(
            "character",
            "林墨",
            &[],
            "主角",
            &serde_json::json!({"weapon": "寒影刀"}),
            0.9,
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose("林墨拔出寒影刀", 0, "Chapter-3", "default", &m);
        assert!(!results
            .iter()
            .any(|result| matches!(result.category, DiagnosticCategory::CanonConflict)));
    }

    #[test]
    fn test_chapter_number_order_avoids_lexicographic_regression() {
        assert!(is_later_chapter("Chapter-10", "Chapter-2"));
        assert!(!is_later_chapter("Chapter-2", "Chapter-10"));
    }

    #[test]
    fn test_promise_opportunity_uses_terms_not_fixed_prefix() {
        let m = test_memory();
        m.add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落",
            "Chapter-1",
            "Chapter-4",
            4,
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose("张三把那枚玉佩放回桌上。", 0, "Chapter-3", "default", &m);
        assert!(results
            .iter()
            .any(|result| matches!(result.category, DiagnosticCategory::UnresolvedPromise)));
    }

    #[test]
    fn test_promise_not_flagged_from_future_chapter() {
        let m = test_memory();
        m.add_promise(
            "object_in_motion",
            "玉佩",
            "张三拿走玉佩，需要交代下落",
            "Chapter-10",
            "Chapter-12",
            4,
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose("张三把那枚玉佩放回桌上。", 0, "Chapter-2", "default", &m);
        assert!(!results
            .iter()
            .any(|result| matches!(result.category, DiagnosticCategory::UnresolvedPromise)));
    }

    #[test]
    fn test_stale_promise_warns_at_payoff_chapter() {
        let m = test_memory();
        m.add_promise(
            "mystery",
            "密道",
            "破庙里有密道，需要揭示用途",
            "Chapter-1",
            "Chapter-3",
            5,
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose(
            "林墨推开门，雨声压住了脚步。",
            0,
            "Chapter-3",
            "default",
            &m,
        );
        assert!(results.iter().any(|result| {
            matches!(result.category, DiagnosticCategory::UnresolvedPromise)
                && matches!(result.severity, DiagnosticSeverity::Warning)
        }));
    }

    #[test]
    fn test_story_contract_boundary_violation_warns() {
        let m = test_memory();
        m.ensure_story_contract_seed(
            "default",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose(
            "张三终于说出真相：玉佩其实来自禁地。",
            0,
            "Chapter-2",
            "default",
            &m,
        );
        let warning = results
            .iter()
            .find(|result| matches!(result.category, DiagnosticCategory::StoryContractViolation))
            .unwrap();
        assert!(warning.message.contains("书级合同违例"));
        assert_eq!(warning.evidence[0].source, "story_contract");
    }

    #[test]
    fn test_chapter_mission_must_not_violation_warns() {
        let m = test_memory();
        m.ensure_chapter_mission_seed(
            "default",
            "Chapter-2",
            "让林墨追查玉佩下落。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
            "test",
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose(
            "林墨直接揭开了真相，玉佩来自禁地。",
            0,
            "Chapter-2",
            "default",
            &m,
        );
        let warning = results
            .iter()
            .find(|result| matches!(result.category, DiagnosticCategory::ChapterMissionViolation))
            .unwrap();
        assert!(warning.message.contains("章节任务违例"));
        assert_eq!(warning.evidence[0].source, "chapter_mission");
    }

    #[test]
    fn test_chapter_mission_must_not_negated_does_not_warn() {
        let m = test_memory();
        m.ensure_chapter_mission_seed(
            "default",
            "Chapter-2",
            "让林墨追查玉佩下落。",
            "玉佩线索",
            "提前揭开真相",
            "以误导线索收束。",
            "test",
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose(
            "林墨没有揭开真相，只把玉佩重新收进袖中。",
            0,
            "Chapter-2",
            "default",
            &m,
        );
        assert!(!results
            .iter()
            .any(|result| matches!(result.category, DiagnosticCategory::ChapterMissionViolation)));
    }

    #[test]
    fn test_story_contract_negated_reveal_does_not_warn() {
        let m = test_memory();
        m.ensure_story_contract_seed(
            "default",
            "寒影录",
            "玄幻",
            "刀客追查玉佩真相。",
            "林墨必须在复仇和守护之间做选择。",
            "不得提前泄露玉佩来源。",
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose(
            "张三没有说出真相，也不肯解释玉佩来源。",
            0,
            "Chapter-2",
            "default",
            &m,
        );
        assert!(!results
            .iter()
            .any(|result| matches!(result.category, DiagnosticCategory::StoryContractViolation)));
    }

    #[test]
    fn test_timeline_dead_character_action_warns() {
        let m = test_memory();
        m.upsert_canon_entity(
            "character",
            "张三",
            &[],
            "上一章已死亡",
            &serde_json::json!({"status": "已死亡"}),
            0.9,
        )
        .unwrap();
        let engine = DiagnosticsEngine::new();
        let results = engine.diagnose(
            "张三推门而入，说道：“我回来了。”",
            0,
            "Chapter-5",
            "default",
            &m,
        );
        assert!(results
            .iter()
            .any(|result| matches!(result.category, DiagnosticCategory::TimelineIssue)));
    }

    #[test]
    fn test_pacing_warning_long_paragraph() {
        let m = test_memory();
        let engine = DiagnosticsEngine::new();
        let long = "x".repeat(2001);
        let results = engine.diagnose(&long, 0, "ch1", "default", &m);
        assert!(results
            .iter()
            .any(|r| matches!(r.category, DiagnosticCategory::PacingNote)));
    }
}
