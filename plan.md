# Forge Writer Agent 4 周冲刺计划

Last updated: 2026-05-06

## 0. 产品北极星

Forge 不是“带 AI 功能的写作工具”，而是“以 agent 为主体的长篇创作系统”。

- 编辑器只是工作台。
- Agent 才是产品主体。
- Agent 必须像作者的第二大脑、第二作者、写作伴侣一样，持续理解这本书、守住承诺、发现风险、提出可审查行动，并从作者反馈中学习。

一句话：

> 作者不是在和 AI 聊天，而是在和一个懂这本书、能一起想、一起写、一起复盘的创作伙伴并肩工作。

北极星约束：

- Forge 不以“生成一段字”为成功，而以“长期写作更稳、更快、更少返工”为成功。
- Forge 不只适用于 20 章中篇，而必须能支撑 1000 章、每章 3500 字 ±500 字的长篇。
- 一切短期冲刺都必须服务这个长期目标，不能为了局部体验牺牲长篇稳定性。

---

## 1. 本轮不变边界

这 4 周冲刺允许重写路径、调整 schema、重排阶段，但不允许破坏以下已成立能力：

- 保存安全边界不能退化：
  - dirty state、chapter switching、autosave、accepted feedback、batch generation dirty protection 必须继续成立。
- 所有写入仍必须是 typed、reviewable、audited：
  - 不允许为了提速重新引入绕过 `WriterOperation`、approval/audit、`TaskReceipt` 的写路径。
- Companion 默认必须保持降噪：
  - trace/debug 留在 Inspect，不允许把右侧面板重新变成运行日志窗口。
- Story Contract / Chapter Mission / Result Feedback Loop 不能失效：
  - 新的 `ChapterContract` 和 `Story OS` 只能叠加，不能替代掉这些已有基础合同。
- provider budget / post-write diagnostics / metacognitive gate 不能被跳过：
  - staged pipeline 和 Sprint v2 必须复用现有边界，而不是另起一套无审计流程。
- `npm run verify`、现有 Rust tests、现有 eval 基线不能被“临时跳过”：
  - 新能力可以增量放行，但不能靠关闭旧 gate 获得通过。

如果某项冲刺改动必须突破这些边界，先停下来重写方案，不允许边做边破。

---

## 2. 冲刺目标

4 周内，把 Forge 从“单章生成器 + 近期记忆助手”推进到“可验证的长篇生产系统第一版”。

本轮只追 5 件事：

1. 把 `3500 ± 500` 字变成系统级 `ChapterContract`，不再只是口号。
2. 把章节生成从单次 draft 改成可控的多阶段流水线。
3. 把上下文与检索从全量扫描改成 `Story OS` 分层查询。
4. 把批量推进从 `Supervised Sprint v1` 升级到可暂停 / 恢复 / checkpoint 的 `Sprint v2`。
5. 把评测从 20 章 synthetic 提升到千章 synthetic，并补 30 章真实作者回归门。

本轮不追：

- Reader Compensation 深化
- Emotional Debt 新玩法
- External Research 真正接外部工具
- 新的大型 UI 面板
- 新的聊天主界面

---

## 3. 当前事实

当前已成立：

- Writer Agent Kernel、TaskPacket、typed operations、memory、trajectory、approval/audit、provider budget、post-write diagnostics、metacognitive gate 已有地基。
- Story Contract、Chapter Mission、Result Feedback Loop、Promise Ledger 已经活跃。
- `supervised_sprint.rs` 已有第一阶段能力，覆盖 preflight → receipt → review → save → settlement。
- 当前验证基线可稳定通过：`agent-harness-core` 89 tests、`agent-writer` 242 tests、`agent-evals` 265/265 passing、`check:audit` 72 commands 0 issues。
- Week 1 已落地：
  - `ChapterContract`
  - 章节生成 continuation / compress / length-validate 骨架
  - `probe:scale`、1000 章 fixture 和 benchmark 报告
- Week 2 已落地：
  - `VectorDB` 候选收敛搜索
  - `Volume / Arc / Book` 数据层和最小 CRUD
- Week 3 已落地：
  - `query_story_os()` 主路径
  - `BookState / ArcSnapshot / VolumeSnapshot` warm tier
  - 分层 `ledger_snapshot()`
