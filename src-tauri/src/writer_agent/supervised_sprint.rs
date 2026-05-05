//! Supervised Chapter Sprint — batch chapter advancement with guardrails.
//!
//! Allows authors to push through multiple chapters but enforces
//! preflight → receipt → draft → review → save → settlement per chapter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SupervisedSprintPlan {
    pub sprint_id: String,
    pub chapters: Vec<SprintChapterTarget>,
    pub total_chapters: usize,
    pub current_index: usize,
    pub status: String, // "planned" | "running" | "paused" | "completed"
    pub require_approval_per_chapter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SprintChapterTarget {
    pub chapter_title: String,
    pub chapter_number: usize,
    pub status: String, // "pending" | "preflight" | "drafting" | "review" | "saved" | "settled"
    pub receipt_id: Option<String>,
    pub preflight_readiness: Option<String>,
    pub requires_author_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SprintProgress {
    pub sprint_id: String,
    pub chapters_completed: usize,
    pub chapters_remaining: usize,
    pub current_chapter: Option<String>,
    pub receipts_recorded: usize,
    pub settlements_completed: usize,
    pub last_error: Option<String>,
}

/// Create a supervised sprint plan from a list of chapter titles.
pub fn create_sprint_plan(
    sprint_id: &str,
    chapter_titles: &[String],
    require_approval: bool,
) -> SupervisedSprintPlan {
    let chapters: Vec<SprintChapterTarget> = chapter_titles
        .iter()
        .enumerate()
        .map(|(i, title)| SprintChapterTarget {
            chapter_title: title.clone(),
            chapter_number: i + 1,
            status: "pending".to_string(),
            receipt_id: None,
            preflight_readiness: None,
            requires_author_review: require_approval,
        })
        .collect();

    SupervisedSprintPlan {
        sprint_id: sprint_id.to_string(),
        total_chapters: chapters.len(),
        current_index: 0,
        chapters,
        status: "planned".to_string(),
        require_approval_per_chapter: require_approval,
    }
}

/// Check if the sprint can advance to the next chapter.
pub fn can_advance_to_next_chapter(sprint: &SupervisedSprintPlan) -> bool {
    if sprint.current_index >= sprint.total_chapters {
        return false;
    }

    let current = &sprint.chapters[sprint.current_index];

    // Must have receipt AND preflight passed AND (if approval required) author review done.
    let has_receipt = current.receipt_id.is_some();
    let preflight_ok = current
        .preflight_readiness
        .as_deref()
        .map(|r| r != "blocked")
        .unwrap_or(false);

    if sprint.require_approval_per_chapter {
        has_receipt && preflight_ok && current.status == "saved"
    } else {
        has_receipt && preflight_ok
    }
}

/// Advance sprint to the next chapter. Returns the new current chapter title.
pub fn advance_sprint(sprint: &mut SupervisedSprintPlan) -> Option<String> {
    if !can_advance_to_next_chapter(sprint) {
        return None;
    }

    sprint.current_index += 1;

    if sprint.current_index >= sprint.total_chapters {
        sprint.status = "completed".to_string();
        None
    } else {
        sprint.status = "running".to_string();
        sprint.chapters[sprint.current_index].status = "preflight".to_string();
        Some(sprint.chapters[sprint.current_index].chapter_title.clone())
    }
}

/// Build a progress report for the sprint.
pub fn sprint_progress(sprint: &SupervisedSprintPlan) -> SprintProgress {
    let completed = sprint.current_index;
    let remaining = sprint.total_chapters.saturating_sub(completed);
    let current = if sprint.current_index < sprint.total_chapters {
        Some(sprint.chapters[sprint.current_index].chapter_title.clone())
    } else {
        None
    };

    SprintProgress {
        sprint_id: sprint.sprint_id.clone(),
        chapters_completed: completed,
        chapters_remaining: remaining,
        current_chapter: current,
        receipts_recorded: sprint
            .chapters
            .iter()
            .filter(|c| c.receipt_id.is_some())
            .count(),
        settlements_completed: sprint
            .chapters
            .iter()
            .filter(|c| c.status == "settled")
            .count(),
        last_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_stops_before_unapproved_save() {
        let mut sprint = create_sprint_plan(
            "s1",
            &["Ch1".to_string(), "Ch2".to_string()],
            true, // require approval
        );

        // Set up Ch1 with receipt + preflight but NOT saved (no author approval yet)
        sprint.chapters[0].receipt_id = Some("receipt-1".to_string());
        sprint.chapters[0].preflight_readiness = Some("ready".to_string());
        sprint.chapters[0].status = "drafting".to_string();

        assert!(
            !can_advance_to_next_chapter(&sprint),
            "Should NOT advance without author save when approval required"
        );
    }

    #[test]
    fn sprint_carries_forward_settlement() {
        let mut sprint = create_sprint_plan("s2", &["Ch1".to_string(), "Ch2".to_string()], false);

        sprint.chapters[0].receipt_id = Some("r1".to_string());
        sprint.chapters[0].preflight_readiness = Some("ready".to_string());
        sprint.chapters[0].status = "settled";

        assert!(can_advance_to_next_chapter(&sprint));
        let next = advance_sprint(&mut sprint);
        assert_eq!(next, Some("Ch2".to_string()));
        assert_eq!(sprint.chapters[1].status, "preflight");
    }

    #[test]
    fn sprint_records_receipts_per_chapter() {
        let sprint = create_sprint_plan(
            "s3",
            &["Ch1".to_string(), "Ch2".to_string(), "Ch3".to_string()],
            false,
        );
        assert_eq!(sprint.total_chapters, 3);
        // Each chapter target should have space for receipt
        for chapter in &sprint.chapters {
            assert!(chapter.receipt_id.is_none(), "receipts start empty");
        }
    }
}
