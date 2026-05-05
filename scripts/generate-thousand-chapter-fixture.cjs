const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const reportsDir = path.join(repoRoot, "reports");
const fixturePath = path.join(reportsDir, "thousand_chapter_fixture.json");

const chapterCount = Number(process.env.CHAPTERS || 1000);
const targetChars = Number(process.env.WORDS_PER_CHAPTER || 3500);
const volumeCount = Number(process.env.VOLUME_COUNT || 10);
const canonEntityCount = Number(process.env.CANON_ENTITY_COUNT || 200);
const promiseDensity = Number(process.env.PROMISE_DENSITY || 0.5);
const crossVolumeRatio = Number(process.env.CROSS_VOLUME_PROMISE_RATIO || 0.15);

const seeds = [
  "寒影刀", "张三", "镜中墟", "霜铃塔", "旧债", "北境宗门", "账册", "密道", "封门", "倒影",
  "雨夜", "茶馆", "门槛", "雾气", "火光", "铁锁", "脚步", "旧友", "誓言", "代价",
];

function rand(seed) {
  let state = seed >>> 0;
  return () => {
    state = (1664525 * state + 1013904223) >>> 0;
    return state / 0xffffffff;
  };
}

function pick(rng, list) {
  return list[Math.floor(rng() * list.length)];
}

function makeChapterText(rng, idx) {
  const target = Math.max(3000, targetChars + Math.floor((rng() - 0.5) * 1000));
  let text = `第${idx}章 `;
  while ([...text].length < target) {
    text += `${pick(rng, seeds)}${pick(rng, ["压了过来", "没有开口", "忽然停住", "像旧伤一样发作", "逼着林墨作出选择", "在门后回响"])}。`;
  }
  return [...text].slice(0, target).join("");
}

function buildFixture() {
  const rng = rand(20260506);
  const chapters = [];
  const canonEntities = Array.from({ length: canonEntityCount }, (_, i) => ({
    id: `entity-${i + 1}`,
    name: `实体${i + 1}`,
  }));
  let promiseId = 1;
  for (let i = 1; i <= chapterCount; i += 1) {
    const volume = Math.ceil((i / chapterCount) * volumeCount);
    const promiseCount = rng() < promiseDensity ? 1 + Math.floor(rng() * 2) : 0;
    const promises = Array.from({ length: promiseCount }, () => ({
      id: `promise-${promiseId++}`,
      kind: rng() < crossVolumeRatio ? "cross_volume" : "local",
      title: `${pick(rng, seeds)}线索`,
      expectedPayoff: `Chapter-${Math.min(chapterCount, i + 5 + Math.floor(rng() * 30))}`,
    }));
    chapters.push({
      chapterNumber: i,
      chapterTitle: `第${i}章`,
      volume,
      text: makeChapterText(rng, i),
      summary: `${pick(rng, seeds)}推动了当前冲突，并让${pick(rng, seeds)}的代价变得更具体。`,
      promises,
      activeEntities: canonEntities.slice((i * 3) % canonEntities.length, ((i * 3) % canonEntityCount) + 8).map((e) => e.name),
    });
  }
  return {
    generatedAt: new Date().toISOString(),
    chapterCount,
    targetChars,
    volumeCount,
    canonEntityCount,
    promiseDensity,
    crossVolumeRatio,
    chapters,
  };
}

fs.mkdirSync(reportsDir, { recursive: true });
const fixture = buildFixture();
fs.writeFileSync(fixturePath, JSON.stringify(fixture, null, 2));
console.log(`wrote ${fixturePath}`);