- Week 4 已落地：
  - `Sprint v2` pause / resume / checkpoint / budget ceiling
  - 最小前端状态显示
  - `real_author_session_thirty_chapter_gate` opt-in 长会话门
- 真实长章调参已落地一轮：
  - `real:long-chapter` 使用真实 provider 校验 `3000-4000` 字章节合同。
  - 2026-05-06 当前配置下已跑通 2 / 3 / 5 章真实样本；5 章报告为 `chapterCount=5`、`operationCount=7`、`failedOperationCount=0`、`complianceRate=1.0`、`minFinalChars=3430`、`maxFinalChars=3916`、`minAnchorCarryRate=0.8`、`p95LatencyMs=73840`。
  - 本轮修复证据包括：4184 字 draft 被 compress 到 3708 字；2497 字 draft 被 bounded continuation 修复到 3438 字，未复现 5893 字超长失控。
- 长篇连续性验证链已落地 smoke 版：
  - `real:long-chapter` 已补入分段生成、动态 required anchors、每章私有全文 artifact、章节事实抽取、连续性风险门（`contradiction / OOC / canon_drift / bad_payoff / state_conflict`）与事实 CSV。
  - 2026-05-06 smoke 报告 `reports/real_long_chapter_contract_probe_functional_smoke_v7.json` 已验证：2 章真实样本下 `hallucinationGateFailureCount=0`、`failedOperationCount=0`、`operationStats.segment_draft.avgLatencyMs=11646`、`operationStats.fact_extract.avgLatencyMs=9509`。
  - 当前 gate 定义已从“禁止发展”改为“允许新发展，但阻断长篇连续性硬错误”；`unsupported new fact` 当前只记录，不再默认判失败。

当前剩余真实缺口：

- 默认 `chapter_draft / continuation / compress` profile 已提升到支持 3500 字量级；当前配置已通过 5 章真实长章样本，但这还不是“千章真实稳定”证据，仍需继续用更多真实 provider / 不同模型 / 更长连续会话样本观察实际稳定性。
- 新的连续性验证链只在 2 章真实 smoke 上闭环，尚未完成 30 / 100 章级别再验证；尤其还没有重新验证 `compress + hard_compress + continuity gate` 组合在长会话下的稳定性。
- `50 章 synthetic 长度合规率 >95%` 已进入 `agent-evals`，并随 `npm run probe:scale` 写入 `reports/eval_report.json` 与 `reports/scale_benchmark.json`。
- `search_hybrid() < 100ms @ 50,000 chunks`、`context assembly < 500ms @ Chapter 500`、`ledger_snapshot() <50ms` 已有正式 Rust gate artifact。
- `30 章真实作者 gate` 已在本地显式执行并归档：`reports/real_author_session_thirty_chapter_gate.json`。

### 3.2 下一阶段补强主线

在 4 周主线闭合后，下一阶段不再优先追求“更多入口 / 更多面板 / 更多 agent 名称”，而是把 Forge 补成更硬的长篇生产内核。只采纳 5 条：

1. 输入治理编译层
   - 把 `ChapterMission / 作者当前指令 / Story OS 检索结果 / 规则优先级` 先编译成可审查工件，再进入正文生成。
   - 默认产物是运行时 `intent / selected evidence / rule stack / trace`，优先进入 `Inspect / run event / audit trail`，不把工作区暴露成大量必须手管的文件。
   - 目标：让“为什么这样写”在每章生成前可见、可复盘、可复用。

2. 结构化权威状态
   - 把章节结算统一收敛到 typed delta：`chapter_fact_delta / promise_delta / arc_delta / book_state_delta`，由代码层 apply，再投影给 UI 与导出层。
   - `BookState / ArcSnapshot / Promise Ledger / current chapter facts` 是权威状态；自然语言摘要和 markdown 只做投影，不再作为唯一真相源。
   - 目标：减少设定漂移、OOC、错误兑现与跨章冲突的隐性累积。

3. 独立长度治理相位
   - 继续把长度 contract 从修订语义中剥离：`draft -> continuation/compress -> hard compress -> audit`，长度修复单独计费、单独记 telemetry。
   - `reviser` 负责内容质量，`normalizer/length phase` 负责字数区间，不再混成一个模糊“修一切”的步骤。
   - 目标：让真实 provider 下的长章稳定性有单独优化面，而不是被审计/修订噪声掩盖。

