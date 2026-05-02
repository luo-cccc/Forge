//! WriterMemory - structured creative ledgers.
//! Canon, promises, style preferences, creative decisions.
//! Ported from the plan's Creative Ledgers specification.

use rusqlite::{Connection, OptionalExtension, Result as SqlResult};
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: i64 = 10;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS canon_entities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL UNIQUE,
    aliases_json TEXT DEFAULT '[]',
    summary TEXT DEFAULT '',
    attributes_json TEXT DEFAULT '{}',
    confidence REAL DEFAULT 0.5,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS canon_facts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    source_ref TEXT DEFAULT '',
    confidence REAL DEFAULT 0.5,
    status TEXT DEFAULT 'active',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS canon_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL,
    priority INTEGER DEFAULT 0,
    source_ref TEXT DEFAULT '',
    status TEXT DEFAULT 'active',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS plot_promises (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT DEFAULT '',
    introduced_chapter TEXT DEFAULT '',
    introduced_ref TEXT DEFAULT '',
    last_seen_chapter TEXT DEFAULT '',
    last_seen_ref TEXT DEFAULT '',
    expected_payoff TEXT DEFAULT '',
    status TEXT DEFAULT 'open',
    priority INTEGER DEFAULT 0,
    risk_level TEXT DEFAULT 'medium',
    related_entities_json TEXT DEFAULT '[]',
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS style_preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL,
    evidence_ref TEXT DEFAULT '',
    confidence REAL DEFAULT 0.5,
    accepted_count INTEGER DEFAULT 0,
    rejected_count INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS creative_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT DEFAULT '',
    title TEXT NOT NULL,
    decision TEXT NOT NULL,
    alternatives_json TEXT DEFAULT '[]',
    rationale TEXT DEFAULT '',
    source_refs_json TEXT DEFAULT '[]',
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proposal_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL,
    action TEXT NOT NULL,
    final_text TEXT DEFAULT '',
    reason TEXT DEFAULT '',
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS story_contracts (
    project_id TEXT PRIMARY KEY,
    title TEXT DEFAULT '',
    genre TEXT DEFAULT '',
    target_reader TEXT DEFAULT '',
    reader_promise TEXT DEFAULT '',
    first_30_chapter_promise TEXT DEFAULT '',
    main_conflict TEXT DEFAULT '',
    structural_boundary TEXT DEFAULT '',
    tone_contract TEXT DEFAULT '',
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS chapter_missions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    chapter_title TEXT NOT NULL,
    mission TEXT DEFAULT '',
    must_include TEXT DEFAULT '',
    must_not TEXT DEFAULT '',
    expected_ending TEXT DEFAULT '',
    status TEXT DEFAULT 'draft',
    source_ref TEXT DEFAULT '',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    UNIQUE(project_id, chapter_title)
);

CREATE TABLE IF NOT EXISTS chapter_result_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    chapter_title TEXT NOT NULL,
    chapter_revision TEXT DEFAULT '',
    summary TEXT DEFAULT '',
    state_changes_json TEXT DEFAULT '[]',
    character_progress_json TEXT DEFAULT '[]',
    new_conflicts_json TEXT DEFAULT '[]',
    new_clues_json TEXT DEFAULT '[]',
    promise_updates_json TEXT DEFAULT '[]',
    canon_updates_json TEXT DEFAULT '[]',
    source_ref TEXT DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    action TEXT NOT NULL,
    title TEXT NOT NULL,
    evidence TEXT DEFAULT '',
    rationale TEXT DEFAULT '',
    reason TEXT DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS writer_observation_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observation_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    reason TEXT NOT NULL,
    chapter_title TEXT DEFAULT '',
    paragraph_snippet TEXT DEFAULT ''
);

CREATE TABLE IF NOT EXISTS writer_proposal_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL UNIQUE,
    observation_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    priority TEXT NOT NULL,
    state TEXT NOT NULL,
    confidence REAL DEFAULT 0.0,
    preview_snippet TEXT DEFAULT '',
    evidence_json TEXT DEFAULT '[]',
    context_budget_json TEXT DEFAULT '',
    created_at INTEGER NOT NULL,
    expires_at INTEGER
);

CREATE TABLE IF NOT EXISTS writer_feedback_trace (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL,
    action TEXT NOT NULL,
    reason TEXT DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS writer_context_recalls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    source TEXT NOT NULL,
    reference TEXT NOT NULL,
    snippet TEXT DEFAULT '',
    recall_count INTEGER DEFAULT 0,
    first_recalled_at INTEGER NOT NULL,
    last_recalled_at INTEGER NOT NULL,
    last_observation_id TEXT DEFAULT '',
    last_proposal_id TEXT DEFAULT '',
    UNIQUE(project_id, source, reference)
);

CREATE TABLE IF NOT EXISTS manual_agent_turns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    observation_id TEXT DEFAULT '',
    chapter_title TEXT DEFAULT '',
    user_text TEXT NOT NULL,
    assistant_text TEXT NOT NULL,
    source_refs_json TEXT DEFAULT '[]',
    created_at INTEGER NOT NULL
);
"#;

const INDEX_SCHEMA: &str = r#"
CREATE INDEX IF NOT EXISTS idx_canon_name ON canon_entities(name);
CREATE INDEX IF NOT EXISTS idx_canon_kind ON canon_entities(kind);
CREATE INDEX IF NOT EXISTS idx_canon_fact_entity ON canon_facts(entity_id);
CREATE INDEX IF NOT EXISTS idx_canon_fact_key ON canon_facts(key);
CREATE INDEX IF NOT EXISTS idx_canon_rule_category ON canon_rules(category);
CREATE INDEX IF NOT EXISTS idx_canon_rule_status ON canon_rules(status);
CREATE INDEX IF NOT EXISTS idx_promise_status ON plot_promises(status);
CREATE INDEX IF NOT EXISTS idx_feedback_proposal ON proposal_feedback(proposal_id);
CREATE INDEX IF NOT EXISTS idx_story_contract_updated_at ON story_contracts(updated_at);
CREATE INDEX IF NOT EXISTS idx_chapter_missions_project_chapter ON chapter_missions(project_id, chapter_title);
CREATE INDEX IF NOT EXISTS idx_chapter_missions_status ON chapter_missions(status);
CREATE INDEX IF NOT EXISTS idx_chapter_result_project_created ON chapter_result_snapshots(project_id, created_at);
CREATE INDEX IF NOT EXISTS idx_chapter_result_project_chapter ON chapter_result_snapshots(project_id, chapter_title);
CREATE INDEX IF NOT EXISTS idx_memory_audit_created_at ON memory_audit_events(created_at);
CREATE INDEX IF NOT EXISTS idx_observation_trace_created_at ON writer_observation_trace(created_at);
CREATE INDEX IF NOT EXISTS idx_proposal_trace_created_at ON writer_proposal_trace(created_at);
CREATE INDEX IF NOT EXISTS idx_proposal_trace_proposal_id ON writer_proposal_trace(proposal_id);
CREATE INDEX IF NOT EXISTS idx_feedback_trace_created_at ON writer_feedback_trace(created_at);
CREATE INDEX IF NOT EXISTS idx_context_recalls_project_last ON writer_context_recalls(project_id, last_recalled_at);
CREATE INDEX IF NOT EXISTS idx_context_recalls_project_count ON writer_context_recalls(project_id, recall_count);
CREATE INDEX IF NOT EXISTS idx_manual_agent_turns_project_created_at ON manual_agent_turns(project_id, created_at);
"#;

