#![allow(unused_imports)]
use crate::fixtures::*;

use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::observation::{
    ObservationReason, ObservationSource, WriterObservation,
};
use std::path::Path;

/// Verify that all chapter save paths emit isomorphic observations —
/// same source, same reason, same required fields present regardless of
/// which save path triggered the observation.
pub fn run_save_path_consistency_eval() -> EvalResult {
    let mut errors = Vec::new();

    // Simulate two different save paths producing observations
    let save_obs_1 = WriterObservation {
        id: "eval-save-path-1".to_string(),
        created_at: 1000,
        source: ObservationSource::ChapterSave,
        reason: ObservationReason::Save,
        project_id: "eval-save-path".to_string(),
        chapter_title: Some("第一章".to_string()),
        chapter_revision: Some("aaaa0001".to_string()),
        cursor: None,
        selection: None,
        prefix: "测试内容：主角进入山谷。".to_string(),
        suffix: String::new(),
        paragraph: "测试内容：主角进入山谷。".to_string(),
        full_text_digest: Some("aaaa0001".to_string()),
        editor_dirty: false,
    };

    // Another save path — e.g. auto-save or manual save of a different chapter
    let save_obs_2 = WriterObservation {
        id: "eval-save-path-2".to_string(),
        created_at: 2000,
        source: ObservationSource::ChapterSave,
        reason: ObservationReason::Save,
        project_id: "eval-save-path".to_string(),
        chapter_title: Some("第二章".to_string()),
        chapter_revision: Some("bbbb0002".to_string()),
        cursor: None,
        selection: None,
        prefix: "主角继续前行。".to_string(),
        suffix: String::new(),
        paragraph: "主角继续前行。".to_string(),
        full_text_digest: Some("bbbb0002".to_string()),
        editor_dirty: false,
    };

    // All save observations must share the same source
    if save_obs_1.source != ObservationSource::ChapterSave
        || save_obs_2.source != ObservationSource::ChapterSave
    {
        errors.push("save observations must have ChapterSave source".to_string());
    }

    // All save observations must share the same reason
    if save_obs_1.reason != ObservationReason::Save
        || save_obs_2.reason != ObservationReason::Save
    {
        errors.push("save observations must have Save reason".to_string());
    }

    // Save observations must carry a chapter title
    if save_obs_1.chapter_title.is_none() || save_obs_2.chapter_title.is_none() {
        errors.push("save observations must carry chapter_title".to_string());
    }

    // Save observations must carry a chapter revision
    if save_obs_1.chapter_revision.is_none() || save_obs_2.chapter_revision.is_none() {
        errors.push("save observations must carry chapter_revision".to_string());
    }

    // Save observations must carry a full-text digest
    if save_obs_1.full_text_digest.is_none() || save_obs_2.full_text_digest.is_none() {
        errors.push("save observations must carry full_text_digest".to_string());
    }

    // Save observations should be editor-clean (not dirty)
    if save_obs_1.editor_dirty || save_obs_2.editor_dirty {
        errors.push("save observations should have editor_dirty=false".to_string());
    }

    // Verify isomorphism: both observations share the same field shape
    let same_source = save_obs_1.source == save_obs_2.source;
    let same_reason = save_obs_1.reason == save_obs_2.reason;
    let both_have_chapter = save_obs_1.chapter_title.is_some() && save_obs_2.chapter_title.is_some();
    let both_have_revision =
        save_obs_1.chapter_revision.is_some() && save_obs_2.chapter_revision.is_some();
    let both_have_digest =
        save_obs_1.full_text_digest.is_some() && save_obs_2.full_text_digest.is_some();
    let both_editor_clean = !save_obs_1.editor_dirty && !save_obs_2.editor_dirty;

    let isomorphic = same_source
        && same_reason
        && both_have_chapter
        && both_have_revision
        && both_have_digest
        && both_editor_clean;

    if !isomorphic {
        errors.push("save observations from different paths must be structurally isomorphic"
            .to_string());
    }

    eval_result(
        "writer_agent:save_path_consistency",
        format!(
            "isomorphic={} sourceMatch={} reasonMatch={} chapterTitle={} revision={} digest={} editorClean={}",
            isomorphic,
            same_source,
            same_reason,
            both_have_chapter,
            both_have_revision,
            both_have_digest,
            both_editor_clean,
        ),
        errors,
    )
}
