const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

const repoRoot = path.resolve(__dirname, "..");
const requestProfileConfigPath = path.join(repoRoot, "config", "llm-request-profiles.json");
const anchorCarryConfigPath = path.join(repoRoot, "config", "anchor-carry-heuristics.json");
const requestProfileConfig = JSON.parse(fs.readFileSync(requestProfileConfigPath, "utf8"));
const anchorCarryConfig = JSON.parse(fs.readFileSync(anchorCarryConfigPath, "utf8"));

function loadDotEnv() {
  const envPath = path.join(repoRoot, ".env");
  if (!fs.existsSync(envPath)) return;
  for (const line of fs.readFileSync(envPath, "utf8").split(/\r?\n/)) {
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

function profileOptions(profile, overrides = {}) {
  const profileMap = {
    chapterDraft: ["chapter_draft", "OPENAI_CHAPTER_DRAFT"],
    chapterContinuation: ["chapter_continuation", "OPENAI_CHAPTER_CONTINUATION"],
    chapterCompress: ["chapter_compress", "OPENAI_CHAPTER_COMPRESS"],
    json: ["json", "OPENAI_JSON"],
  };
  const [configKey, envPrefix] = profileMap[profile];
  const defaults = requestProfileConfig[configKey];
  const options = {
    temperature: parseNumber(`${envPrefix}_TEMPERATURE`, defaults.temperature, 0, 2),
    maxTokens: parseInteger(`${envPrefix}_MAX_TOKENS`, defaults.maxTokens, 16, 65536),
    disableReasoning: parseBool(`${envPrefix}_DISABLE_REASONING`, defaults.disableReasoning),
  };
  if (overrides.temperature != null) {
    options.temperature = Math.max(0, Math.min(2, Number(overrides.temperature)));
  }
  if (overrides.maxTokens != null) {
    options.maxTokens = Math.max(16, Math.min(65536, Math.round(Number(overrides.maxTokens))));
  }
  if (overrides.disableReasoning != null) {
    options.disableReasoning = Boolean(overrides.disableReasoning);
  }
  return options;
}

function charCount(text) {
  return [...String(text ?? "")].length;
}

function preview(text, limit = 220) {
  const chars = [...String(text ?? "")];
  return chars.slice(0, limit).join("") + (chars.length > limit ? "..." : "");
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

function sum(values) {
  return values
    .filter((value) => Number.isFinite(value))
    .reduce((total, value) => total + value, 0);
}

function countBy(values) {
  return values.reduce((counts, value) => {
    const key = String(value ?? "unknown");
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
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
  } catch (error) {
    return {
      ok: false,
      latencyMs: Date.now() - started,
      status: error.name === "AbortError" ? "timeout" : "error",
      error: String(error.message || error)
        .replace(/sk-[A-Za-z0-9_-]+/g, "[REDACTED]")
        .slice(0, 500),
    };
  } finally {
    clearTimeout(timer);
  }
}

function retryableProviderResult(result) {
  if (result.ok) return false;
  if (result.status === "timeout" || result.status === "error") return true;
  return typeof result.status === "number" && (result.status === 429 || result.status >= 500);
}

function retryOptions(options, attemptIndex) {
  if (attemptIndex <= 0) return options;
  return {
    ...options,
    temperature: Math.max(0.3, Number((options.temperature - 0.07 * attemptIndex).toFixed(2))),
    maxTokens: Math.max(96, Math.round(options.maxTokens * 0.85 ** attemptIndex)),
  };
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function chat(apiBase, apiKey, model, profile, messages, timeoutMs = 180000, overrides = {}) {
  const baseOptions = profileOptions(profile, overrides);
  const maxRetries = parseInteger("FORGE_LONG_CHAPTER_MAX_RETRIES", 1, 0, 3);
  const attempts = [];
  let lastResult = null;
  let lastOptions = baseOptions;

  for (let attemptIndex = 0; attemptIndex <= maxRetries; attemptIndex += 1) {
    const options = retryOptions(baseOptions, attemptIndex);
    lastOptions = options;
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
    lastResult = result;
    attempts.push({
      attempt: attemptIndex + 1,
      ok: result.ok,
      status: result.status,
      latencyMs: result.latencyMs,
      options,
      error: result.error,
    });
    if (result.ok) {
      const text = result.json?.choices?.[0]?.message?.content ?? "";
      return {
        ok: true,
        profile,
        options,
        attempts,
        retryCount: attemptIndex,
        latencyMs: result.latencyMs,
        text,
        chars: charCount(text),
        finishReason: result.json?.choices?.[0]?.finish_reason ?? null,
        usage: result.json?.usage ?? null,
      };
    }
    if (!retryableProviderResult(result) || attemptIndex >= maxRetries) break;
    await wait(1000 * (attemptIndex + 1));
  }

  return { ...lastResult, profile, options: lastOptions, attempts, retryCount: maxRetries };
}

function parseJsonText(text) {
  const trimmed = String(text ?? "").trim();
  try {
    return JSON.parse(trimmed);
  } catch {
    const fenced = trimmed
      .replace(/^```json\s*/i, "")
      .replace(/^```\s*/i, "")
      .replace(/\s*```$/i, "")
      .trim();
    try {
      return JSON.parse(fenced);
    } catch {
      const start = fenced.indexOf("{");
      const end = fenced.lastIndexOf("}");
      if (start >= 0 && end > start) {
        return JSON.parse(fenced.slice(start, end + 1));
      }
      throw new Error("json_parse_failed");
    }
  }
}

async function chatJson(apiBase, apiKey, model, messages, timeoutMs = 120000, overrides = {}) {
  const result = await chat(apiBase, apiKey, model, "json", messages, timeoutMs, overrides);
  if (!result.ok) return result;
  try {
    return { ...result, parsed: parseJsonText(result.text) };
  } catch (error) {
    const repair = await chat(
      apiBase,
      apiKey,
      model,
      "json",
      [
        {
          role: "system",
          content: "把下面内容修复为合法 JSON。只输出 JSON，不要解释。",
        },
        {
          role: "user",
          content: result.text,
        },
      ],
      Math.min(timeoutMs, 60000),
      { temperature: 0.0, maxTokens: Math.max(512, Math.round(result.options.maxTokens * 0.8)) },
    );
    if (repair.ok) {
      try {
        return {
          ...repair,
          profile: "json",
          attempts: [...(result.attempts ?? []), ...(repair.attempts ?? [])],
          retryCount: Number(result.retryCount ?? 0) + Number(repair.retryCount ?? 0),
          parsed: parseJsonText(repair.text),
        };
      } catch {
        // fall through to typed failure below
      }
    }
    return {
      ok: false,
      profile: "json",
      options: result.options,
      attempts: [...(result.attempts ?? []), ...(repair.attempts ?? [])],
      retryCount: Number(result.retryCount ?? 0) + Number(repair.retryCount ?? 0),
      latencyMs: Number(result.latencyMs ?? 0) + Number(repair.latencyMs ?? 0),
      status: "json_parse_error",
      error: String(error.message || error).slice(0, 240),
      text: result.text,
    };
  }
}

function chapterOutcome(chars, contract) {
  if (chars < contract.minChars) return "underMin";
  if (chars > contract.maxChars) return "overMax";
  return "valid";
}

function sha256(text) {
  return crypto.createHash("sha256").update(String(text ?? ""), "utf8").digest("hex");
}

function uniqueStrings(values) {
  return [...new Set((values ?? []).map((value) => String(value ?? "").trim()).filter(Boolean))];
}

function minOrZero(values) {
  const nums = values.filter((value) => Number.isFinite(value));
  return nums.length ? Math.min(...nums) : 0;
}

function maxOrZero(values) {
  const nums = values.filter((value) => Number.isFinite(value));
  return nums.length ? Math.max(...nums) : 0;
}

function median(values) {
  const nums = values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
  if (!nums.length) return 0;
  return nums[Math.floor(nums.length / 2)];
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function tokenCapForChineseChars(chars) {
  const charsPerToken = parseNumber("FORGE_CHINESE_CHARS_PER_TOKEN_ESTIMATE", 1.45, 0.8, 3);
  return Math.max(96, Math.ceil(chars / charsPerToken));
}

function continuationPlan(currentChars, contract) {
  const deficitToMin = Math.max(0, contract.minChars - currentChars);
  const roomToMax = Math.max(0, contract.maxChars - currentChars);
  if (deficitToMin <= 0 || roomToMax <= 0) return null;
  const addMinChars = Math.min(roomToMax, Math.max(deficitToMin + 80, 260));
  const addMaxChars = Math.min(roomToMax, Math.max(addMinChars, deficitToMin + 420, 520));
  return {
    currentChars,
    deficitToMin,
    roomToMax,
    addMinChars,
    addMaxChars,
    targetFinalMinChars: currentChars + addMinChars,
    targetFinalMaxChars: currentChars + addMaxChars,
    maxTokens: tokenCapForChineseChars(addMaxChars),
  };
}

function compressionPlan(currentChars, contract, attempt) {
  const softTargetChars =
    attempt === 1
      ? contract.targetChars
      : Math.max(contract.minChars + 250, contract.targetChars - 350);
  const spread = attempt === 1 ? 300 : 150;
  const targetMinChars = clamp(softTargetChars - spread, contract.minChars, contract.maxChars);
  const targetMaxChars = clamp(softTargetChars + spread, targetMinChars, contract.maxChars);
  const tokenScale = attempt === 1 ? 1 : 0.9;
  return {
    currentChars,
    attempt,
    targetMinChars,
    targetMaxChars,
    hardMaxChars: contract.maxChars,
    requiredCutChars: Math.max(0, currentChars - targetMaxChars),
    maxTokens: Math.max(1800, Math.round(tokenCapForChineseChars(targetMaxChars) * tokenScale)),
  };
}

function ngramSet(text, size = 3) {
  const chars = [...String(text ?? "").replace(/\s+/g, "")];
  const grams = new Set();
  for (let i = 0; i <= chars.length - size; i += 1) {
    grams.add(chars.slice(i, i + size).join(""));
  }
  return grams;
}

function jaccardSimilarity(a, b) {
  const left = ngramSet(a);
  const right = ngramSet(b);
  if (!left.size || !right.size) return 0;
  let intersection = 0;
  for (const value of left) {
    if (right.has(value)) intersection += 1;
  }
  return intersection / (left.size + right.size - intersection);
}

function chapterPlanFor(index) {
  const volume = Math.floor((index - 1) / 25) + 1;
  const withinVolume = ((index - 1) % 25) + 1;
  const arc = Math.floor((withinVolume - 1) / 5) + 1;
  const phase = ["setup", "pressure", "reversal", "payoff", "hook"][(withinVolume - 1) % 5];
  const phasePlans = {
    setup: "把上一章留下的代价具体化，让林墨用行动确认新的债务边界。",
    pressure: "让张三或北境宗门逼迫林墨做选择，至少一个旧承诺变得更难兑现。",
    reversal: "揭示镜中墟或霜铃塔账册的新矛盾，迫使寒影刀承担额外后果。",
    payoff: "兑现一个前文小债，同时制造更大的未偿债，不允许把关系彻底抹平。",
    hook: "以明确的新危险或人物选择收束本轮五章小弧，推动下一弧启动。",
  };
  return {
    volume,
    arc,
    phase,
    label: `第${index}章 V${volume}-A${arc}-${phase}`,
    plan: `${phasePlans[phase]} 当前卷重点：寒影刀旧债、张三隐瞒代价、镜中墟入口、霜铃塔账册、北境宗门追索必须至少推进两项。`,
  };
}

function dynamicRequiredAnchors(chapterHistory, chapterPlan, anchorPool) {
  const recency = chapterHistory
    .slice(-3)
    .flatMap((chapter) => chapter.keyAnchors ?? [])
    .filter(Boolean);
  const phaseBias = {
    setup: ["寒影刀", "张三", "旧债"],
    pressure: ["张三", "旧债", "镜中墟"],
    reversal: ["镜中墟", "霜铃塔", "寒影刀"],
    payoff: ["旧债", "霜铃塔", "张三"],
    hook: ["镜中墟", "寒影刀", "北境宗门"],
  }[chapterPlan.phase] ?? [];
  const anchors = uniqueStrings([...recency, ...phaseBias, ...anchorPool]).slice(0, 6);
  return anchors.length >= 3 ? anchors : uniqueStrings([...anchors, ...anchorPool]).slice(0, 5);
}

function chooseContinuityFacts(chapterHistory) {
  return chapterHistory
    .slice(-4)
    .flatMap((chapter) => chapter.continuityFacts ?? [])
    .filter((fact) => ["supported", "inferred"].includes(fact.status))
    .slice(-10)
    .map((fact) => fact.statement);
}

async function planChapterSegments(apiBase, apiKey, model, chapterPlan, rollingSummary, dynamicAnchors, continuityFacts) {
  const planned = await chatJson(
    apiBase,
    apiKey,
    model,
    [
      {
        role: "system",
        content:
          '输出合法 JSON：{"segments":[{"id":"s1","goal":"...","mustInclude":["..."],"targetChars":900}],"attention":["..."],"riskChecks":["..."]}。只输出 JSON。',
      },
      {
        role: "user",
        content: [
          `章节计划: ${chapterPlan.label} ${chapterPlan.plan}`,
          rollingSummary ? `前文摘要: ${rollingSummary}` : "前文摘要: 无",
          `动态 required anchors: ${dynamicAnchors.join("、")}`,
          continuityFacts.length > 0
            ? `必须延续的连续性事实: ${continuityFacts.join("；")}`
            : "必须延续的连续性事实: 无",
          "把本章拆成 3 个连续 segment，每个 segment 给出 goal、mustInclude、targetChars。总长度围绕 3500 字。",
        ].join("\n"),
      },
    ],
    90000,
  );
  if (!planned.ok || !planned.parsed || !Array.isArray(planned.parsed.segments)) {
    return {
      ok: false,
      status: planned.status,
      error: planned.error || "segment_plan_invalid",
      raw: planned.text,
    };
  }
  return {
    ok: true,
    operation: planned,
    segments: planned.parsed.segments.slice(0, 4).map((segment, index) => ({
      id: String(segment.id ?? `s${index + 1}`),
      goal: String(segment.goal ?? "").trim(),
      mustInclude: uniqueStrings(segment.mustInclude ?? []).slice(0, 5),
      targetChars: clamp(Number(segment.targetChars ?? 950), 500, 1400),
    })),
    attention: uniqueStrings(planned.parsed.attention ?? []).slice(0, 6),
    riskChecks: uniqueStrings(planned.parsed.riskChecks ?? []).slice(0, 6),
  };
}

async function extractChapterFacts(apiBase, apiKey, model, finalText, dynamicAnchors, chapterPlan) {
  const extracted = await chatJson(
    apiBase,
    apiKey,
    model,
    [
      {
        role: "system",
        content:
          '输出合法 JSON：{"facts":[{"type":"character_state|location|promise|causal_change|object","subject":"...","predicate":"...","object":"...","statement":"...","sourceType":"segment|summary|carryover","confidence":0.0,"novelty":"new|carryover|ephemeral|local"}],"newAnchors":["..."],"resolvedPromises":["..."],"risks":["..."],"continuityWarnings":[{"type":"ooc|canon_drift|bad_payoff|state_conflict","subject":"...","statement":"...","severity":"low|medium|high"}]}。只输出 JSON。',
      },
      {
        role: "user",
        content: [
          `章节: ${chapterPlan.label}`,
          `动态 required anchors: ${dynamicAnchors.join("、")}`,
          "从正文中抽取新增事实、人物状态、地点、承诺、因果变化。不要复述旧背景，只保留本章新增或明确改写的信息。",
          "临时场景细节、现场情绪、一次性地点细节标记 novelty=ephemeral 或 local；只有会改写长期连续性的设定才标记为 new。",
          "facts 最多输出 6 条，只保留最高价值、最影响后续连续性的条目。",
          "continuityWarnings 最多输出 2 条。ooc 只用于角色行为明显违背既有动机/底线；canon_drift 只用于长期设定飘移；bad_payoff 只用于错误兑现；state_conflict 只用于同一时间切片里的人物状态/位置/伤势/持有物不相容。普通剧情压力、期限冲突、二选一困境不算 state_conflict。",
          `正文:\n${finalText}`,
        ].join("\n"),
      },
    ],
    120000,
  );
  if (!extracted.ok || !extracted.parsed || !Array.isArray(extracted.parsed.facts)) {
    return {
      ok: false,
      status: extracted.status,
      error: extracted.error || "fact_extract_invalid",
      raw: extracted.text,
    };
  }
  return {
    ok: true,
    operation: extracted,
    facts: extracted.parsed.facts,
    newAnchors: uniqueStrings(extracted.parsed.newAnchors ?? []).slice(0, 6),
    resolvedPromises: uniqueStrings(extracted.parsed.resolvedPromises ?? []).slice(0, 6),
    risks: uniqueStrings(extracted.parsed.risks ?? []).slice(0, 8),
    continuityWarnings: (extracted.parsed.continuityWarnings ?? []).map((warning) => ({
      type: String(warning?.type ?? "canon_drift").trim().toLowerCase(),
      subject: String(warning?.subject ?? "").trim(),
      statement: String(warning?.statement ?? "").trim(),
      severity: String(warning?.severity ?? "medium").trim().toLowerCase(),
    })),
  };
}

async function repairHallucinationChapter(
  apiBase,
  apiKey,
  model,
  finalText,
  chapterPlan,
  dynamicAnchors,
  gate,
  unsupportedFacts,
) {
  return chat(
    apiBase,
    apiKey,
    model,
    "chapterCompress",
    [
      {
        role: "system",
        content:
          "只输出修正后的完整中文章节正文。这是 hallucination repair，不是续写。删除或改写无来源的新设定，保留已有锚点、主因果链、人物选择、代价和结尾钩子。",
      },
      {
        role: "user",
        content: [
          `章节: ${chapterPlan.label}`,
          `动态 required anchors: ${dynamicAnchors.join("、")}`,
          `hallucination gate reasons: ${(gate.reasons ?? []).join("、")}`,
          `必须删除或改写的无来源设定: ${unsupportedFacts.map((fact) => fact.statement).join("；")}`,
          "规则：不要新增命名人物、组织、法器、钥匙、旧债、承诺、地点背景；如果当前正文依赖这些设定，就改写成已知锚点和当前场景内可见动作。",
          `当前正文:\n${finalText}`,
        ].join("\n"),
      },
    ],
    120000,
    { temperature: 0.25, maxTokens: 2200 },
  );
}

async function draftChapterBySegments(
  apiBase,
  apiKey,
  model,
  chapterPlan,
  plannedSegments,
  dynamicAnchors,
  rollingSummary,
  continuityFacts,
) {
  const outputs = [];
  const operations = [];
  let accumulated = "";
  for (const segment of plannedSegments) {
    const segmentPrompt = [
      `章节: ${chapterPlan.label}`,
      `segment: ${segment.id}`,
      `segment goal: ${segment.goal}`,
      `segment target chars: ${segment.targetChars}`,
      `must include: ${segment.mustInclude.join("、") || dynamicAnchors.slice(0, 3).join("、")}`,
      rollingSummary ? `前文摘要: ${rollingSummary}` : "前文摘要: 无",
      continuityFacts.length > 0 ? `连续性事实: ${continuityFacts.join("；")}` : "连续性事实: 无",
      accumulated ? `已有本章正文:\n${accumulated}` : "已有本章正文: 无",
      "只输出这一段新的中文正文，不要重写已有部分，不要解释。",
    ].join("\n");
    const drafted = await chat(
      apiBase,
      apiKey,
      model,
      "chapterDraft",
      [
      {
        role: "system",
        content:
          "You are drafting one segment of a Chinese novel chapter. Write only the next segment prose. Keep continuity stable and stop when the requested segment beat lands cleanly. Do not invent new named people, organizations, artifacts, debts, promises, keys, contracts, locations, or backstory unless they are explicitly present in the provided continuity facts or chapter plan.",
        },
        { role: "user", content: segmentPrompt },
      ],
      120000,
      { maxTokens: tokenCapForChineseChars(segment.targetChars + 180), temperature: 0.72 },
    );
    operations.push({ kind: "segment_draft", segmentId: segment.id, ...drafted, preview: preview(drafted.text) });
    if (!drafted.ok) {
      return { ok: false, operations, error: drafted.error, status: drafted.status };
    }
    const text = drafted.text.trim();
    outputs.push({
      id: segment.id,
      goal: segment.goal,
      targetChars: segment.targetChars,
      chars: charCount(text),
      text,
    });
    accumulated = accumulated ? `${accumulated}\n${text}` : text;
  }
  return {
    ok: true,
    operations,
    segments: outputs,
    mergedText: outputs.map((segment) => segment.text).join("\n").trim(),
  };
}

function updateRollingSummary(history, limitChars = 1800) {
  const lines = history
    .slice(-8)
    .map((chapter) => `第${chapter.chapter}章(${chapter.phase}): ${chapter.summary}`);
  let text = lines.join("\n");
  while (charCount(text) > limitChars && lines.length > 1) {
    lines.shift();
    text = lines.join("\n");
  }
  return text;
}

function resolveArtifactsDir(reportPath) {
  const baseDir = reportPath.replace(/\.json$/i, ".artifacts");
  fs.mkdirSync(baseDir, { recursive: true });
  return baseDir;
}

function chapterArtifactDir(artifactsDir, chapter) {
  const dir = path.join(artifactsDir, `chapter-${String(chapter).padStart(3, "0")}`);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function writePrivateArtifact(artifactsDir, chapter, name, payload, format = "json") {
  const dir = chapterArtifactDir(artifactsDir, chapter);
  const filePath = path.join(dir, `${name}.${format}`);
  const content =
    format === "json"
      ? JSON.stringify(payload, null, 2)
      : typeof payload === "string"
        ? payload
        : String(payload ?? "");
  fs.writeFileSync(filePath, content, "utf8");
  return {
    kind: name,
    format,
    path: path.relative(repoRoot, filePath).replace(/\\/g, "/"),
    sha256: sha256(content),
    chars: charCount(content),
  };
}

function operationStats(operations) {
  const kinds = [...new Set(operations.map((operation) => operation.kind))].sort();
  return kinds.reduce((stats, kind) => {
    const items = operations.filter((operation) => operation.kind === kind);
    const successful = items.filter((operation) => operation.ok);
    stats[kind] = {
      count: items.length,
      successCount: successful.length,
      failureCount: items.length - successful.length,
      avgLatencyMs: Math.round(avg(successful.map((operation) => operation.latencyMs))),
      p95LatencyMs: Math.round(p95(successful.map((operation) => operation.latencyMs))),
      avgChars: Math.round(avg(successful.map((operation) => operation.chars))),
      finishReasons: countBy(successful.map((operation) => operation.finishReason)),
      retryCount: sum(successful.map((operation) => Number(operation.retryCount))),
      cost: Number(sum(successful.map((operation) => Number(operation.usage?.cost))).toFixed(8)),
    };
    return stats;
  }, {});
}

function buildOptimizationRecommendations(summary, chapters) {
  const recommendations = [];
  const compressRate = chapters.length ? (summary.repairCounts.compress ?? 0) / chapters.length : 0;
  const continuationRate = chapters.length
    ? (summary.repairCounts.continuation ?? 0) / chapters.length
    : 0;
  const lengthFinishRate = summary.operationCount
    ? (summary.finishReasons.length ?? 0) / summary.operationCount
    : 0;

  if (summary.failedOperationCount > 0 || summary.providerRetryCount > 0) {
    recommendations.push({
      area: "provider_resilience",
      evidence: {
        failedOperationCount: summary.failedOperationCount,
        providerRetryCount: summary.providerRetryCount,
        p95LatencyMs: summary.p95LatencyMs,
      },
      action:
        "Keep retry evidence visible and prefer bounded draft calls; if this repeats, split chapter drafting into smaller scene segments instead of one long provider call.",
    });
  }
  if (summary.p95LatencyMs > 90000) {
    recommendations.push({
      area: "generation_latency",
      evidence: { p95LatencyMs: summary.p95LatencyMs },
      action:
        "Move real-time UX toward staged scene drafting or background sprint mode; a single 3500-char call is not consistently interactive at this latency.",
    });
  }
  if (compressRate > 0.25) {
    recommendations.push({
      area: "over_generation",
      evidence: { compressRate, repairCounts: summary.repairCounts },
      action:
        "Tighten draft prompt around a softer 3400-3700 target or lower draft maxTokens further; too many chapters are relying on compress to become saveable.",
    });
  }
  if (continuationRate > 0.25) {
    recommendations.push({
      area: "under_generation",
      evidence: { continuationRate, repairCounts: summary.repairCounts },
      action:
        "Raise draft floor instructions or use scene-count guidance; too many chapters require continuation to satisfy the contract.",
    });
  }
  if (lengthFinishRate > 0.4) {
    recommendations.push({
      area: "token_budget",
      evidence: { lengthFinishRate, finishReasons: summary.finishReasons },
      action:
        "Completion frequently hits token limits; tune tokens per phase and avoid using finish_reason=length as the normal path.",
    });
  }
  if (summary.minAnchorCarryRate < 0.8) {
    recommendations.push({
      area: "anchor_carry",
      evidence: { minAnchorCarryRate: summary.minAnchorCarryRate },
      action:
        "Strengthen anchor selection and per-chapter mission grounding; named anchors are sometimes present without enough action/consequence support.",
    });
  }
  if (summary.maxAdjacentSimilarity > 0.32) {
    recommendations.push({
      area: "long_session_repetition",
      evidence: { maxAdjacentSimilarity: summary.maxAdjacentSimilarity },
      action:
        "Improve long-session planning diversity and rolling-summary compaction; adjacent chapters are becoming too textually similar.",
    });
  }
  if (summary.hallucinationGateFailureCount > 0) {
    recommendations.push({
      area: "hallucination_gate",
      evidence: {
        hallucinationGateFailureCount: summary.hallucinationGateFailureCount,
        unsupportedFactCount: summary.unsupportedFactCount,
        contradictionFactCount: summary.contradictionFactCount,
      },
      action:
        "Tighten fact extraction and preflight context grounding; unsupported new facts and contradictions must block final saveable output, not just appear in diagnostics.",
    });
  }
  return recommendations;
}

function resolveReportPath(chapterCount) {
  const configured = process.env.FORGE_LONG_CHAPTER_REPORT_PATH;
  if (configured && configured.trim()) {
    return path.isAbsolute(configured)
      ? configured
      : path.join(repoRoot, configured);
  }
  const filename =
    chapterCount >= 100
      ? "real_long_chapter_contract_probe_100.json"
      : "real_long_chapter_contract_probe.json";
  return path.join(repoRoot, "reports", filename);
}

function buildSummary({ apiBase, model, contract, chapters, operations, startedAt, completed }) {
  const failedOperations = operations.filter((operation) => operation.ok === false);
  const successfulCalls = operations.filter((operation) => operation.ok && operation.latencyMs);
  const complianceRate = avg(chapters.map((chapter) => (chapter.contractCompliant ? 1 : 0)));
  const repairCounts = countBy(chapters.flatMap((chapter) => chapter.repairs));
  const finishReasons = countBy(operations.filter((operation) => operation.ok).map((operation) => operation.finishReason));
  const totalCost = sum(
    operations
      .filter((operation) => operation.ok)
      .map((operation) => Number(operation.usage?.cost)),
  );
  const providerRetryCount = sum(
    operations
      .filter((operation) => operation.ok)
      .map((operation) => Number(operation.retryCount)),
  );
  const adjacentSimilarities = chapters.map((chapter) => chapter.adjacentSimilarity);
  const allFacts = chapters.flatMap((chapter) => chapter.extractedFacts ?? []);
  const allWarnings = chapters.flatMap((chapter) => chapter.continuityWarnings ?? []);
  const hallucinationGateFailureCount = chapters.filter(
    (chapter) => chapter.hallucinationGate?.passed === false,
  ).length;
  const summary = {
    createdAt: new Date().toISOString(),
    startedAt,
    completed,
    apiBase,
    model,
    contract,
    repairStrategy: {
      continuation: "deficit-bounded additions with dynamic token caps",
      compression: "strict deletion rewrite with dynamic token caps and up to two attempts",
      chineseCharsPerTokenEstimate: parseNumber("FORGE_CHINESE_CHARS_PER_TOKEN_ESTIMATE", 1.45, 0.8, 3),
    },
    profiles: {
      chapterDraft: profileOptions("chapterDraft"),
      chapterContinuation: profileOptions("chapterContinuation"),
      chapterCompress: profileOptions("chapterCompress"),
    },
    chapterCount: chapters.length,
    operationCount: operations.length,
    failedOperationCount: failedOperations.length,
    avgFinalChars: Math.round(avg(chapters.map((chapter) => chapter.finalChars))),
    minFinalChars: minOrZero(chapters.map((chapter) => chapter.finalChars)),
    maxFinalChars: maxOrZero(chapters.map((chapter) => chapter.finalChars)),
    complianceRate,
    minAnchorMentionRate: minOrZero(chapters.map((chapter) => chapter.anchorMentionRate)),
    minAnchorCarryRate: minOrZero(chapters.map((chapter) => chapter.anchorCarryRate)),
    avgLatencyMs: Math.round(avg(successfulCalls.map((operation) => operation.latencyMs))),
    p95LatencyMs: Math.round(p95(successfulCalls.map((operation) => operation.latencyMs))),
    repairCounts,
    finishReasons,
    totalCost: Number(totalCost.toFixed(8)),
    providerRetryCount,
    operationStats: operationStats(operations),
    repairRate: chapters.length
      ? Number(
          (
            chapters.filter((chapter) => chapter.repairs.length > 0).length / chapters.length
          ).toFixed(4),
        )
      : 0,
    maxAdjacentSimilarity: Number(maxOrZero(adjacentSimilarities).toFixed(4)),
    avgAdjacentSimilarity: Number(avg(adjacentSimilarities).toFixed(4)),
    medianFinalChars: Math.round(median(chapters.map((chapter) => chapter.finalChars))),
    supportedFactCount: allFacts.filter((fact) => fact.status === "supported").length,
    inferredFactCount: allFacts.filter((fact) => fact.status === "inferred").length,
    unsupportedFactCount: allFacts.filter((fact) => fact.status === "unsupported").length,
    contradictionFactCount: allFacts.filter((fact) => fact.status === "contradiction").length,
    continuityWarningCount: allWarnings.length,
    oocWarningCount: allWarnings.filter((warning) => warning.type === "ooc").length,
    canonDriftWarningCount: allWarnings.filter((warning) => warning.type === "canon_drift").length,
    badPayoffWarningCount: allWarnings.filter((warning) => warning.type === "bad_payoff").length,
    stateConflictWarningCount: allWarnings.filter((warning) => warning.type === "state_conflict").length,
    hallucinationGateFailureCount,
    findings: [],
  };

  if (summary.failedOperationCount > 0) {
    summary.findings.push({
      severity: "p1",
      area: "provider",
      evidence: failedOperations.map((operation) => ({
        chapter: operation.chapter,
        kind: operation.kind,
        status: operation.status,
        error: operation.error,
      })),
      recommendation: "Fix provider failures before tuning chapter quality.",
    });
  }
  if (chapters.length > 0 && summary.complianceRate < 1) {
    const nonCompliant = chapters.filter((chapter) => !chapter.contractCompliant);
    summary.findings.push({
      severity: "p1",
      area: "chapter_contract",
      evidence: nonCompliant.map((chapter) => ({
        chapter: chapter.chapter,
        draftChars: chapter.draftChars,
        finalChars: chapter.finalChars,
        outcome: chapter.outcome,
        repairs: chapter.repairs,
        repairDetails: chapter.repairDetails,
      })),
      recommendation:
        "Tune production prompt or repair loop before claiming real 3500-character stability.",
    });
  }
  if (chapters.length > 0 && summary.minAnchorCarryRate < 0.6) {
    summary.findings.push({
      severity: "p2",
      area: "anchor_carry",
      evidence: { minAnchorCarryRate: summary.minAnchorCarryRate },
      recommendation: "Strengthen anchor participation in the draft and continuation prompts.",
    });
  }
  if (hallucinationGateFailureCount > 0) {
    summary.findings.push({
      severity: "p1",
      area: "hallucination_gate",
      evidence: chapters
        .filter((chapter) => chapter.hallucinationGate?.passed === false)
        .map((chapter) => ({
          chapter: chapter.chapter,
          reasons: chapter.hallucinationGate?.reasons ?? [],
          unsupportedFacts: (chapter.extractedFacts ?? []).filter((fact) => fact.status === "unsupported"),
          contradictions: (chapter.extractedFacts ?? []).filter((fact) => fact.status === "contradiction"),
          continuityWarnings: chapter.continuityWarnings ?? [],
        })),
      recommendation:
        "Block final saveable output when continuity breaks emerge: contradiction, invalid payoff, OOC drift, or state conflict.",
    });
  }
  if (summary.p95LatencyMs > 90000) {
    summary.findings.push({
      severity: "p3",
      area: "latency",
      evidence: { p95LatencyMs: summary.p95LatencyMs },
      recommendation: "Consider smaller chapter segments or staged scene drafting for real-time UX.",
    });
  }
  summary.optimizationRecommendations = buildOptimizationRecommendations(summary, chapters);

  return summary;
}

function buildReport(summary, chapters, operations) {
  return {
    summary,
    chapters: chapters.map((chapter) => ({
      ...chapter,
      fullPreviewForSimilarity: undefined,
    })),
    operations: operations.map((operation) => ({
      chapter: operation.chapter,
      kind: operation.kind,
      profile: operation.profile,
      ok: operation.ok,
      status: operation.status,
      latencyMs: operation.latencyMs,
      chars: operation.chars,
      finishReason: operation.finishReason,
      options: operation.options,
      usage: operation.usage,
      repairPlan: operation.repairPlan,
      retryCount: operation.retryCount,
      attempts: operation.attempts,
      parsed: operation.parsed,
      preview: operation.preview,
      error: operation.error,
    })),
  };
}

function csvEscape(value) {
  const text = String(value ?? "");
  if (/[",\r\n]/.test(text)) {
    return `"${text.replace(/"/g, '""')}"`;
  }
  return text;
}

function buildContinuityIndex(chapterHistory) {
  const facts = chapterHistory.flatMap((chapter) => chapter.extractedFacts ?? []);
  const byKey = new Map();
  for (const fact of facts) {
    const key = fact.key;
    if (!key) continue;
    if (!byKey.has(key)) byKey.set(key, []);
    byKey.get(key).push(fact);
  }
  return byKey;
}

function normalizeFact(raw, chapter, segment, textHash) {
  const type = String(raw?.type ?? "fact").trim().toLowerCase();
  const subject = String(raw?.subject ?? "").trim();
  const predicate = String(raw?.predicate ?? "").trim();
  const object = String(raw?.object ?? "").trim();
  const statement = String(raw?.statement ?? `${subject} ${predicate} ${object}`.trim()).trim();
  const sourceType = String(raw?.sourceType ?? "segment").trim().toLowerCase();
  const key = [type, subject, predicate].filter(Boolean).join("|").toLowerCase();
  return {
    key,
    type,
    subject,
    predicate,
    object,
    statement,
    sourceType,
    sourceRef: raw?.sourceRef ?? `${segment}:${textHash.slice(0, 12)}`,
    chapter,
    confidence: Number(raw?.confidence ?? 0.5),
    status: "inferred",
    novelty: String(raw?.novelty ?? "new").trim().toLowerCase(),
  };
}

function evaluateFactStatus(fact, continuityIndex, dynamicAnchors) {
  const prior = continuityIndex.get(fact.key) ?? [];
  const sameObject = prior.find((item) => item.object && item.object === fact.object);
  const conflicting = prior.find(
    (item) =>
      item.object &&
      fact.object &&
      item.object !== fact.object &&
      item.subject === fact.subject &&
      item.predicate === fact.predicate,
  );
  if (conflicting) return "contradiction";
  if (sameObject) return "supported";
  if (
    fact.type === "location" ||
    fact.type === "character_state" ||
    fact.novelty === "ephemeral" ||
    fact.novelty === "local"
  ) {
    return "inferred";
  }
  if (
    dynamicAnchors.includes(fact.subject) ||
    dynamicAnchors.includes(fact.object) ||
    fact.sourceType === "carryover" ||
    fact.sourceType === "summary"
  ) {
    return "inferred";
  }
  return "unsupported";
}

function buildHallucinationGate(extractedFacts, continuityWarnings = []) {
  const reasons = [];
  const contradictions = extractedFacts.filter((fact) => fact.status === "contradiction");
  const invalidPayoffs = extractedFacts.filter(
    (fact) =>
      fact.type === "promise" &&
      fact.predicate === "resolved" &&
      (fact.status === "unsupported" || fact.status === "contradiction"),
  );
  const highRiskWarnings = continuityWarnings.filter((warning) =>
    ["ooc", "canon_drift", "bad_payoff", "state_conflict"].includes(warning.type) &&
    ["medium", "high"].includes(warning.severity),
  );
  if (contradictions.length > 0) reasons.push("contradiction");
  if (invalidPayoffs.length > 0) reasons.push("invalid_promise_payoff");
  for (const warning of highRiskWarnings) {
    if (!reasons.includes(warning.type)) reasons.push(warning.type);
  }
  return {
    passed: reasons.length === 0,
    reasons,
    unsupportedCount: extractedFacts.filter((fact) => fact.status === "unsupported").length,
    contradictionCount: contradictions.length,
    invalidPayoffCount: invalidPayoffs.length,
    warningCount: continuityWarnings.length,
    highRiskWarningCount: highRiskWarnings.length,
  };
}

function writeFactCsv(reportPath, chapters) {
  const csvPath = reportPath.replace(/\.json$/i, ".facts.csv");
  const facts = chapters.flatMap((chapter) =>
    (chapter.extractedFacts ?? []).map((fact) => ({
      chapter: chapter.chapter,
      phase: chapter.phase,
      key: fact.key,
      type: fact.type,
      subject: fact.subject,
      predicate: fact.predicate,
      object: fact.object,
      statement: fact.statement,
      sourceType: fact.sourceType,
      status: fact.status,
      confidence: fact.confidence,
    })),
  );
  const headers = [
    "chapter",
    "phase",
    "key",
    "type",
    "subject",
    "predicate",
    "object",
    "statement",
    "sourceType",
    "status",
    "confidence",
  ];
  const rows = facts.map((fact) => headers.map((header) => csvEscape(fact[header])).join(","));
  fs.writeFileSync(csvPath, [headers.join(","), ...rows].join("\n"));
}

function writeChapterCsv(reportPath, chapters) {
  const csvPath = reportPath.replace(/\.json$/i, ".chapters.csv");
  const headers = [
    "chapter",
    "volume",
    "arc",
    "phase",
    "draftChars",
    "finalChars",
    "outcome",
    "repairs",
    "contractCompliant",
    "anchorMentionRate",
    "anchorCarryRate",
    "adjacentSimilarity",
    "rollingSummaryChars",
    "contextChars",
    "dynamicAnchors",
    "hallucinationGatePassed",
    "hallucinationGateReasons",
    "factCount",
    "continuityWarningCount",
  ];
  const rows = chapters.map((chapter) =>
    headers
      .map((header) => {
        const value =
          header === "repairs"
            ? chapter.repairs.join("|")
            : header === "dynamicAnchors"
              ? (chapter.dynamicAnchors ?? []).join("|")
              : header === "hallucinationGateReasons"
                ? (chapter.hallucinationGate?.reasons ?? []).join("|")
                : header === "hallucinationGatePassed"
                    ? chapter.hallucinationGate?.passed
                    : header === "factCount"
                      ? (chapter.extractedFacts ?? []).length
                      : header === "continuityWarningCount"
                        ? (chapter.continuityWarnings ?? []).length
                      : chapter[header];
        return csvEscape(value);
      })
      .join(","),
  );
  fs.writeFileSync(csvPath, [headers.join(","), ...rows].join("\n"));
}

function writeReport(reportPath, summary, chapters, operations) {
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(buildReport(summary, chapters, operations), null, 2));
  writeChapterCsv(reportPath, chapters);
  writeFactCsv(reportPath, chapters);
}

async function main() {
  loadDotEnv();
  const apiKey = process.env.OPENAI_API_KEY;
  if (!apiKey) throw new Error("OPENAI_API_KEY is required in environment or .env");
  const apiBase = process.env.OPENAI_API_BASE || "https://openrouter.ai/api/v1";
  const model = process.env.OPENAI_MODEL || "deepseek/deepseek-v4-flash";
  const chapterCount = parseInteger("FORGE_LONG_CHAPTER_PROBE_COUNT", 3, 1, 120);
  const reportPath = resolveReportPath(chapterCount);
  const artifactsDir = resolveArtifactsDir(reportPath);
  const startedAt = new Date().toISOString();
  const contract = {
    targetChars: 3500,
    minChars: 3000,
    maxChars: 4000,
    saveHardFloorChars: 2800,
    saveHardCeilingChars: 4300,
  };
  const anchors = ["寒影刀", "张三", "镜中墟", "霜铃塔", "旧债"];
  let rollingSummary = "";
  const chapterHistory = [];
  const chapters = [];
  const operations = [];

  for (let index = 1; index <= chapterCount; index += 1) {
    const chapterPlan = chapterPlanFor(index);
    const dynamicAnchors = dynamicRequiredAnchors(chapterHistory, chapterPlan, anchors);
    const continuityFacts = chooseContinuityFacts(chapterHistory);
    const segmentPlan = await planChapterSegments(
      apiBase,
      apiKey,
      model,
      chapterPlan,
      rollingSummary,
      dynamicAnchors,
      continuityFacts,
    );
    let plannedSegments = [
      {
        id: "s1",
        goal: chapterPlan.plan,
        mustInclude: dynamicAnchors.slice(0, 3),
        targetChars: 1200,
      },
      {
        id: "s2",
        goal: "加压并推进因果后果。",
        mustInclude: dynamicAnchors.slice(1, 4),
        targetChars: 1200,
      },
      {
        id: "s3",
        goal: "做出选择并留下结尾钩子。",
        mustInclude: dynamicAnchors.slice(0, 2),
        targetChars: 1000,
      },
    ];
    let segmentAttention = dynamicAnchors.slice(0, 4);
    let segmentRiskChecks = [];
    if (segmentPlan.ok) {
      operations.push({
        chapter: index,
        kind: "segment_plan",
        ...segmentPlan.operation,
        parsed: segmentPlan.operation.parsed,
        preview: preview(segmentPlan.operation.text),
      });
      plannedSegments = segmentPlan.segments;
      segmentAttention = segmentPlan.attention;
      segmentRiskChecks = segmentPlan.riskChecks;
    } else {
      operations.push({
        chapter: index,
        kind: "segment_plan",
        ok: false,
        status: segmentPlan.status ?? "segment_plan_failed",
        error: segmentPlan.error,
        preview: preview(segmentPlan.raw),
      });
    }
    const context = [
      "项目: 镜中墟",
      "Story Contract: 都市玄幻长篇。主角林墨追查寒影刀旧债，张三掌握镜中墟入口，霜铃塔账册牵出北境宗门。章节要持续制造情绪债务并逐步兑现，不能自动抹平旧伏笔。",
      `当前计划: ${chapterPlan.label}：${chapterPlan.plan}`,
      rollingSummary ? `前文摘要: ${rollingSummary}` : "前文摘要: 无",
      continuityFacts.length > 0 ? `连续性事实: ${continuityFacts.join("；")}` : "连续性事实: 无",
      `动态 required anchors: ${dynamicAnchors.join("、")}。不是固定 5 锚点，按本章计划和近章连续性动态选取。`,
      `分段计划: ${plannedSegments.map((segment) => `${segment.id}:${segment.goal}[${segment.targetChars}]`).join(" / ")}`,
      segmentAttention.length > 0 ? `注意力焦点: ${segmentAttention.join("、")}` : "注意力焦点: 无",
      `ChapterContract: 目标 ${contract.targetChars} 中文字符；合格区间 ${contract.minChars}-${contract.maxChars}；硬保存边界 ${contract.saveHardFloorChars}-${contract.saveHardCeilingChars}。`,
      `硬约束锚点: ${dynamicAnchors.join("、")}。至少 3 个动态锚点必须通过行动、对话、后果或债务压力参与当前场景，不允许只点名。`,
      "结构要求: 写完整正文，不写摘要；至少 3 个连续场景拍点；包含人物选择、代价升级、结尾钩子；不得解释，不得 Markdown。",
    ].join("\n");
    const segmented = await draftChapterBySegments(
      apiBase,
      apiKey,
      model,
      chapterPlan,
      plannedSegments,
      dynamicAnchors,
      rollingSummary,
      continuityFacts,
    );
    let draft;
    if (segmented.ok) {
      for (const op of segmented.operations) {
        operations.push({
          chapter: index,
          ...op,
        });
      }
      const mergedText = segmented.mergedText;
      draft = {
        ok: true,
        profile: "chapterDraft",
        options: {
          temperature: 0.72,
          maxTokens: plannedSegments.reduce((sum, segment) => sum + segment.targetChars, 0),
          disableReasoning: true,
        },
        attempts: [],
        retryCount: 0,
        latencyMs: sum(segmented.operations.map((op) => op.latencyMs)),
        text: mergedText,
        chars: charCount(mergedText),
        finishReason: "segmented_merge",
        usage: {
          cost: Number(
            sum(segmented.operations.map((op) => Number(op.usage?.cost))).toFixed(8),
          ),
        },
      };
    } else {
      draft = await chat(apiBase, apiKey, model, "chapterDraft", [
        {
          role: "system",
          content:
            "You are a professional Chinese novelist drafting a complete production-length chapter. Write only chapter prose. The chapter is incomplete if it ends below the requested character range; continue with scene action, dialogue, consequence, and payoff pressure until the range is satisfied.",
        },
        { role: "user", content: context },
      ]);
    }
    operations.push({
      chapter: index,
      kind: segmented.ok ? "draft_merge" : "draft",
      ...draft,
      preview: preview(draft.text),
    });
    if (!draft.ok) {
      const summary = buildSummary({
        apiBase,
        model,
        contract,
        chapters,
        operations,
        startedAt,
        completed: false,
      });
      writeReport(reportPath, summary, chapters, operations);
      console.log(
        JSON.stringify({
          event: "chapter_failed",
          chapter: index,
          kind: "draft",
          status: draft.status,
          completed: chapters.length,
          failedOperationCount: summary.failedOperationCount,
          reportPath,
        }),
      );
      break;
    }

    let finalText = draft.text.trim();
    const repairs = [];
    const repairDetails = [];
    let outcome = chapterOutcome(charCount(finalText), contract);

    while (outcome === "underMin" && repairs.filter((repair) => repair === "continuation").length < 2) {
      const plan = continuationPlan(charCount(finalText), contract);
      if (!plan) break;
      const continuation = await chat(apiBase, apiKey, model, "chapterContinuation", [
        {
          role: "system",
          content:
            `只输出追加的中文正文。这是缺口修复，不是第二章，不要复述已有内容，不要新开大支线。` +
            `只补 ${plan.addMinChars}-${plan.addMaxChars} 字；输出后全文应落在 ${plan.targetFinalMinChars}-${plan.targetFinalMaxChars} 字，绝不能超过 ${contract.maxChars} 字。` +
            "用行动、对话、后果或钩子完成当前未收束拍点，达到可保存位置立刻停止。",
        },
        {
          role: "user",
          content:
            `当前章节 ${plan.currentChars} 字，低于最低 ${contract.minChars} 字。` +
            `\n本次只允许追加 ${plan.addMinChars}-${plan.addMaxChars} 字，追加后全文目标 ${plan.targetFinalMinChars}-${plan.targetFinalMaxChars} 字。` +
            `\n\n已有正文:\n${finalText}` +
            `\n\n项目上下文:\n${context}` +
            "\n\n继续写新的正文，只输出追加部分。",
        },
      ], 120000, { maxTokens: plan.maxTokens });
      operations.push({
        chapter: index,
        kind: "continuation",
        repairPlan: plan,
        ...continuation,
        preview: preview(continuation.text),
      });
      if (!continuation.ok) break;
      if (continuation.ok && continuation.text.trim()) {
        repairs.push("continuation");
        repairDetails.push({
          kind: "continuation",
          plannedAddMinChars: plan.addMinChars,
          plannedAddMaxChars: plan.addMaxChars,
          outputChars: continuation.chars,
          maxTokens: plan.maxTokens,
        });
        finalText = `${finalText}\n${continuation.text.trim()}`.trim();
      }
      outcome = chapterOutcome(charCount(finalText), contract);
    }

    let compressAttempts = 0;
    while (outcome === "overMax" && compressAttempts < 3) {
      compressAttempts += 1;
      const plan = compressionPlan(charCount(finalText), contract, compressAttempts);
      const compressed = await chat(apiBase, apiKey, model, "chapterCompress", [
        {
          role: "system",
          content:
            `只输出压缩后的完整中文章节正文。这是删改压缩任务，不是续写。` +
            `目标 ${plan.targetMinChars}-${plan.targetMaxChars} 字，硬上限 ${plan.hardMaxChars} 字。` +
            (compressAttempts === 1
              ? "必须删除重复环境描写、旁支动作、路人细节、重复心理和解释性句子；保留锚点参与、人物选择、因果后果和结尾钩子。"
              : "前一轮压缩未满足硬合同。必须优先满足字数合同；如果细节与字数冲突，删除支线、次要动作、解释性背景和重复对话，只保留主因果链、锚点参与、人物选择和结尾钩子。"),
        },
        {
          role: "user",
          content:
            `当前章节 ${plan.currentChars} 字，超过最高 ${contract.maxChars} 字。` +
            `\n至少需要删减 ${plan.requiredCutChars} 字。压缩目标 ${plan.targetMinChars}-${plan.targetMaxChars} 字，绝不能超过 ${plan.hardMaxChars} 字。` +
            `\n\n当前正文:\n${finalText}` +
            `\n\n项目上下文:\n${context}` +
            `\n\n重写为一章完整正文，只输出压缩版正文。第 ${compressAttempts} 轮压缩必须落入目标区间。`,
        },
      ], 150000, { maxTokens: plan.maxTokens });
      operations.push({
        chapter: index,
        kind: "compress",
        repairPlan: plan,
        ...compressed,
        preview: preview(compressed.text),
      });
      if (compressed.ok && compressed.text.trim()) {
        repairs.push("compress");
        repairDetails.push({
          kind: "compress",
          attempt: compressAttempts,
          targetMinChars: plan.targetMinChars,
          targetMaxChars: plan.targetMaxChars,
          outputChars: compressed.chars,
          maxTokens: plan.maxTokens,
        });
        finalText = compressed.text.trim();
      }
      outcome = chapterOutcome(charCount(finalText), contract);
    }

    const fullTextArtifact = writePrivateArtifact(artifactsDir, index, "chapter_full_text", finalText, "txt");
    let factExtraction = await extractChapterFacts(
      apiBase,
      apiKey,
      model,
      finalText,
      dynamicAnchors,
      chapterPlan,
    );
    if (factExtraction.ok) {
      operations.push({
        chapter: index,
        kind: "fact_extract",
        ...factExtraction.operation,
        parsed: factExtraction.operation.parsed,
        preview: preview(factExtraction.operation.text),
      });
    } else {
      operations.push({
        chapter: index,
        kind: "fact_extract",
        ok: false,
        status: factExtraction.status ?? "fact_extract_failed",
        error: factExtraction.error,
        preview: preview(factExtraction.raw),
      });
    }
    const continuityIndex = buildContinuityIndex(chapterHistory);
    let extractedFacts = factExtraction.ok
      ? factExtraction.facts.map((fact, segmentIdx) => {
          const normalized = normalizeFact(fact, index, `fact-${segmentIdx + 1}`, fullTextArtifact.sha256);
          normalized.status = evaluateFactStatus(normalized, continuityIndex, dynamicAnchors);
          return normalized;
        })
      : [];
    let continuityWarnings = factExtraction.ok ? factExtraction.continuityWarnings : [];
    let hallucinationGate = buildHallucinationGate(extractedFacts, continuityWarnings);
    if (hallucinationGate.passed === false) {
      const unsupportedFacts = extractedFacts.filter((fact) => fact.status === "unsupported");
      const repaired = await repairHallucinationChapter(
        apiBase,
        apiKey,
        model,
        finalText,
        chapterPlan,
        dynamicAnchors,
        hallucinationGate,
        unsupportedFacts,
      );
      operations.push({
        chapter: index,
        kind: "hallucination_repair",
        ...repaired,
        preview: preview(repaired.text),
      });
      if (repaired.ok && repaired.text.trim()) {
        finalText = repaired.text.trim();
        const repairedTextArtifact = writePrivateArtifact(
          artifactsDir,
          index,
          "chapter_full_text_repaired",
          finalText,
          "txt",
        );
        factExtraction = await extractChapterFacts(
          apiBase,
          apiKey,
          model,
          finalText,
          dynamicAnchors,
          chapterPlan,
        );
        if (factExtraction.ok) {
          operations.push({
            chapter: index,
            kind: "fact_extract_recheck",
            ...factExtraction.operation,
            parsed: factExtraction.operation.parsed,
            preview: preview(factExtraction.operation.text),
          });
          extractedFacts = factExtraction.facts.map((fact, segmentIdx) => {
            const normalized = normalizeFact(
              fact,
              index,
              `fact-recheck-${segmentIdx + 1}`,
              repairedTextArtifact.sha256,
            );
            normalized.status = evaluateFactStatus(normalized, continuityIndex, dynamicAnchors);
            return normalized;
          });
          continuityWarnings = factExtraction.continuityWarnings;
          hallucinationGate = buildHallucinationGate(extractedFacts, continuityWarnings);
        }
      }
    }
    const factArtifact = writePrivateArtifact(
      artifactsDir,
      index,
      "chapter_fact_extract",
      {
        chapter: index,
        dynamicAnchors,
        extractedFacts,
        continuityWarnings,
        hallucinationGate,
        risks: factExtraction.ok ? factExtraction.risks : [],
        resolvedPromises: factExtraction.ok ? factExtraction.resolvedPromises : [],
        newAnchors: factExtraction.ok ? factExtraction.newAnchors : [],
      },
      "json",
    );
    const carry = scoreAnchorCarry(finalText, dynamicAnchors);
    const finalChars = charCount(finalText);
    const previousChapter = chapters.at(-1);
    const adjacentSimilarity = previousChapter
      ? jaccardSimilarity(finalText, previousChapter.fullPreviewForSimilarity)
      : 0;
    chapters.push({
      chapter: index,
      volume: chapterPlan.volume,
      arc: chapterPlan.arc,
      phase: chapterPlan.phase,
      draftChars: draft.chars,
      finalChars,
      outcome,
      contractCompliant: outcome === "valid",
      repairs,
      repairDetails,
      dynamicAnchors,
      segmentPlan: plannedSegments,
      segmentAttention,
      segmentRiskChecks,
      anchorMentionRate: carry.mentionRate,
      anchorCarryRate: carry.carryRate,
      anchorCarryItems: carry.items,
      extractedFacts,
      continuityWarnings,
      hallucinationGate,
      artifactRefs: [fullTextArtifact, factArtifact],
      adjacentSimilarity: Number(adjacentSimilarity.toFixed(4)),
      rollingSummaryChars: charCount(rollingSummary),
      contextChars: charCount(context),
      preview: preview(finalText),
      fullPreviewForSimilarity: preview(finalText, 1800),
    });
    chapterHistory.push({
      chapter: index,
      phase: chapterPlan.phase,
      summary: preview(finalText, 260),
      keyAnchors: factExtraction.ok
        ? uniqueStrings([
            ...dynamicAnchors,
            ...(factExtraction.newAnchors ?? []).filter((anchor) =>
              extractedFacts.some(
                (fact) =>
                  ["supported", "inferred"].includes(fact.status) &&
                  (fact.subject === anchor || fact.object === anchor),
              ),
            ),
          ]).slice(0, 6)
        : dynamicAnchors,
      extractedFacts: extractedFacts.filter((fact) => fact.status !== "unsupported"),
      continuityFacts: extractedFacts
        .filter((fact) => ["supported", "inferred"].includes(fact.status))
        .slice(-8),
    });
    rollingSummary = updateRollingSummary(chapterHistory);

    const progressSummary = buildSummary({
      apiBase,
      model,
      contract,
      chapters,
      operations,
      startedAt,
      completed: index === chapterCount,
    });
    writeReport(reportPath, progressSummary, chapters, operations);
    console.log(
      JSON.stringify({
        event: "chapter_completed",
        chapter: index,
        chapterCount,
        finalChars,
        outcome,
        repairs,
        complianceRate: progressSummary.complianceRate,
        failedOperationCount: progressSummary.failedOperationCount,
        avgLatencyMs: progressSummary.avgLatencyMs,
        p95LatencyMs: progressSummary.p95LatencyMs,
        totalCost: progressSummary.totalCost,
        reportPath,
      }),
    );
  }

  const summary = buildSummary({
    apiBase,
    model,
    contract,
    chapters,
    operations,
    startedAt,
    completed: chapters.length === chapterCount,
  });
  writeReport(reportPath, summary, chapters, operations);
  console.log(JSON.stringify(summary, null, 2));
  console.error(`Report saved to ${reportPath}`);
  if (
    summary.failedOperationCount > 0 ||
    summary.complianceRate < 1 ||
    summary.hallucinationGateFailureCount > 0
  ) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
