const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const reportPath = path.join(repoRoot, "reports", "real_author_session_metrics.json");
const anchorCarryConfigPath = path.join(repoRoot, "config", "anchor-carry-heuristics.json");
const requestProfileConfigPath = path.join(repoRoot, "config", "llm-request-profiles.json");
const anchorCarryConfig = JSON.parse(fs.readFileSync(anchorCarryConfigPath, "utf8"));
const requestProfileConfig = JSON.parse(fs.readFileSync(requestProfileConfigPath, "utf8"));

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

function profileOptions(profile) {
  const defaults = {
    chapter: requestProfileConfig.chapter_draft,
    ghost: requestProfileConfig.ghost_preview,
    analysis: requestProfileConfig.analysis,
    parallel: requestProfileConfig.parallel_draft,
    manual: requestProfileConfig.manual_rewrite,
    json: requestProfileConfig.json,
  }[profile];
  const envPrefix = {
    chapter: "OPENAI_CHAPTER_DRAFT",
    ghost: "OPENAI_GHOST_PREVIEW",
    analysis: "OPENAI_ANALYSIS",
    parallel: "OPENAI_PARALLEL_DRAFT",
    manual: "OPENAI_MANUAL_REWRITE",
    json: "OPENAI_JSON",
  }[profile];
  return {
    temperature: parseNumber(`${envPrefix}_TEMPERATURE`, defaults.temperature, 0, 2),
    maxTokens: parseInteger(`${envPrefix}_MAX_TOKENS`, defaults.maxTokens, 16, 65536),
    disableReasoning: parseBool(`${envPrefix}_DISABLE_REASONING`, defaults.disableReasoning),
  };
}

function charCount(text) {
  return [...(text ?? "")].length;
}

function preview(text, limit = 180) {
  const chars = [...(text ?? "")];
  return chars.slice(0, limit).join("") + (chars.length > limit ? "..." : "");
}

