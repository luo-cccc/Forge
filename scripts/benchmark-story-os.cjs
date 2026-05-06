const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const zlib = require("zlib");

const repoRoot = path.resolve(__dirname, "..");
const reportsDir = path.join(repoRoot, "reports");
const fixturePath = path.join(reportsDir, "thousand_chapter_fixture.json");
const evalReportPath = path.join(reportsDir, "eval_report.json");
const reportPath = path.join(reportsDir, "scale_benchmark.json");
const chartPath = path.join(reportsDir, "scale_benchmark_chart.png");
const requestProfilesPath = path.join(repoRoot, "config", "llm-request-profiles.json");

const authoritativeFixtures = [
  "writer_agent:chapter_contract_length_compliance_over_50_chapters",
  "writer_agent:search_hybrid_50k_chunks_under_100ms",
  "writer_agent:thousand_chapter_context_assembly_under_500ms",
  "writer_agent:ledger_snapshot_tiered_latency",
];

function nowMs() {
  return Number(process.hrtime.bigint() / 1000000n);
}

function charCount(text) {
  return [...String(text || "")].length;
}

function time(label, fn) {
  const start = nowMs();
  const value = fn();
  return { label, latencyMs: nowMs() - start, value };
}

function loadFixture() {
  if (!fs.existsSync(fixturePath)) {
    throw new Error(`missing fixture: ${fixturePath}. Run generate-thousand-chapter-fixture.cjs first.`);
  }
  return JSON.parse(fs.readFileSync(fixturePath, "utf8"));
}

function loadProfiles() {
  return JSON.parse(fs.readFileSync(requestProfilesPath, "utf8"));
}

function runRustEvals() {
  if (process.env.SKIP_RUST_EVALS === "1") {
    return { skipped: true, status: 0 };
  }
  if (fs.existsSync(evalReportPath)) {
    fs.unlinkSync(evalReportPath);
  }
  const result = spawnSync("cargo", ["run", "-p", "agent-evals"], {
    cwd: repoRoot,
    stdio: "inherit",
  });
  return { skipped: false, status: result.status ?? 1 };
}

function loadAuthoritativeGateResults() {
  if (!fs.existsSync(evalReportPath)) {
    throw new Error(`missing Rust eval report: ${evalReportPath}`);
  }
  const report = JSON.parse(fs.readFileSync(evalReportPath, "utf8"));
  const byName = new Map(report.results.map((result) => [result.fixture, result]));
  return authoritativeFixtures.map((fixture) => {
    const result = byName.get(fixture);
    if (!result) {
      throw new Error(`missing authoritative eval fixture: ${fixture}`);
    }
    return {
      fixture,
      passed: result.passed,
      actual: result.actual,
      errors: result.errors,
    };
  });
}