4. Hook debt 治理
   - 在现有 `Promise Ledger` 之上增加 `stale / blocked / promoted / core` 机制，并在卷边界、长静默、错误兑现时提高优先级。
   - 规划与结算都必须显式处理旧债：`advance / resolve / defer(with reason)`，不能只新增 hook 不结账。
   - 目标：控制长篇最常见的“旧债堆积、兑现失焦、后期散掉”问题。

5. Snapshot / repair-state 路径
   - 维持每章快照、回滚、repair-state 为一等能力；章节正文、state delta、审计结果、provider telemetry 必须能定位到最近安全点。
   - 当章节正文可保留但 state 结算失败时，允许 `repair-state`，而不是强迫整章重写。
   - 目标：把“可恢复”做成生产特性，而不是调试手段。

这一阶段明确不采纳：

- 不把 Forge 扩展成 `CLI + TUI + Studio + chat shell + skill` 的多入口产品矩阵。
- 不把长期控制面外露为大量必须手工维护的 truth files。
- 不为了“看起来像多 agent 系统”而继续膨胀 agent 数量。

这一阶段的完成定义：

- 章节生成前能看到 `intent / selected evidence / rule stack / trace`。
- 章节生成后能看到 typed delta 的 apply 结果与失败原因，而不是只看到一段 prose。
- 长度问题、连续性问题、state 结算问题各有独立 telemetry 与恢复路径。
- `Promise Ledger` 能区分普通未回收、陈旧债务、核心债务、阻塞债务。
- `repair-state` 可以在不改正文的前提下修复 state 并重新过 gate。

### 3.3 底层封顶冲刺

在 `3.2` 主线推进后，必须插入一个单独的“底层封顶冲刺”，先把内核语义封死，再进入用户感受层和前端打磨。

这不是新功能阶段，而是“把已经做出来的能力收成一套稳定生产内核”的阶段。

顺序必须是：

1. 底层封顶
2. 用户感受层打磨
3. 前端表达层完善

#### 3.3.1 为什么必须单独封顶

当前 Forge 已经有：

- `ChapterContract`
- staged generation
- `Story OS`
- `Sprint v2`
- typed settlement
- `repair-state`
- `TodayFiveSummary` 雏形

这说明骨架已经足够，但“真相源 / 结算链 / 恢复链 / 默认行为”还没有完全定死。

如果现在直接大做前端，会把仍在变化的底层语义包进 UI，后续每次改 state、settlement、save feedback、Companion 默认行为，都会带来前端返工。

所以这一阶段的目标不是“再多做”，而是“彻底收口”。

#### 3.3.2 本阶段只收 5 件事

1. 权威状态封顶
   - 章节保存、章节结算、结果反馈、`next beat`、`story debt`、Companion 默认状态，全部只认一套 authoritative state。
   - 禁止再出现“后台一套推断、前端一套推断、save-observe 又一套推断”。

2. settlement extraction 封顶
   - 把 `ChapterSettlementDelta` 的 `chapter_result / promise_updates / book_state_updates` 固化成显式 extraction pipeline：
     - candidate
     - confidence
     - evidence
     - materialize
     - apply
   - `settlement` 必须可回放、可比较、可审计，而不是“看起来结构化”。

3. 恢复与时序封顶
   - `repair-state`、回滚、重新 apply、重建 artifact 必须严格幂等。
   - 维修动作不能改变章节时间顺序、recent results 排序、`next beat` 语义。

4. provider budget / 审计封顶
   - 所有 provider call 必须进入同一种 `budget / approval / run-event` 体系。
   - 不允许再有 save 后隐形模型调用、旁路抽取、未记账 provider 路径。

5. 默认用户面封顶
   - “作者今天最该看的 5 件事”必须变成后端 schema，而不是前端 display helper 即时推断。
   - write mode 与 Inspect mode 的信息边界必须固定。

#### 3.3.3 本阶段明确不做

- 不新增大型产品面板
- 不扩张 agent 数量
- 不扩入口形态
- 不先做视觉重构
- 不先做营销/展示型前端包装

#### 3.3.4 本阶段完成定义

只有同时满足下面条件，才算“底层封顶完成”：

