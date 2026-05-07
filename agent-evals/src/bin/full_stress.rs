// Forge Full-System Real-API Stress Test
// Exercises: real LLM → settlement extraction → entity apply → entity tracking
// Run: FORGE_STRESS_CHAPTER_COUNT=30 cargo run --bin full_stress -p agent-evals
use agent_writer_lib::chapter_generation::{
    build_basic_chapter_settlement_delta, ChapterResultDelta,
};
use agent_writer_lib::writer_agent::memory::WriterMemory;
use agent_writer_lib::writer_agent::settlement_apply::apply_chapter_settlement_delta;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChapterRecord {
    index: usize,
    chapter_title: String,
    final_chars: usize,
    outcome: String,
    draft_latency_ms: u64,
    settlement_applied: bool,
    character_count: usize,
    promise_count: usize,
    knowledge_count: usize,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FullReport {
    chapter_count: usize,
    completed: usize,
    failed: usize,
    compliance_rate: f64,
    avg_final_chars: f64,
    min_final_chars: usize,
    max_final_chars: usize,
    avg_latency_ms: f64,
    p95_latency_ms: u64,
    entity_stats_after: String,
    chapters: Vec<ChapterRecord>,
    errors: Vec<String>,
}

fn load_env() {
    let path = std::path::Path::new(".env");
    if !path.exists() { return; }
    for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            std::env::set_var(k.trim(), v);
        }
    }
}

impl Default for FullReport {
    fn default() -> Self {
        Self {
            chapter_count: 0, completed: 0, failed: 0, compliance_rate: 0.0,
            avg_final_chars: 0.0, min_final_chars: 0, max_final_chars: 0,
            avg_latency_ms: 0.0, p95_latency_ms: 0, entity_stats_after: String::new(),
            chapters: vec![], errors: vec![],
        }
    }
}

fn main() {
    load_env();
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set in .env");
    let api_base = std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.openai.com/v1".into());
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".into());
    let chapter_count: usize = std::env::var("FORGE_STRESS_CHAPTER_COUNT").unwrap_or_else(|_| "10".into()).parse().unwrap_or(10);
    let api_timeout: u64 = std::env::var("FORGE_STRESS_TIMEOUT_SECS").unwrap_or_else(|_| "180".into()).parse().unwrap_or(180);

    eprintln!("Forge Full Stress — {} chapters, {} @ {}", chapter_count, model, api_base);

    let memory = WriterMemory::open(Path::new(":memory:")).unwrap();
    let pid = "stress";
    memory.ensure_story_contract_seed(pid, "test", "fantasy", "quest", "redemption", "").unwrap();
    memory.upsert_character("林墨", &[], "protagonist", "主角，背负寻找北境宗主真相的使命").unwrap();
    memory.upsert_character("张三", &["三哥".to_string()], "supporting", "林墨同伴").unwrap();
    memory.upsert_character("李四", &[], "supporting", "北境商队幸存者").unwrap();
    memory.upsert_knowledge_item("寒玉戒指的下落", "objective", "seed").unwrap();
    memory.upsert_knowledge_item("北境宗主的真实身份", "objective", "seed").unwrap();
    let _ = memory.upsert_knowledge_ownership(1, "character", 1, "suspecting", "Chapter-1", "seed");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(api_timeout))
        .build().expect("build client");

    let mut report = FullReport { chapter_count, ..Default::default() };
    let mut latencies: Vec<u64> = vec![];

    for i in 1..=chapter_count {
        let title = format!("Chapter-{}", i);
        eprint!("[{}/{}] {} ", i, chapter_count, title);

        let mut errors: Vec<String> = vec![];
        let system = format!(
            "你是长篇小说的作者助手。故事：主角林墨寻找北境宗主真相。当前章节：{}。用中文写{}字左右，只输出章节正文。",
            title, if i == 1 { "3000" } else { "3500" }
        );

        let body = serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": format!("请写第{}章", i)}
            ],
            "max_tokens": 4096,
            "temperature": 0.8
        });

        let start = Instant::now();
        let generated = match client
            .post(format!("{}/chat/completions", api_base.trim_end_matches('/')))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body).send()
        {
            Ok(resp) => {
                let json: serde_json::Value = resp.json().unwrap_or_default();
                json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()
            }
            Err(e) => { errors.push(format!("API: {}", e)); String::new() }
        };
        let latency = start.elapsed().as_millis() as u64;
        latencies.push(latency);

        let chars = generated.chars().count();
        let outcome = if generated.is_empty() { "failed" }
            else if chars >= 3000 && chars <= 4000 { "valid" }
            else if chars < 3000 { "under" } else { "over" };

        if generated.is_empty() {
            report.failed += 1;
            report.errors.push(format!("Ch{} empty", i));
            eprintln!("FAILED");
        } else {
            let cr = ChapterResultDelta {
                summary: generated.chars().take(300).collect(),
                state_changes: vec![], character_progress: vec![],
                new_conflicts: vec![], new_clues: vec![],
                promise_updates: vec![], canon_updates: vec![],
            };
            let delta = build_basic_chapter_settlement_delta(pid, &title, "a001", &generated, 1000, &memory, vec![]);
            let setl_ok = apply_chapter_settlement_delta(&memory, pid, &delta).is_ok();
            if !setl_ok { errors.push("settlement failed".into()); }

            let cc = memory.list_characters(None).unwrap_or_default().len();
            let pc = memory.get_open_promise_summaries().unwrap_or_default().len();
            let kc = memory.list_knowledge_items(None).unwrap_or_default().len();

            report.chapters.push(ChapterRecord {
                index: i, chapter_title: title, final_chars: chars, outcome: outcome.into(),
                draft_latency_ms: latency, settlement_applied: setl_ok,
                character_count: cc, promise_count: pc, knowledge_count: kc, errors: errors.clone(),
            });
            report.completed += 1;

            eprintln!("chars={} {} lat={}ms setl={} ent={}c/{}p/{}k", chars, outcome, latency, setl_ok, cc, pc, kc);

            // Incremental report
            report.avg_final_chars = report.chapters.iter().map(|c| c.final_chars as f64).sum::<f64>() / report.completed as f64;
            report.compliance_rate = report.chapters.iter().filter(|c| c.outcome == "valid").count() as f64 / report.completed as f64;
            let mut s: Vec<u64> = latencies.clone(); s.sort();
            report.p95_latency_ms = s.get((s.len() as f64 * 0.95) as usize).copied().unwrap_or(0);
            report.avg_latency_ms = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
            report.min_final_chars = report.chapters.iter().map(|c| c.final_chars).min().unwrap_or(0);
            report.max_final_chars = report.chapters.iter().map(|c| c.final_chars).max().unwrap_or(0);
            if let Some(l) = report.chapters.last() { report.entity_stats_after = format!("{}c/{}p/{}k", l.character_count, l.promise_count, l.knowledge_count); }

            let rp = format!("reports/full_stress_{}.json", chapter_count);
            let _ = std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap_or_default());
        }
    }

    eprintln!("\nDone. {} completed, {} failed. Compliance: {:.0}% Avg: {:.0}ms P95: {}ms",
        report.completed, report.failed, report.compliance_rate*100.0, report.avg_latency_ms, report.p95_latency_ms);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
