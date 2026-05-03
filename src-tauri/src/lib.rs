mod agent_runtime;
mod ambient_agents;
mod app_state;
mod brain_service;
pub mod chapter_generation;
mod commands;
mod editor_realtime;
mod llm_runtime;
mod manual_agent;
mod memory_context;
mod observation_bridge;
mod semantic_lint;
mod storage;
mod tool_bridge;
pub mod writer_agent;
pub(crate) use app_state::{
    lock_editor_prediction, lock_harness_state, lock_hermes, startup_error, AppState,
    EditorPredictionTask, HarnessState,
};
pub(crate) use editor_realtime::{
    abort_editor_prediction_task, emit_ambient_output, emit_editor_ghost_end,
    emit_writer_ghost_proposal, realtime_cowrite_enabled, spawn_llm_ghost_proposal,
    EditorGhostRenderTarget,
};
pub(crate) use memory_context::{
    auto_embed_chapter, build_context_injection, collect_user_profile_entries,
    extract_skills_from_recent, spawn_llm_memory_proposals,
};
pub(crate) use observation_bridge::{
    build_manual_writer_observation, build_writer_observation_from_editor_state,
    to_writer_observation, AskAgentContext, AskAgentMode, EditorStatePayload,
};
#[cfg(test)]
pub(crate) use observation_bridge::{split_context_for_cursor, test_editor_state_payload};
pub(crate) use semantic_lint::{
    find_semantic_lint, semantic_lint_enabled, EditorSemanticLint, SemanticLintPayload,
};

const KEYRING_SERVICE: &str = "agent-writer";
pub(crate) mod events {
    pub const AGENT_CHAIN_OF_THOUGHT: &str = "agent-chain-of-thought";
    pub const AGENT_EPIPHANY: &str = "agent-epiphany";
    pub const AGENT_ERROR: &str = "agent-error";
    pub const AGENT_PROPOSAL: &str = "agent-proposal";
    pub const AGENT_SUGGESTION: &str = "agent-suggestion";
    pub const AGENT_STREAM_CHUNK: &str = "agent-stream-chunk";
    pub const AGENT_STREAM_END: &str = "agent-stream-end";
    pub const BATCH_STATUS: &str = "batch-status";
    pub const CHAPTER_GENERATION: &str = "chapter-generation";
    pub const EDITOR_GHOST_CHUNK: &str = "editor-ghost-chunk";
    pub const EDITOR_GHOST_END: &str = "editor-ghost-end";
    pub const EDITOR_SEMANTIC_LINT: &str = "editor-semantic-lint";
    pub const EDITOR_ENTITY_CARD: &str = "editor-entity-card";
    pub const EDITOR_HOVER_HINT: &str = "editor-hover-hint";
    pub const INLINE_WRITER_OPERATION: &str = "inline-writer-operation";
    pub const STORYBOARD_MARKER: &str = "storyboard-marker";
}

fn load_api_key_from_keychain() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "openai").ok()?;
    entry.get_password().ok()
}

pub(crate) fn safe_filename_component(raw: &str) -> String {
    let mut safe = String::new();
    let mut last_was_dash = false;
    for ch in raw.trim().to_lowercase().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch == ' ' || ch == '-' || ch == '_' {
            Some('-')
        } else {
            None
        };

        if let Some(ch) = next {
            if ch == '-' {
                if last_was_dash {
                    continue;
                }
                last_was_dash = true;
            } else {
                last_was_dash = false;
            }
            safe.push(ch);
        }
    }
    let safe = safe.trim_matches('-');
    if safe.is_empty() {
        "default".to_string()
    } else {
        safe.to_string()
    }
}

pub(crate) fn log_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .map(|p| {
                std::path::PathBuf::from(p)
                    .join("agent-writer")
                    .join("logs")
            })
            .map_err(|_| "APPDATA not set".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .map(|p| p.join(".config").join("agent-writer").join("logs"))
            .ok_or_else(|| "Home dir not found".to_string())
    }
}

pub(crate) fn resolve_api_key() -> Option<String> {
    load_api_key_from_keychain()
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .filter(|k| !k.is_empty())
}