- 所有保存路径产出同构的 authoritative chapter result
- settlement extraction 有显式 artifact，可重放
- `repair-state` 幂等，且不改 chronology
- 所有 provider call 都能在 run events 中被追到
- Companion write mode 只消费后端 `TodayFiveSummary`
- `npm run verify` 全绿
- `cargo run -p agent-evals` 全绿
- 新增 3 个专项 gate：
  - `save path consistency`
  - `settlement replay consistency`
  - `chronology preservation`

#### 3.3.5 封顶后再进入的阶段

底层封顶完成后，下一阶段才进入“用户感受层打磨”。

这层不是前端样式，而是默认行为 contract，重点只处理：

- 什么时候打断作者
- 什么时候只记录不说话
- 什么时候必须提示风险
- `TodayFiveSummary` 的排序规则
- 什么信息进入 write mode
- 什么信息只进入 Inspect
- 哪些失败自动修复
- 哪些失败必须作者确认

只有这一层稳定下来，前端表达层才值得细做。

#### 3.3.6 最终一句话

本阶段的唯一目标是：

> 把 Forge 从“功能已经很多的长篇系统”，收成“语义闭合、真相统一、恢复可靠、默认行为稳定的长篇生产内核”。

### 3.1 当前完成度

按 4 周主线看，代码完成度是：

- Week 1：已完成最小闭环
- Week 2：已完成最小闭环
- Week 3：已完成最小闭环
- Week 4：已完成最小闭环

按最终验收门看，完成度是：

- 已完成：
  - `ChapterContract` 已进入生成/保存校验
  - `Sprint v2` pause / resume / checkpoint / budget ceiling / 持久化恢复 / 生成路径预算记录
  - `1000` 章 synthetic fixture 与 `probe:scale` 入口
  - `50` 章 synthetic 长度合规 gate
  - `search_hybrid()` 50k、Chapter 500 context assembly、`ledger_snapshot()` latency gate
  - `30` 章真实作者 gate 的 opt-in 测试入口与本地归档结果
- 部分完成：
  - staged generation 已有骨架，但不是完整 scene planner / segment orchestrator
  - `Story OS` 已接入主链路，但 cold tier 仍是保守实现
- 未完成：
  - 更深的 Sprint 自动编排，而不只是状态机与命令面

---

## 4. 本轮完成定义

当前按验收门核对如下：

| 验收门 | 状态 | 备注 |
|------|------|------|
| 默认 autonomous chapter generation 保存结果落在 `3000-4000` 字区间 | 已完成 | `ChapterContract` 在 model-output / save 双阶段校验；默认长章 profile 已提升到 3500 字量级 |
| `50` 章 synthetic 连续生成长度合规率 `>95%` | 已完成 | `writer_agent:chapter_contract_length_compliance_over_50_chapters` 当前为 `100%` |
| Chapter 500 的 context assembly `<500ms` | 已完成 | `writer_agent:thousand_chapter_context_assembly_under_500ms` 当前 `1ms` |
| `search_hybrid() <100ms @ 50,000 chunks` | 已完成 | `writer_agent:search_hybrid_50k_chunks_under_100ms` 当前 `4ms` |
| `ledger_snapshot() <50ms @ 1000 章 fixture` | 已完成 | `writer_agent:ledger_snapshot_tiered_latency` 当前 `0ms` |
| `Sprint v2` 支持 pause / resume / checkpoint / cumulative budget ceiling | 已完成 | 后端、命令、持久化、生成预算记录、最小 UI、eval 都已落地 |
| `1000` 章 synthetic gate 可跑 | 已完成 | `npm run probe:scale` 会生成 fixture、跑 Rust gates、写 benchmark JSON 和 PNG |
| `30` 章真实作者 gate opt-in 可跑 | 已完成 | 已显式执行通过；归档见 `reports/real_author_session_thirty_chapter_gate.json` |

判定标准不变：

- 如果只做到“架构更优雅”，但没有 gate 或 artifact，就不算最终完成。
- 当前状态更准确地说是：**4 周开发主线和原关键验收证据已闭合；后续重点转为真实模型质量、章节编排深度和用户体验打磨。**

---

## 5. 冲刺原则

1. 先上生产 contract，再上记忆分层。
2. 先量出退化曲线，再改搜索与上下文路径。
3. 每周必须有 hard gate；不过 gate，不进入下一周。
4. 所有新能力优先复用现有边界：`TaskPacket`、`TaskReceipt`、`RunEventStore`、`Supervised Sprint v1`。
5. 前端只做最低限度适配，不做额外产品包装。

