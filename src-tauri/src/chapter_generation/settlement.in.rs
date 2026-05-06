use crate::writer_agent::kernel::{
    chapter_result_from_observation, extract_plot_promises, split_sentences,
};
use crate::writer_agent::memory::{ChapterResultSummary, PlotPromiseSummary, WriterMemory};
use crate::writer_agent::observation::{ObservationReason, ObservationSource, WriterObservation};

pub fn build_basic_chapter_settlement_delta(
    project_id: &str,
    chapter_title: &str,
    chapter_revision: &str,
    generated_content: &str,
    created_at_ms: u64,
    memory: &WriterMemory,
    continuity_issues: Vec<String>,
) -> ChapterSettlementDelta {
    let observation = settlement_observation(
        project_id,
        chapter_title,
        chapter_revision,
        generated_content,
        created_at_ms,
    );
    let chapter_result = chapter_result_from_observation(&observation, memory);
    let open_promises = memory.get_open_promise_summaries().unwrap_or_default();
    let promise_updates = derive_promise_delta_entries(
        generated_content,
        &observation,
        &chapter_result,
        &open_promises,
    );
    let book_state_updates = derive_book_state_updates(&chapter_result, &promise_updates);
    let arc_updates = derive_arc_updates(&chapter_result, &promise_updates);
    let summary = chapter_result.summary.clone();

    ChapterSettlementDelta {
        chapter_title: chapter_title.to_string(),
        chapter_revision: chapter_revision.to_string(),
        summary,
        chapter_result: ChapterResultDelta {
            summary: chapter_result.summary.clone(),
            state_changes: chapter_result.state_changes.clone(),
            character_progress: chapter_result.character_progress.clone(),
            new_conflicts: chapter_result.new_conflicts.clone(),
            new_clues: chapter_result.new_clues.clone(),
            promise_updates: chapter_result.promise_updates.clone(),
            canon_updates: chapter_result.canon_updates.clone(),
        },
        promise_updates: promise_updates.clone(),
        arc_updates: arc_updates.clone(),
        book_state_updates: book_state_updates.clone(),
        chapter_fact_delta: chapter_fact_lines(&chapter_result),
        promise_delta: promise_updates.iter().map(render_promise_delta_line).collect(),
        arc_delta: arc_updates
            .iter()
            .map(|entry| format!("{}: {}", entry.scope, entry.value))
            .collect(),
        book_state_delta: book_state_updates
            .iter()
            .map(render_book_state_delta_line)
            .collect(),
        continuity_issues,
        repairable: true,
    }
}

fn settlement_observation(
    project_id: &str,
    chapter_title: &str,
    chapter_revision: &str,
    generated_content: &str,
    created_at_ms: u64,
) -> WriterObservation {
    WriterObservation {
        id: format!("settlement-{}-{}", chapter_title, chapter_revision),
        created_at: created_at_ms,
        source: ObservationSource::ChapterSave,
        reason: ObservationReason::Save,
        project_id: project_id.to_string(),
        chapter_title: Some(chapter_title.to_string()),
        chapter_revision: Some(chapter_revision.to_string()),
        cursor: None,
        selection: None,
        prefix: generated_content.to_string(),
        suffix: String::new(),
        paragraph: generated_content
            .lines()
            .rev()
            .find(|line| line.trim().chars().count() >= 8)
            .unwrap_or(generated_content)
            .trim()
            .to_string(),
        full_text_digest: Some(crate::storage::content_revision(generated_content)),
        editor_dirty: false,
    }
}

fn derive_promise_delta_entries(
    generated_content: &str,
    observation: &WriterObservation,
    chapter_result: &ChapterResultSummary,
    open_promises: &[PlotPromiseSummary],
) -> Vec<ChapterPromiseDeltaEntry> {
    let mut updates = Vec::new();
    let lowercase = generated_content.to_lowercase();

    for promise in open_promises {
        let title_hit =
            !promise.title.trim().is_empty() && generated_content.contains(&promise.title);
        let desc_hit = !promise.description.trim().is_empty()
            && split_sentences(&promise.description)
                .into_iter()
                .any(|fragment| generated_content.contains(fragment.trim()));
        if !(title_hit || desc_hit) {
            continue;
        }
        let resolved = title_hit
            && ["交代", "说出", "揭开", "归还", "放回", "找到", "解释", "兑现", "真相大白", "水落石出"]
                .iter()
                .any(|cue| lowercase.contains(cue));
        let blocked_reason = if chapter_result
            .new_conflicts
            .iter()
            .any(|line| line.contains(&promise.title))
        {
            chapter_result
                .new_conflicts
                .iter()
                .find(|line| line.contains(&promise.title))
                .cloned()
                .unwrap_or_default()
        } else {
            String::new()
        };
        updates.push(ChapterPromiseDeltaEntry {
            action: if resolved {
                ChapterPromiseDeltaAction::Resolved
            } else if !blocked_reason.is_empty() {
                ChapterPromiseDeltaAction::Deferred
            } else {
                ChapterPromiseDeltaAction::Advanced
            },
            promise_id: Some(promise.id),
            kind: promise.kind.clone(),
            title: promise.title.clone(),
            description: promise.description.clone(),
            chapter: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".to_string()),
            source_ref: chapter_result.source_ref.clone(),
            expected_payoff: if !blocked_reason.is_empty() {
                next_chapter_label(&chapter_result.chapter_title)
            } else {
                promise.expected_payoff.clone()
            },
            priority: promise.priority,
            related_entities: Vec::new(),
            core: promise.core || promise.priority >= 7,
            promoted: promise.promoted || promise.priority >= 5,
            blocked_reason,
            evidence: chapter_result.summary.clone(),
        });
    }

    for promise in extract_plot_promises(generated_content, observation) {
        if updates.iter().any(|existing| existing.title == promise.title)
            || open_promises.iter().any(|existing| existing.title == promise.title)
        {
            continue;
        }
        updates.push(ChapterPromiseDeltaEntry {
            action: ChapterPromiseDeltaAction::Introduced,
            promise_id: None,
            kind: promise.kind.clone(),
            title: promise.title.clone(),
            description: promise.description.clone(),
            chapter: promise.introduced_chapter.clone(),
            source_ref: chapter_result.source_ref.clone(),
            expected_payoff: promise.expected_payoff.clone(),
            priority: promise.priority,
            related_entities: promise.related_entities.clone(),
            core: promise.priority >= 5,
            promoted: promise.priority >= 4,
            blocked_reason: String::new(),
            evidence: promise.description.clone(),
        });
    }

    updates
}

