//! Ghost proposal helpers for WriterAgentKernel.

use super::context::{ContextSource, WritingContextPack};
use super::intent::WritingIntent;
use super::observation::WriterObservation;
use super::operation::WriterOperation;
use super::proposal::{EvidenceRef, EvidenceSource, ProposalAlternative};

pub(crate) fn draft_continuation(
    intent: &WritingIntent,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
) -> String {
    let paragraph = observation.paragraph.trim();
    let lead = if paragraph.ends_with('。')
        || paragraph.ends_with('！')
        || paragraph.ends_with('？')
        || paragraph.ends_with('.')
        || paragraph.ends_with('!')
        || paragraph.ends_with('?')
    {
        "\n"
    } else {
        ""
    };

    let canon_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::CanonSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");
    let promise_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");

    let text = if !promise_hint.is_empty() {
        "他忽然想起那件还没交代清楚的旧事，原本要出口的话在舌尖停住了。"
    } else if canon_hint.contains("weapon") || canon_hint.contains("武器") {
        "他没有急着开口，只让手指重新落回熟悉的兵器旁，像是在确认自己仍握着选择。"
    } else {
        match intent {
            WritingIntent::Dialogue => "他没有立刻回答，只把真正想说的话压在喉咙后面。",
            WritingIntent::Action => "下一瞬，他侧身避开逼近的锋芒，顺势把局面逼向更窄的角落。",
            WritingIntent::ConflictEscalation => {
                "偏在这时，门外传来第三个人的脚步声，把所有尚未出口的话都截断了。"
            }
            WritingIntent::Description => {
                "风从缝隙里钻进来，带着潮湿的冷意，让这片沉默显得更不安稳。"
            }
            _ => "他停了半息，终于做出那个无法再撤回的决定。",
        }
    };

    format!("{lead}{text}")
}

pub(crate) fn ghost_alternatives(
    intent: &WritingIntent,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
    chapter: &str,
    insert_at: usize,
    revision: &str,
) -> Vec<ProposalAlternative> {
    let candidates = ghost_candidate_texts(intent, observation, context_pack);
    let labels = ghost_candidate_labels(intent);
    let branch_evidence = per_branch_evidence(intent, context_pack);
    candidates
        .into_iter()
        .enumerate()
        .map(|(idx, preview)| {
            let id = ["a", "b", "c"].get(idx).unwrap_or(&"x").to_string();
            ProposalAlternative {
                id: id.clone(),
                label: labels[idx].to_string(),
                operation: Some(WriterOperation::TextInsert {
                    chapter: chapter.to_string(),
                    at: insert_at,
                    text: preview.clone(),
                    revision: revision.to_string(),
                }),
                rationale: format!("multi-ghost branch {}", id.to_ascii_uppercase()),
                evidence: branch_evidence[idx].clone(),
                preview,
            }
        })
        .collect()
}

fn per_branch_evidence(
    intent: &WritingIntent,
    context_pack: &WritingContextPack,
) -> [Vec<EvidenceRef>; 3] {
    let mission_snippet = context_pack
        .sources
        .iter()
        .find(|s| s.source == ContextSource::ChapterMission)
        .map(|s| super::kernel::snippet(&s.content, 80))
        .unwrap_or_default();
    let canon_snippet = context_pack
        .sources
        .iter()
        .find(|s| s.source == ContextSource::CanonSlice)
        .and_then(|s| s.content.lines().next().map(|l| l.to_string()))
        .unwrap_or_default();
    let promise_snippet = context_pack
        .sources
        .iter()
        .find(|s| s.source == ContextSource::PromiseSlice)
        .and_then(|s| s.content.lines().next().map(|l| l.to_string()))
        .unwrap_or_default();

    let a_evidence = if !canon_snippet.is_empty() {
        vec![EvidenceRef {
            source: EvidenceSource::Canon,
            reference: "current-canon".to_string(),
            snippet: canon_snippet.clone(),
        }]
    } else if !mission_snippet.is_empty() {
        vec![EvidenceRef {
            source: EvidenceSource::ChapterMission,
            reference: "active-mission".to_string(),
            snippet: mission_snippet.clone(),
        }]
    } else {
        vec![]
    };

    let b_evidence = if !promise_snippet.is_empty() {
        vec![EvidenceRef {
            source: EvidenceSource::PromiseLedger,
            reference: "open-promise".to_string(),
            snippet: promise_snippet.clone(),
        }]
    } else if !mission_snippet.is_empty() {
        vec![EvidenceRef {
            source: EvidenceSource::ChapterMission,
            reference: "active-mission".to_string(),
            snippet: mission_snippet.clone(),
        }]
    } else {
        vec![]
    };

    let c_evidence = if matches!(
        intent,
        WritingIntent::ConflictEscalation | WritingIntent::Dialogue
    ) && !mission_snippet.is_empty()
    {
        vec![EvidenceRef {
            source: EvidenceSource::ChapterMission,
            reference: "active-mission".to_string(),
            snippet: mission_snippet.clone(),
        }]
    } else if !canon_snippet.is_empty() {
        vec![EvidenceRef {
            source: EvidenceSource::Canon,
            reference: "current-canon".to_string(),
            snippet: canon_snippet.clone(),
        }]
    } else {
        vec![]
    };

    [a_evidence, b_evidence, c_evidence]
}