---

## 6. 4 周路线

### Week 1：量化基线 + ChapterContract

目标：

- 拿到千章退化曲线。
- 把“3500 ± 500”变成 typed contract。
- 给章节生成补 staged pipeline 骨架。

范围：

- `scripts/generate-thousand-chapter-fixture.cjs`
- `scripts/benchmark-story-os.cjs`
- `src-tauri/src/chapter_generation/types_and_utils.in.rs`
- `src-tauri/src/chapter_generation/context.in.rs`
- `src-tauri/src/chapter_generation/draft_and_save.in.rs`
- `src-tauri/src/chapter_generation/pipeline/*`
- `config/llm-request-profiles.json`

本周负责文件：

- 后端：`src-tauri/src/chapter_generation/types_and_utils.in.rs`、`src-tauri/src/chapter_generation/context.in.rs`、`src-tauri/src/chapter_generation/draft_and_save.in.rs`、`src-tauri/src/chapter_generation/pipeline/*`
- 配置：`config/llm-request-profiles.json`
- 脚本：`scripts/generate-thousand-chapter-fixture.cjs`、`scripts/benchmark-story-os.cjs`
- 测试：`agent-evals/src/evals/*` 中与 chapter contract / generation pipeline 直接相关的文件

负责人 / 协作接口：

- 后端负责人：负责 `ChapterContract`、staged pipeline、generation run event、保存前长度 gate。
- 前端负责人：本周默认不接主要实现，只负责确认 staged pipeline 事件和 diagnostics payload 不会破坏现有 UI。
- 评测负责人：负责 `probe:scale`、新 chapter contract eval、长度合规率统计。
- 周中必须对齐的结果：
  - 周中前半：`ChapterContract` 字段、阶段名、run event payload 定稿
  - 周中后半：`probe:scale` 输出格式定稿，确保 Week 2 可直接消费

本周交付：

- 新增 `ChapterContract`
  - `target_chars = 3500`
  - `min_chars = 3000`
  - `max_chars = 4000`
  - `save_hard_floor = 2800`
  - `save_hard_ceiling = 4300`
- 章节流水线切成：
  - `preflight`
  - `scene_plan`
  - `segment_draft`
  - `merge`
  - `polish`
  - `length_validate`
  - `save`
  - `settlement`
- `< 3000` 字自动 continuation。
- `> 4000` 字自动 compress/polish。
- `probe:scale` 可输出：
  - context assembly latency
  - Project Brain query latency
  - ledger snapshot latency
  - WriterMemory 大小
  - chunk 数量

本周 gate：

- `npm run probe:scale` 可跑，且能输出 10/50/100/200/500/1000 章数据。
- `validate_generated_content()` 已接入 `ChapterContract`。
- 新增 eval：
  - `writer_agent:chapter_contract_continues_under_min_chars`
  - `writer_agent:chapter_contract_compresses_over_max_chars`
  - `writer_agent:chapter_contract_receipt_persists_output_bounds`

周末必须提交的 artifacts：

- `reports/scale_benchmark.json`
- `reports/scale_benchmark_chart.png`
- 一份 `ChapterContract` 结构定义与字段说明
- 一份 staged pipeline 阶段列表与 run event 样例
- 一份 Week 1 gate 结果摘要：哪些门过了，哪些门没过

失败时的降级：

- 不做完整 polish，只保留 continuation / compress gate。
- 不动前端。

### Week 2：搜索后端 + Volume / Arc / Book 数据层

目标：

- 解决最重的 O(n) 搜索。
- 把 Story OS 的结构性存储层搭出来。

范围：

- `agent-harness-core/src/vector_db.rs`
- `src-tauri/src/writer_agent/memory/*`
- `src-tauri/src/commands/outline.rs`
- `src-tauri/src/app_state.rs`
- `src/protocol.ts`
- `src/store.ts`

本周负责文件：

- 搜索后端：`agent-harness-core/src/vector_db.rs`
- 存储与 schema：`src-tauri/src/writer_agent/memory/*`
- command / state：`src-tauri/src/commands/outline.rs`、`src-tauri/src/app_state.rs`
- 前端协议：`src/protocol.ts`、`src/store.ts`
- 测试：`agent-harness-core` 搜索测试、`agent-evals` 中与 volume isolation 直接相关的文件