function containsAny(text, terms) {
  return terms.some((term) => text.includes(term));
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

async function chat(apiBase, apiKey, model, profile, messages, timeoutMs = 90000) {
  const options = profileOptions(profile);
  const providerOptions =
    options.disableReasoning && apiBase.toLowerCase().includes("openrouter.ai")
      ? { reasoning: { effort: "none", exclude: true } }
      : {};
  const result = await postJson(
    `${apiBase.replace(/\/$/, "")}/chat/completions`,
    apiKey,
    {
      model,
      messages,
      stream: false,
      temperature: options.temperature,
      max_tokens: options.maxTokens,
      ...(profile === "json" ? { response_format: { type: "json_object" } } : {}),
      ...providerOptions,
    },
    timeoutMs,
  );
  if (!result.ok) return { ...result, profile, options };
  const text = result.json?.choices?.[0]?.message?.content ?? "";
  return {
    ok: true,
    profile,
    options,
    latencyMs: result.latencyMs,
    text,
    chars: charCount(text),
    finishReason: result.json?.choices?.[0]?.finish_reason ?? null,
    usage: result.json?.usage ?? null,
  };
}

async function embed(apiBase, apiKey, model, input, timeoutMs = 45000) {
  const result = await postJson(
    `${apiBase.replace(/\/$/, "")}/embeddings`,
    apiKey,
    { model, input },
    timeoutMs,
  );
  if (!result.ok) return result;
  const vector = result.json?.data?.[0]?.embedding ?? [];
  return {
    ok: true,
    latencyMs: result.latencyMs,
    dimensions: Array.isArray(vector) ? vector.length : 0,
    nonZero: Array.isArray(vector) ? vector.some((value) => value !== 0) : false,
    usage: result.json?.usage ?? null,
  };
}

function avg(values) {
  const nums = values.filter((value) => Number.isFinite(value));
  return nums.length ? nums.reduce((sum, value) => sum + value, 0) / nums.length : 0;
}

function p95(values) {
  const nums = values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
  if (!nums.length) return 0;
  return nums[Math.min(nums.length - 1, Math.floor(nums.length * 0.95))];
}

async function main() {
  loadDotEnv();
  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) {
    throw new Error("OPENAI_API_KEY is required in environment or .env");
  }
  const apiBase = process.env.OPENAI_API_BASE || "https://openrouter.ai/api/v1";
  const model = process.env.OPENAI_MODEL || "deepseek/deepseek-v4-flash";
  const embeddingModel = process.env.OPENAI_EMBEDDING_MODEL || "text-embedding-3-small";

  const project = {
    title: "镜中墟",
    contract:
      "都市玄幻长篇。主角林墨追查寒影刀旧债，张三掌握镜中墟入口，霜铃塔账册牵出北境宗门。章节要持续制造情绪债务并逐步兑现，不能自动抹平旧伏笔。",
    constraints: ["寒影刀", "张三", "镜中墟", "霜铃塔", "旧债"],
  };
  const chapterPlans = [
    "第一章：林墨在雨夜茶馆逼问张三，亮出寒影刀，旧债被重新点燃。",
    "第二章：张三交出半页霜铃塔账册，但隐瞒镜中墟入口代价。",
    "第三章：林墨进入镜中墟，看见旧友的倒影，意识到寒影刀曾被用来封门。",
    "第四章：北境宗门来人抢账册，张三被迫承认自己也是债务人。",
    "第五章：林墨选择先救张三还是追账册，必须兑现前四章积压的信任债。",
  ];

  const chapters = [];
  const operations = [];
  let rollingSummary = "";
  let priorDraft = "";

  for (let index = 0; index < chapterPlans.length; index += 1) {
    const chapterNo = index + 1;
    const plan = chapterPlans[index];
    const context = [
        `项目: ${project.title}`,
        `Story Contract: ${project.contract}`,
        `当前计划: ${plan}`,
        rollingSummary ? `前文摘要: ${rollingSummary}` : "前文摘要: 无",
        priorDraft ? `上一章尾句: ${preview(priorDraft, 120)}` : "",
        `硬约束锚点: ${project.constraints.join("、")}。除非本章计划明确不涉及，否则正文必须至少保留 3 个锚点，并说明它们和当前情绪债务的关系。`,
        "要求: 写 320-500 字中文正文；保留商业网文节奏；制造或兑现情绪债务；不要解释，不要 Markdown。",
      ]
      .filter(Boolean)
      .join("\n");

    const draft = await chat(apiBase, apiKey, model, "chapter", [
      {
        role: "system",
        content:
          "你是中文长篇小说作者。只输出正文。重视人物关系、伏笔、情绪债务、兑现节奏和章节钩子。不得静默丢失当前上下文中的命名锚点和未偿债务。",
      },
      { role: "user", content: context },
    ]);
    operations.push({ chapterNo, kind: "chapter", ...draft, preview: preview(draft.text) });
    if (!draft.ok) break;

    const analysis = await chat(apiBase, apiKey, model, "analysis", [
      {
        role: "system",
        content:
          "你是严厉但具体的网文编辑。用 JSON 外的中文短段落指出：情绪债务、兑现、伏笔、节奏的最大问题。不要改写正文。",
      },
      { role: "user", content: draft.text },
    ]);
    operations.push({ chapterNo, kind: "analysis", ...analysis, preview: preview(analysis.text) });

    const ghost = await chat(apiBase, apiKey, model, "ghost", [
      { role: "system", content: "只补一两句可直接接在章节末尾的中文正文，不解释。" },
      { role: "user", content: `章节尾部:\n${preview(draft.text, 260)}\n\n续写一句制造下一章钩子。` },
    ]);
    operations.push({ chapterNo, kind: "ghost", ...ghost, preview: preview(ghost.text) });

    const parallel = await chat(apiBase, apiKey, model, "parallel", [
      {
        role: "user",
        content:
          `基于本章结尾，输出 A/B/C 三个下一章方向，每个 30 字以内，保留标签。\n本章:\n${preview(draft.text, 500)}`,
      },
    ]);
    operations.push({ chapterNo, kind: "parallel", ...parallel, preview: preview(parallel.text) });

    const manual = await chat(apiBase, apiKey, model, "manual", [
      {
        role: "user",
        content:
          `把下面句子改得更有压迫感，40 字以内，只输出改写句:\n${preview(draft.text, 80)}`,
      },
    ]);
    operations.push({ chapterNo, kind: "manual", ...manual, preview: preview(manual.text) });

    const structure = await chat(apiBase, apiKey, model, "json", [
      {
        role: "system",
        content:
          '输出合法 JSON：{"debts":["..."],"payoffs":["..."],"openPromises":["..."],"risks":["..."]}。只输出 JSON。',
      },
      { role: "user", content: draft.text },
    ]);
    let parsedStructure = null;
    try {
      parsedStructure = JSON.parse(structure.text);
    } catch {
      parsedStructure = null;
    }
    operations.push({
      chapterNo,
      kind: "json",
      ...structure,
      jsonValid: parsedStructure != null,
      preview: preview(structure.text),
    });

    const embedding = await embed(apiBase, apiKey, embeddingModel, draft.text);
    operations.push({ chapterNo, kind: "embedding", ...embedding });

    const anchorScore = scoreAgainstTerms(draft.text, project.constraints);
    const anchorCarry = scoreAnchorCarry(draft.text, project.constraints);
    const chapterMetrics = {
      chapterNo,
      draftChars: draft.chars,
      draftLatencyMs: draft.latencyMs,
      anchorHits: anchorScore.hits,
      anchorHitRate: anchorScore.hitRate,
      anchorCarryRate: anchorCarry.carryRate,
      anchorCarryItems: anchorCarry.items,
      hasHook:
        containsAny(draft.text, ["？", "?", "忽然", "却", "但", "门", "账册", "镜中墟"]) ||
        containsAny(ghost.text ?? "", ["？", "?", "忽然", "却", "但", "门", "账册", "镜中墟"]),
      analysisChars: analysis.chars ?? 0,
      ghostChars: ghost.chars ?? 0,
      parallelHasABC:
        typeof parallel.text === "string" &&
        parallel.text.includes("A") &&
        parallel.text.includes("B") &&
        parallel.text.includes("C"),
      manualChars: manual.chars ?? 0,
      jsonValid: parsedStructure != null,
      debtCount: Array.isArray(parsedStructure?.debts) ? parsedStructure.debts.length : 0,
      payoffCount: Array.isArray(parsedStructure?.payoffs) ? parsedStructure.payoffs.length : 0,
      promiseCount: Array.isArray(parsedStructure?.openPromises)
        ? parsedStructure.openPromises.length
        : 0,
      riskCount: Array.isArray(parsedStructure?.risks) ? parsedStructure.risks.length : 0,
      embeddingDimensions: embedding.dimensions ?? 0,
    };
    chapters.push(chapterMetrics);
    priorDraft = draft.text;
    rollingSummary = [
      rollingSummary,
      `第${chapterNo}章: ${preview(draft.text, 180)}`,
      parsedStructure
        ? `结构: debts=${chapterMetrics.debtCount}, payoffs=${chapterMetrics.payoffCount}, promises=${chapterMetrics.promiseCount}, risks=${chapterMetrics.riskCount}`
        : "",
    ]
      .filter(Boolean)
      .join("\n")
      .slice(-2400);
  }

  const failedOperations = operations.filter((op) => op.ok === false);
  const chatOps = operations.filter((op) => op.ok && op.latencyMs && op.kind !== "embedding");
  const summary = {
    createdAt: new Date().toISOString(),
    apiBase,
    model,
    embeddingModel,
    profiles: {
      chapter: profileOptions("chapter"),
      ghost: profileOptions("ghost"),
      analysis: profileOptions("analysis"),
      parallel: profileOptions("parallel"),
      manual: profileOptions("manual"),
      json: profileOptions("json"),
    },
    chapterCount: chapters.length,
    operationCount: operations.length,
    failedOperationCount: failedOperations.length,
    avgChatLatencyMs: Math.round(avg(chatOps.map((op) => op.latencyMs))),
    p95ChatLatencyMs: Math.round(p95(chatOps.map((op) => op.latencyMs))),
    avgDraftChars: Math.round(avg(chapters.map((chapter) => chapter.draftChars))),
    minAnchorHitRate: Math.min(...chapters.map((chapter) => chapter.anchorHitRate)),
    minAnchorCarryRate: Math.min(...chapters.map((chapter) => chapter.anchorCarryRate)),
    jsonValidRate: avg(chapters.map((chapter) => (chapter.jsonValid ? 1 : 0))),
    parallelABCRate: avg(chapters.map((chapter) => (chapter.parallelHasABC ? 1 : 0))),
    hookRate: avg(chapters.map((chapter) => (chapter.hasHook ? 1 : 0))),
    avgDebtCount: avg(chapters.map((chapter) => chapter.debtCount)),
    avgPayoffCount: avg(chapters.map((chapter) => chapter.payoffCount)),
    avgPromiseCount: avg(chapters.map((chapter) => chapter.promiseCount)),
    embeddingDimensions: [...new Set(chapters.map((chapter) => chapter.embeddingDimensions))],
    findings: [],
  };

  if (summary.failedOperationCount > 0) {
    summary.findings.push({
      severity: "p1",
      area: "provider",
      evidence: failedOperations.map((op) => ({
        chapterNo: op.chapterNo,
        kind: op.kind,
        status: op.status,
        error: op.error,
      })),
      recommendation: "Fix provider failures before tuning quality.",
    });
  }
  if (summary.jsonValidRate < 1) {
    summary.findings.push({
      severity: "p2",
      area: "json",
      evidence: { jsonValidRate: summary.jsonValidRate },
      recommendation: "Lower JSON temperature or strengthen response_format fallback parsing.",
    });
  }
  if (summary.parallelABCRate < 1) {
    summary.findings.push({
      severity: "p2",
      area: "parallel_draft",
      evidence: { parallelABCRate: summary.parallelABCRate },
      recommendation: "Make A/B/C output schema stricter or parse numbered alternatives.",
    });
  }
  if (summary.minAnchorHitRate < 0.4) {
    summary.findings.push({
      severity: "p2",
      area: "context_adherence",
      evidence: { minAnchorHitRate: summary.minAnchorHitRate },
      recommendation: "Increase chapter prompt grounding or inject required anchors as hard constraints.",
    });
  }
  if (summary.minAnchorCarryRate < 0.4) {
    summary.findings.push({
      severity: "p2",
      area: "anchor_carry",
      evidence: { minAnchorCarryRate: summary.minAnchorCarryRate },
      recommendation:
        "Strengthen prompts and evaluation so anchors participate in action, dialogue, consequence, or payoff pressure instead of surface mentions only.",
    });
  }
  if (summary.p95ChatLatencyMs > 15000) {
    summary.findings.push({
      severity: "p3",
      area: "latency",
      evidence: { p95ChatLatencyMs: summary.p95ChatLatencyMs },
      recommendation: "Reduce draft max tokens or split analysis/parallel calls behind explicit author actions.",
    });
  }

  const report = {
    summary,
    chapters,
    operations: operations.map((op) => ({
      chapterNo: op.chapterNo,
      kind: op.kind,
      profile: op.profile,
      ok: op.ok,
      status: op.status,
      latencyMs: op.latencyMs,
      chars: op.chars,
      finishReason: op.finishReason,
      options: op.options,
      usage: op.usage,
      jsonValid: op.jsonValid,
      dimensions: op.dimensions,
      nonZero: op.nonZero,
      preview: op.preview,
      error: op.error,
    })),
  };
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  console.error(`Report saved to ${reportPath}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
