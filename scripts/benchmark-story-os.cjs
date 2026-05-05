const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const reportsDir = path.join(repoRoot, "reports");
const fixturePath = path.join(reportsDir, "thousand_chapter_fixture.json");
const reportPath = path.join(reportsDir, "scale_benchmark.json");
const chartPath = path.join(reportsDir, "scale_benchmark_chart.png");

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

function writePlaceholderChart(points) {
  const content = Buffer.from(`scale benchmark placeholder\n${JSON.stringify(points, null, 2)}`, "utf8");
  fs.writeFileSync(chartPath, content);
}

fs.mkdirSync(reportsDir, { recursive: true });
const fixture = loadFixture();
const samplePoints = [10, 50, 100, 200, 500, 1000].filter((count) => count <= fixture.chapters.length);
const points = samplePoints.map((count) => benchmarkPoint(fixture.chapters, count));
const report = {
  generatedAt: new Date().toISOString(),
  sourceFixture: fixturePath,
  points,
};
fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
writePlaceholderChart(points);
console.log(`wrote ${reportPath}`);
console.log(`wrote ${chartPath}`);