负责人 / 协作接口：

- 后端负责人：负责 ANN 搜索、schema migration、Volume / Arc / Book 数据层。
- 前端负责人：负责 `protocol.ts`、`store.ts`、最小 volume 协议接线，确保 Week 3 查询层可直接消费。
- 评测负责人：负责 ANN latency 基准、volume isolation eval、schema 回归验证。
- 周中必须对齐的结果：
  - 周中前半：`VectorDB` 新接口行为、`index_rebuild()` 生命周期、benchmark 口径
  - 周中后半：Volume / Arc / Book schema 字段冻结，避免 Week 3 查询层重复改模型

本周交付：

- `VectorDB` 改为 ANN + BM25 + keywords rerank。
- 增加 `index_rebuild()`。
- 新增数据结构：
  - `volumes`
  - `volume_snapshots`
  - `cross_volume_promises`
  - `arc_snapshots`
  - `book_state`
- 基础 `Volume CRUD`。
- `BookState` 至少记录：
  - 全书长期约束
  - mega-promises
  - irreversible changes

本周 gate：

- `search_hybrid()` 在 50,000 chunks 下 `< 100ms`。
- `vector_db` 原有测试继续通过。
- 新增 harness test：
  - `vector_db_hnsw_approximate_recall_within_95_percent`
- 新增 eval：
  - `writer_agent:volume_isolation_context_scope`

周末必须提交的 artifacts：

- ANN 搜索基准结果：50,000 chunks 下的延迟报告
- `VectorDB` 搜索前后对比摘要
- Volume / Arc / Book schema 变更清单
- `index_rebuild()` 使用说明
- 一份 Week 2 gate 结果摘要

失败时的降级：

- `ArcSnapshot` / `BookState` 先上 schema 和最小读写，不追完整 UI。

### Week 3：Story OS 查询路径 + 增量快照

目标：

- 把“全量拉取再过滤”换成“分层查询再评分”。
- 把保存后的状态更新改成差分。

范围：

- `src-tauri/src/writer_agent/context/assembly.in.rs`
- `src-tauri/src/writer_agent/context/types.in.rs`
- `src-tauri/src/writer_agent/context/spine.in.rs`
- `src-tauri/src/writer_agent/context_relevance/scoring.in.rs`
- `src-tauri/src/writer_agent/kernel/snapshots.rs`
- `src-tauri/src/writer_agent/kernel/metrics.rs`
- `src-tauri/src/writer_agent/kernel/prompts.rs`
- `agent-harness-core/src/task_packet.rs`
- `agent-harness-core/src/domain.rs`

本周负责文件：

- 上下文主路径：`src-tauri/src/writer_agent/context/assembly.in.rs`、`src-tauri/src/writer_agent/context/types.in.rs`、`src-tauri/src/writer_agent/context/spine.in.rs`
- relevance：`src-tauri/src/writer_agent/context_relevance/scoring.in.rs`
- 快照与指标：`src-tauri/src/writer_agent/kernel/snapshots.rs`、`src-tauri/src/writer_agent/kernel/metrics.rs`、`src-tauri/src/writer_agent/kernel/prompts.rs`
- runtime 契约：`agent-harness-core/src/task_packet.rs`、`agent-harness-core/src/domain.rs`
- 测试：`agent-evals/src/evals/*` 中 tiered memory / snapshot / incremental update 相关文件

负责人 / 协作接口：

- 后端负责人：负责 `query_story_os()`、增量更新、分层快照、tiered required context。
- 前端负责人：负责确认 `ledger_snapshot()` 与 Inspect/Companion 的兼容边界，不主导查询层实现。
- 评测负责人：负责 tiered memory eval、snapshot latency、Chapter 500 context 抽样校验。
- 周中必须对齐的结果：
  - 周中前半：hot / warm / cold source 归属表冻结
  - 周中后半：Chapter 500 context 示例和 snapshot 输出示例冻结，供 Week 4 Sprint/前端直接复用

本周交付：