pub(crate) fn require_api_key() -> Result<String, String> {
    resolve_api_key().ok_or_else(|| "API key not set. Go to Settings.".to_string())
}
use serde::Serialize;
use tauri::{Emitter, Manager};
#[derive(Serialize, Clone)]
pub(crate) struct StreamChunk {
    pub(crate) content: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct StreamEnd {
    pub(crate) reason: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InlineWriterOperationEvent {
    pub(crate) request_id: String,
    pub(crate) proposal: writer_agent::proposal::AgentProposal,
    pub(crate) operation: writer_agent::operation::WriterOperation,
}

use commands::backups::{get_project_storage_diagnostics, list_file_backups, restore_file_backup};
use commands::chapters::{
    create_chapter, get_chapter_revision, load_chapter, read_project_dir, rename_chapter_file,
    save_chapter,
};
use commands::diagnostics::{export_diagnostic_logs, export_writer_agent_trajectory};
use commands::editor::{abort_editor_prediction, report_editor_state, report_semantic_lint_state};
use commands::generation::{
    analyze_chapter, analyze_pacing, ask_project_brain, batch_generate_chapter,
    generate_chapter_autonomous, generate_parallel_drafts,
};
use commands::graph::get_project_graph_data;
use commands::lore::{delete_lore_entry, get_lorebook, save_lore_entry};
use commands::manual_agent::ask_agent;
use commands::outline::{
    delete_outline_node, get_outline, reorder_outline_nodes, save_outline_node,
    update_outline_status,
};
use commands::settings::{check_api_key, set_api_key};
use commands::writer_agent::{
    agent_observe, apply_proposal_feedback, approve_writer_operation, get_agent_domain_profile,
    get_agent_kernel_status, get_agent_tools, get_effective_agent_tool_inventory,
    get_story_debt_snapshot, get_story_review_queue, get_writer_agent_ledger,
    get_writer_agent_pending_proposals, get_writer_agent_status, get_writer_agent_trace,
    record_implicit_ghost_rejection, record_writer_operation_durable_save,
};
pub(crate) use manual_agent::ManualAgentTurn;

pub(crate) fn audit_project_file_write(
    app: &tauri::AppHandle,
    scope: &str,
    title: &str,
    decision: &str,
    rationale: &str,
    sources: &[String],
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(kernel) = state.writer_kernel.lock() else {
        return;
    };
    if let Err(e) = kernel
        .memory
        .record_decision(scope, title, decision, &[], rationale, sources)
    {
        tracing::warn!("WriterAgent file-write audit failed: {}", e);
    }
}

pub(crate) fn backup_target_label(target: &storage::BackupTarget) -> String {
    match target {
        storage::BackupTarget::Lorebook => "lorebook".to_string(),
        storage::BackupTarget::Outline => "outline".to_string(),
        storage::BackupTarget::ProjectBrain => "project_brain".to_string(),
        storage::BackupTarget::Chapter { title } => format!("chapter:{}", title),
    }
}

pub(crate) fn observe_chapter_save(
    app: &tauri::AppHandle,
    title: &str,
    content: &str,
    revision: &str,
) -> Result<(), String> {
    let project_id = storage::active_project_id(app)?;
    let text = html_to_plain_text(content);
    let paragraph = last_meaningful_paragraph(&text).unwrap_or_else(|| char_tail(&text, 400));
    let cursor = text.chars().count();
    let observation = writer_agent::observation::WriterObservation {
        id: format!("save-{}", agent_runtime::now_ms()),
        created_at: agent_runtime::now_ms(),
        source: writer_agent::observation::ObservationSource::ChapterSave,
        reason: writer_agent::observation::ObservationReason::Save,
        project_id,
        chapter_title: Some(title.to_string()),
        chapter_revision: Some(revision.to_string()),
        cursor: Some(writer_agent::observation::TextRange {
            from: cursor,
            to: cursor,
        }),
        selection: None,
        prefix: char_tail(&text, 3_000),
        suffix: String::new(),
        paragraph,
        full_text_digest: Some(storage::content_revision(&text)),
        editor_dirty: false,
    };

    let state = app.state::<AppState>();
    let proposals = {
        let mut kernel = state.writer_kernel.lock().map_err(|e| e.to_string())?;
        refresh_kernel_canon_from_lorebook(app, &mut kernel);
        kernel.observe(observation.clone())?
    };
    for proposal in proposals {
        app.emit(events::AGENT_PROPOSAL, proposal)
            .map_err(|e| format!("Failed to emit agent proposal: {}", e))?;
    }

    if resolve_api_key().is_some() {
        spawn_llm_memory_proposals(app.clone(), observation);
    }

    Ok(())
}

pub(crate) fn observe_generated_chapter_result(
    app: &tauri::AppHandle,
    saved: &chapter_generation::SaveGeneratedChapterOutput,
    generated_content: &str,
) {
    if let Err(e) = observe_chapter_save(
        app,
        &saved.chapter_title,
        generated_content,
        &saved.new_revision,
    ) {
        tracing::warn!(
            "WriterAgent generated-chapter result feedback failed for '{}': {}",
            saved.chapter_title,
            e
        );
    }
}

pub(crate) fn last_meaningful_paragraph(text: &str) -> Option<String> {
    text.split('\n')
        .rev()
        .map(str::trim)
        .find(|line| line.chars().count() >= 8)
        .map(ToString::to_string)
}

pub(crate) fn html_to_plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut entity = String::new();
    let mut in_entity = false;

    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            continue;
        }

