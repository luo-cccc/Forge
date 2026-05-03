//! Generation, analysis, and creative drafting Tauri commands.

use crate::llm_runtime;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReviewItem {
    quote: String,
    #[serde(rename = "type")]
    review_type: String,
    issue: String,
    suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReviewReport {
    reviews: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParallelDraft {
    id: String,
    label: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ParallelDraftPayload {
    prefix: String,
    suffix: String,
    paragraph: String,
    selected_text: String,
    chapter_title: Option<String>,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn trim_parallel_draft(text: &str) -> String {
    text.trim_matches(|c: char| c == '`' || c.is_whitespace())
        .chars()
        .take(1200)
        .collect::<String>()
}

fn parse_parallel_drafts(raw: &str) -> Vec<ParallelDraft> {
    let labels = ["A 顺势推进", "B 冲突加压", "C 情绪转折"];
    let ids = ["a", "b", "c"];
    let mut drafts = Vec::new();
    let mut current_idx: Option<usize> = None;
    let mut current_text = String::new();

    let flush = |drafts: &mut Vec<ParallelDraft>,
                 current_idx: &mut Option<usize>,
                 current_text: &mut String| {
        let Some(idx) = current_idx.take() else {
            current_text.clear();
            return;
        };
        let text = trim_parallel_draft(current_text);
        current_text.clear();
        if text.is_empty() {
            return;
        }
        drafts.push(ParallelDraft {
            id: ids[idx].to_string(),
            label: labels[idx].to_string(),
            text,
        });
    };

    for line in raw.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .split_once(':')
            .or_else(|| trimmed.split_once('：'))
            .and_then(|(head, body)| {
                let idx = match head.trim().chars().next().map(|c| c.to_ascii_uppercase()) {
                    Some('A') => 0,
                    Some('B') => 1,
                    Some('C') => 2,
                    _ => return None,
                };
                Some((idx, body.trim_start()))
            });

        if let Some((idx, body)) = marker {
            flush(&mut drafts, &mut current_idx, &mut current_text);
            current_idx = Some(idx);
            current_text.push_str(body);
        } else if current_idx.is_some() {
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        }
    }
    flush(&mut drafts, &mut current_idx, &mut current_text);
    drafts.truncate(3);
    drafts
}

// ── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn analyze_chapter(
    _app: tauri::AppHandle,
    content: String,
) -> Result<Vec<ReviewItem>, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);

    let system_prompt = r#"You are a professional novel editor. Analyze the chapter and output a JSON object with a "reviews" array.

Each review must have:
- "quote": exact text from the chapter (copy verbatim, at least 10 characters)
- "type": one of "logic" | "ooc" | "pacing" | "prose"
- "issue": what the problem is
- "suggestion": how to fix it (in Chinese, specific rewrite suggestion)

Output ONLY the JSON object, no explanation outside. Example:
{"reviews":[{"quote":"他走出了房间","type":"prose","issue":"缺乏画面感","suggestion":"他推开吱呀作响的木门，幽暗的走廊里只有自己的脚步声在回荡。"}]}"#;

    let truncated = agent_harness_core::truncate_context(&content, 8000);
    let body = llm_runtime::chat_json(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": format!("Analyze this chapter:\n\n{}", truncated)}),
        ],
        60,
    )
    .await?;

    let report: ReviewReport =
        serde_json::from_value(body).map_err(|e| format!("Failed to parse review JSON: {}", e))?;

    Ok(report.reviews)
}

#[tauri::command]
pub async fn ask_project_brain(app: tauri::AppHandle, query: String) -> Result<(), String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);

    crate::brain_service::answer_query(&app, &settings, &query, |content| {
        let _ = app.emit(
            crate::events::AGENT_STREAM_CHUNK,
            crate::StreamChunk { content },
        );
        Ok(llm_runtime::StreamControl::Continue)
    })
    .await?;

    let _ = app.emit(
        crate::events::AGENT_STREAM_END,
        crate::StreamEnd {
            reason: "complete".to_string(),
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn generate_parallel_drafts(
    payload: ParallelDraftPayload,
) -> Result<Vec<ParallelDraft>, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let chapter = payload
        .chapter_title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or("当前章节");
    let focus = if payload.selected_text.trim().is_empty() {
        payload.paragraph.trim()
    } else {
        payload.selected_text.trim()
    };

    let prompt = format!(
        "你是中文小说共创写手。请顺着用户已有文本，生成三个不同方向的平行草稿。\n\
         输出格式必须严格为：\n\
         A: ...\nB: ...\nC: ...\n\
         每个版本 2-5 句，可以分段；不要解释，不要 Markdown。\n\
         A 偏顺势推进，B 偏冲突加压，C 偏情绪转折。\n\
         ## 章节\n{}\n## 光标前文\n{}\n## 光标后文\n{}\n## 当前焦点\n{}",
        chapter,
        agent_harness_core::truncate_context(&payload.prefix, 3000),
        agent_harness_core::truncate_context(&payload.suffix, 1000),
        focus,
    );

    let text = llm_runtime::chat_text(
        &settings,
        vec![serde_json::json!({"role": "user", "content": prompt})],
        false,
        45,
    )
    .await?;
    let drafts = parse_parallel_drafts(&text);
    if drafts.is_empty() {
        let fallback = trim_parallel_draft(&text);
        if fallback.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![ParallelDraft {
            id: "a".to_string(),
            label: "A 顺势推进".to_string(),
            text: fallback,
        }]);
    }
    Ok(drafts)
}

#[tauri::command]
pub async fn analyze_pacing(summaries: String) -> Result<String, String> {
    let api_key = crate::require_api_key()?;
    let settings = llm_runtime::settings(api_key);
    let text = llm_runtime::chat_text(
        &settings,
        vec![
            serde_json::json!({"role": "system", "content": "You are a structural editor. Analyze the chapter sequence for pacing issues, slow sections, abrupt transitions, and unresolved arcs. Be specific and concise."}),
            serde_json::json!({"role": "user", "content": format!("Chapter summaries:\n{}", summaries)}),
        ],
        false,
        60,
    )
    .await?;

    Ok(if text.is_empty() {
        "No analysis generated".to_string()
    } else {
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_parallel_drafts_keeps_multiline_branches() {
        let drafts = parse_parallel_drafts(
            "A: 林墨没有立刻回答。\n他只是把刀压低。\nB：门外忽然传来脚步声。\nC: 她看见他眼里的犹豫。",
        );

        assert_eq!(drafts.len(), 3);
        assert_eq!(drafts[0].id, "a");
        assert!(drafts[0].text.contains("把刀压低"));
        assert_eq!(drafts[1].label, "B 冲突加压");
        assert_eq!(drafts[2].id, "c");
    }

    #[test]
    fn trim_parallel_draft_removes_markdown_fence_noise() {
        assert_eq!(
            trim_parallel_draft("```\n林墨停下脚步。\n```"),
            "林墨停下脚步。"
        );
    }
}
