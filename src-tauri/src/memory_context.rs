use agent_harness_core::truncate_context;
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::{brain_service, events, llm_runtime, lock_hermes, resolve_api_key, AppState};

pub(crate) async fn auto_embed_chapter(app: &tauri::AppHandle, chapter_title: &str, content: &str) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);

    if let Err(e) = brain_service::embed_chapter(app, &settings, chapter_title, content).await {
        tracing::warn!(
            "Failed to update Project Brain for '{}': {}",
            chapter_title,
            e
        );
    }
}

#[derive(Serialize, Clone)]
struct Epiphany {
    skill: String,
    category: String,
    id: i64,
}

pub(crate) async fn extract_skills_from_recent(app: &tauri::AppHandle) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);

    let state = app.state::<AppState>();
    let recent = {
        let Ok(db) = lock_hermes(&state) else {
            tracing::error!("Failed to lock Hermes memory for recent interactions");
            return;
        };
        db.recent_interactions(20).unwrap_or_default()
    };

    if recent.len() < 4 {
        return;
    }

    let transcript: String = recent
        .iter()
        .map(|r| format!("[{}]: {}", r.role, r.content))
        .collect::<Vec<_>>()
        .join("\n");

    let parsed = match llm_runtime::chat_json(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": "You are a reflection engine. Analyze the recent interaction transcript and extract 1-2 reusable writing rules or user preferences. Output JSON: {\"skills\": [{\"skill\": \"...\", \"category\": \"style|character|pacing|preference\"}]}. If nothing new, output {\"skills\": []}."}),
            serde_json::json!({"role": "user", "content": format!("Transcript:\n{}", transcript)}),
        ],
        30,
    )
    .await
    {
        Ok(b) => b,
        Err(_) => return,
    };

    let skills = parsed["skills"].as_array();
    if let Some(skills) = skills {
        let Ok(db) = lock_hermes(&state) else {
            tracing::error!("Failed to lock Hermes memory for skill extraction");
            return;
        };
        for s in skills {
            let skill_text = s["skill"].as_str().unwrap_or("").to_string();
            let category = s["category"].as_str().unwrap_or("general").to_string();
            if skill_text.is_empty() || skill_text.len() < 10 {
                continue;
            }
            if let Ok(id) = db.insert_skill(&skill_text, &category) {
                let _ = app.emit(
                    events::AGENT_EPIPHANY,
                    Epiphany {
                        skill: skill_text,
                        category,
                        id,
                    },
                );
            }
        }
        let _ = db.consolidate();
        let _ = db.clean_old_sessions();
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

fn budget_items(items: &[String], max_tokens: usize) -> Vec<String> {
    let mut accepted = Vec::new();
    let mut consumed = 0;
    for item in items {
        let cost = estimate_tokens(item);
        if consumed + cost > max_tokens {
            break;
        }
        accepted.push(item.clone());
        consumed += cost;
    }
    accepted
}

pub(crate) fn build_context_injection(app: &tauri::AppHandle, query: &str) -> String {
    let state = app.state::<AppState>();
    let Ok(db) = lock_hermes(&state) else {
        tracing::error!("Failed to lock Hermes memory for context injection");
        return String::new();
    };

    let mut parts = Vec::new();

    if let Ok(profiles) = db.get_drift_profiles() {
        if !profiles.is_empty() {
            let profile_text: Vec<String> = profiles
                .iter()
                .map(|p| {
                    format!(
                        "- {}: {} (confidence {:.0}%)",
                        p.key,
                        p.value,
                        p.confidence * 100.0
                    )
                })
                .collect();
            let budgeted = budget_items(&profile_text, 200);
            if !budgeted.is_empty() {
                parts.push(format!(
                    "## User Preferences (learned over time)\n{}\n",
                    budgeted.join("\n")
                ));
            }
        }
    }

    if !query.is_empty() {
        if let Ok(skills) = db.search_skills(query) {
            if !skills.is_empty() {
                let skill_text: Vec<String> = skills
                    .iter()
                    .map(|s| format!("- [{}] {}", s.category, s.skill))
                    .collect();
                let budgeted = budget_items(&skill_text, 300);
                if !budgeted.is_empty() {
                    parts.push(format!(
                        "## Relevant Learned Skills\n{}\n",
                        budgeted.join("\n")
                    ));
                }
            }
        }
    }

    drop(db);

    parts.join("\n")
}

pub(crate) fn collect_user_profile_entries(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let db = lock_hermes(&state)?;
    let profiles = db
        .get_drift_profiles()
        .map_err(|e| format!("Failed to read user profile: {}", e))?;
    Ok(profiles
        .iter()
        .map(|profile| {
            format!(
                "- {}: {} (confidence {:.0}%)",
                profile.key,
                profile.value,
                profile.confidence * 100.0
            )
        })
        .collect())
}

fn writer_agent_memory_messages(
    observation: &crate::writer_agent::observation::WriterObservation,
) -> Vec<serde_json::Value> {
    let text = observation.prefix.trim();
    vec![
        serde_json::json!({
            "role": "system",
            "content": "你是中文长篇小说项目的记忆抽取器。只从用户已经写出的正文中抽取值得长期保存的设定 canon 和未回收伏笔 promises。不要发明正文没有的信息。输出严格 JSON，不要 Markdown。JSON schema: {\"canon\":[{\"kind\":\"character|object|place|rule|entity\",\"name\":\"\",\"aliases\":[],\"summary\":\"\",\"attributes\":{},\"confidence\":0.0}],\"promises\":[{\"kind\":\"open_question|object_in_motion|foreshadowing|mystery\",\"title\":\"\",\"description\":\"\",\"introducedChapter\":\"\",\"expectedPayoff\":\"\",\"priority\":0,\"confidence\":0.0}]}。只返回置信度 >=0.55 的条目，最多 canon 5 条、promises 5 条。"
        }),
        serde_json::json!({
            "role": "user",
            "content": format!(
                "章节: {}\n当前段落:\n{}\n\n章节文本摘录:\n{}",
                observation.chapter_title.as_deref().unwrap_or("current chapter"),
                observation.paragraph,
                truncate_context(text, 3_500)
            )
        }),
    ]
}

pub(crate) fn spawn_llm_memory_proposals(
    app: tauri::AppHandle,
    observation: crate::writer_agent::observation::WriterObservation,
) {
    let Some(api_key) = resolve_api_key() else {
        return;
    };
    let settings = llm_runtime::settings(api_key);
    let model = settings.model.clone();
    let messages = writer_agent_memory_messages(&observation);

    tokio::spawn(async move {
        let value = match llm_runtime::chat_json(&settings, messages, 20).await {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!("Writer Agent LLM memory extraction failed: {}", e);
                return;
            }
        };

        let state = app.state::<AppState>();
        let proposals = {
            let mut kernel = match state.writer_kernel.lock() {
                Ok(kernel) => kernel,
                Err(e) => {
                    tracing::error!("Writer kernel lock poisoned: {}", e);
                    return;
                }
            };
            kernel.create_llm_memory_proposals(observation, value, &model)
        };

        for proposal in proposals {
            let _ = app.emit(events::AGENT_PROPOSAL, proposal);
        }
    });
}