fn ghost_candidate_texts(
    intent: &WritingIntent,
    observation: &WriterObservation,
    context_pack: &WritingContextPack,
) -> [String; 3] {
    let base = draft_continuation(intent, observation, context_pack);
    let promise_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::PromiseSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");
    let canon_hint = context_pack
        .sources
        .iter()
        .find(|source| source.source == ContextSource::CanonSlice)
        .and_then(|source| source.content.lines().next())
        .unwrap_or("");

    let branch_b = if !promise_hint.is_empty() {
        "他没有继续逼问，只把那件悬而未决的旧事重新压回心底，等对方先露出破绽。"
    } else {
        match intent {
            WritingIntent::Dialogue => "他垂下眼，像是随口一问：“你刚才避开的，究竟是哪一句？”",
            WritingIntent::Action => "他故意慢了半拍，让对方以为自己占了先机，再突然切进空门。",
            WritingIntent::ConflictEscalation => {
                "他还没来得及判断局势，屋内的灯先灭了，黑暗把所有退路一并吞没。"
            }
            WritingIntent::Description => {
                "潮气沿着墙根蔓延，旧木与灰尘的味道混在一起，像某种迟迟不肯散去的警告。"
            }
            _ => "他没有立刻推进，只把目光移向最安静的那个人，等一个真正的答案。",
        }
    };

    let branch_c = if canon_hint.contains("weapon") || canon_hint.contains("武器") {
        "他松开那句差点出口的话，先确认掌心熟悉的重量仍在，才重新抬眼看向对方。"
    } else {
        match intent {
            WritingIntent::Dialogue => {
                "那句话到了嘴边又被他咽回去，只剩一个短促的笑，听不出是承认还是挑衅。"
            }
            WritingIntent::Action => {
                "可就在他发力之前，身后传来一声轻响，迫使他把所有动作硬生生收住。"
            }
            WritingIntent::ConflictEscalation => {
                "更糟的是，来人没有藏脚步，仿佛正等着他们意识到自己已经无处可躲。"
            }
            WritingIntent::Description => {
                "远处的声响被夜色压得很低，低到像是从每个人心里慢慢渗出来的。"
            }
            _ => "他终于意识到，真正该被追问的不是眼前这句话，而是此前一直没人敢提的沉默。",
        }
    };

    [base, branch_b.to_string(), branch_c.to_string()]
}

fn ghost_candidate_labels(intent: &WritingIntent) -> [&'static str; 3] {
    match intent {
        WritingIntent::Dialogue => ["A 直接表态", "B 言语试探", "C 压住情绪"],
        WritingIntent::Action => ["A 快节奏", "B 诱敌试探", "C 外部打断"],
        WritingIntent::ConflictEscalation => ["A 顺势加压", "B 黑暗反转", "C 来人压迫"],
        WritingIntent::Description => ["A 氛围推进", "B 感官细化", "C 情绪映射"],
        _ => ["A 顺势推进", "B 关系试探", "C 伏笔回扣"],
    }
}

pub(crate) fn sanitize_continuation(text: &str) -> String {
    text.trim()
        .trim_matches('`')
        .trim()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(260)
        .collect()
}

pub(crate) fn context_pack_evidence(
    pack: &WritingContextPack,
    observation: &WriterObservation,
) -> Vec<EvidenceRef> {
    let mut evidence = Vec::new();
    for source in &pack.sources {
        let evidence_source = match source.source {
            ContextSource::CursorPrefix
            | ContextSource::CursorSuffix
            | ContextSource::SelectedText => EvidenceSource::ChapterText,
            ContextSource::CanonSlice => EvidenceSource::Canon,
            ContextSource::PromiseSlice => EvidenceSource::PromiseLedger,
            ContextSource::ProjectBrief => EvidenceSource::StoryContract,
            ContextSource::ChapterMission => EvidenceSource::ChapterMission,
            ContextSource::DecisionSlice => EvidenceSource::AuthorFeedback,
            ContextSource::AuthorStyle => EvidenceSource::StyleLedger,
            ContextSource::OutlineSlice => EvidenceSource::Outline,
            ContextSource::ResultFeedback => EvidenceSource::ChapterText,
            _ => EvidenceSource::ChapterText,
        };
        evidence.push(EvidenceRef {
            source: evidence_source,
            reference: format!("{:?}", source.source),
            snippet: source.content.chars().take(140).collect(),
        });
    }

    if evidence.is_empty() {
        evidence.push(EvidenceRef {
            source: EvidenceSource::ChapterText,
            reference: observation
                .chapter_title
                .clone()
                .unwrap_or_else(|| "current chapter".into()),
            snippet: observation.paragraph.chars().take(120).collect(),
        });
    }

    evidence
}