function benchmarkPoint(chapters, take) {
  const subset = chapters.slice(0, take);
  const contextAssembly = time("contextAssembly", () => {
    const target = subset[subset.length - 1];
    return [
      target.summary,
      ...subset.slice(Math.max(0, subset.length - 3)).map((chapter) => chapter.summary),
    ].join("\n");
  });
  const projectBrainQuery = time("projectBrainQuery", () =>
    subset
      .flatMap((chapter) => chapter.activeEntities)
      .filter((name) => name.includes("实体"))
      .slice(0, 20),
  );
  const ledgerSnapshot = time("ledgerSnapshot", () => ({
    openPromises: subset.flatMap((chapter) => chapter.promises).slice(-50).length,
    activeEntities: subset.flatMap((chapter) => chapter.activeEntities).slice(-50).length,
  }));
  const writerMemoryBytes = Buffer.byteLength(JSON.stringify(subset), "utf8");
  const chunkCount = subset.reduce((sum, chapter) => sum + Math.ceil(charCount(chapter.text) / 500), 0);
  return {
    chapterCount: take,
    contextAssemblyLatencyMs: contextAssembly.latencyMs,
    projectBrainQueryLatencyMs: projectBrainQuery.latencyMs,
    ledgerSnapshotLatencyMs: ledgerSnapshot.latencyMs,
    writerMemoryBytes,
    chunkCount,
  };
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBuffer = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function writePngChart(points, gates) {
  const width = 900;
  const height = 360;
  const pixels = Buffer.alloc(width * height * 3, 255);
  const setPixel = (x, y, r, g, b) => {
    if (x < 0 || x >= width || y < 0 || y >= height) return;
    const idx = (y * width + x) * 3;
    pixels[idx] = r;
    pixels[idx + 1] = g;
    pixels[idx + 2] = b;
  };
  const rect = (x, y, w, h, color) => {
    for (let yy = y; yy < y + h; yy += 1) {
      for (let xx = x; xx < x + w; xx += 1) {
        setPixel(xx, yy, color[0], color[1], color[2]);
      }
    }
  };

  rect(0, 0, width, height, [248, 250, 252]);
  rect(70, 40, 2, 260, [31, 41, 55]);
  rect(70, 300, 760, 2, [31, 41, 55]);
  const maxLatency = Math.max(
    1,
    ...points.flatMap((point) => [
      point.contextAssemblyLatencyMs,
      point.projectBrainQueryLatencyMs,
      point.ledgerSnapshotLatencyMs,
    ]),
  );
  const colors = {
    context: [37, 99, 235],
    search: [5, 150, 105],
    ledger: [217, 119, 6],
    failedGate: [220, 38, 38],
  };
  const groupWidth = Math.floor(720 / points.length);
  points.forEach((point, i) => {
    const baseX = 90 + i * groupWidth;
    const values = [
      [point.contextAssemblyLatencyMs, colors.context],
      [point.projectBrainQueryLatencyMs, colors.search],
      [point.ledgerSnapshotLatencyMs, colors.ledger],
    ];
    values.forEach(([value, color], j) => {
      const barHeight = Math.max(1, Math.round((value / maxLatency) * 220));
      rect(baseX + j * 16, 300 - barHeight, 11, barHeight, color);
    });
  });
  const failedGateCount = gates.filter((gate) => !gate.passed).length;
  if (failedGateCount > 0) {
    rect(840, 40, 26, 260, colors.failedGate);
  }

  const raw = Buffer.alloc((width * 3 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const rowStart = y * (width * 3 + 1);
    raw[rowStart] = 0;
    pixels.copy(raw, rowStart + 1, y * width * 3, (y + 1) * width * 3);
  }
  const signature = Buffer.from("89504e470d0a1a0a", "hex");
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  fs.writeFileSync(
    chartPath,
    Buffer.concat([
      signature,
      pngChunk("IHDR", ihdr),
      pngChunk("IDAT", zlib.deflateSync(raw)),
      pngChunk("IEND", Buffer.alloc(0)),
    ]),
  );
}

fs.mkdirSync(reportsDir, { recursive: true });
const fixture = loadFixture();
const profiles = loadProfiles();
const evalRun = runRustEvals();
const authoritativeGateResults = loadAuthoritativeGateResults();
const failedGates = authoritativeGateResults.filter((gate) => !gate.passed);
if (failedGates.length > 0) {
  throw new Error(`authoritative Rust gates failed: ${failedGates.map((gate) => gate.fixture).join(", ")}`);
}
if (evalRun.status !== 0) {
  throw new Error(`cargo run -p agent-evals exited with status ${evalRun.status}`);
}
const samplePoints = [10, 50, 100, 200, 500, 1000].filter((count) => count <= fixture.chapters.length);
const points = samplePoints.map((count) => benchmarkPoint(fixture.chapters, count));
const report = {
  generatedAt: new Date().toISOString(),
  sourceFixture: fixturePath,
  authoritativeGateSource: evalReportPath,
  authoritativeGateResults,
  chapterContract: {
    targetChars: 3500,
    minChars: 3000,
    maxChars: 4000,
    saveHardFloorChars: 2800,
    saveHardCeilingChars: 4300,
  },
  chapterProfiles: {
    chapterDraft: profiles.chapter_draft,
    chapterContinuation: profiles.chapter_continuation,
    chapterCompress: profiles.chapter_compress,
  },
  offlineFixtureShapePoints: points,
};
fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
writePngChart(points, authoritativeGateResults);
console.log(`wrote ${reportPath}`);
console.log(`wrote ${chartPath}`);
