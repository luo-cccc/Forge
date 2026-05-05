use crate::llm_runtime;

const REAL_API_TESTS_FLAG: &str = "FORGE_REAL_API_TESTS";

fn load_env() {
    dotenvy::dotenv().ok();
}

fn real_api_tests_enabled() -> bool {
    load_env();
    std::env::var(REAL_API_TESTS_FLAG)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn test_settings(test_name: &str) -> Option<llm_runtime::LlmSettings> {
    load_env();
    if !real_api_tests_enabled() {
        eprintln!(
            "skip {test_name}: set {REAL_API_TESTS_FLAG}=1 and OPENAI_API_KEY to run real provider tests"
        );
        return None;
    }

    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("{REAL_API_TESTS_FLAG}=1 requires OPENAI_API_KEY"));
    let settings = llm_runtime::settings(api_key);
    eprintln!(
        "run {test_name}: api_base={} model={} embedding_model={} chat={:?} json={:?} chapter={:?} ghost={:?} analysis={:?} parallel={:?} manual={:?} tool={:?} project_brain={:?}",
        settings.api_base,
        settings.model,
        settings.embedding_model,
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::GeneralChat),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::Json),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::ChapterDraft),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::GhostPreview),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::Analysis),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::ParallelDraft),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::ManualRewrite),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::ToolContinuation),
        llm_runtime::request_options(&settings, llm_runtime::LlmRequestProfile::ProjectBrainStream),
    );
    Some(settings)
}

fn preview_text(text: &str, max_chars: usize) -> String {
    let mut preview = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview
}

fn elapsed_ms(started: std::time::Instant) -> u128 {
    started.elapsed().as_millis()
}

async fn run_text_profile_smoke(
    settings: &llm_runtime::LlmSettings,
    test_name: &str,
    profile: llm_runtime::LlmRequestProfile,
    messages: Vec<serde_json::Value>,
    timeout_secs: u64,
) -> String {
    let started = std::time::Instant::now();
    let text = llm_runtime::chat_text_profile(settings, messages, profile, timeout_secs)
        .await
        .unwrap_or_else(|e| panic!("{test_name} failed: {e}"));
    assert!(!text.trim().is_empty(), "{test_name} response empty");
    eprintln!(
        "{test_name} ok profile={profile:?} latency_ms={} chars={} preview={}",
        elapsed_ms(started),
        text.chars().count(),
        preview_text(&text, 220)
    );
    text
}

#[tokio::test]
async fn health_check_models_endpoint() {
    let Some(settings) = test_settings("health_check_models_endpoint") else {
        return;
    };
    let started = std::time::Instant::now();
    let client = llm_runtime::client(30).expect("build client");
    let resp = client
        .get(format!(
            "{}/models",
            settings.api_base.trim_end_matches('/')
        ))
        .header("Authorization", format!("Bearer {}", settings.api_key))
        .send()
        .await
        .expect("GET /models");
    assert!(
        resp.status().is_success(),
        "GET /models failed: {}",
        resp.status()
    );
    let body = resp.text().await.unwrap();
    assert!(body.contains("\"data\""), "response missing data: {body}");
    eprintln!(
        "health_check_models_endpoint ok latency_ms={}",
        elapsed_ms(started)
    );
}