#[cfg(test)]
mod tests {
    use super::super::context::{AgentTask, ContextBudgetReport, ContextExcerpt, SourceReport};
    use super::super::observation::{ObservationReason, ObservationSource, TextRange};
    use super::*;

    fn observation(paragraph: &str) -> WriterObservation {
        WriterObservation {
            id: "obs-1".to_string(),
            created_at: 1,
            source: ObservationSource::Editor,
            reason: ObservationReason::Idle,
            project_id: "default".to_string(),
            chapter_title: Some("Chapter-1".to_string()),
            chapter_revision: Some("rev".to_string()),
            cursor: Some(TextRange { from: 10, to: 10 }),
            selection: None,
            prefix: paragraph.to_string(),
            suffix: String::new(),
            paragraph: paragraph.to_string(),
            full_text_digest: None,
            editor_dirty: true,
        }
    }

    fn context_pack(sources: Vec<(ContextSource, &str)>) -> WritingContextPack {
        let excerpts = sources
            .into_iter()
            .map(|(source, content)| ContextExcerpt {
                source,
                content: content.to_string(),
                char_count: content.chars().count(),
                truncated: false,
                priority: 1,
                evidence_ref: None,
            })
            .collect::<Vec<_>>();
        let total_chars = excerpts.iter().map(|source| source.char_count).sum();
        WritingContextPack {
            task: AgentTask::GhostWriting,
            sources: excerpts,
            total_chars,
            budget_limit: 3000,
            budget_report: ContextBudgetReport {
                total_budget: 3000,
                used: total_chars,
                wasted: 3000usize.saturating_sub(total_chars),
                source_reports: vec![SourceReport {
                    source: "test".to_string(),
                    requested: total_chars,
                    provided: total_chars,
                    truncated: false,
                    reason: "test".to_string(),
                    truncation_reason: None,
                }],
            },
        }
    }

    #[test]
    fn draft_continuation_leads_after_sentence_end() {
        let observation = observation("他说完了。");
        let pack = context_pack(Vec::new());

        let continuation = draft_continuation(&WritingIntent::Dialogue, &observation, &pack);

        assert!(continuation.starts_with('\n'));
        assert!(continuation.contains("没有立刻回答"));
    }

    #[test]
    fn ghost_alternatives_build_typed_insert_operations() {
        let observation = observation("他抬头");
        let pack = context_pack(Vec::new());

        let alternatives = ghost_alternatives(
            &WritingIntent::Action,
            &observation,
            &pack,
            "Chapter-1",
            12,
            "rev-1",
        );

        assert_eq!(alternatives.len(), 3);
        assert_eq!(alternatives[0].label, "A 快节奏");
        assert!(matches!(
            alternatives[0].operation,
            Some(WriterOperation::TextInsert { at: 12, .. })
        ));
    }

    #[test]
    fn sanitize_continuation_trims_fences_and_blank_lines() {
        let sanitized = sanitize_continuation("  ```\n\n  第一行\n\n  第二行  \n```  ");

        assert_eq!(sanitized, "第一行\n第二行");
    }

    #[test]
    fn context_pack_evidence_maps_sources_and_falls_back() {
        let observation = observation("这是当前段落");
        let pack = context_pack(vec![
            (ContextSource::CanonSlice, "canon detail"),
            (ContextSource::PromiseSlice, "promise detail"),
        ]);

        let evidence = context_pack_evidence(&pack, &observation);

        assert!(evidence
            .iter()
            .any(|item| item.source == EvidenceSource::Canon));
        assert!(evidence
            .iter()
            .any(|item| item.source == EvidenceSource::PromiseLedger));

        let fallback = context_pack_evidence(&context_pack(Vec::new()), &observation);
        assert_eq!(fallback[0].source, EvidenceSource::ChapterText);
        assert_eq!(fallback[0].reference, "Chapter-1");
    }
}
