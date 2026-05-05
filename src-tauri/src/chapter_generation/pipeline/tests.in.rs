#[cfg(test)]
mod tests {
    use super::*;

    fn outline() -> Vec<storage::OutlineNode> {
        vec![
            storage::OutlineNode {
                chapter_title: "第一章".to_string(),
                summary: "林墨抵达破庙。".to_string(),
                status: "drafted".to_string(),
            },
            storage::OutlineNode {
                chapter_title: "第二章".to_string(),
                summary: "林墨发现壁画。".to_string(),
                status: "drafted".to_string(),
            },
            storage::OutlineNode {
                chapter_title: "第三章".to_string(),
                summary: "林墨发现密道并遭遇毒雾机关。".to_string(),
                status: "empty".to_string(),
            },
        ]
    }

    #[test]
    fn counts_unicode_chars_instead_of_bytes_for_chinese_text() {
        assert_eq!(char_count("破庙密道"), 4);
        assert_eq!("破庙密道".len(), 12);
    }

    #[test]
    fn truncates_chinese_at_valid_utf8_boundary() {
        let (text, included, truncated) = truncate_text_report("林墨推开破庙石门", 4);
        assert_eq!(text, "林墨推开");
        assert_eq!(included, 4);
        assert!(truncated);
    }

    #[test]
    fn prefers_chinese_sentence_boundary_when_truncating() {
        let (text, _, truncated) =
            truncate_text_report("林墨停下脚步。毒雾从密道深处涌来，像潮水一样。", 16);
        assert_eq!(text, "林墨停下脚步。");
        assert!(truncated);
    }

    #[test]
    fn handles_mixed_chinese_english_and_emoji_without_corruption() {
        let (text, included, truncated) = truncate_text_report("AI提醒林墨：run！🔥继续。", 10);
        assert_eq!(char_count(&text), included);
        assert!(text.is_char_boundary(text.len()));
        assert!(truncated);
    }

    #[test]
    fn resolves_target_chapter_by_outline_number_and_returns_metadata() {
        let target = resolve_target_from_outline(&outline(), None, Some(3), None).unwrap();
        assert_eq!(target.title, "第三章");
        assert_eq!(target.number, Some(3));
        assert!(target.summary.contains("密道"));
    }

    #[test]
    fn rejects_missing_target_chapter_with_typed_error() {
        let err = resolve_target_from_outline(&outline(), Some("第九章"), None, None).unwrap_err();
        assert_eq!(err.code, "TARGET_CHAPTER_NOT_FOUND");
    }

    #[test]
    fn rejects_ambiguous_target_chapter_with_typed_error() {
        let mut data = outline();
        data.push(storage::OutlineNode {
            chapter_title: "第三章".to_string(),
            summary: "重复节点".to_string(),
            status: "empty".to_string(),
        });
        let err = resolve_target_from_outline(&data, Some("第三章"), None, None).unwrap_err();
        assert_eq!(err.code, "TARGET_CHAPTER_AMBIGUOUS");
    }

    #[test]
    fn replaces_chapter_when_revision_matches_and_frontend_is_clean() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "abc",
            Some(&FrontendChapterStateSnapshot {
                open_chapter_title: Some("第三章".to_string()),
                open_chapter_revision: Some("abc".to_string()),
                dirty: false,
            }),
        );
        assert!(matches!(decision, SaveDecision::WriteTarget));
    }

    #[test]
    fn rejects_dirty_open_target_chapter_without_writing() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "abc",
            Some(&FrontendChapterStateSnapshot {
                open_chapter_title: Some("第三章".to_string()),
                open_chapter_revision: Some("abc".to_string()),
                dirty: true,
            }),
        );
        match decision {
            SaveDecision::Conflict(conflict) => {
                assert_eq!(conflict.reason, "frontend_dirty_open_chapter");
            }
            _ => panic!("expected conflict"),
        }
    }

    #[test]
    fn rejects_revision_mismatch_without_writing() {
        let decision = decide_save_action(
            "第三章",
            "req-1",
            SaveMode::ReplaceIfClean,
            "abc",
            "def",
            None,
        );
        match decision {
            SaveDecision::Conflict(conflict) => {
                assert_eq!(conflict.reason, "revision_mismatch");
            }
            _ => panic!("expected conflict"),
        }
    }

    #[test]
    fn saves_draft_copy_on_conflict_only_when_requested() {
        let decision = decide_save_action(
            "第三章",
            "request-abcdef",
            SaveMode::SaveAsDraft,
            "abc",
            "def",
            None,
        );
        match decision {
            SaveDecision::WriteDraft {
                draft_title,
                conflict,
            } => {
                assert!(draft_title.contains("第三章 draft"));
                assert_eq!(conflict.reason, "revision_mismatch");
            }
            _ => panic!("expected draft decision"),
        }
    }

    #[test]
    fn rejects_empty_generated_content_with_content_empty() {
        let err = validate_generated_content("  ").unwrap_err();
        assert_eq!(err.code, "MODEL_OUTPUT_EMPTY");
    }

    #[test]
    fn maps_http_429_to_provider_rate_limited() {
        let err = map_provider_error("API error 429: too many requests".to_string());
        assert_eq!(err.code, "PROVIDER_RATE_LIMITED");
    }

    #[test]
    fn provider_budget_error_preserves_report_evidence() {
        let target = ChapterTarget {
            title: "第三章".to_string(),
            filename: "第三章.md".to_string(),
            number: Some(3),
            summary: "林墨发现密道。".to_string(),
            status: "empty".to_string(),
        };
        let receipt = build_chapter_generation_receipt(
            "budget-test-1",
            &target,
            "rev-1",
            "写第三章。",
            &[ChapterContextSource {
                source_type: "instruction".to_string(),
                id: "user-instruction".to_string(),
                label: "User instruction".to_string(),
                original_chars: 5,
                included_chars: 5,
                truncated: false,
                score: None,
            }],
            10,
        );
        let report = evaluate_provider_budget(WriterProviderBudgetRequest::new(
            WriterProviderBudgetTask::ChapterGeneration,
            "gpt-4o",
            90_000,
            24_000,
        ));

        let error = provider_budget_error("budget-test-1", &receipt, report);

        assert_eq!(error.code, "PROVIDER_BUDGET_APPROVAL_REQUIRED");
        let evidence = error.evidence.expect("budget error has evidence");
        assert_eq!(evidence.category, WriterFailureCategory::ProviderFailed);
        assert!(evidence.details.get("providerBudget").is_some());
        assert!(!evidence.remediation.is_empty());
    }
}