fn derive_book_state_updates(
    chapter_result: &ChapterResultSummary,
    promise_updates: &[ChapterPromiseDeltaEntry],
) -> Vec<ChapterBookStateDeltaEntry> {
    let mut updates = Vec::new();
    for line in chapter_result.state_changes.iter().take(3) {
        if looks_irreversible(line) {
            updates.push(ChapterBookStateDeltaEntry {
                bucket: ChapterBookStateDeltaBucket::IrreversibleChange,
                value: line.clone(),
                source_ref: chapter_result.source_ref.clone(),
                reason: "chapter state change appears durable".to_string(),
            });
        }
    }
    for promise in promise_updates.iter().filter(|entry| entry.core).take(3) {
        updates.push(ChapterBookStateDeltaEntry {
            bucket: ChapterBookStateDeltaBucket::MegaPromise,
            value: format!("{} -> {}", promise.title, promise.expected_payoff),
            source_ref: promise.source_ref.clone(),
            reason: "core promise should remain visible at book scope".to_string(),
        });
    }
    updates
}

fn derive_arc_updates(
    chapter_result: &ChapterResultSummary,
    promise_updates: &[ChapterPromiseDeltaEntry],
) -> Vec<ChapterArcDeltaEntry> {
    let mut updates = Vec::new();
    if let Some(conflict) = chapter_result.new_conflicts.first() {
        updates.push(ChapterArcDeltaEntry {
            scope: "conflict".to_string(),
            value: conflict.clone(),
            reason: "new conflict should shape upcoming arc planning".to_string(),
        });
    }
    if let Some(promoted) = promise_updates.iter().find(|entry| entry.promoted) {
        updates.push(ChapterArcDeltaEntry {
            scope: "hook".to_string(),
            value: promoted.title.clone(),
            reason: "promoted promise should enter arc planning priority".to_string(),
        });
    }
    updates
}

fn chapter_fact_lines(result: &ChapterResultSummary) -> Vec<String> {
    let mut lines = Vec::new();
    if !result.summary.trim().is_empty() {
        lines.push(result.summary.clone());
    }
    lines.extend(result.state_changes.iter().take(3).cloned());
    lines.extend(result.character_progress.iter().take(2).cloned());
    lines.extend(
        result
            .new_clues
            .iter()
            .map(|clue| format!("线索: {}", clue))
            .take(2),
    );
    lines
}

fn render_promise_delta_line(entry: &ChapterPromiseDeltaEntry) -> String {
    let action = match entry.action {
        ChapterPromiseDeltaAction::Introduced => "introduced",
        ChapterPromiseDeltaAction::Advanced => "advanced",
        ChapterPromiseDeltaAction::Resolved => "resolved",
        ChapterPromiseDeltaAction::Deferred => "deferred",
        ChapterPromiseDeltaAction::Abandoned => "abandoned",
    };
    let mut line = format!("{}: {}", action, entry.title);
    if !entry.expected_payoff.trim().is_empty() {
        line.push_str(&format!(" -> {}", entry.expected_payoff));
    }
    if !entry.blocked_reason.trim().is_empty() {
        line.push_str(&format!(" | blocked: {}", entry.blocked_reason));
    }
    line
}

fn render_book_state_delta_line(entry: &ChapterBookStateDeltaEntry) -> String {
    let bucket = match entry.bucket {
        ChapterBookStateDeltaBucket::LongTermConstraint => "constraint",
        ChapterBookStateDeltaBucket::MegaPromise => "mega_promise",
        ChapterBookStateDeltaBucket::IrreversibleChange => "irreversible_change",
    };
    format!("{}: {}", bucket, entry.value)
}

fn looks_irreversible(line: &str) -> bool {
    ["失去", "死亡", "断绝", "背叛", "毁掉", "归还", "公开", "暴露"]
        .iter()
        .any(|cue| line.contains(cue))
}

fn next_chapter_label(chapter: &str) -> String {
    let digits = chapter
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits
        .parse::<i64>()
        .ok()
        .map(|number| format!("Chapter-{}", number + 1))
        .unwrap_or_else(|| "later chapter".to_string())
}
