impl WriterAgentKernel {
    pub fn today_five_summary(&self) -> TodayFiveSummary {
        let ledger = self.ledger_snapshot();
        let debt = self.story_debt_snapshot();
        let trace = self.trace_snapshot(20);
        let current_chapter = self.active_chapter.clone();
        let ranked_promises = ledger.open_promises.clone();
        let contract_debt = debt
            .entries
            .iter()
            .find(|entry| entry.category == StoryDebtCategory::StoryContract);
        let mission_debt = debt
            .entries
            .iter()
            .find(|entry| entry.category == StoryDebtCategory::ChapterMission);
        let canon_risk = debt.entries.iter().find(|entry| {
            matches!(
                entry.category,
                StoryDebtCategory::CanonRisk | StoryDebtCategory::TimelineRisk
            )
        });
        let open_promise = ranked_promises.first();
        let next_beat = ledger.next_beat.as_ref();
        let latest_result = current_chapter
            .as_deref()
            .and_then(|chapter| {
                ledger
                    .recent_chapter_results
                    .iter()
                    .find(|result| result.chapter_title == chapter)
            })
            .or_else(|| ledger.recent_chapter_results.first());
        let chapter_mission = ledger.active_chapter_mission.as_ref().or_else(|| {
            current_chapter.as_deref().and_then(|chapter| {
                ledger
                    .chapter_missions
                    .iter()
                    .find(|mission| mission.chapter_title == chapter)
            })
        });
        let guard_value = if !debt.entries.is_empty() {
            format!("{} active guards", debt.open_count)
        } else if trace
            .task_packets
            .first()
            .is_some_and(|packet| packet.foundation_complete)
        {
            "aligned".to_string()
        } else {
            "quiet".to_string()
        };
        let guard_detail = if !debt.entries.is_empty() {
            debt.entries
                .first()
                .map(|entry| entry.title.clone())
                .unwrap_or_else(|| "Story debt needs attention.".to_string())
        } else {
            "No active story debt surfaced.".to_string()
        };

        TodayFiveSummary {
            chapter_title: current_chapter.clone(),
            items: vec![
                TodayFiveItem {
                    slot: "guard".to_string(),
                    label: "Agent Guard".to_string(),
                    value: guard_value,
                    detail: guard_detail,
                    tone: if debt.canon_risk_count > 0 || debt.mission_count > 0 {
                        "danger".to_string()
                    } else if debt.open_count > 0 {
                        "accent".to_string()
                    } else {
                        "success".to_string()
                    },
                },
                TodayFiveItem {
                    slot: "contract".to_string(),
                    label: "Book Contract".to_string(),
                    value: contract_debt
                        .map(|entry| entry.title.clone())
                        .or_else(|| {
                            ledger
                                .story_contract
                                .as_ref()
                                .map(|contract| contract.reader_promise.clone())
                        })
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "No story contract".to_string()),
                    detail: contract_debt
                        .map(|entry| entry.message.clone())
                        .or_else(|| {
                            ledger
                                .story_contract
                                .as_ref()
                                .map(|contract| contract.main_conflict.clone())
                        })
                        .unwrap_or_else(|| "Set the book-level promise.".to_string()),
                    tone: if contract_debt.is_some() {
                        "danger".to_string()
                    } else {
                        "accent".to_string()
                    },
                },
                TodayFiveItem {
                    slot: "mission".to_string(),
                    label: "Chapter Mission".to_string(),
                    value: mission_debt
                        .map(|entry| entry.title.clone())
                        .or_else(|| chapter_mission.map(|mission| mission.mission.clone()))
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "No chapter mission".to_string()),
                    detail: mission_debt
                        .map(|entry| entry.message.clone())
                        .or_else(|| {
                            chapter_mission.map(|mission| mission.expected_ending.clone())
                        })
                        .unwrap_or_else(|| "Open a chapter mission.".to_string()),
                    tone: if mission_debt.is_some() {
                        "danger".to_string()
                    } else {
                        "accent".to_string()
                    },
                },
                TodayFiveItem {
                    slot: "promise".to_string(),
                    label: "Open Promise".to_string(),
                    value: open_promise
                        .map(|promise| promise.title.clone())
                        .unwrap_or_else(|| "No open promise".to_string()),
                    detail: open_promise
                        .map(|promise| {
                            if promise.expected_payoff.trim().is_empty() {
                                promise.description.clone()
                            } else {
                                format!("{} -> {}", promise.description, promise.expected_payoff)
                            }
                        })
                        .unwrap_or_else(|| "Ledger has no unresolved promise.".to_string()),
                    tone: if open_promise.is_some() {
                        "accent".to_string()
                    } else {
                        "success".to_string()
                    },
                },
                TodayFiveItem {
                    slot: "next".to_string(),
                    label: "Next Move".to_string(),
                    value: next_beat
                        .map(|beat| beat.goal.clone())
                        .or_else(|| latest_result.map(|result| result.summary.clone()))
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "No next move".to_string()),
                    detail: canon_risk
                        .map(|entry| entry.message.clone())
                        .or_else(|| latest_result.and_then(|result| result.new_conflicts.first().cloned()))
                        .unwrap_or_else(|| "No immediate blocker.".to_string()),
                    tone: if canon_risk.is_some() {
                        "danger".to_string()
                    } else {
                        "accent".to_string()
                    },
                },
            ],
        }
    }
}