#[tokio::test]
async fn profile_smoke_feature_text_calls() {
    let Some(settings) = test_settings("profile_smoke_feature_text_calls") else {
        return;
    };

    let chapter = run_text_profile_smoke(
        &settings,
        "chapter_draft_profile",
        llm_runtime::LlmRequestProfile::ChapterDraft,
        vec![
            serde_json::json!({"role": "system", "content": "你是中文网文写手。只输出正文，不要解释。"}),
            serde_json::json!({"role": "user", "content": "写一小段80字以内的紧张对峙：林墨拔出寒影刀，张三知道旧债要还。"}),
        ],
        60,
    )
    .await;
    assert!(
        chapter.contains("林墨") || chapter.contains("寒影刀"),
        "chapter profile ignored story anchor: {chapter}"
    );

    let ghost = run_text_profile_smoke(
        &settings,
        "ghost_preview_profile",
        llm_runtime::LlmRequestProfile::GhostPreview,
        vec![
            serde_json::json!({"role": "system", "content": "你是低延迟续写助手。只补一句中文。"}),
            serde_json::json!({"role": "user", "content": "林墨拔出寒影刀，"}),
        ],
        30,
    )
    .await;
    assert!(
        ghost.chars().count() <= 120,
        "ghost response too long: {ghost}"
    );

    let analysis = run_text_profile_smoke(
        &settings,
        "analysis_profile",
        llm_runtime::LlmRequestProfile::Analysis,
        vec![
            serde_json::json!({"role": "system", "content": "你是小说结构编辑。用中文给出一句风险判断。"}),
            serde_json::json!({"role": "user", "content": "连续三章都只铺垫寒影刀来历，没有兑现冲突。"}),
        ],
        45,
    )
    .await;
    assert!(
        analysis.contains("风险") || analysis.contains("问题") || analysis.contains("兑现"),
        "analysis profile missed diagnostic framing: {analysis}"
    );

    let parallel = run_text_profile_smoke(
        &settings,
        "parallel_draft_profile",
        llm_runtime::LlmRequestProfile::ParallelDraft,
        vec![serde_json::json!({"role": "user", "content": "输出 A/B/C 三个中文续写方向，每个不超过20字：林墨拔出寒影刀，"})],
        45,
    )
    .await;
    assert!(
        parallel.contains('A') && parallel.contains('B') && parallel.contains('C'),
        "parallel draft profile missed A/B/C branches: {parallel}"
    );

    let manual = run_text_profile_smoke(
        &settings,
        "manual_rewrite_profile",
        llm_runtime::LlmRequestProfile::ManualRewrite,
        vec![serde_json::json!({"role": "user", "content": "把这句改得更有压迫感，30字以内：他走过去。"})],
        45,
    )
    .await;
    assert!(
        manual.contains('他') || manual.contains("压"),
        "manual rewrite profile missed rewrite target: {manual}"
    );

    let tool = run_text_profile_smoke(
        &settings,
        "tool_continuation_profile",
        llm_runtime::LlmRequestProfile::ToolContinuation,
        vec![serde_json::json!({"role": "user", "content": "继续一句中文：门外忽然传来"})],
        45,
    )
    .await;
    assert!(
        tool.chars().count() <= 160,
        "tool continuation too long: {tool}"
    );
}

#[tokio::test]
async fn chat_text_with_openrouter() {
    let Some(settings) = test_settings("chat_text_with_openrouter") else {
        return;
    };
    let messages = vec![
        serde_json::json!({"role": "system", "content": "You are a concise assistant. Reply in Chinese."}),
        serde_json::json!({"role": "user", "content": "用一句话描述月亮。"}),
    ];
    let started = std::time::Instant::now();
    let result = llm_runtime::chat_text(&settings, messages, false, 60).await;
    match &result {
        Ok(text) => {
            assert!(!text.is_empty(), "response empty");
            eprintln!(
                "chat_text ok latency_ms={} chars={} preview={}",
                elapsed_ms(started),
                text.chars().count(),
                preview_text(text, 180)
            );
        }
        Err(e) => panic!("chat_text failed: {e}"),
    }
}

#[tokio::test]
async fn chat_text_chinese_capability() {
    let Some(settings) = test_settings("chat_text_chinese_capability") else {
        return;
    };
    let messages = vec![
        serde_json::json!({"role": "system", "content": "你是一位中国文学教授。请用中文回复，保持简洁。"}),
        serde_json::json!({"role": "user", "content": "请续写以下句子（只需一句话）：林墨拔出寒影刀，"}),
    ];
    let started = std::time::Instant::now();
    let result = llm_runtime::chat_text(&settings, messages, false, 60).await;
    match &result {
        Ok(text) => {
            assert!(!text.is_empty(), "response empty");
            eprintln!(
                "chat_text_chinese ok latency_ms={} chars={} preview={}",
                elapsed_ms(started),
                text.chars().count(),
                preview_text(text, 180)
            );
        }
        Err(e) => panic!("chat_text_chinese failed: {e}"),
    }
}

