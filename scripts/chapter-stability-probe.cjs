const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const reportPath = path.join(repoRoot, "reports", "chapter_stability_probe.json");
const anchorCarryConfig = JSON.parse(
  fs.readFileSync(path.join(repoRoot, "config", "anchor-carry-heuristics.json"), "utf8"),
);
const requestProfileConfig = JSON.parse(
  fs.readFileSync(path.join(repoRoot, "config", "llm-request-profiles.json"), "utf8"),
);

function loadDotEnv() {
  const envPath = path.join(repoRoot, ".env");
  if (!fs.existsSync(envPath)) return;
  const lines = fs.readFileSync(envPath, "utf8").split(/\r?\n/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).trim();
    let value = trimmed.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    if (!process.env[key]) process.env[key] = value;
  }
}

function parseNumber(name, fallback, min, max) {
  const raw = process.env[name];
  const value = raw == null ? NaN : Number(raw);
  if (!Number.isFinite(value) || value < min || value > max) return fallback;
  return value;
}

function parseInteger(name, fallback, min, max) {
  return Math.round(parseNumber(name, fallback, min, max));
}

function parseBool(name, fallback) {
  const raw = process.env[name];
  if (raw == null) return fallback;
  const value = raw.trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(value)) return true;
  if (["0", "false", "no", "off"].includes(value)) return false;
  return fallback;
}

function profileOptions() {
  const defaults = requestProfileConfig.chapter_draft;
  return {
    temperature: parseNumber("OPENAI_CHAPTER_DRAFT_TEMPERATURE", defaults.temperature, 0, 2),
    maxTokens: parseInteger("OPENAI_CHAPTER_DRAFT_MAX_TOKENS", defaults.maxTokens, 16, 65536),
    disableReasoning: parseBool(
      "OPENAI_CHAPTER_DRAFT_DISABLE_REASONING",
      defaults.disableReasoning,
    ),
  };
}

function preview(text, limit = 180) {
  const chars = [...(text ?? "")];
  return chars.slice(0, limit).join("") + (chars.length > limit ? "..." : "");
}

function scoreAgainstTerms(text, terms) {
  const hits = terms.filter((term) => text.includes(term));
  return { hits, hitRate: terms.length ? hits.length / terms.length : 1 };
}

function scoreAnchorCarry(text, anchors) {
  const delimiterSet = new Set(
    anchorCarryConfig.sentenceDelimiters.flatMap((delimiter) => [...delimiter]),
  );
  const sentences = [];
  let current = "";
  for (const ch of [...String(text ?? "")]) {
    current += ch;
    if (delimiterSet.has(ch)) {
      if (current.trim()) sentences.push(current.trim());
      current = "";
    }
  }
  if (current.trim()) sentences.push(current.trim());

  const items = anchors.map((anchor) => {
    const related = sentences.filter((sentence) => sentence.includes(anchor));
    const carryModes = [];
    const supportingTerms = [];
    for (const sentence of related) {
      for (const mode of anchorCarryConfig.modes) {
        const matched = mode.terms.filter((term) => sentence.includes(term)).slice(0, 3);
        if (matched.length > 0) {
          carryModes.push(mode.id);
          supportingTerms.push(...matched);
        }
      }
    }
    return {
      anchor,
      mentioned: related.length > 0,
      carried: carryModes.length > 0,
      carryModes: [...new Set(carryModes)].sort(),
      supportingTerms: [...new Set(supportingTerms)].sort(),
    };
  });
  const mentionedCount = items.filter((item) => item.mentioned).length;
  const carriedCount = items.filter((item) => item.carried).length;
  return {
    mentionRate: items.length ? mentionedCount / items.length : 1,
    carryRate: items.length ? carriedCount / items.length : 1,
    items,
  };
}

function average(values) {
  return values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : 0;
}

function min(values) {
  return values.length ? values.reduce((a, b) => Math.min(a, b), values[0]) : 0;
}

function max(values) {
  return values.length ? values.reduce((a, b) => Math.max(a, b), values[0]) : 0;
}

