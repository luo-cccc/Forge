#![allow(unused_imports)]
use crate::fixtures::*;
use agent_writer_lib::chapter_generation::FocusState;

pub fn run_focus_rebuild_eval() -> EvalResult {
    // Test: needs_rebuild returns true when chapter changes
    let state = FocusState::default();
    assert!(state.needs_rebuild("Chapter-1", None, "hash-a", "hash-b"));

    // Test: needs_rebuild returns false when same chapter + same hashes
    let mut state2 = FocusState::default();
    state2.record_rebuild("Chapter-1", None, "hash-a", "hash-b");
    let same = !state2.needs_rebuild("Chapter-1", None, "hash-a", "hash-b");
    assert!(same);

    // Test: needs_rebuild returns true when result_hash changes
    let changed = state2.needs_rebuild("Chapter-1", None, "hash-c", "hash-b");
    assert!(changed);

    // Test: needs_rebuild returns true when chapter changes
    let diff_chapter = state2.needs_rebuild("Chapter-2", None, "hash-a", "hash-b");
    assert!(diff_chapter);

    // Verify rebuild count is 1 (only one rebuild recorded)
    let count_ok = state2.rebuild_count == 1;

    let ok = same && changed && diff_chapter && count_ok;
    EvalResult::pass_if(
        "focus_rebuild",
        ok,
        format!(
            "sameChapterNoRebuild={} changedHashTriggersRebuild={} diffChapterTriggersRebuild={} rebuildCountOk={}",
            same, changed, diff_chapter, count_ok,
        ),
    )
}