pub struct WriterMemory {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonEntitySummary {
    pub kind: String,
    pub name: String,
    pub summary: String,
    pub attributes: serde_json::Value,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonRuleSummary {
    pub rule: String,
    pub category: String,
    pub priority: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotPromiseSummary {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub introduced_chapter: String,
    pub last_seen_chapter: String,
    pub last_seen_ref: String,
    pub expected_payoff: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreativeDecisionSummary {
    pub scope: String,
    pub title: String,
    pub decision: String,
    pub rationale: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StylePreferenceSummary {
    pub key: String,
    pub value: String,
    pub accepted_count: i64,
    pub rejected_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromiseKind {
    PlotPromise,
    EmotionalDebt,
    ObjectWhereabouts,
    CharacterCommitment,
    MysteryClue,
    RelationshipTension,
    #[serde(other)]
    Other,
}

impl Default for PromiseKind {
    fn default() -> Self {
        PromiseKind::PlotPromise
    }
}

impl PromiseKind {
    pub fn from_kind_str(kind: &str) -> Self {
        match kind {
            "plot_promise" => PromiseKind::PlotPromise,
            "emotional_debt" => PromiseKind::EmotionalDebt,
            "object_whereabouts" => PromiseKind::ObjectWhereabouts,
            "character_commitment" => PromiseKind::CharacterCommitment,
            "mystery_clue" => PromiseKind::MysteryClue,
            "relationship_tension" => PromiseKind::RelationshipTension,
            _ => PromiseKind::Other,
        }
    }

    pub fn as_kind_str(&self) -> &'static str {
        match self {
            PromiseKind::PlotPromise => "plot_promise",
            PromiseKind::EmotionalDebt => "emotional_debt",
            PromiseKind::ObjectWhereabouts => "object_whereabouts",
            PromiseKind::CharacterCommitment => "character_commitment",
            PromiseKind::MysteryClue => "mystery_clue",
            PromiseKind::RelationshipTension => "relationship_tension",
            PromiseKind::Other => "other",
        }
    }

    pub fn default_risk(&self) -> &'static str {
        match self {
            PromiseKind::ObjectWhereabouts | PromiseKind::MysteryClue => "high",
            PromiseKind::RelationshipTension | PromiseKind::EmotionalDebt => "medium",
            PromiseKind::CharacterCommitment | PromiseKind::PlotPromise => "medium",
            PromiseKind::Other => "low",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoryContractSummary {
    pub project_id: String,
    pub title: String,
    pub genre: String,
    pub target_reader: String,
    pub reader_promise: String,
    pub first_30_chapter_promise: String,
    pub main_conflict: String,
    pub structural_boundary: String,
    pub tone_contract: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMissionSummary {
    pub id: i64,
    pub project_id: String,
    pub chapter_title: String,
    pub mission: String,
    pub must_include: String,
    pub must_not: String,
    pub expected_ending: String,
    pub status: String,
    pub source_ref: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChapterResultSummary {
    pub id: i64,
    pub project_id: String,
    pub chapter_title: String,
    pub chapter_revision: String,
    pub summary: String,
    pub state_changes: Vec<String>,
    pub character_progress: Vec<String>,
    pub new_conflicts: Vec<String>,
    pub new_clues: Vec<String>,
    pub promise_updates: Vec<String>,
    pub canon_updates: Vec<String>,
    pub source_ref: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextBeatSummary {
    pub chapter_title: String,
    pub goal: String,
    pub carryovers: Vec<String>,
    pub blockers: Vec<String>,
    pub source_refs: Vec<String>,
}

impl ChapterMissionSummary {
    pub fn is_empty(&self) -> bool {
        [
            &self.mission,
            &self.must_include,
            &self.must_not,
            &self.expected_ending,
        ]
        .iter()
        .all(|value| value.trim().is_empty())
    }

    pub fn render_for_context(&self) -> String {
        let mut lines = Vec::new();
        push_contract_line(&mut lines, "章节", &self.chapter_title);
        push_contract_line(&mut lines, "本章任务", &self.mission);
        push_contract_line(&mut lines, "必保事项", &self.must_include);
        push_contract_line(&mut lines, "禁止事项", &self.must_not);
        push_contract_line(&mut lines, "预期收束", &self.expected_ending);
        push_contract_line(&mut lines, "任务状态", &self.status);
        lines.join("\n")
    }
}

impl ChapterResultSummary {
    pub fn is_empty(&self) -> bool {
        self.summary.trim().is_empty()
            && self.state_changes.is_empty()
            && self.character_progress.is_empty()
            && self.new_conflicts.is_empty()
            && self.new_clues.is_empty()
            && self.promise_updates.is_empty()
            && self.canon_updates.is_empty()
    }

    pub fn render_for_context(&self) -> String {
        let mut lines = Vec::new();
        push_contract_line(&mut lines, "章节结果", &self.chapter_title);
        push_contract_line(&mut lines, "结果摘要", &self.summary);
        push_list_line(&mut lines, "状态变化", &self.state_changes);
        push_list_line(&mut lines, "角色推进", &self.character_progress);
        push_list_line(&mut lines, "新冲突", &self.new_conflicts);
        push_list_line(&mut lines, "新线索", &self.new_clues);
        push_list_line(&mut lines, "伏笔变化", &self.promise_updates);
        push_list_line(&mut lines, "设定变化", &self.canon_updates);
        push_contract_line(&mut lines, "来源", &self.source_ref);
        lines.join("\n")
    }
}

impl NextBeatSummary {
    pub fn is_empty(&self) -> bool {
        self.goal.trim().is_empty() && self.carryovers.is_empty() && self.blockers.is_empty()
    }

    pub fn render_for_context(&self) -> String {
        let mut lines = Vec::new();
        push_contract_line(&mut lines, "接力章节", &self.chapter_title);
        push_contract_line(&mut lines, "下一拍目标", &self.goal);
        push_list_line(&mut lines, "需要接住", &self.carryovers);
        push_list_line(&mut lines, "阻塞/风险", &self.blockers);
        push_list_line(&mut lines, "依据", &self.source_refs);
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryContractQuality {
    Missing,
    Vague,
    Usable,
    Strong,
}

impl StoryContractSummary {
    pub fn is_empty(&self) -> bool {
        [
            &self.title,
            &self.genre,
            &self.target_reader,
            &self.reader_promise,
            &self.first_30_chapter_promise,
            &self.main_conflict,
            &self.structural_boundary,
            &self.tone_contract,
        ]
        .iter()
        .all(|value| value.trim().is_empty())
    }

    pub fn quality(&self) -> StoryContractQuality {
        let reader_promise_len = self.reader_promise.trim().chars().count();
        let main_conflict_len = self.main_conflict.trim().chars().count();
        let tone_contract_len = self.tone_contract.trim().chars().count();
        let structural_boundary_len = self.structural_boundary.trim().chars().count();
        let first_30_len = self.first_30_chapter_promise.trim().chars().count();

        if self.is_empty() {
            return StoryContractQuality::Missing;
        }

        let weak_fields = [
            ("title", self.title.trim().is_empty(), 0),
            ("genre", self.genre.trim().is_empty(), 0),
            ("reader_promise", reader_promise_len < 8, reader_promise_len),
            ("main_conflict", main_conflict_len < 8, main_conflict_len),
            ("tone_contract", tone_contract_len < 6, tone_contract_len),
            (
                "structural_boundary",
                structural_boundary_len < 8,
                structural_boundary_len,
            ),
            ("first_30_chapter_promise", first_30_len < 8, first_30_len),
        ];
        let vague_count = weak_fields.iter().filter(|(_, weak, _)| *weak).count();

        if vague_count >= 3 {
            return StoryContractQuality::Vague;
        }

        if vague_count >= 1
            || reader_promise_len < 20
            || main_conflict_len < 20
            || tone_contract_len < 12
        {
            return StoryContractQuality::Usable;
        }

        StoryContractQuality::Strong
    }

    pub fn quality_gaps(&self) -> Vec<String> {
        let mut gaps = Vec::new();
        let quality = self.quality();

        if quality == StoryContractQuality::Strong {
            return gaps;
        }

        let checks: Vec<(&str, &str, usize, usize)> = vec![
            ("title", &self.title, 0, 1),
            ("genre", &self.genre, 0, 1),
            ("target_reader", &self.target_reader, 0, 1),
            ("reader_promise", &self.reader_promise, 8, 20),
            (
                "first_30_chapter_promise",
                &self.first_30_chapter_promise,
                8,
                12,
            ),
            ("main_conflict", &self.main_conflict, 8, 20),
            ("structural_boundary", &self.structural_boundary, 8, 12),
            ("tone_contract", &self.tone_contract, 6, 12),
        ];

        let labels: std::collections::HashMap<&str, &str> = [
            ("title", "标题"),
            ("genre", "题材定位"),
            ("target_reader", "目标读者"),
            ("reader_promise", "读者承诺"),
            ("first_30_chapter_promise", "前30章承诺"),
            ("main_conflict", "核心冲突"),
            ("structural_boundary", "结构边界"),
            ("tone_contract", "语气/风格合同"),
        ]
        .into();

        for (key, value, min_chars, strong_chars) in checks {
            let char_count = value.trim().chars().count();
            if char_count == 0 {
                gaps.push(format!("{}: 缺失", labels[key]));
            } else if char_count < min_chars {
                gaps.push(format!(
                    "{}: 过于简略 ({}字，至少需要{}字)",
                    labels[key], char_count, min_chars
                ));
            } else if quality == StoryContractQuality::Usable && char_count < strong_chars {
                gaps.push(format!(
                    "{}: 可以更具体 ({}字，建议{}字以上)",
                    labels[key], char_count, strong_chars
                ));
            }
        }

        gaps
    }

    pub fn render_for_context(&self) -> String {
        let mut lines = Vec::new();
        let quality = self.quality();
        lines.push(format!(
            "合同质量: {}",
            match quality {
                StoryContractQuality::Missing => "缺失 — 请在设置中填写故事合同",
                StoryContractQuality::Vague => "模糊 — 关键字段不够具体，会影响AI写作质量",
                StoryContractQuality::Usable => "可用",
                StoryContractQuality::Strong => "完整 — 所有关键约束清晰",
            }
        ));
        push_contract_line(&mut lines, "标题", &self.title);
        push_contract_line(&mut lines, "题材定位", &self.genre);
        push_contract_line(&mut lines, "目标读者", &self.target_reader);
        push_contract_line(&mut lines, "读者承诺", &self.reader_promise);
        push_contract_line(&mut lines, "前30章承诺", &self.first_30_chapter_promise);
        push_contract_line(&mut lines, "核心冲突", &self.main_conflict);
        push_contract_line(&mut lines, "结构边界", &self.structural_boundary);
        push_contract_line(&mut lines, "语气/风格合同", &self.tone_contract);
        lines.join("\n")
    }
}

fn push_contract_line(lines: &mut Vec<String>, label: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        lines.push(format!("{}: {}", label, value));
    }
}

fn push_list_line(lines: &mut Vec<String>, label: &str, values: &[String]) {
    let values = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .take(5)
        .collect::<Vec<_>>();
    if !values.is_empty() {
        lines.push(format!("{}: {}", label, values.join("；")));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAuditSummary {
    pub proposal_id: String,
    pub kind: String,
    pub action: String,
    pub title: String,
    pub evidence: String,
    pub rationale: String,
    pub reason: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone)]
pub struct ObservationTraceSummary {
    pub id: String,
    pub created_at: u64,
    pub reason: String,
    pub chapter_title: Option<String>,
    pub paragraph_snippet: String,
}

#[derive(Debug, Clone)]
pub struct ProposalTraceSummary {
    pub id: String,
    pub observation_id: String,
    pub kind: String,
    pub priority: String,
    pub state: String,
    pub confidence: f64,
    pub preview_snippet: String,
    pub evidence: Vec<super::proposal::EvidenceRef>,
    pub context_budget: Option<ContextBudgetTrace>,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextBudgetTrace {
    pub task: String,
    pub used: usize,
    pub total_budget: usize,
    pub wasted: usize,
    pub source_reports: Vec<ContextSourceBudgetTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextSourceBudgetTrace {
    pub source: String,
    pub requested: usize,
    pub provided: usize,
    pub truncated: bool,
    pub reason: String,
    pub truncation_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FeedbackTraceSummary {
    pub proposal_id: String,
    pub action: String,
    pub reason: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextRecallSummary {
    pub source: String,
    pub reference: String,
    pub snippet: String,
    pub recall_count: u64,
    pub first_recalled_at: u64,
    pub last_recalled_at: u64,
    pub last_observation_id: String,
    pub last_proposal_id: String,
}

#[derive(Debug, Clone)]
pub struct ManualAgentTurnSummary {
    pub project_id: String,
    pub observation_id: String,
    pub chapter_title: Option<String>,
    pub user: String,
    pub assistant: String,
    pub source_refs: Vec<String>,
    pub created_at: u64,
}

impl WriterMemory {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        initialize_schema(&conn)?;
        Ok(Self { conn })
    }

    // -- Canon Entities --

    pub fn upsert_canon_entity(
        &self,
        kind: &str,
        name: &str,
        aliases: &[String],
        summary: &str,
        attributes: &serde_json::Value,
        confidence: f64,
    ) -> rusqlite::Result<i64> {
        let aliases_json = serde_json::to_string(aliases).unwrap();
        let attrs_json = attributes.to_string();
        self.conn.execute(
            "INSERT INTO canon_entities (kind, name, aliases_json, summary, attributes_json, confidence, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(name) DO UPDATE SET
                kind=excluded.kind,
                aliases_json=excluded.aliases_json,
                summary=excluded.summary,
                attributes_json=excluded.attributes_json,
                confidence=excluded.confidence,
                updated_at=datetime('now')",
            rusqlite::params![kind, name, aliases_json, summary, attrs_json, confidence],
        )?;

        let entity_id: i64 = self.conn.query_row(
            "SELECT id FROM canon_entities WHERE name=?1",
            rusqlite::params![name],
            |row| row.get(0),
        )?;

        if let Some(map) = attributes.as_object() {
            for (key, value) in map {
                let fact_value = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => continue,
                    other => other.to_string(),
                };
                self.conn.execute(
                    "DELETE FROM canon_facts WHERE entity_id=?1 AND key=?2",
                    rusqlite::params![entity_id, key],
                )?;
                self.conn.execute(
                    "INSERT INTO canon_facts (entity_id, key, value, source_ref, confidence, status, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'active', datetime('now'))",
                    rusqlite::params![entity_id, key, fact_value, "canon_entity.attributes", confidence],
                )?;
            }
        }

        Ok(entity_id)
    }

    pub fn get_canon_facts_for_entity(
        &self,
        entity_name: &str,
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let resolved_name = self
            .resolve_canon_entity_name(entity_name)?
            .unwrap_or_else(|| entity_name.to_string());
        let mut stmt = self.conn.prepare(
            "SELECT f.key, f.value FROM canon_facts f
             JOIN canon_entities e ON f.entity_id = e.id
             WHERE e.name = ?1 AND f.status = 'active'",
        )?;
        let rows = stmt.query_map(rusqlite::params![resolved_name], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.collect()
    }

    pub fn update_canon_attribute(
        &self,
        entity_name: &str,
        attribute: &str,
        value: &str,
        confidence: f64,
    ) -> rusqlite::Result<()> {
        let Some(resolved_name) = self.resolve_canon_entity_name(entity_name)? else {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        };

        let (entity_id, attributes_json): (i64, String) = self.conn.query_row(
            "SELECT id, attributes_json FROM canon_entities WHERE name=?1",
            rusqlite::params![resolved_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut attributes =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&attributes_json)
                .unwrap_or_default();
        attributes.insert(
            attribute.to_string(),
            serde_json::Value::String(value.to_string()),
        );
        self.conn.execute(
            "UPDATE canon_entities SET attributes_json=?1, confidence=?2, updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![serde_json::Value::Object(attributes).to_string(), confidence, entity_id],
        )?;
        self.conn.execute(
            "DELETE FROM canon_facts WHERE entity_id=?1 AND key=?2",
            rusqlite::params![entity_id, attribute],
        )?;
        self.conn.execute(
            "INSERT INTO canon_facts (entity_id, key, value, source_ref, confidence, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', datetime('now'))",
            rusqlite::params![entity_id, attribute, value, "canon.update_attribute", confidence],
        )?;
        Ok(())
    }

    pub fn resolve_canon_entity_name(&self, entity_name: &str) -> rusqlite::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, aliases_json FROM canon_entities ORDER BY length(name) DESC")?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let aliases_json: String = row.get(1)?;
            Ok((name, aliases_json))
        })?;

        for row in rows {
            let (name, aliases_json) = row?;
            if name == entity_name {
                return Ok(Some(name));
            }
            if let Ok(aliases) = serde_json::from_str::<Vec<String>>(&aliases_json) {
                if aliases.iter().any(|alias| alias == entity_name) {
                    return Ok(Some(name));
                }
            }
        }
        Ok(None)
    }

    pub fn get_canon_entity_names(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, aliases_json FROM canon_entities ORDER BY length(name) DESC")?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let aliases_json: String = row.get(1)?;
            Ok((name, aliases_json))
        })?;

        let mut names = Vec::new();
        for row in rows {
            let (name, aliases_json) = row?;
            if !name.trim().is_empty() {
                names.push(name);
            }
            if let Ok(aliases) = serde_json::from_str::<Vec<String>>(&aliases_json) {
                for alias in aliases {
                    if !alias.trim().is_empty() && !names.contains(&alias) {
                        names.push(alias);
                    }
                }
            }
        }
        Ok(names)
    }

    pub fn list_canon_entities(&self) -> rusqlite::Result<Vec<CanonEntitySummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, name, summary, attributes_json, confidence
             FROM canon_entities ORDER BY updated_at DESC, name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let attributes_json: String = row.get(3)?;
            let attributes = serde_json::from_str(&attributes_json)
                .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
            Ok(CanonEntitySummary {
                kind: row.get(0)?,
                name: row.get(1)?,
                summary: row.get(2)?,
                attributes,
                confidence: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_canon_rule(
        &self,
        rule: &str,
        category: &str,
        priority: i32,
        source_ref: &str,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO canon_rules (rule, category, priority, source_ref, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'active', datetime('now'))
             ON CONFLICT(rule) DO UPDATE SET
                category=excluded.category,
                priority=excluded.priority,
                source_ref=excluded.source_ref,
                status='active',
                updated_at=datetime('now')",
            rusqlite::params![rule, category, priority, source_ref],
        )?;
        self.conn.query_row(
            "SELECT id FROM canon_rules WHERE rule=?1",
            rusqlite::params![rule],
            |row| row.get(0),
        )
    }

    pub fn list_canon_rules(&self, limit: usize) -> rusqlite::Result<Vec<CanonRuleSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT rule, category, priority, status
             FROM canon_rules
             WHERE status = 'active'
             ORDER BY priority DESC, updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(CanonRuleSummary {
                rule: row.get(0)?,
                category: row.get(1)?,
                priority: row.get(2)?,
                status: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // -- Plot Promises --

    pub fn add_promise(
        &self,
        kind: &str,
        title: &str,
        description: &str,
        chapter: &str,
        payoff: &str,
        priority: i32,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO plot_promises
             (kind, title, description, introduced_chapter, introduced_ref, last_seen_chapter,
              last_seen_ref, expected_payoff, priority, related_entities_json)
             VALUES (?1, ?2, ?3, ?4, '', ?4, '', ?5, ?6, '[]')",
            rusqlite::params![kind, title, description, chapter, payoff, priority],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_promise_with_entities(
        &self,
        kind: &str,
        title: &str,
        description: &str,
        chapter: &str,
        payoff: &str,
        priority: i32,
        related_entities: &[String],
    ) -> rusqlite::Result<i64> {
        let entities_json = serde_json::to_string(related_entities).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO plot_promises
             (kind, title, description, introduced_chapter, introduced_ref, last_seen_chapter,
              last_seen_ref, expected_payoff, priority, related_entities_json)
             VALUES (?1, ?2, ?3, ?4, '', ?4, '', ?5, ?6, ?7)",
            rusqlite::params![
                kind,
                title,
                description,
                chapter,
                payoff,
                priority,
                entities_json
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_open_promises(&self) -> rusqlite::Result<Vec<(String, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, title, description, introduced_chapter FROM plot_promises
             WHERE status = 'open' ORDER BY priority DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect()
    }

    pub fn get_open_promise_summaries(&self) -> rusqlite::Result<Vec<PlotPromiseSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, title, description, introduced_chapter,
                    last_seen_chapter, last_seen_ref, expected_payoff, priority
             FROM plot_promises WHERE status = 'open' ORDER BY priority DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PlotPromiseSummary {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                introduced_chapter: row.get(4)?,
                last_seen_chapter: row.get(5)?,
                last_seen_ref: row.get(6)?,
                expected_payoff: row.get(7)?,
                priority: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn touch_promise_last_seen(
        &self,
        promise_id: i64,
        chapter: &str,
        source_ref: &str,
    ) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises
             SET last_seen_chapter=?1, last_seen_ref=?2
             WHERE id=?3 AND status='open'",
            rusqlite::params![chapter, source_ref, promise_id],
        )?;
        Ok(changed > 0)
    }

    pub fn resolve_promise(&self, promise_id: i64, _chapter: &str) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises SET status='resolved' WHERE id=?1 AND status='open'",
            rusqlite::params![promise_id],
        )?;
        Ok(changed > 0)
    }

    pub fn defer_promise(&self, promise_id: i64, expected_payoff: &str) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises SET expected_payoff=?1 WHERE id=?2 AND status='open'",
            rusqlite::params![expected_payoff, promise_id],
        )?;
        Ok(changed > 0)
    }

    pub fn abandon_promise(&self, promise_id: i64) -> rusqlite::Result<bool> {
        let changed = self.conn.execute(
            "UPDATE plot_promises SET status='abandoned' WHERE id=?1 AND status='open'",
            rusqlite::params![promise_id],
        )?;
        Ok(changed > 0)
    }

    // -- Style Preferences --

    pub fn upsert_style_preference(
        &self,
        key: &str,
        value: &str,
        accepted: bool,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO style_preferences (key, value, accepted_count, rejected_count, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
             accepted_count = accepted_count + ?3,
             rejected_count = rejected_count + ?4,
             updated_at = datetime('now')",
            rusqlite::params![
                key,
                value,
                if accepted { 1 } else { 0 },
                if accepted { 0 } else { 1 }
            ],
        )?;
        Ok(())
    }

    pub fn list_style_preferences(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<StylePreferenceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, value, accepted_count, rejected_count
             FROM style_preferences
             ORDER BY accepted_count DESC, updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(StylePreferenceSummary {
                key: row.get(0)?,
                value: row.get(1)?,
                accepted_count: row.get(2)?,
                rejected_count: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // -- Story Contract --

    pub fn upsert_story_contract(&self, contract: &StoryContractSummary) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO story_contracts
             (project_id, title, genre, target_reader, reader_promise, first_30_chapter_promise,
              main_conflict, structural_boundary, tone_contract, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
             ON CONFLICT(project_id) DO UPDATE SET
                title=excluded.title,
                genre=excluded.genre,
                target_reader=excluded.target_reader,
                reader_promise=excluded.reader_promise,
                first_30_chapter_promise=excluded.first_30_chapter_promise,
                main_conflict=excluded.main_conflict,
                structural_boundary=excluded.structural_boundary,
                tone_contract=excluded.tone_contract,
                updated_at=datetime('now')",
            rusqlite::params![
                contract.project_id,
                contract.title,
                contract.genre,
                contract.target_reader,
                contract.reader_promise,
                contract.first_30_chapter_promise,
                contract.main_conflict,
                contract.structural_boundary,
                contract.tone_contract,
            ],
        )?;
        Ok(())
    }

    pub fn get_story_contract(
        &self,
        project_id: &str,
    ) -> rusqlite::Result<Option<StoryContractSummary>> {
        self.conn
            .query_row(
                "SELECT project_id, title, genre, target_reader, reader_promise,
                        first_30_chapter_promise, main_conflict, structural_boundary,
                        tone_contract, updated_at
                 FROM story_contracts WHERE project_id=?1",
                rusqlite::params![project_id],
                |row| {
                    Ok(StoryContractSummary {
                        project_id: row.get(0)?,
                        title: row.get(1)?,
                        genre: row.get(2)?,
                        target_reader: row.get(3)?,
                        reader_promise: row.get(4)?,
                        first_30_chapter_promise: row.get(5)?,
                        main_conflict: row.get(6)?,
                        structural_boundary: row.get(7)?,
                        tone_contract: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()
    }

    pub fn ensure_story_contract_seed(
        &self,
        project_id: &str,
        title: &str,
        genre: &str,
        reader_promise: &str,
        main_conflict: &str,
        structural_boundary: &str,
    ) -> rusqlite::Result<bool> {
        if self.get_story_contract(project_id)?.is_some() {
            return Ok(false);
        }
        let contract = StoryContractSummary {
            project_id: project_id.to_string(),
            title: title.to_string(),
            genre: genre.to_string(),
            target_reader: String::new(),
            reader_promise: reader_promise.to_string(),
            first_30_chapter_promise: String::new(),
            main_conflict: main_conflict.to_string(),
            structural_boundary: structural_boundary.to_string(),
            tone_contract: String::new(),
            updated_at: String::new(),
        };
        self.upsert_story_contract(&contract)?;
        Ok(true)
    }

    // -- Chapter Mission --

    pub fn upsert_chapter_mission(&self, mission: &ChapterMissionSummary) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO chapter_missions
             (project_id, chapter_title, mission, must_include, must_not, expected_ending,
              status, source_ref, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
             ON CONFLICT(project_id, chapter_title) DO UPDATE SET
                mission=excluded.mission,
                must_include=excluded.must_include,
                must_not=excluded.must_not,
                expected_ending=excluded.expected_ending,
                status=excluded.status,
                source_ref=excluded.source_ref,
                updated_at=datetime('now')",
            rusqlite::params![
                mission.project_id,
                mission.chapter_title,
                mission.mission,
                mission.must_include,
                mission.must_not,
                mission.expected_ending,
                mission.status,
                mission.source_ref,
            ],
        )?;
        self.conn.query_row(
            "SELECT id FROM chapter_missions WHERE project_id=?1 AND chapter_title=?2",
            rusqlite::params![mission.project_id, mission.chapter_title],
            |row| row.get(0),
        )
    }

    pub fn get_chapter_mission(
        &self,
        project_id: &str,
        chapter_title: &str,
    ) -> rusqlite::Result<Option<ChapterMissionSummary>> {
        self.conn
            .query_row(
                "SELECT id, project_id, chapter_title, mission, must_include, must_not,
                        expected_ending, status, source_ref, updated_at
                 FROM chapter_missions WHERE project_id=?1 AND chapter_title=?2",
                rusqlite::params![project_id, chapter_title],
                chapter_mission_from_row,
            )
            .optional()
    }

    pub fn list_chapter_missions(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ChapterMissionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, chapter_title, mission, must_include, must_not,
                    expected_ending, status, source_ref, updated_at
             FROM chapter_missions
             WHERE project_id=?1
             ORDER BY id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            chapter_mission_from_row(row)
        })?;
        rows.collect()
    }

    pub fn ensure_chapter_mission_seed(
        &self,
        project_id: &str,
        chapter_title: &str,
        mission: &str,
        must_include: &str,
        must_not: &str,
        expected_ending: &str,
        source_ref: &str,
    ) -> rusqlite::Result<bool> {
        if self
            .get_chapter_mission(project_id, chapter_title)?
            .is_some()
        {
            return Ok(false);
        }
        let summary = ChapterMissionSummary {
            id: 0,
            project_id: project_id.to_string(),
            chapter_title: chapter_title.to_string(),
            mission: mission.to_string(),
            must_include: must_include.to_string(),
            must_not: must_not.to_string(),
            expected_ending: expected_ending.to_string(),
            status: "draft".to_string(),
            source_ref: source_ref.to_string(),
            updated_at: String::new(),
        };
        self.upsert_chapter_mission(&summary)?;
        Ok(true)
    }

    // -- Chapter Result Snapshots --

    pub fn record_chapter_result(&self, result: &ChapterResultSummary) -> rusqlite::Result<i64> {
        if !result.chapter_revision.trim().is_empty() {
            if let Some(existing_id) = self
                .conn
                .query_row(
                    "SELECT id FROM chapter_result_snapshots
                     WHERE project_id=?1 AND chapter_title=?2 AND chapter_revision=?3
                     ORDER BY created_at DESC, id DESC
                     LIMIT 1",
                    rusqlite::params![
                        result.project_id,
                        result.chapter_title,
                        result.chapter_revision
                    ],
                    |row| row.get(0),
                )
                .optional()?
            {
                return Ok(existing_id);
            }
        }

        self.conn.execute(
            "INSERT INTO chapter_result_snapshots
             (project_id, chapter_title, chapter_revision, summary, state_changes_json,
              character_progress_json, new_conflicts_json, new_clues_json, promise_updates_json,
              canon_updates_json, source_ref, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                result.project_id,
                result.chapter_title,
                result.chapter_revision,
                result.summary,
                string_vec_json(&result.state_changes),
                string_vec_json(&result.character_progress),
                string_vec_json(&result.new_conflicts),
                string_vec_json(&result.new_clues),
                string_vec_json(&result.promise_updates),
                string_vec_json(&result.canon_updates),
                result.source_ref,
                result.created_at as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_recent_chapter_results(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ChapterResultSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, chapter_title, chapter_revision, summary,
                    state_changes_json, character_progress_json, new_conflicts_json,
                    new_clues_json, promise_updates_json, canon_updates_json,
                    source_ref, created_at
             FROM chapter_result_snapshots
             WHERE project_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            chapter_result_from_row(row)
        })?;
        rows.collect()
    }

    pub fn latest_chapter_result(
        &self,
        project_id: &str,
        chapter_title: &str,
    ) -> rusqlite::Result<Option<ChapterResultSummary>> {
        self.conn
            .query_row(
                "SELECT id, project_id, chapter_title, chapter_revision, summary,
                        state_changes_json, character_progress_json, new_conflicts_json,
                        new_clues_json, promise_updates_json, canon_updates_json,
                        source_ref, created_at
                 FROM chapter_result_snapshots
                 WHERE project_id = ?1 AND chapter_title = ?2
                 ORDER BY created_at DESC, id DESC
                 LIMIT 1",
                rusqlite::params![project_id, chapter_title],
                chapter_result_from_row,
            )
            .optional()
    }

    // -- Creative Decisions --

    pub fn record_decision(
        &self,
        scope: &str,
        title: &str,
        decision: &str,
        alternatives: &[String],
        rationale: &str,
        sources: &[String],
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO creative_decisions (scope, title, decision, alternatives_json, rationale, source_refs_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![scope, title, decision,
                serde_json::to_string(alternatives).unwrap(),
                rationale,
                serde_json::to_string(sources).unwrap()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_recent_decisions(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<CreativeDecisionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT scope, title, decision, rationale, created_at
             FROM creative_decisions ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(CreativeDecisionSummary {
                scope: row.get(0)?,
                title: row.get(1)?,
                decision: row.get(2)?,
                rationale: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn record_manual_agent_turn(&self, turn: &ManualAgentTurnSummary) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO manual_agent_turns
             (project_id, observation_id, chapter_title, user_text, assistant_text, source_refs_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                turn.project_id,
                turn.observation_id,
                turn.chapter_title.clone().unwrap_or_default(),
                turn.user,
                turn.assistant,
                serde_json::to_string(&turn.source_refs).unwrap_or_else(|_| "[]".to_string()),
                turn.created_at as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_manual_agent_turns(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ManualAgentTurnSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT project_id, observation_id, chapter_title, user_text, assistant_text, source_refs_json, created_at
             FROM manual_agent_turns
             WHERE project_id = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            let chapter_title: String = row.get(2)?;
            let source_refs_json: String = row.get(5)?;
            let source_refs =
                serde_json::from_str::<Vec<String>>(&source_refs_json).unwrap_or_default();
            let created_at: i64 = row.get(6)?;
            Ok(ManualAgentTurnSummary {
                project_id: row.get(0)?,
                observation_id: row.get(1)?,
                chapter_title: if chapter_title.trim().is_empty() {
                    None
                } else {
                    Some(chapter_title)
                },
                user: row.get(3)?,
                assistant: row.get(4)?,
                source_refs,
                created_at: created_at.max(0) as u64,
            })
        })?;
        let mut turns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        turns.reverse();
        Ok(turns)
    }

    // -- Feedback --

    pub fn record_feedback(
        &self,
        proposal_id: &str,
        action: &str,
        reason: &str,
        final_text: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO proposal_feedback (proposal_id, action, reason, final_text) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![proposal_id, action, reason, final_text],
        )?;
        Ok(())
    }

    pub fn record_memory_audit(&self, entry: &MemoryAuditSummary) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO memory_audit_events
             (proposal_id, kind, action, title, evidence, rationale, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                entry.proposal_id,
                entry.kind,
                entry.action,
                entry.title,
                entry.evidence,
                entry.rationale,
                entry.reason.clone().unwrap_or_default(),
                entry.created_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_memory_audit(&self, limit: usize) -> rusqlite::Result<Vec<MemoryAuditSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT proposal_id, kind, action, title, evidence, rationale, reason, created_at
             FROM memory_audit_events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let reason: String = row.get(6)?;
            let created_at: i64 = row.get(7)?;
            Ok(MemoryAuditSummary {
                proposal_id: row.get(0)?,
                kind: row.get(1)?,
                action: row.get(2)?,
                title: row.get(3)?,
                evidence: row.get(4)?,
                rationale: row.get(5)?,
                reason: if reason.trim().is_empty() {
                    None
                } else {
                    Some(reason)
                },
                created_at: created_at.max(0) as u64,
            })
        })?;
        rows.collect()
    }

    // -- Writer Agent Trace --

    pub fn record_observation_trace(
        &self,
        id: &str,
        created_at: u64,
        reason: &str,
        chapter_title: Option<&str>,
        paragraph_snippet: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_observation_trace
             (observation_id, created_at, reason, chapter_title, paragraph_snippet)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id,
                created_at as i64,
                reason,
                chapter_title.unwrap_or(""),
                paragraph_snippet,
            ],
        )?;
        Ok(())
    }

    pub fn record_proposal_trace(
        &self,
        proposal: &ProposalTraceSummary,
        created_at: u64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_proposal_trace
             (proposal_id, observation_id, kind, priority, state, confidence, preview_snippet, evidence_json, context_budget_json, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(proposal_id) DO UPDATE SET
                observation_id=excluded.observation_id,
                kind=excluded.kind,
                priority=excluded.priority,
                state=excluded.state,
                confidence=excluded.confidence,
                preview_snippet=excluded.preview_snippet,
                evidence_json=excluded.evidence_json,
                context_budget_json=excluded.context_budget_json,
                created_at=excluded.created_at,
                expires_at=excluded.expires_at",
            rusqlite::params![
                proposal.id,
                proposal.observation_id,
                proposal.kind,
                proposal.priority,
                proposal.state,
                proposal.confidence,
                proposal.preview_snippet,
                serde_json::to_string(&proposal.evidence).unwrap_or_else(|_| "[]".to_string()),
                proposal
                    .context_budget
                    .as_ref()
                    .and_then(|budget| serde_json::to_string(budget).ok())
                    .unwrap_or_default(),
                created_at as i64,
                proposal.expires_at.map(|value| value as i64),
            ],
        )?;
        Ok(())
    }

    pub fn update_proposal_trace_state(
        &self,
        proposal_id: &str,
        state: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE writer_proposal_trace SET state=?1 WHERE proposal_id=?2",
            rusqlite::params![state, proposal_id],
        )?;
        Ok(())
    }

    pub fn record_feedback_trace(&self, feedback: &FeedbackTraceSummary) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO writer_feedback_trace (proposal_id, action, reason, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                feedback.proposal_id,
                feedback.action,
                feedback.reason.clone().unwrap_or_default(),
                feedback.created_at as i64,
            ],
        )?;
        Ok(())
    }

    pub fn record_context_recalls(
        &self,
        project_id: &str,
        proposal_id: &str,
        observation_id: &str,
        evidence: &[super::proposal::EvidenceRef],
        recalled_at: u64,
    ) -> rusqlite::Result<()> {
        for evidence in evidence
            .iter()
            .filter(|entry| !entry.reference.trim().is_empty() || !entry.snippet.trim().is_empty())
        {
            let source = format!("{:?}", evidence.source);
            let reference = if evidence.reference.trim().is_empty() {
                snippet_for_storage(&evidence.snippet, 80)
            } else {
                evidence.reference.trim().to_string()
            };
            let snippet = snippet_for_storage(&evidence.snippet, 240);
            self.conn.execute(
                "INSERT INTO writer_context_recalls
                 (project_id, source, reference, snippet, recall_count, first_recalled_at, last_recalled_at, last_observation_id, last_proposal_id)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5, ?6, ?7)
                 ON CONFLICT(project_id, source, reference) DO UPDATE SET
                    snippet=excluded.snippet,
                    recall_count=writer_context_recalls.recall_count + 1,
                    last_recalled_at=excluded.last_recalled_at,
                    last_observation_id=excluded.last_observation_id,
                    last_proposal_id=excluded.last_proposal_id",
                rusqlite::params![
                    project_id,
                    source,
                    reference,
                    snippet,
                    recalled_at as i64,
                    observation_id,
                    proposal_id,
                ],
            )?;
        }
        Ok(())
    }

    pub fn list_context_recalls(
        &self,
        project_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<ContextRecallSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, reference, snippet, recall_count, first_recalled_at, last_recalled_at, last_observation_id, last_proposal_id
             FROM writer_context_recalls
             WHERE project_id=?1
             ORDER BY recall_count DESC, last_recalled_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![project_id, limit as i64], |row| {
            let recall_count: i64 = row.get(3)?;
            let first_recalled_at: i64 = row.get(4)?;
            let last_recalled_at: i64 = row.get(5)?;
            Ok(ContextRecallSummary {
                source: row.get(0)?,
                reference: row.get(1)?,
                snippet: row.get(2)?,
                recall_count: recall_count.max(0) as u64,
                first_recalled_at: first_recalled_at.max(0) as u64,
                last_recalled_at: last_recalled_at.max(0) as u64,
                last_observation_id: row.get(6)?,
                last_proposal_id: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_observation_traces(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<ObservationTraceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT observation_id, created_at, reason, chapter_title, paragraph_snippet
             FROM writer_observation_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let chapter_title: String = row.get(3)?;
            let created_at: i64 = row.get(1)?;
            Ok(ObservationTraceSummary {
                id: row.get(0)?,
                created_at: created_at.max(0) as u64,
                reason: row.get(2)?,
                chapter_title: if chapter_title.trim().is_empty() {
                    None
                } else {
                    Some(chapter_title)
                },
                paragraph_snippet: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_proposal_traces(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<ProposalTraceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT proposal_id, observation_id, kind, priority, state, confidence, preview_snippet, evidence_json, context_budget_json, expires_at
             FROM writer_proposal_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let evidence_json: String = row.get(7)?;
            let context_budget_json: String = row.get(8)?;
            let context_budget = if context_budget_json.trim().is_empty() {
                None
            } else {
                serde_json::from_str::<ContextBudgetTrace>(&context_budget_json).ok()
            };
            let expires_at: Option<i64> = row.get(9)?;
            Ok(ProposalTraceSummary {
                id: row.get(0)?,
                observation_id: row.get(1)?,
                kind: row.get(2)?,
                priority: row.get(3)?,
                state: row.get(4)?,
                confidence: row.get(5)?,
                preview_snippet: row.get(6)?,
                evidence: serde_json::from_str::<Vec<super::proposal::EvidenceRef>>(&evidence_json)
                    .unwrap_or_default(),
                context_budget,
                expires_at: expires_at.map(|value| value.max(0) as u64),
            })
        })?;
        rows.collect()
    }

    pub fn list_feedback_traces(
        &self,
        limit: usize,
    ) -> rusqlite::Result<Vec<FeedbackTraceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT proposal_id, action, reason, created_at
             FROM writer_feedback_trace ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let reason: String = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            Ok(FeedbackTraceSummary {
                proposal_id: row.get(0)?,
                action: row.get(1)?,
                reason: if reason.trim().is_empty() {
                    None
                } else {
                    Some(reason)
                },
                created_at: created_at.max(0) as u64,
            })
        })?;
        rows.collect()
    }

    #[cfg(test)]
    pub fn feedback_stats(&self, proposal_id: &str) -> rusqlite::Result<(i64, i64)> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*) FILTER(WHERE action='accepted'), COUNT(*) FILTER(WHERE action='rejected')
             FROM proposal_feedback WHERE proposal_id=?1"
        )?;
        let (accepted, rejected) = stmt.query_row(rusqlite::params![proposal_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok((accepted, rejected))
    }
}

fn initialize_schema(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(SCHEMA)?;
    migrate_writer_memory_schema(conn)?;
    conn.execute_batch(INDEX_SCHEMA)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

fn migrate_writer_memory_schema(conn: &Connection) -> SqlResult<()> {
    ensure_column(
        conn,
        "canon_entities",
        "aliases_json",
        "aliases_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(conn, "canon_entities", "summary", "summary TEXT DEFAULT ''")?;
    ensure_column(
        conn,
        "canon_entities",
        "attributes_json",
        "attributes_json TEXT DEFAULT '{}'",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "confidence",
        "confidence REAL DEFAULT 0.5",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_entities",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "canon_entities", "created_at")?;
    backfill_empty_timestamp(conn, "canon_entities", "updated_at")?;

    ensure_column(
        conn,
        "canon_facts",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "confidence",
        "confidence REAL DEFAULT 0.5",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "status",
        "status TEXT DEFAULT 'active'",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_facts",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "canon_facts", "created_at")?;
    backfill_empty_timestamp(conn, "canon_facts", "updated_at")?;

    ensure_column(
        conn,
        "canon_rules",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_rules",
        "status",
        "status TEXT DEFAULT 'active'",
    )?;
    ensure_column(
        conn,
        "canon_rules",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "canon_rules",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "canon_rules", "created_at")?;
    backfill_empty_timestamp(conn, "canon_rules", "updated_at")?;

    ensure_column(
        conn,
        "plot_promises",
        "description",
        "description TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "introduced_chapter",
        "introduced_chapter TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "introduced_ref",
        "introduced_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "last_seen_chapter",
        "last_seen_chapter TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "last_seen_ref",
        "last_seen_ref TEXT DEFAULT ''",
    )?;
    conn.execute_batch(
        "UPDATE plot_promises
         SET last_seen_chapter=introduced_chapter
         WHERE last_seen_chapter IS NULL OR last_seen_chapter='';
         UPDATE plot_promises
         SET last_seen_ref=introduced_ref
         WHERE last_seen_ref IS NULL OR last_seen_ref='';",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "expected_payoff",
        "expected_payoff TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "status",
        "status TEXT DEFAULT 'open'",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "priority",
        "priority INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "plot_promises", "created_at")?;

    ensure_column(
        conn,
        "style_preferences",
        "evidence_ref",
        "evidence_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "confidence",
        "confidence REAL DEFAULT 0.5",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "accepted_count",
        "accepted_count INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "rejected_count",
        "rejected_count INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "style_preferences",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "style_preferences", "updated_at")?;

    ensure_column(conn, "creative_decisions", "scope", "scope TEXT DEFAULT ''")?;
    ensure_column(
        conn,
        "creative_decisions",
        "alternatives_json",
        "alternatives_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "creative_decisions",
        "rationale",
        "rationale TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "creative_decisions",
        "source_refs_json",
        "source_refs_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "creative_decisions",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "creative_decisions", "created_at")?;

    ensure_column(
        conn,
        "proposal_feedback",
        "final_text",
        "final_text TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "proposal_feedback",
        "reason",
        "reason TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "proposal_feedback",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "proposal_feedback", "created_at")?;

    ensure_column(
        conn,
        "writer_proposal_trace",
        "evidence_json",
        "evidence_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "writer_proposal_trace",
        "context_budget_json",
        "context_budget_json TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "writer_proposal_trace",
        "expires_at",
        "expires_at INTEGER",
    )?;
    ensure_column(
        conn,
        "writer_context_recalls",
        "snippet",
        "snippet TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "writer_context_recalls",
        "last_observation_id",
        "last_observation_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "writer_context_recalls",
        "last_proposal_id",
        "last_proposal_id TEXT DEFAULT ''",
    )?;

    ensure_column(
        conn,
        "manual_agent_turns",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "observation_id",
        "observation_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "chapter_title",
        "chapter_title TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "user_text",
        "user_text TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "assistant_text",
        "assistant_text TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "source_refs_json",
        "source_refs_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "manual_agent_turns",
        "created_at",
        "created_at INTEGER DEFAULT 0",
    )?;

    ensure_column(
        conn,
        "story_contracts",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(conn, "story_contracts", "title", "title TEXT DEFAULT ''")?;
    ensure_column(conn, "story_contracts", "genre", "genre TEXT DEFAULT ''")?;
    ensure_column(
        conn,
        "story_contracts",
        "target_reader",
        "target_reader TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "reader_promise",
        "reader_promise TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "first_30_chapter_promise",
        "first_30_chapter_promise TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "main_conflict",
        "main_conflict TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "structural_boundary",
        "structural_boundary TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "tone_contract",
        "tone_contract TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "story_contracts",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "story_contracts", "updated_at")?;

    ensure_column(
        conn,
        "chapter_missions",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "chapter_title",
        "chapter_title TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "mission",
        "mission TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "must_include",
        "must_include TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "must_not",
        "must_not TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "expected_ending",
        "expected_ending TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "status",
        "status TEXT DEFAULT 'draft'",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "created_at",
        "created_at TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_missions",
        "updated_at",
        "updated_at TEXT DEFAULT ''",
    )?;
    backfill_empty_timestamp(conn, "chapter_missions", "created_at")?;
    backfill_empty_timestamp(conn, "chapter_missions", "updated_at")?;

    ensure_column(
        conn,
        "chapter_result_snapshots",
        "project_id",
        "project_id TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "chapter_title",
        "chapter_title TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "chapter_revision",
        "chapter_revision TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "summary",
        "summary TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "state_changes_json",
        "state_changes_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "character_progress_json",
        "character_progress_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "new_conflicts_json",
        "new_conflicts_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "new_clues_json",
        "new_clues_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "promise_updates_json",
        "promise_updates_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "canon_updates_json",
        "canon_updates_json TEXT DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "source_ref",
        "source_ref TEXT DEFAULT ''",
    )?;
    ensure_column(
        conn,
        "chapter_result_snapshots",
        "created_at",
        "created_at INTEGER DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "risk_level",
        "risk_level TEXT DEFAULT 'medium'",
    )?;
    ensure_column(
        conn,
        "plot_promises",
        "related_entities_json",
        "related_entities_json TEXT DEFAULT '[]'",
    )?;

    Ok(())
}

fn chapter_mission_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChapterMissionSummary> {
    Ok(ChapterMissionSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_title: row.get(2)?,
        mission: row.get(3)?,
        must_include: row.get(4)?,
        must_not: row.get(5)?,
        expected_ending: row.get(6)?,
        status: row.get(7)?,
        source_ref: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn chapter_result_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChapterResultSummary> {
    let created_at: i64 = row.get(12)?;
    Ok(ChapterResultSummary {
        id: row.get(0)?,
        project_id: row.get(1)?,
        chapter_title: row.get(2)?,
        chapter_revision: row.get(3)?,
        summary: row.get(4)?,
        state_changes: string_vec_from_json(row.get::<_, String>(5)?.as_str()),
        character_progress: string_vec_from_json(row.get::<_, String>(6)?.as_str()),
        new_conflicts: string_vec_from_json(row.get::<_, String>(7)?.as_str()),
        new_clues: string_vec_from_json(row.get::<_, String>(8)?.as_str()),
        promise_updates: string_vec_from_json(row.get::<_, String>(9)?.as_str()),
        canon_updates: string_vec_from_json(row.get::<_, String>(10)?.as_str()),
        source_ref: row.get(11)?,
        created_at: created_at.max(0) as u64,
    })
}

fn string_vec_json(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn string_vec_from_json(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn snippet_for_storage(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    normalized.chars().take(max_chars).collect()
}

fn backfill_empty_timestamp(conn: &Connection, table: &str, column: &str) -> SqlResult<()> {
    if !table_exists(conn, table)? || !table_has_column(conn, table, column)? {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "UPDATE {table} SET {column}=datetime('now') WHERE {column} IS NULL OR {column}=''"
    ))?;
    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    column_definition: &str,
) -> SqlResult<()> {
    if !table_exists(conn, table)? || table_has_column(conn, table, column)? {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "ALTER TABLE {table} ADD COLUMN {column_definition}"
    ))?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> SqlResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        rusqlite::params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> SqlResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> WriterMemory {
        WriterMemory::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_canon_entity_upsert() {
        let m = memory();
        let id = m
            .upsert_canon_entity(
                "character",
                "主角",
                &["林墨".into()],
                "主角",
                &serde_json::json!({"weapon": "剑"}),
                0.9,
            )
            .unwrap();
        assert!(id > 0);
        let facts = m.get_canon_facts_for_entity("主角").unwrap();
        assert_eq!(facts, vec![("weapon".to_string(), "剑".to_string())]);
        let entities = m.list_canon_entities().unwrap();
        assert_eq!(entities[0].name, "主角");
    }

    #[test]
    fn test_canon_rule_upsert() {
        let m = memory();
        let id = m
            .upsert_canon_rule("林墨绝不主动弃刀。", "character_rule", 7, "test")
            .unwrap();
        assert!(id > 0);
        m.upsert_canon_rule("林墨绝不主动弃刀。", "combat_rule", 9, "test2")
            .unwrap();
        let rules = m.list_canon_rules(10).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].category, "combat_rule");
        assert_eq!(rules[0].priority, 9);
    }

    #[test]
    fn test_promise_lifecycle() {
        let m = memory();
        let id = m
            .add_promise("clue", "密道", "第2章破庙有密道", "ch2", "ch8", 5)
            .unwrap();
        assert!(id > 0);
        let open = m.get_open_promises().unwrap();
        assert_eq!(open.len(), 1);
        let summaries = m.get_open_promise_summaries().unwrap();
        assert_eq!(summaries[0].title, "密道");
        assert!(m.resolve_promise(id, "ch8").unwrap());
        let open2 = m.get_open_promises().unwrap();
        assert_eq!(open2.len(), 0);
    }

    #[test]
    fn test_promise_defer_and_abandon() {
        let m = memory();
        let deferred_id = m
            .add_promise("clue", "密道", "第2章破庙有密道", "ch2", "ch8", 5)
            .unwrap();
        assert!(m.defer_promise(deferred_id, "ch10").unwrap());
        let summaries = m.get_open_promise_summaries().unwrap();
        assert_eq!(summaries[0].expected_payoff, "ch10");

        let abandoned_id = m
            .add_promise("clue", "铜铃", "铜铃声需要解释", "ch2", "ch6", 5)
            .unwrap();
        assert!(m.abandon_promise(abandoned_id).unwrap());
        let open_titles = m
            .get_open_promise_summaries()
            .unwrap()
            .into_iter()
            .map(|promise| promise.title)
            .collect::<Vec<_>>();
        assert!(!open_titles.contains(&"铜铃".to_string()));
    }

    #[test]
    fn test_style_preference_update() {
        let m = memory();
        m.upsert_style_preference("dialog_style", "prefers_subtext", true)
            .unwrap();
        m.upsert_style_preference("exposition", "rejects_info_dump", false)
            .unwrap();
        let prefs = m.list_style_preferences(5).unwrap();
        assert_eq!(prefs.len(), 2);
        assert!(prefs.iter().any(|p| p.key == "dialog_style"));
    }

    #[test]
    fn test_feedback_record() {
        let m = memory();
        m.record_feedback("prop_1", "accepted", "", "").unwrap();
        m.record_feedback("prop_2", "rejected", "not my style", "")
            .unwrap();
        let (accepted, rejected) = m.feedback_stats("prop_1").unwrap();
        assert_eq!(accepted, 1);
        assert_eq!(rejected, 0);
    }

    #[test]
    fn test_memory_audit_record() {
        let m = memory();
        m.record_memory_audit(&MemoryAuditSummary {
            proposal_id: "prop_1".to_string(),
            kind: "CanonUpdate".to_string(),
            action: "Accepted".to_string(),
            title: "沈照 [character]".to_string(),
            evidence: "那个少年名叫沈照".to_string(),
            rationale: "durable character".to_string(),
            reason: Some("approved".to_string()),
            created_at: 42,
        })
        .unwrap();
        let audit = m.list_memory_audit(5).unwrap();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].proposal_id, "prop_1");
        assert_eq!(audit[0].reason.as_deref(), Some("approved"));
    }

    #[test]
    fn test_manual_agent_turns_persist_and_filter_by_project() {
        let m = memory();
        m.record_manual_agent_turn(&ManualAgentTurnSummary {
            project_id: "novel-a".to_string(),
            observation_id: "obs-a-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            user: "上一轮怎么处理玉佩？".to_string(),
            assistant: "让张三暂时隐瞒玉佩。".to_string(),
            source_refs: vec!["PromiseLedger".to_string()],
            created_at: 10,
        })
        .unwrap();
        m.record_manual_agent_turn(&ManualAgentTurnSummary {
            project_id: "novel-b".to_string(),
            observation_id: "obs-b-1".to_string(),
            chapter_title: None,
            user: "另一个项目".to_string(),
            assistant: "不应混入".to_string(),
            source_refs: Vec::new(),
            created_at: 11,
        })
        .unwrap();
        m.record_manual_agent_turn(&ManualAgentTurnSummary {
            project_id: "novel-a".to_string(),
            observation_id: "obs-a-2".to_string(),
            chapter_title: Some("第二章".to_string()),
            user: "继续上一轮".to_string(),
            assistant: "把玉佩变成下一章冲突。".to_string(),
            source_refs: vec!["CreativeDecision".to_string()],
            created_at: 12,
        })
        .unwrap();

        let turns = m.list_manual_agent_turns("novel-a", 10).unwrap();

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].observation_id, "obs-a-1");
        assert_eq!(turns[0].chapter_title.as_deref(), Some("第一章"));
        assert_eq!(turns[0].source_refs, vec!["PromiseLedger".to_string()]);
        assert_eq!(turns[1].user, "继续上一轮");
    }

    #[test]
    fn test_writer_trace_record() {
        let m = memory();
        m.record_observation_trace("obs_1", 10, "Idle", Some("Chapter-1"), "林墨停下脚步")
            .unwrap();
        m.record_proposal_trace(
            &ProposalTraceSummary {
                id: "prop_1".to_string(),
                observation_id: "obs_1".to_string(),
                kind: "Ghost".to_string(),
                priority: "Ambient".to_string(),
                state: "pending".to_string(),
                confidence: 0.7,
                preview_snippet: "他没有立刻回答".to_string(),
                evidence: vec![crate::writer_agent::proposal::EvidenceRef {
                    source: crate::writer_agent::proposal::EvidenceSource::ChapterMission,
                    reference: "Chapter-1:mission".to_string(),
                    snippet: "本章任务".to_string(),
                }],
                context_budget: Some(ContextBudgetTrace {
                    task: "GhostWriting".to_string(),
                    used: 40,
                    total_budget: 100,
                    wasted: 60,
                    source_reports: vec![ContextSourceBudgetTrace {
                        source: "CursorPrefix".to_string(),
                        requested: 80,
                        provided: 40,
                        truncated: true,
                        reason:
                            "GhostWriting required source reserved 240 chars before priority fill."
                                .to_string(),
                        truncation_reason: Some(
                            "Source content was limited by its per-source budget of 80 chars."
                                .to_string(),
                        ),
                    }],
                }),
                expires_at: Some(1000),
            },
            11,
        )
        .unwrap();
        m.record_feedback_trace(&FeedbackTraceSummary {
            proposal_id: "prop_1".to_string(),
            action: "Accepted".to_string(),
            reason: Some("fits".to_string()),
            created_at: 12,
        })
        .unwrap();
        m.update_proposal_trace_state("prop_1", "feedback:Accepted")
            .unwrap();

        assert_eq!(m.list_observation_traces(5).unwrap()[0].id, "obs_1");
        let proposal = m.list_proposal_traces(5).unwrap().remove(0);
        assert_eq!(proposal.state, "feedback:Accepted");
        assert_eq!(proposal.evidence.len(), 1);
        assert_eq!(
            proposal.evidence[0].source,
            crate::writer_agent::proposal::EvidenceSource::ChapterMission
        );
        let budget = proposal.context_budget.unwrap();
        assert_eq!(budget.task, "GhostWriting");
        assert_eq!(budget.used, 40);
        assert!(budget.source_reports[0].truncated);
        assert_eq!(
            m.list_feedback_traces(5).unwrap()[0].reason.as_deref(),
            Some("fits")
        );
    }

    #[test]
    fn test_context_recall_records_only_surfaced_evidence() {
        let m = memory();
        let evidence = vec![
            crate::writer_agent::proposal::EvidenceRef {
                source: crate::writer_agent::proposal::EvidenceSource::ChapterMission,
                reference: "Chapter-2:mission".to_string(),
                snippet: "本章必须推进玉佩线索。".to_string(),
            },
            crate::writer_agent::proposal::EvidenceRef {
                source: crate::writer_agent::proposal::EvidenceSource::Canon,
                reference: "林墨.weapon".to_string(),
                snippet: "林墨惯用寒影刀。".to_string(),
            },
        ];
        m.record_context_recalls("novel-a", "prop_1", "obs_1", &evidence, 10)
            .unwrap();
        m.record_context_recalls("novel-a", "prop_2", "obs_2", &evidence[..1], 20)
            .unwrap();

        let recalls = m.list_context_recalls("novel-a", 10).unwrap();

        assert_eq!(recalls.len(), 2);
        assert_eq!(recalls[0].source, "ChapterMission");
        assert_eq!(recalls[0].reference, "Chapter-2:mission");
        assert_eq!(recalls[0].recall_count, 2);
        assert_eq!(recalls[0].first_recalled_at, 10);
        assert_eq!(recalls[0].last_recalled_at, 20);
        assert_eq!(recalls[0].last_proposal_id, "prop_2");
        assert_eq!(recalls[1].reference, "林墨.weapon");
        assert_eq!(recalls[1].recall_count, 1);
    }

    #[test]
    fn test_decision_summary() {
        let m = memory();
        m.record_decision(
            "Chapter-1",
            "续写建议",
            "accepted",
            &[],
            "符合角色声音",
            &[],
        )
        .unwrap();
        let decisions = m.list_recent_decisions(5).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision, "accepted");
    }

    #[test]
    fn test_story_contract_upsert_and_seed() {
        let m = memory();
        assert!(m
            .ensure_story_contract_seed(
                "novel-a",
                "寒影录",
                "玄幻",
                "刀客追查玉佩真相。",
                "复仇与守护的冲突。",
                "不得提前泄露玉佩来源。",
            )
            .unwrap());
        assert!(!m
            .ensure_story_contract_seed("novel-a", "不覆盖", "悬疑", "", "", "",)
            .unwrap());

        let mut contract = m.get_story_contract("novel-a").unwrap().unwrap();
        assert_eq!(contract.title, "寒影录");
        assert!(contract.render_for_context().contains("读者承诺"));

        contract.reader_promise = "新的读者承诺".to_string();
        m.upsert_story_contract(&contract).unwrap();
        let updated = m.get_story_contract("novel-a").unwrap().unwrap();
        assert_eq!(updated.reader_promise, "新的读者承诺");
        assert!(!updated.is_empty());
    }

    #[test]
    fn test_chapter_mission_upsert_list_and_seed() {
        let m = memory();
        assert!(m
            .ensure_chapter_mission_seed(
                "novel-a",
                "第一章",
                "林墨发现玉佩线索。",
                "推进玉佩线索",
                "不要提前揭开真相",
                "以新的疑问收束。",
                "test",
            )
            .unwrap());
        assert!(!m
            .ensure_chapter_mission_seed("novel-a", "第一章", "不覆盖", "", "", "", "test")
            .unwrap());

        let mut mission = m.get_chapter_mission("novel-a", "第一章").unwrap().unwrap();
        assert_eq!(mission.mission, "林墨发现玉佩线索。");
        assert!(mission.render_for_context().contains("本章任务"));

        mission.expected_ending = "以冲突升级收束。".to_string();
        m.upsert_chapter_mission(&mission).unwrap();

        let missions = m.list_chapter_missions("novel-a", 10).unwrap();
        assert_eq!(missions.len(), 1);
        assert_eq!(missions[0].expected_ending, "以冲突升级收束。");
        assert!(!missions[0].is_empty());
    }

    #[test]
    fn test_chapter_result_record_and_render() {
        let m = memory();
        let id = m
            .record_chapter_result(&ChapterResultSummary {
                id: 0,
                project_id: "novel-a".to_string(),
                chapter_title: "第一章".to_string(),
                chapter_revision: "rev-1".to_string(),
                summary: "林墨发现玉佩线索，张三隐瞒下落。".to_string(),
                state_changes: vec!["林墨得知玉佩存在风险".to_string()],
                character_progress: vec!["张三选择隐瞒".to_string()],
                new_conflicts: vec!["林墨与张三信任受损".to_string()],
                new_clues: vec!["玉佩".to_string()],
                promise_updates: vec!["玉佩仍需后续解释".to_string()],
                canon_updates: vec!["林墨惯用寒影刀".to_string()],
                source_ref: "chapter_save:第一章:rev-1".to_string(),
                created_at: 42,
            })
            .unwrap();
        assert!(id > 0);

        let recent = m.list_recent_chapter_results("novel-a", 5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].chapter_title, "第一章");
        assert_eq!(recent[0].new_clues, vec!["玉佩".to_string()]);
        assert!(recent[0].render_for_context().contains("章节结果"));

        let latest = m
            .latest_chapter_result("novel-a", "第一章")
            .unwrap()
            .unwrap();
        assert_eq!(latest.summary, "林墨发现玉佩线索，张三隐瞒下落。");
        assert!(!latest.is_empty());
    }

    #[test]
    fn open_migrates_legacy_writer_memory_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE canon_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL UNIQUE
            );
            INSERT INTO canon_entities (kind, name) VALUES ('character', '林墨');
            CREATE TABLE plot_promises (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                title TEXT NOT NULL
            );
            INSERT INTO plot_promises (kind, title) VALUES ('clue', '玉佩');
            CREATE TABLE writer_proposal_trace (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                proposal_id TEXT NOT NULL UNIQUE,
                observation_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                priority TEXT NOT NULL,
                state TEXT NOT NULL,
                confidence REAL DEFAULT 0.0,
                preview_snippet TEXT DEFAULT '',
                created_at INTEGER NOT NULL
            );",
        )
        .unwrap();

        initialize_schema(&conn).unwrap();

        assert!(table_has_column(&conn, "canon_entities", "attributes_json").unwrap());
        assert!(table_has_column(&conn, "plot_promises", "expected_payoff").unwrap());
        assert!(table_has_column(&conn, "plot_promises", "last_seen_chapter").unwrap());
        assert!(table_has_column(&conn, "writer_proposal_trace", "evidence_json").unwrap());
        assert!(table_has_column(&conn, "writer_proposal_trace", "expires_at").unwrap());
        assert!(table_exists(&conn, "manual_agent_turns").unwrap());
        assert!(table_has_column(&conn, "manual_agent_turns", "source_refs_json").unwrap());
        assert!(table_exists(&conn, "story_contracts").unwrap());
        assert!(table_has_column(&conn, "story_contracts", "reader_promise").unwrap());
        assert!(table_exists(&conn, "chapter_missions").unwrap());
        assert!(table_has_column(&conn, "chapter_missions", "expected_ending").unwrap());
        assert!(table_exists(&conn, "chapter_result_snapshots").unwrap());
        assert!(table_has_column(&conn, "chapter_result_snapshots", "new_clues_json").unwrap());
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let m = WriterMemory { conn };
        let entities = m.list_canon_entities().unwrap();
        assert_eq!(entities[0].name, "林墨");
        let promises = m.get_open_promise_summaries().unwrap();
        assert_eq!(promises[0].title, "玉佩");
        assert_eq!(promises[0].last_seen_chapter, "");
    }
}