        if in_entity {
            if ch == ';' {
                out.push_str(&decode_html_entity(&entity));
                entity.clear();
                in_entity = false;
            } else if entity.chars().count() < 12 {
                entity.push(ch);
            } else {
                out.push('&');
                out.push_str(&entity);
                out.push(ch);
                entity.clear();
                in_entity = false;
            }
            continue;
        }

        match ch {
            '<' => in_tag = true,
            '&' => in_entity = true,
            '\r' => {}
            _ => out.push(ch),
        }
    }

    if in_entity {
        out.push('&');
        out.push_str(&entity);
    }

    out.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn decode_html_entity(entity: &str) -> String {
    match entity {
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "nbsp" => " ".to_string(),
        entity if entity.starts_with("#x") || entity.starts_with("#X") => {
            u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_else(|| format!("&{};", entity))
        }
        entity if entity.starts_with('#') => entity[1..]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_else(|| format!("&{};", entity)),
        _ => format!("&{};", entity),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentKernelStatus {
    tool_generation: u64,
    tool_count: usize,
    effective_tool_count: usize,
    blocked_tool_count: usize,
    model_callable_tool_count: usize,
    approval_required_tool_count: usize,
    write_tool_count: usize,
    domain_id: String,
    capability_count: usize,
    quality_gate_count: usize,
    trace_enabled: bool,
}

pub(crate) fn refresh_kernel_canon_from_lorebook(
    app: &tauri::AppHandle,
    kernel: &mut writer_agent::WriterAgentKernel,
) {
    let entries = match storage::load_lorebook(app) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("WriterAgent canon refresh skipped lorebook: {}", e);
            return;
        }
    };

    for entry in entries {
        let keyword = entry.keyword.trim();
        if keyword.is_empty() {
            continue;
        }

        let mut attributes = serde_json::Map::new();
        if let Some(weapon) = extract_weapon_from_lore(&entry.content) {
            attributes.insert("weapon".to_string(), serde_json::Value::String(weapon));
        }

        if attributes.is_empty() {
            continue;
        }

        let summary: String = entry.content.chars().take(240).collect();
        let aliases = Vec::<String>::new();
        let _ = kernel.memory.upsert_canon_entity(
            "character",
            keyword,
            &aliases,
            &summary,
            &serde_json::Value::Object(attributes),
            0.8,
        );
    }
}

fn extract_weapon_from_lore(content: &str) -> Option<String> {
    if !["武器", "惯用", "用刀", "用剑", "佩刀", "佩剑", "兵器"]
        .iter()
        .any(|cue| content.contains(cue))
    {
        return None;
    }

    [
        "寒影刀",
        "长剑",
        "短剑",
        "匕首",
        "弓",
        "枪",
        "棍",
        "鞭",
        "斧",
        "刀",
        "剑",
    ]
    .iter()
    .find(|weapon| content.contains(**weapon))
    .map(|weapon| (*weapon).to_string())
}

pub(crate) fn render_writer_context_pack(
    pack: &writer_agent::context::WritingContextPack,
) -> String {
    writer_agent::kernel::render_context_pack_for_prompt(pack)
}