- `query_story_os()` 替换旧 `assemble_context_pack()` 路径。
- 三层查询：
  - hot：当前卷 live state + 当前 arc active threads + 当前章 ± 3
  - warm：`BookState` + 邻近 `ArcSnapshot` + `VolumeSnapshot` + chapter summaries
  - cold：仅跨卷 promise / 显式 recall 时触发
- relevance 改为预筛选后评分，候选池上限 30。
- `incremental_state_update()` 只更新受影响条目，不扫全库。
- `ledger_snapshot()` 改为分层快照。

本周 gate：

- Chapter 500 默认写作上下文不读冷数据全文。
- Chapter 500 上下文可拿到当前 `arc` + `BookState`。
- `ledger_snapshot()` 在 1000 章 fixture 下 `< 50ms`。
- 新增 eval：
  - `writer_agent:tiered_memory_cold_tier_boundary`
  - `writer_agent:tiered_memory_cross_volume_promotion`
  - `writer_agent:arc_snapshot_available_in_warm_tier`
  - `writer_agent:book_state_present_without_cold_recall`
  - `writer_agent:incremental_update_bounded_entries`
  - `writer_agent:ledger_snapshot_tiered_latency`

周末必须提交的 artifacts：

- 一份 `query_story_os()` 设计摘要：hot / warm / cold 的命中规则
- 一份 Chapter 500 context 示例，标注各 source 来自哪一层
- `ledger_snapshot()` 1000 章 fixture 延迟报告
- 增量更新受影响条目统计样例
- 一份 Week 3 gate 结果摘要

失败时的降级：

- 先保证 chapter generation 和 ghost writing 两条路径走新 `Story OS`。
- Planning / Review / ManualRequest 可以下一周再切。

### Week 4：Sprint v2 + 真实回归门

目标：

- 把系统从“能安全写一章”推进到“能连续推进多章”。
- 把真实作者 gate 接上。

范围：

- `src-tauri/src/writer_agent/supervised_sprint.rs`
- `src-tauri/src/commands/generation.rs`
- `src/protocol.ts`
- `src/components/CompanionPanel.tsx`
- `src/components/OutlinePanel.tsx`
- `src/components/ProjectTree.tsx`
- `src/components/WriterInspectorPanel.tsx`
- `agent-evals/src/evals/*`
- `src-tauri/src/api_integration_tests.rs`

本周负责文件：

- 调度核心：`src-tauri/src/writer_agent/supervised_sprint.rs`
- command / integration：`src-tauri/src/commands/generation.rs`、`src-tauri/src/api_integration_tests.rs`
- 前端协议与最小 UI：`src/protocol.ts`、`src/components/CompanionPanel.tsx`、`src/components/OutlinePanel.tsx`、`src/components/ProjectTree.tsx`、`src/components/WriterInspectorPanel.tsx`
- 测试：`agent-evals/src/evals/*` 中 sprint / product metrics / long-session gate 相关文件

负责人 / 协作接口：

- 后端负责人：负责 Sprint v2 状态机、checkpoint、budget ceiling、真实作者 gate 后端入口。
- 前端负责人：负责 Sprint 状态显示、Volume 过滤、Inspect 最小分页/筛选。
- 评测负责人：负责 1000 章 synthetic gate、30 章真实作者 gate、50 章 soak 统计。
- 周中必须对齐的结果：
  - 周中前半：Sprint v2 状态机、checkpoint payload、command/event 协议冻结
  - 周中后半：真实作者 gate 执行步骤、统计口径、结果摘要模板冻结

本周交付：

- `Supervised Sprint v2`
  - chapter queue
  - pause / resume
  - durable checkpoint
  - cumulative provider budget ceiling
  - retry / skip-to-planning-review / rollback-to-last-save
- 前端最小适配：
  - Volume 过滤
  - Sprint 状态显示
  - Inspect 分页/筛选
- 评测升级：
  - 1000 章 synthetic gate
  - 30 章真实作者 gate（opt-in）
  - 50 章 soak gate（opt-in）

本周 gate：

- Sprint 可在 Chapter 37 暂停并恢复，receipt lineage 不丢。
- Sprint 在累计 budget 越界前阻止继续推进。
- 50 章 synthetic 长度合规率 `> 95%`。
- 1000 章 synthetic product metrics 无明显退化。
- 新增 eval：
  - `writer_agent:supervised_sprint_resume_from_checkpoint`
  - `writer_agent:supervised_sprint_budget_ceiling_blocks_advance`
  - `writer_agent:supervised_sprint_recovery_preserves_receipts`
  - `writer_agent:chapter_contract_length_compliance_over_50_chapters`
  - `writer_agent:product_metrics_no_degradation_at_500_chapters`
  - `writer_agent:thousand_chapter_context_assembly_under_500ms`