#[tokio::test]
async fn chat_json_mode() {
    let Some(settings) = test_settings("chat_json_mode") else {
        return;
    };
    let messages = vec![
        serde_json::json!({"role": "system", "content": "Reply with valid JSON only."}),
        serde_json::json!({"role": "user", "content": r#"List 2 emotions in this JSON format: {"emotions": ["emotion1", "emotion2"]}"#}),
    ];
    let started = std::time::Instant::now();
    let result = llm_runtime::chat_json(&settings, messages, 60).await;
    match &result {
        Ok(val) => {
            eprintln!(
                "chat_json ok latency_ms={} response={val}",
                elapsed_ms(started)
            );
            assert!(val.get("emotions").is_some(), "missing emotions key: {val}");
            let arr = val["emotions"].as_array().expect("emotions is array");
            assert!(arr.len() >= 2, "expected >=2 emotions, got {arr:?}");
        }
        Err(e) => panic!("chat_json failed: {e}"),
    }
}

#[tokio::test]
async fn stream_chat_delta_received() {
    let Some(settings) = test_settings("stream_chat_delta_received") else {
        return;
    };
    let messages = vec![
        serde_json::json!({"role": "system", "content": "You are a helpful assistant. Keep it short."}),
        serde_json::json!({"role": "user", "content": "What is 2+2?"}),
    ];
    let deltas = std::sync::Mutex::new(Vec::<String>::new());
    let started = std::time::Instant::now();
    let result = llm_runtime::stream_chat_profile(
        &settings,
        messages,
        llm_runtime::LlmRequestProfile::ProjectBrainStream,
        60,
        |delta| {
            deltas.lock().unwrap().push(delta);
            Ok(llm_runtime::StreamControl::Continue)
        },
    )
    .await;
    match result {
        Ok(full) => {
            let d = deltas.lock().unwrap();
            eprintln!(
                "stream_chat ok latency_ms={} deltas={} chars={} preview={}",
                elapsed_ms(started),
                d.len(),
                full.chars().count(),
                preview_text(&full, 180)
            );
            assert!(!full.is_empty(), "stream produced no text");
            assert!(!d.is_empty(), "no deltas received");
            let reassembled: String = d.iter().map(|s| s.as_str()).collect();
            assert_eq!(full, reassembled, "full !== reassembled deltas");
        }
        Err(e) => panic!("stream_chat failed: {e}"),
    }
}

#[tokio::test]
async fn embed_returns_valid_vector() {
    let Some(settings) = test_settings("embed_returns_valid_vector") else {
        return;
    };
    let started = std::time::Instant::now();
    let result = llm_runtime::embed(&settings, "林墨拔出寒影刀", 30).await;
    match result {
        Ok(vec) => {
            eprintln!(
                "embed ok latency_ms={} dim={}",
                elapsed_ms(started),
                vec.len()
            );
            assert!(!vec.is_empty(), "empty embedding");
            assert!(vec.iter().any(|&v| v != 0.0), "all zero embedding");
        }
        Err(e) => panic!("embed failed: {e}"),
    }
}

#[tokio::test]
async fn chat_text_handles_long_chinese_context() {
    let Some(settings) = test_settings("chat_text_handles_long_chinese_context") else {
        return;
    };
    let context = "林墨拔出寒影刀。张三后退一步，脸色苍白。月华如水，刀光似雪。".repeat(20);
    let messages = vec![
        serde_json::json!({"role": "system", "content": "你是一个网络小说编辑。请用中文分析这段文字的氛围。"}),
        serde_json::json!({"role": "user", "content": context}),
    ];
    let started = std::time::Instant::now();
    let result = llm_runtime::chat_text(&settings, messages, false, 90).await;
    match &result {
        Ok(text) => {
            eprintln!(
                "long_context ok latency_ms={} chars={} preview={}",
                elapsed_ms(started),
                text.chars().count(),
                preview_text(text, 240)
            );
            assert!(!text.is_empty(), "response empty");
        }
        Err(e) => panic!("long_context failed: {e}"),
    }
}

#[tokio::test]
async fn stream_chat_early_cancel() {
    let Some(settings) = test_settings("stream_chat_early_cancel") else {
        return;
    };
    let messages =
        vec![serde_json::json!({"role": "user", "content": "Count from 1 to 100, one per line."})];
    let count = std::sync::atomic::AtomicUsize::new(0);
    let started = std::time::Instant::now();
    let result = llm_runtime::stream_chat_profile(
        &settings,
        messages,
        llm_runtime::LlmRequestProfile::ProjectBrainStream,
        60,
        |_delta| {
            let c = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if c >= 1 {
                return Err("cancel after first observed delta".to_string());
            }
            Ok(llm_runtime::StreamControl::Continue)
        },
    )
    .await;
    eprintln!(
        "early_cancel latency_ms={} deltas={} result={result:?}",
        elapsed_ms(started),
        count.load(std::sync::atomic::Ordering::Relaxed)
    );
    // Either we get partial content or the cancellation error propagates.
    let c = count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(c >= 1, "stream produced no delta before cancel");
}