function stddev(values) {
  if (!values.length) return 0;
  const mean = average(values);
  const variance = average(values.map((value) => (value - mean) ** 2));
  return Math.sqrt(variance);
}

async function postJson(url, apiKey, body, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const started = Date.now();
  try {
    const res = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
      signal: controller.signal,
    });
    const raw = await res.text();
    const latencyMs = Date.now() - started;
    if (!res.ok) {
      return {
        ok: false,
        latencyMs,
        status: res.status,
        error: raw.replace(/sk-[A-Za-z0-9_-]+/g, "[REDACTED]").slice(0, 500),
      };
    }
    return { ok: true, latencyMs, status: res.status, json: JSON.parse(raw) };
  } finally {
    clearTimeout(timer);
  }
}

async function runChapterDraft(apiBase, apiKey, model, payload) {
  const options = profileOptions();
  const messages = [
    {
      role: "system",
      content:
        "You are a professional Chinese novelist drafting a complete chapter. Use the provided project context, preserve continuity, and write only chapter prose. Do not include analysis, markdown fences, action tags, or meta commentary. Preserve the named anchors, unresolved debts, and chapter mission constraints from the context; do not silently drop active named entities, artifacts, promises, or reader-debt payoffs unless the context says they are resolved. If the context names active anchors, carry the relevant anchors into the scene through action, dialogue, consequence, or payoff pressure; do not merely mention them in passing. Unless the chapter plan explicitly narrows scope, at least three active anchors from the context must materially participate in the scene, and at least one of them must change the immediate choice, pressure, or consequence of the chapter.",
    },
    {
      role: "user",
      content: payload.userPrompt,
    },
  ];
  const requestBody = {
    model,
    messages,
    stream: false,
    temperature: options.temperature,
    max_tokens: options.maxTokens,
  };
  if (options.disableReasoning && apiBase.toLowerCase().includes("openrouter.ai")) {
    requestBody.reasoning = { effort: "none", exclude: true };
  }
  const result = await postJson(
    `${apiBase.replace(/\/$/, "")}/chat/completions`,
    apiKey,
    requestBody,
    90000,
  );
  if (!result.ok) return { ok: false, ...result };
  const text = result.json?.choices?.[0]?.message?.content ?? "";
  const anchorHit = scoreAgainstTerms(text, payload.anchors);
  const anchorCarry = scoreAnchorCarry(text, payload.anchors);
  return {
    ok: true,
    latencyMs: result.latencyMs,
    chars: [...text].length,
    finishReason: result.json?.choices?.[0]?.finish_reason ?? null,
    reasoningTokens: result.json?.usage?.completion_tokens_details?.reasoning_tokens ?? 0,
    anchorHitRate: anchorHit.hitRate,
    anchorCarryRate: anchorCarry.carryRate,
    anchorHits: anchorHit.hits,
    anchorCarryItems: anchorCarry.items,
    preview: preview(text),
  };
}