周末必须提交的 artifacts：

- 一份 Sprint v2 状态机说明
- 一份 checkpoint payload 样例
- 1000 章 synthetic gate 报告
- 30 章真实作者 gate 运行说明与结果摘要
- 一份 Week 4 gate 结果摘要

失败时的降级：

- UI 只保留 Inspect 和 Companion 的最小 Sprint 状态，不做更多操作面。
- 真实作者 gate 可先以命令行方式存在，不要求完整前端按钮。

---

## 7. 周间依赖

- Week 2 不能早于 Week 1 的 `probe:scale` 数据。
- Week 3 不能早于 Week 2 的 ANN 和 schema 落地。
- Week 4 不能早于 Week 3 的 `query_story_os()` 主路径稳定。

如果某周 gate 未过：

- 不开始下一周。
- 先把本周 scope 缩到最小闭环。
- 不插入任何“顺手做”的高级能力。

---

## 8. 每周验收命令

Week 1：

- `npm run probe:scale`
- `cargo test -p agent-writer chapter_generation`
- `cargo run -p agent-evals`

Week 2：

- `cargo test -p agent-harness-core`
- `cargo run -p agent-evals`
- `npm run verify`

Week 3：

- `cargo test -p agent-writer`
- `cargo run -p agent-evals`
- `npm run verify`

Week 4：

- `cargo run -p agent-evals`
- `npm run verify`
- opt-in:
  - `cargo test -p agent-writer api_integration_tests::real_author_session_thirty_chapter_gate -- --nocapture`

---

## 9. 每周例会模板

只保留两场固定例会，所有周都按同一节奏走。

### 9.1 周中对齐会

时间：

- 每周中段，持续 20-30 分钟。

参与角色：

- 后端负责人
- 前端负责人
- 评测负责人

固定议程：

1. 本周 `周中必须对齐的结果` 是否已经冻结。
2. 当前协议 / schema / payload / 样例是否仍存在分歧。
3. 是否有任何变更会破坏 `本轮不变边界`。
4. 若未冻结，本周剩余时间内由谁在何时拍板。

会后必须产出：

- 一份“已冻结接口清单”
- 一份“仍未冻结项 + 截止时间”

### 9.2 周末 Gate 复盘会

时间：

- 每周末，持续 30-45 分钟。

参与角色：

- 后端负责人
- 前端负责人
- 评测负责人

固定议程：

1. 本周 `gate` 是否全部通过。
2. 本周 `artifacts` 是否全部齐备。
3. 若 gate 未过，最小闭环收缩方案是什么。
4. 是否允许进入下一周。

会后必须产出：

- 一份“Week N gate 结果摘要”
- 一个明确结论：`进入下一周 / 不进入下一周`

规则：

- 没有 artifacts，不算过周。
- gate 未过，不进入下一周。
- 例会不讨论新需求，只处理本周承诺项。

---

## 10. 风险

最高风险：

- 搜索性能解决了，但长度 contract 仍不稳定。
- Story OS 查询路径接通了，但 sprint 无法恢复。
- synthetic 指标很好，真实作者 gate 不成立。

对应控制：

- 任何一周都必须同时看性能指标和长度合规率。
- Sprint v2 必须复用现有 `receipt / settlement / provider budget` 边界，不另起一套状态机。
- 真实作者 gate 必须在第 4 周落地，不能再往后推。

---

## 11. 冲刺后再做

4 周版完成后，再考虑：

- Reader Compensation 深化
- Emotional Debt 更完整建模
- External Research 真工具接入
- 更强的产品化 UI

在此之前，这些都不是关键路径。

---

## 12. 最后的判断

这 4 周计划不是“把所有长篇能力做完”，而是只做最短关键路径：

- 有长度 contract
- 有 staged generation
- 有分层记忆
- 有可恢复 sprint
- 有 synthetic + real 两层 gate

如果这 5 件事成立，Forge 才算真正开始从“单章生成器”变成“长篇生产系统”。