pub(crate) fn char_tail(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars().rev().take(max_chars).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();

    // Shared context cache for ambient agents
    let cache: std::sync::Arc<tokio::sync::Mutex<ambient_agents::context_fetcher::ContextCache>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(
            ambient_agents::context_fetcher::ContextCache::default(),
        ));

    // Capture clones before the setup closure
    let cache1 = cache.clone();
    let cache_for_state = cache.clone();

    if let Err(e) = tauri::Builder::default()
        .setup(move |app| {
            let app_state = AppState::open(app.handle()).map_err(startup_error)?;

            let mut event_bus = agent_harness_core::AmbientEventBus::new(256);
            let ah = app.handle().clone();
            let output_app = ah.clone();
            event_bus.set_output_handler(std::sync::Arc::new(move |output| {
                emit_ambient_output(&output_app, output);
            }));

            event_bus.spawn_agent(std::sync::Arc::new(
                ambient_agents::context_fetcher::ContextFetcherAgent {
                    app: ah.clone(),
                    cache: cache1,
                },
            ));

            app.manage(std::sync::Mutex::new(event_bus));
            app.manage(app_state);
            app.manage(cache_for_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            abort_editor_prediction,
            report_editor_state,
            report_semantic_lint_state,
            ask_agent,
            agent_observe,
            get_agent_domain_profile,
            get_agent_kernel_status,
            get_project_storage_diagnostics,
            list_file_backups,
            restore_file_backup,
            get_writer_agent_status,
            get_writer_agent_ledger,
            get_writer_agent_pending_proposals,
            get_story_review_queue,
            get_story_debt_snapshot,
            get_writer_agent_trace,
            apply_proposal_feedback,
            record_implicit_ghost_rejection,
            approve_writer_operation,
            record_writer_operation_durable_save,
            get_agent_tools,
            get_effective_agent_tool_inventory,
            get_lorebook,
            save_lore_entry,
            delete_lore_entry,
            read_project_dir,
            create_chapter,
            save_chapter,
            load_chapter,
            get_chapter_revision,
            get_outline,
            save_outline_node,
            delete_outline_node,
            update_outline_status,
            reorder_outline_nodes,
            batch_generate_chapter,
            generate_chapter_autonomous,
            analyze_chapter,
            ask_project_brain,
            generate_parallel_drafts,
            get_project_graph_data,
            analyze_pacing,
            rename_chapter_file,
            set_api_key,
            check_api_key,
            export_diagnostic_logs,
            export_writer_agent_trajectory
        ])
        .run(tauri::generate_context!())
    {
        let message = format!("Error while running Tauri application: {}", e);
        tracing::error!("{}", message);
        let _ = msgbox::create("Agent-Writer Error", &message, msgbox::IconType::Error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_protocol_does_not_expose_legacy_commands_or_xml_actions() {
        let protocol = include_str!("../../src/protocol.ts");
        assert!(!protocol.contains("harness_echo"));
        assert!(!protocol.contains("harnessEcho"));
        assert!(!protocol.contains("ACTION_INSERT"));
        assert!(!protocol.contains("ACTION_REPLACE"));
        assert!(!protocol.contains("extractActions"));
    }

    fn test_context_pack() -> writer_agent::context::WritingContextPack {
        writer_agent::context::WritingContextPack {
            task: writer_agent::context::AgentTask::InlineRewrite,
            total_chars: 18,
            budget_limit: 32,
            budget_report: writer_agent::context::ContextBudgetReport {
                total_budget: 32,
                used: 18,
                wasted: 14,
                source_reports: vec![
                    writer_agent::context::SourceReport {
                        source: "SelectedText".to_string(),
                        requested: 20,
                        provided: 12,
                        truncated: false,
                        reason:
                            "InlineRewrite required source reserved 400 chars before priority fill."
                                .to_string(),
                        truncation_reason: None,
                    },
                    writer_agent::context::SourceReport {
                        source: "CursorPrefix".to_string(),
                        requested: 20,
                        provided: 6,
                        truncated: true,
                        reason:
                            "InlineRewrite required source reserved 160 chars before priority fill."
                                .to_string(),
                        truncation_reason: Some(
                            "Source content was limited by its per-source budget of 20 chars."
                                .to_string(),
                        ),
                    },
                ],
            },
            sources: vec![
                writer_agent::context::ContextExcerpt {
                    source: writer_agent::context::ContextSource::SelectedText,
                    content: "林墨握紧寒影刀。".to_string(),
                    char_count: 8,
                    truncated: false,
                    priority: 10,
                    evidence_ref: Some("selection".to_string()),
                },
                writer_agent::context::ContextExcerpt {
                    source: writer_agent::context::ContextSource::CursorPrefix,
                    content: "张三退后".to_string(),
                    char_count: 4,
                    truncated: true,
                    priority: 9,
                    evidence_ref: None,
                },
            ],
        }
    }

    #[test]
    fn render_writer_context_pack_includes_budget_report() {
        let rendered = render_writer_context_pack(&test_context_pack());

        assert!(rendered.contains("# ContextPack Budget"));
        assert!(rendered.contains("task: InlineRewrite"));
        assert!(rendered.contains("used/budget: 18/32"));
        assert!(rendered.contains("wasted: 14"));
        assert!(rendered.contains("- CursorPrefix: requested 20, provided 6, truncated true"));
        assert!(rendered.contains("reason: InlineRewrite required source"));
        assert!(rendered.contains("truncation: Source content was limited"));
        assert!(rendered.contains("# ContextPack Sources"));
        assert!(rendered.contains("## SelectedText"));
        assert!(rendered.contains("林墨握紧寒影刀。"));
    }

    #[test]
    fn inline_operation_messages_include_context_budget_report() {
        let observation = build_manual_writer_observation(
            "改写得更紧张",
            "林墨握紧寒影刀。张三退后。",
            "林墨握紧寒影刀。",
            "寒影刀",
            None,
            "novel-a",
        );
        let messages = commands::manual_agent::writer_agent_inline_operation_messages(
            "改写得更紧张",
            &observation,
            &test_context_pack(),
        );
        let user_content = messages[1]["content"].as_str().unwrap();

        assert!(user_content.contains("ContextPack:"));
        assert!(user_content.contains("# ContextPack Budget"));
        assert!(user_content.contains("used/budget: 18/32"));
        assert!(user_content.contains("truncated true"));
    }

    #[test]
    fn manual_observation_uses_char_cursor_and_selection_range() {
        let context = "林墨拔出寒影刀。\n张三后退一步。";
        let observation =
            build_manual_writer_observation("这段有冲突吗", context, "", "寒影刀", None, "test");

        assert_eq!(
            observation.source,
            writer_agent::observation::ObservationSource::ManualRequest
        );
        assert_eq!(
            observation.reason,
            writer_agent::observation::ObservationReason::Explicit
        );
        assert_eq!(
            observation.cursor.unwrap().to,
            context.chars().count(),
            "cursor must use character index, not UTF-8 bytes"
        );
        let selection = observation.selection.unwrap();
        assert_eq!(selection.from, 4);
        assert_eq!(selection.to, 7);
        assert_eq!(selection.text, "寒影刀");
        assert_eq!(observation.paragraph, "寒影刀");
    }

    #[test]
    fn manual_agent_history_keeps_only_matching_project_recent_first() {
        let mut history = manual_agent::ManualAgentHistory::default();
        history.append(ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 1,
            observation_id: "obs-a-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            user: "第一问".to_string(),
            assistant: "第一答".to_string(),
            source_refs: vec!["Lorebook".to_string()],
        });
        history.append(ManualAgentTurn {
            project_id: "novel-b".to_string(),
            created_at: 2,
            observation_id: "obs-b-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            user: "另一项目".to_string(),
            assistant: "不应出现".to_string(),
            source_refs: Vec::new(),
        });
        history.append(ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 3,
            observation_id: "obs-a-2".to_string(),
            chapter_title: Some("第二章".to_string()),
            user: "第二问".to_string(),
            assistant: "第二答".to_string(),
            source_refs: vec!["Outline".to_string()],
        });

        let recent = history.recent_for_project("novel-a", 8, 12_000);

        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].user, "第一问");
        assert_eq!(recent[1].user, "第二问");
        assert_eq!(recent[1].source_refs, vec!["Outline".to_string()]);
    }

    #[test]
    fn manual_agent_history_applies_turn_and_char_budgets() {
        let mut history = manual_agent::ManualAgentHistory::default();
        for idx in 0..5 {
            history.append(ManualAgentTurn {
                project_id: "novel-a".to_string(),
                created_at: idx,
                observation_id: format!("obs-a-{}", idx),
                chapter_title: None,
                user: format!("问题{}", idx),
                assistant: "答复".repeat(80),
                source_refs: Vec::new(),
            });
        }

        let recent = history.recent_for_project("novel-a", 2, 12_000);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].user, "问题3");
        assert_eq!(recent[1].user, "问题4");

        let budgeted = history.recent_for_project("novel-a", 8, 180);
        assert_eq!(budgeted.len(), 1);
        assert_eq!(budgeted[0].user, "问题4");
    }

    #[test]
    fn manual_agent_history_messages_restore_dialog_roles() {
        let messages = manual_agent::manual_agent_history_messages(&[ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 42,
            observation_id: "obs-a-42".to_string(),
            chapter_title: Some("第三章".to_string()),
            user: "上一轮怎么处理张三？".to_string(),
            assistant: "先让张三隐瞒玉佩。".to_string(),
            source_refs: vec!["PromiseLedger".to_string()],
        }]);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert!(messages[0]
            .content
            .as_ref()
            .unwrap()
            .contains("上一轮怎么处理张三"));
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .contains("PromiseLedger"));
    }

    #[test]
    fn merge_manual_agent_history_dedupes_persisted_and_runtime_turns() {
        let persisted = vec![ManualAgentTurn {
            project_id: "novel-a".to_string(),
            created_at: 1,
            observation_id: "obs-a-1".to_string(),
            chapter_title: None,
            user: "已持久化问题".to_string(),
            assistant: "已持久化答复".to_string(),
            source_refs: Vec::new(),
        }];
        let runtime = vec![
            ManualAgentTurn {
                project_id: "novel-a".to_string(),
                created_at: 1,
                observation_id: "obs-a-1".to_string(),
                chapter_title: None,
                user: "重复问题".to_string(),
                assistant: "重复答复".to_string(),
                source_refs: Vec::new(),
            },
            ManualAgentTurn {
                project_id: "novel-a".to_string(),
                created_at: 2,
                observation_id: "obs-a-2".to_string(),
                chapter_title: None,
                user: "新问题".to_string(),
                assistant: "新答复".to_string(),
                source_refs: Vec::new(),
            },
        ];

        let merged =
            manual_agent::merge_manual_agent_history("novel-a", persisted, runtime, 8, 12_000);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].user, "已持久化问题");
        assert_eq!(merged[1].user, "新问题");
    }

    #[test]
    fn split_context_for_cursor_keeps_suffix_after_cursor() {
        let (prefix, suffix) = split_context_for_cursor("甲乙丙丁戊", 3, 2, 2);
        assert_eq!(prefix, "乙丙");
        assert_eq!(suffix, "丁戊");
    }

    #[test]
    fn html_to_plain_text_keeps_editor_text_without_tags() {
        let text = html_to_plain_text(
            "<p>林墨&nbsp;拔出寒影刀。</p><p>张三说：&quot;别动&quot;&amp;后退。</p>",
        );

        assert_eq!(text, "林墨 拔出寒影刀。\n张三说：\"别动\"&后退。");
    }

    #[test]
    fn last_meaningful_paragraph_ignores_short_trailing_lines() {
        let paragraph = last_meaningful_paragraph("第一段很长。\n短\n最后一段足够长。").unwrap();
        assert_eq!(paragraph, "最后一段足够长。");
    }

    #[test]
    fn editor_state_observation_uses_text_cursor_for_kernel() {
        let payload = test_editor_state_payload("林墨拔出", "寒影刀", "林墨拔出", 99, Some(4));
        let observation = build_writer_observation_from_editor_state(&payload, "test");

        assert_eq!(
            observation.cursor.unwrap().to,
            4,
            "WriterAgent coordinates must be plain-text character indexes"
        );
        assert_eq!(observation.prefix, "林墨拔出");
        assert_eq!(observation.suffix, "寒影刀");
        assert_eq!(
            observation.reason,
            writer_agent::observation::ObservationReason::Idle
        );
    }

    #[test]
    fn editor_state_observation_falls_back_to_prefix_char_count() {
        let payload = test_editor_state_payload("林墨拔出", "", "", 99, None);
        let observation = build_writer_observation_from_editor_state(&payload, "test");

        assert_eq!(observation.cursor.unwrap().to, "林墨拔出".chars().count());
        assert!(observation.paragraph.contains("Current paragraph"));
    }

    #[test]
    fn migrate_legacy_db_copies_when_target_is_missing() {
        let root = std::env::temp_dir().join(format!(
            "forge-db-migrate-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy_path = root.join("legacy.db");
        let target_path = root.join("app-data").join("writer_memory.db");

        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&legacy_path, b"legacy-memory").unwrap();

        app_state::migrate_legacy_db_if_needed(&target_path, Some(legacy_path.clone())).unwrap();

        assert_eq!(std::fs::read(&target_path).unwrap(), b"legacy-memory");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migrate_legacy_db_does_not_overwrite_existing_target() {
        let root = std::env::temp_dir().join(format!(
            "forge-db-migrate-existing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy_path = root.join("legacy.db");
        let target_dir = root.join("app-data");
        let target_path = target_dir.join("writer_memory.db");

        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(&legacy_path, b"legacy-memory").unwrap();
        std::fs::write(&target_path, b"current-memory").unwrap();

        app_state::migrate_legacy_db_if_needed(&target_path, Some(legacy_path)).unwrap();

        assert_eq!(std::fs::read(&target_path).unwrap(), b"current-memory");
        let _ = std::fs::remove_dir_all(&root);
    }
}