function buildProbePayloads() {
  const anchors = ["寒影刀", "张三", "镜中墟", "霜铃塔", "旧债"];
  const projectTitle = "镜中墟";
  const contract =
    "都市玄幻长篇。主角林墨追查寒影刀旧债，张三掌握镜中墟入口，霜铃塔账册牵出北境宗门。章节要持续制造情绪债务并逐步兑现，不能自动抹平旧伏笔。";
  return [
    {
      id: "chapter3",
      chapterNo: 3,
      chapterTitle: "第三章",
      anchors,
      userPrompt: [
        `项目: ${projectTitle}`,
        `Story Contract: ${contract}`,
        "当前计划: 第三章：林墨进入镜中墟，看见旧友的倒影，意识到寒影刀曾被用来封门。",
        "前文摘要: 第1章 林墨在雨夜茶馆逼问张三，亮出寒影刀，旧债被重新点燃。第2章 张三交出半页霜铃塔账册，但隐瞒镜中墟入口代价。",
        "上一章尾句: 张三盯着寒影刀，终于把那半页霜铃塔账册推了出来，说镜中墟的入口只在子时开一次。",
        "硬约束锚点: 寒影刀、张三、镜中墟、霜铃塔、旧债。正文必须至少保留 3 个锚点，并说明它们和当前情绪债务的关系。",
        "要求: 写 320-500 字中文正文；保留商业网文节奏；制造或兑现情绪债务；不要解释，不要 Markdown。",
      ].join("\n"),
    },
    {
      id: "chapter4",
      chapterNo: 4,
      chapterTitle: "第四章",
      anchors,
      userPrompt: [
        `项目: ${projectTitle}`,
        `Story Contract: ${contract}`,
        "当前计划: 第四章：北境宗门来人抢账册，张三被迫承认自己也是债务人。",
        "前文摘要: 第1章 林墨在雨夜茶馆逼问张三，亮出寒影刀，旧债被重新点燃。第2章 张三交出半页霜铃塔账册，但隐瞒镜中墟入口代价。第3章 林墨进入镜中墟，看见旧友倒影，意识到寒影刀曾被用来封门。",
        "上一章尾句: 镜中墟的门后传来旧友的声音，寒影刀在林墨掌心里发烫，张三却低声说北境宗门已经顺着账册找来了。",
        "硬约束锚点: 寒影刀、张三、镜中墟、霜铃塔、旧债。正文必须至少保留 3 个锚点，并说明它们和当前情绪债务的关系。",
        "要求: 写 320-500 字中文正文；保留商业网文节奏；制造或兑现情绪债务；不要解释，不要 Markdown。",
      ].join("\n"),
    },
  ];
}

function summarizeProbeRuns(probe, runs) {
  const successful = runs.filter((run) => run.ok);
  const failed = runs.filter((run) => !run.ok);
  const latency = successful.map((run) => run.latencyMs);
  const chars = successful.map((run) => run.chars);
  const hit = successful.map((run) => run.anchorHitRate);
  const carry = successful.map((run) => run.anchorCarryRate);
  const reasoning = successful.map((run) => run.reasoningTokens);
  const finishReasons = [...new Set(successful.map((run) => run.finishReason))];

  return {
    probeId: probe.id,
    chapterNo: probe.chapterNo,
    runs: runs.length,
    successCount: successful.length,
    failedCount: failed.length,
    avgLatencyMs: Math.round(average(latency)),
    latencyStddevMs: Math.round(stddev(latency)),
    minLatencyMs: Math.round(min(latency)),
    maxLatencyMs: Math.round(max(latency)),
    avgChars: Math.round(average(chars)),
    charsStddev: Math.round(stddev(chars)),
    minAnchorHitRate: Number(min(hit).toFixed(2)),
    maxAnchorHitRate: Number(max(hit).toFixed(2)),
    minAnchorCarryRate: Number(min(carry).toFixed(2)),
    maxAnchorCarryRate: Number(max(carry).toFixed(2)),
    avgReasoningTokens: Math.round(average(reasoning)),
    finishReasons,
  };
}

async function main() {
  loadDotEnv();
  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) throw new Error("OPENAI_API_KEY is required in environment or .env");
  const apiBase = process.env.OPENAI_API_BASE || "https://openrouter.ai/api/v1";
  const model = process.env.OPENAI_MODEL || "deepseek/deepseek-v4-flash";
  const repeats = parseInteger("FORGE_CHAPTER_STABILITY_REPEATS", 5, 2, 20);
  const probes = buildProbePayloads();
  const probeRuns = [];

  for (const probe of probes) {
    const runs = [];
    for (let i = 0; i < repeats; i += 1) {
      const result = await runChapterDraft(apiBase, apiKey, model, probe);
      runs.push({ runIndex: i + 1, ...result });
    }
    probeRuns.push({
      probeId: probe.id,
      chapterNo: probe.chapterNo,
      payload: probe,
      summary: summarizeProbeRuns(probe, runs),
      runs,
    });
  }

  const report = {
    createdAt: new Date().toISOString(),
    apiBase,
    model,
    chapterProfile: profileOptions(),
    repeats,
    probes: probeRuns,
  };

  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  console.error(`Report saved to ${reportPath}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
