# Forge Writer Agent 4 周冲刺计划

Last updated: 2026-05-07

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

1. 输入治理编译层 ✅ (2026-05-07)
   - 已落地：`CompiledInput { intent_text, selected_evidence, rule_stack, trace_hint }` 编译工件
   - 已接入 `BuiltChapterContext`，持久化为 `compiled_input.json`
   - 目标：让“为什么这样写”在每章生成前可见、可复盘、可复用。 ✅

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

#### 3.3.7 实体状态内核封顶

在 `3.3` 封顶冲刺内部，还必须单独补上“实体状态内核”这条主线。否则 Forge 仍然只能停留在“章节级生产内核”，无法真正封顶成“实体级长篇状态系统”。

当前已知硬缺口：

- `Character` 不是一等业务实体，只是通用 `canon_entities`
- 没有 `CharacterState` 这种“章节区间有效”的版本化状态层
- 关系没有作为权威实体存在
- `Promise` 没有 `subject`
- `settlement` 仍然主要是章节级 typed，而不是实体级 typed
- `TodayFiveSummary` 虽然已经 schema 化，但还不是实体状态驱动的控制面

##### 3.3.7.1 本阶段的唯一目标

> 把 Forge 从“章节级 typed state 系统”推进到“实体级 typed state 系统”。

##### 3.3.7.2 必补的 4 组权威结构

1. `characters`
   - 唯一 `CharacterID`
   - `name`
   - `aliases`
   - `role_type`：主角 / 配角 / 功能角色
   - `current_state_summary`
   - `updated_at`

2. `character_state_versions`
   - `character_id`
   - `valid_from_chapter`
   - `valid_to_chapter`
   - `core_commitments_json`
   - `goal_state_json`
   - `identity_state_json`
   - `relationship_refs_json`
   - `source_ref`
   - `created_at`

3. `character_relationships`
   - 唯一 `RelationshipID`
   - `character_a_id`
   - `character_b_id`
   - `relation_type`：盟友 / 敌对 / 隐藏 / 复杂
   - `visibility`
   - `valid_from_chapter`
   - `valid_to_chapter`
   - `source_ref`

4. `plot_promises.subject`
   - `subject_ids_json`
   - `subject_type`
   - 后续预留：
     - `governing_state_version_id`
     - `governing_relationship_id`

##### 3.3.7.3 为什么这不是“可选增强”

如果不补这一层，Forge 将继续存在这些上限：

- 角色 OOC 无法被严肃建模，只能做文本碰撞
- 关系线无法作为长期债务管理
- Promise 无法稳定归属到角色 / 关系 / 物件主体
- `settlement` 只能章节级收敛，不能实体级 apply
- `TodayFiveSummary` 很难成为真正可靠的作者当天控制面

也就是说，长篇最关键的三类稳定性问题：

- 设定漂移
- 角色承诺漂移
- 关系演化漂移

都还没有真正的权威源。

##### 3.3.7.4 本阶段只做 5 件事

1. 实体主表落地
   - 新增 `characters`
   - 现有 `canon_entities` 退为投影/补充设定层，不再承担全部角色权威语义

2. 角色状态版本层落地
   - 新增 `character_state_versions`
   - 明确“哪个章节区间内，这个角色的核心承诺 / 目标 / 身份是什么”

3. 关系实体层落地
   - 新增 `character_relationships`
   - 明确关系何时成立、何时变更、何时失效

4. Promise 主体化
   - `plot_promises` 增加 `subject`
   - 让 promise 从“全局浮动事项”变成“归属某个角色/关系/状态的债务对象”

5. entity-scoped settlement / planning / debt
   - `ChapterSettlementDelta` 的 `promise_updates / book_state_updates / chapter_result`
     必须能映射到实体
   - `StoryDebtSnapshot` 与 `TodayFiveSummary` 要能消费实体级状态，而不只是章节级摘要

##### 3.3.7.5 本阶段明确不做

- 不先做复杂关系图 UI
- 不先做角色百科前端
- 不先做可视化时间轴
- 不先做“角色卡片大面板”

这些都属于表达层，不是封顶层。

##### 3.3.7.6 本阶段完成定义

只有同时满足下面条件，才算“实体状态内核封顶完成”：

- `Character` 拥有稳定业务 ID，而不是只靠名字
- 同一角色可拥有多个按章节区间生效的状态版本
- 同一对角色可拥有按章节区间生效的关系版本
- Promise 能显式绑定主体，而不是只保留字符串 `related_entities`
- `settlement apply` 能把章节结算映射到实体级状态变化
- `TodayFiveSummary` 至少有一半内容来自实体级状态，而不是章节级 helper 拼装
- `npm run verify` 全绿
- `cargo run -p agent-evals` 全绿
- 新增 4 个专项 gate：
  - `character_state_versioning_consistency`
  - `relationship_validity_window_consistency`
  - `promise_subject_binding_consistency`
  - `entity_scoped_settlement_apply_consistency`

##### 3.3.7.7 最后的判断

这不是“后续有空再做”的增强项，而是长篇内核封顶前必须补齐的一层。

如果 `3.3` 的目标是让 Forge 成为“稳定生产内核”，那么 `3.3.7` 的目标就是：

> 让 Forge 不再只懂章节，而是真的开始懂“谁、在什么时候、对谁、背着什么承诺、处在什么关系里”。 

##### 3.3.7.8 执行版：负责文件 / 迁移顺序 / 影响面

本阶段按以下顺序实施，不允许跳步：

1. schema 迁移
   - `src-tauri/src/writer_agent/memory/schema.in.rs`
   - `src-tauri/src/writer_agent/memory/tracing_migrate.in.rs`
   - 新增：
     - `characters`
     - `character_state_versions`
     - `character_relationships`
   - 扩展：
     - `plot_promises.subject_ids_json`
     - `plot_promises.subject_type`

2. memory methods
   - `src-tauri/src/writer_agent/memory.rs`
   - `src-tauri/src/writer_agent/memory/canon_methods.in.rs`
   - 新增：
     - `character_methods.in.rs`
     - `character_state_methods.in.rs`
     - `relationship_methods.in.rs`

3. typed ops
   - `src-tauri/src/writer_agent/operation.rs`
   - 新增：
     - `character.upsert`
     - `character_state.upsert`
     - `relationship.upsert`
     - `promise.bind_subject`

4. settlement / planning / debt 接入
   - `src-tauri/src/chapter_generation/settlement.in.rs`
   - `src-tauri/src/writer_agent/settlement_apply.rs`
   - `src-tauri/src/writer_agent/kernel/review.rs`
   - `src-tauri/src/writer_agent/promise_planner.rs`
   - `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

##### 3.3.7.9 现有功能需要补足和增强的点

- `canon_entities`
  - 退为投影，不再承担角色权威源
- `Promise Ledger`
  - 所有 open/stale/blocked/promoted/core 判断都要开始吃 subject，而不是只吃 title/description
- `StoryDebtSnapshot`
  - 需要能区分“这是主角债 / 配角债 / 关系债”
- `TodayFiveSummary`
  - 至少有 2 项必须直接来自角色或关系状态，而不是章节摘要

##### 3.3.7.10 新增 gate

- `character_state_versioning_consistency`
- `relationship_validity_window_consistency`
- `promise_subject_binding_consistency`
- `entity_scoped_settlement_apply_consistency`

#### 3.3.8 知识与身份状态封顶

在实体状态内核之后，必须立刻补“知识 / 秘密 / 身份层”。否则 Forge 仍然只能记录“发生了什么”，却不能稳定表达“谁知道什么、谁误以为什么、什么还不能揭露”。

##### 3.3.8.1 本阶段的唯一目标

> 让 Forge 开始区分“客观真相”、“角色已知信息”、“角色误判信息”、“对外可见身份”。

##### 3.3.8.2 必补的权威结构

1. `knowledge_items`
   - 唯一 `KnowledgeID`
   - `topic`
   - `truth_state`
   - `source_ref`
   - `created_at`

2. `knowledge_ownership`
   - `knowledge_id`
   - `holder_type`：角色 / 关系 / 阵营 / 公众
   - `holder_id`
   - `knowledge_mode`：真实知情 / 误判 / 怀疑 / 隐瞒
   - `valid_from_chapter`
   - `valid_to_chapter`

3. `identity_layers`
   - `character_id`
   - `public_identity`
   - `private_identity`
   - `revealed_to_json`
   - `valid_from_chapter`
   - `valid_to_chapter`

4. `reveal_events`
   - `subject_id`
   - `reveal_type`
   - `revealed_to`
   - `chapter`
   - `source_ref`

##### 3.3.8.3 本阶段只做 4 件事

1. 知识条目主表落地
2. 知识归属与误判状态落地
3. 身份层与 reveal 状态落地
4. settlement / diagnostics / planning 可消费知识与身份状态

##### 3.3.8.4 本阶段完成定义

- 同一秘密可以区分：
  - 客观真相
  - 某角色知情
  - 某角色误判
  - 公众未知
- 某角色的公开身份与真实身份可在不同章节区间共存
- reveal 事件可以改变知识归属，而不是只改 prose
- `bad payoff / canon drift / OOC` 的至少一部分检查开始基于知识层而不是纯文本
- 新增 3 个专项 gate：
  - `knowledge_visibility_consistency`
  - `identity_reveal_consistency`
  - `false_belief_preservation_consistency`

##### 3.3.8.5 明确不做

- 不先做知识图谱可视化
- 不先做“秘密管理面板”
- 不先做复杂阵营 UI

##### 3.3.8.6 执行版：负责文件 / 迁移顺序 / 影响面

1. schema 迁移
   - `src-tauri/src/writer_agent/memory/schema.in.rs`
   - `src-tauri/src/writer_agent/memory/tracing_migrate.in.rs`
   - 新增：
     - `knowledge_items`
     - `knowledge_ownership`
     - `identity_layers`
     - `reveal_events`

2. memory methods
   - 新增：
     - `knowledge_methods.in.rs`
     - `identity_methods.in.rs`
     - `reveal_methods.in.rs`

3. settlement / diagnostics / belief conflict 接入
   - `src-tauri/src/chapter_generation/settlement.in.rs`
   - `src-tauri/src/writer_agent/belief_conflict.rs`
   - `src-tauri/src/writer_agent/diagnostics.rs`
   - `src-tauri/src/writer_agent/kernel/chapters.rs`

4. planning / context / Companion 接入
   - `src-tauri/src/writer_agent/context/assembly.in.rs`
   - `src-tauri/src/writer_agent/kernel/prompts.rs`
   - `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`

##### 3.3.8.7 现有功能需要补足和增强的点

- `bad payoff`
  - 不能只看 payoff 早晚，还要看 reveal_to / knowledge_mode 是否成立
- `canon drift`
  - 不能只看事实矛盾，还要看公开身份 / 私下身份是否混线
- `OOC`
  - 需要开始吃“角色知道什么、误以为什么”
- `TodayFiveSummary`
  - 需要能明确告诉作者“当前哪条秘密还不能揭、谁已经知道、谁还误判”

##### 3.3.8.8 新增 gate

- `knowledge_visibility_consistency`
- `identity_reveal_consistency`
- `false_belief_preservation_consistency`

#### 3.3.9 场景编排内核封顶

Forge 当前已经有 staged generation，但还没有真正的一等场景对象层。没有 scene object，就很难稳定表达一章内“哪个场景承担哪个任务、哪个场景兑现哪个债、哪个场景推进哪段关系”。

##### 3.3.9.1 本阶段的唯一目标

> 让 Forge 开始按 Scene 组织章节，而不是只按整章文本组织章节。

##### 3.3.9.2 必补的权威结构

1. `scenes`
   - `scene_id`
   - `chapter_title`
   - `sequence`
   - `scene_type`
   - `summary`

2. `scene_state`
   - `scene_id`
   - `objective`
   - `participants_json`
   - `location_ref`
   - `time_slice_ref`
   - `entry_state_json`
   - `exit_state_json`

3. `scene_obligations`
   - `scene_id`
   - `promise_ids_json`
   - `mission_refs_json`
   - `payoff_targets_json`

4. `scene_results`
   - `scene_id`
   - `outcome`
   - `consequence`
   - `source_ref`

##### 3.3.9.3 本阶段只做 5 件事

1. scene object 主表落地
2. 章节内 scene sequence 落地
3. scene 与 mission / promise / payoff 的绑定落地
4. settlement 支持最小 scene 级结果投影
5. generation pipeline 开始消费 scene schema，而不是只保留 phase 名字

##### 3.3.9.4 本阶段完成定义

- 一章可以拆成多个 scene object
- 每个 scene 至少有：
  - objective
  - participants
  - entry / exit state
- 至少一部分 promise / mission / payoff 可以绑定到 scene
- `scene_plan` 不再只是事件名，而有可审查结构工件
- 新增 3 个专项 gate：
  - `scene_sequence_consistency`
  - `scene_obligation_binding_consistency`
  - `scene_result_projection_consistency`

##### 3.3.9.5 明确不做

- 不先做复杂分镜 UI
- 不先做 scene 拖拽板
- 不先做电影式 storyboard 前端

##### 3.3.9.6 执行版：负责文件 / 迁移顺序 / 影响面

1. scene schema
   - `src-tauri/src/writer_agent/memory/schema.in.rs`
   - 新增：
     - `scenes`
     - `scene_state`
     - `scene_obligations`
     - `scene_results`

2. generation pipeline 接入
   - `src-tauri/src/chapter_generation/types_and_utils.in.rs`
   - `src-tauri/src/chapter_generation/context.in.rs`
   - `src-tauri/src/chapter_generation/pipeline/main.in.rs`
   - `src-tauri/src/chapter_generation/runtime_artifacts.in.rs`

3. settlement / next beat / debt 接入
   - `src-tauri/src/chapter_generation/settlement.in.rs`
   - `src-tauri/src/writer_agent/kernel/chapters.rs`
   - `src-tauri/src/writer_agent/kernel/review.rs`
   - `src-tauri/src/writer_agent/kernel/snapshots.rs`

4. TodayFiveSummary 接入
   - `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`
   - 至少一项要能显示“当前章最关键 scene 目标”

##### 3.3.9.7 现有功能需要补足和增强的点

- staged generation
  - `scene_plan` 必须从 phase 名字变成结构化工件
- settlement
  - `chapter_result` 需要能投影最小 scene 结果
- debt / planning
  - promise、mission、payoff 不再只绑定 chapter，而要能绑定 scene
- Companion
  - `Next Move` 不能只给章级目标，要能给 scene 级目标

##### 3.3.9.8 新增 gate

- `scene_sequence_consistency`
- `scene_obligation_binding_consistency`
- `scene_result_projection_consistency`

#### 3.3.10 时间轴内核封顶

如果没有故事时间轴，Forge 最终只能稳定处理“线性按章节前进”的小说。长篇一旦出现倒叙、插叙、平行线、延迟揭露，就会迅速失去稳定性。

##### 3.3.10.1 本阶段的唯一目标

> 让 Forge 能区分“章节顺序”和“故事时间顺序”。

##### 3.3.10.2 必补的权威结构

1. `story_time_slices`
   - `time_slice_id`
   - `label`
   - `start_ref`
   - `end_ref`
   - `relative_order`

2. `chapter_time_mapping`
   - `chapter_title`
   - `scene_id`
   - `time_slice_id`
   - `narrative_mode`：当前时 / 倒叙 / 插叙 / 平行

3. `timeline_events`
   - `event_id`
   - `subject_ids_json`
   - `event_type`
   - `time_slice_id`
   - `source_ref`

##### 3.3.10.3 本阶段只做 4 件事

1. 故事时间片结构落地
2. scene / chapter 到故事时间的映射落地
3. 关键事件进入 timeline event 层
4. diagnostics / debt / save feedback 开始使用 story-time，而不是只看 chapter number

##### 3.3.10.4 本阶段完成定义

- 章节顺序与故事顺序可以分离
- 同一章可以映射到多个故事时间片
- `TimelineIssue` 诊断不再只靠章节号推断
- `relationship / identity / state version` 可以绑定故事时间片，而不是只绑定章节
- 新增 3 个专项 gate：
  - `story_time_mapping_consistency`
  - `flashback_identity_consistency`
  - `timeline_event_order_consistency`

##### 3.3.10.5 明确不做

- 不先做可视化时间轴 UI
- 不先做日历/世界年表前端
- 不先做复杂编辑器时间线插件

##### 3.3.10.6 执行版：负责文件 / 迁移顺序 / 影响面

1. timeline schema
   - `src-tauri/src/writer_agent/memory/schema.in.rs`
   - 新增：
     - `story_time_slices`
     - `chapter_time_mapping`
     - `timeline_events`

2. timeline helpers
   - `src-tauri/src/writer_agent/kernel/snapshots/helpers.in.rs`
   - `src-tauri/src/writer_agent/kernel/chapters.rs`
   - `src-tauri/src/writer_agent/diagnostics.rs`

3. settlement / state version 绑定
   - `src-tauri/src/chapter_generation/settlement.in.rs`
   - `src-tauri/src/writer_agent/settlement_apply.rs`
   - `character_state_versions`
   - `character_relationships`
   - `identity_layers`

4. Companion / Inspector 接入
   - `src-tauri/src/writer_agent/kernel/snapshots/today_five.in.rs`
   - `src-tauri/src/writer_agent/inspector.rs`

##### 3.3.10.7 现有功能需要补足和增强的点

- `TimelineIssue`
  - 不能再主要靠 `chapter_number_from_title`
- `repair-state`
  - 需要保住 story-time 映射，不只是 chapter chronology
- `reveal / identity / relationship`
  - 都要能挂到 story time slice，而不是只挂 chapter
- `TodayFiveSummary`
  - 需要能在倒叙/插叙时告诉作者“当前写的是哪段故事时间”

##### 3.3.10.8 新增 gate

- `story_time_mapping_consistency`
- `flashback_identity_consistency`
- `timeline_event_order_consistency`

#### 3.3.12 开发顺序约束

这四段不能并行乱做，必须按下面顺序推进：

1. `3.3.7` 人物 / 关系 / Promise.subject
2. `3.3.8` 知识 / 身份 / reveal
3. `3.3.9` 场景对象层
4. `3.3.10` 时间轴

原因很简单：

- 没有人物和关系，就没有稳定 subject
- 没有 subject，就没有稳定知识归属
- 没有知识和身份层，就无法正确做 scene obligation
- 没有 scene object，就无法稳定做时间轴映射

如果顺序打乱，后面每一层都会返工。 

#### 3.3.13 集中冲刺开发顺序表

把 `3.3.7` 到 `3.3.10` 压成一个集中冲刺表，默认按 4 个 Sprint 执行。

| Sprint | 主题 | 目标 | Day 1-2 | Day 3-4 | Day 5-6 | Day 7 | 交付 artifact | Gate |
|------|------|------|------|------|------|------|------|------|
| Sprint A | `3.3.7` 实体状态内核 | 让人物 / 关系 / Promise.subject 成为权威层 | schema 迁移：`characters` / `character_state_versions` / `character_relationships` / `plot_promises.subject_*` | memory methods + typed ops | settlement / debt / planning / TodayFive 接实体 subject | 补 eval 与 verify | schema 变更文档、entity apply trace、subject 绑定样例 | `character_state_versioning_consistency` / `relationship_validity_window_consistency` / `promise_subject_binding_consistency` / `entity_scoped_settlement_apply_consistency` |
| Sprint B | `3.3.8` 知识与身份 | 让系统区分真相 / 已知 / 误判 / 可见身份 | schema 迁移：`knowledge_items` / `knowledge_ownership` / `identity_layers` / `reveal_events` | memory methods + reveal apply | settlement / diagnostics / belief conflict / TodayFive 接知识状态 | 补 eval 与 verify | knowledge/reveal state trace、identity visibility 样例 | `knowledge_visibility_consistency` / `identity_reveal_consistency` / `false_belief_preservation_consistency` |
| Sprint C | `3.3.9` 场景编排内核 | 让章节按 Scene 而不是整章组织 | schema 迁移：`scenes` / `scene_state` / `scene_obligations` / `scene_results` | generation pipeline 接 scene schema | settlement / next beat / planning / TodayFive 接 scene | 补 eval 与 verify | `scene_plan` 结构工件、scene result 投影样例 | `scene_sequence_consistency` / `scene_obligation_binding_consistency` / `scene_result_projection_consistency` |
| Sprint D | `3.3.10` 时间轴内核 | 区分章节顺序与故事时间顺序 | schema 迁移：`story_time_slices` / `chapter_time_mapping` / `timeline_events` | state version / reveal / relationship 绑定时间片 | diagnostics / debt / save feedback / TodayFive 接 story-time | 补 eval 与 verify | story-time mapping trace、timeline consistency 样例 | `story_time_mapping_consistency` / `flashback_identity_consistency` / `timeline_event_order_consistency` |

##### 3.3.13.1 每个 Sprint 的共通规则

每个 Sprint 都必须按同一节奏执行：

1. Day 1-2：只做 schema / protocol / typed op，不碰大 UI
2. Day 3-4：只做 memory methods / apply path / settlement 接入
3. Day 5-6：只做 planning / debt / diagnostics / TodayFive 接入
4. Day 7：只做 tests / evals / verify / artifact 固化

##### 3.3.13.2 每个 Sprint 现有功能要补足的点

Sprint A 必须补：

- `canon_entities` 退为投影
- `Promise Ledger` 接 subject
- `StoryDebtSnapshot` 可区分角色债 / 关系债
- `TodayFiveSummary` 至少两项来自实体状态

Sprint B 必须补：

- `bad payoff` 基于知识可见性
- `canon drift` 基于身份层
- `OOC` 基于“角色知道什么 / 误判什么”
- `TodayFiveSummary` 能告诉作者“谁知道了什么、什么还不能揭”

Sprint C 必须补：

- `scene_plan` 从 phase 名变成结构工件
- settlement 增加最小 scene 结果投影
- promise / mission / payoff 支持 scene 绑定
- `Next Move` 默认能给出 scene 级目标

Sprint D 必须补：

- `TimelineIssue` 不再主要靠 chapter number
- `repair-state` 保住 story-time 映射
- reveal / identity / relationship 能绑定 story-time
- `TodayFiveSummary` 能明确当前写的是哪段故事时间

##### 3.3.13.3 集中冲刺结束定义

只有当 Sprint A-D 全部完成，并且以下条件同时成立，才算“底层封顶冲刺真正闭合”：

- 人物 / 关系 / 知识 / 场景 / 时间轴五层都有权威结构
- settlement extraction 可从章节级推进到实体级 / 场景级 / 时间轴级投影
- save / settlement / planning / debt / TodayFive 使用同一套真相源
- `npm run verify` 全绿
- `cargo run -p agent-evals` 全绿
- 不为补底层而引入新的 write-mode 调试噪声

##### 3.3.13.4 底层算法适配原则

这轮不是“为了新结构把整套 agent 推倒重写”，而是“在现有 typed 内核上补语义层，并保证热路径不被拖垮”。

唯一原则：

> 不用“禁止变化”控制长篇，而用“版本化状态 + 章节有效区间 + subject 绑定”控制长篇。

这意味着新增的 `Character / CharacterState / Relationship / Knowledge / Scene / StoryTime` 不应逼迫主流程全量重算，而应作为增量约束层接入现有：

- `TaskPacket`
- `context assembly`
- `ChapterSettlementDelta`
- `Promise Ledger`
- `StoryDebtSnapshot`
- `TodayFiveSummary`

##### 3.3.13.5 必须适配的算法面

1. 检索与上下文组装
   - 从“词命中 + 章节邻近”升级为“typed filter + 小集合 rerank”
   - 先按角色 / 关系 / 知识 / 场景 / 时间片做 cheap filter，再做文本重排
   - 禁止为了拿到新语义，每次生成扫描全书所有状态版本

2. settlement / apply
   - 从“章节级结果推断”升级为“本章触达对象的 typed delta apply”
   - 除 `chapter_result / promise_updates / book_state_updates` 外，逐步增加：
     - `character_state_deltas`
     - `relationship_deltas`
     - `knowledge_deltas`
     - `scene_deltas`
     - `story_time_deltas`
   - 只 apply 本章新增或被命中的对象，不做全量重建

3. promise planner / debt ranking
   - 从“title / description / chapter number”主导，升级为“subject / state pressure / relationship pressure / knowledge readiness / timeline due”主导
   - 旧的词命中和章节邻近先保留为 fallback，不允许一次性打掉现有稳定行为

4. diagnostics / belief conflict
   - 从“文本冲突解释”升级为“实体状态 / 知识可见性 / 身份层 / story-time”联合诊断
   - `OOC / canon drift / bad payoff / state conflict / reveal timing` 都必须开始吃 typed state

5. story impact / Companion
   - story impact 节点要补：
     - 角色状态节点
     - 关系节点
     - 知识节点
     - scene 节点
     - story-time 节点
   - Companion 只消费压缩后的控制信号，不直接暴露底层调试细节

##### 3.3.13.6 性能红线

新增语义层之后，以下实现方式禁止进入主写作热路径：

- 禁止每次生成前全量扫描全部章节状态历史
- 禁止每次保存后重建整本书的实体 / 关系 / 知识索引
- 禁止每次 settlement 重新刷新全量 embedding / VectorDB
- 禁止把复杂图推理、离线修复、全量回填塞进 save path

必须坚持的 4 条实现约束：

1. 写入只做本章 delta apply
2. 查询只读 active snapshot + recent window
3. 召回一律走“粗召回 -> typed filter -> 小集合 rerank”
4. backfill / repair / reindex 只走离线路径，不进入默认写作主链

##### 3.3.13.7 可开发的适配顺序

按落地顺序，所有算法适配只允许按下面顺序推进：

1. 先补 schema 与 typed ops
   - `characters`
   - `character_state_versions`
   - `character_relationships`
   - `knowledge_items`
   - `identity_layers`
   - `scenes`
   - `story_time_slices`
   - `plot_promises.subject_*`

2. 再补 settlement delta 与 authoritative apply
   - 保证新增结构先能被可靠写入、回放、修复

3. 再改 retrieval / planner / diagnostics 的评分因子
   - 让算法开始消费新语义，但不破坏旧 fallback

4. 最后再收 Companion / TodayFive / 默认行为
   - 只给作者当天真正要看的 5 件事，不把系统重新做成调试台

##### 3.3.13.8 本轮新增验收门

在原有 gate 之外，再补 4 个约束 gate，防止“语义增强成功、性能却退化”：

- `typed_context_filter_under_large_state_fixture`
- `entity_scoped_settlement_apply_without_global_rebuild`
- `planner_subject_scoring_fallback_consistency`
- `save_path_without_full_state_rescan`

#### 3.3.11 最后一句话

如果 `3.3` 是底层封顶冲刺，那么 `3.3.7` 到 `3.3.10` 就是底层封顶真正的四根梁：

> **2026-05-07 状态更新：四根梁全部完成，加上算法适配、性能 gate、用户感受层、前端表达层、Feedback Learning、Reader Compensation、Emotional Debt。**
> **当前基线：89 + 247 + 307 evals，0 audit issues。**



- 人物
- 知识与身份
- 场景
- 时间轴

补齐这四层之后，Forge 才能真正从“会写长章”升级成“理解整本长篇小说结构并持续稳定共创”的系统。

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

---

## 13. 正文质量不降前提下的延迟优化与底层强化计划

### 13.1 新约束

这一轮优化只允许解决“慢”和“不稳定”，不允许通过以下方式换速度：

- 不允许把高质量正文能力退回成短回答 / 段落续写器
- 不允许为了省时砍掉 `Story Contract / Chapter Mission / Result Feedback / Promise Ledger`
- 不允许把长篇连续性问题转移给作者手工兜底
- 不允许把 save 后的权威状态、receipt、audit、provider budget 旁路掉
- 不允许把“平均更快”建立在“repair 更多、返工更多、质量漂移更多”之上

一句话：

> 这一轮不是“让模型少写一点”，而是“让系统少重复做无效工作，让真正高价值的上下文更稳定地进入模型”。

### 13.2 当前延迟问题的真实结构

当前章节生成的慢，不是单个函数慢，而是 3 类成本叠加：

1. 章节上下文每次全量重建
   - outline / previous chapters / target existing text / lorebook / RAG / user profile 每次重新读取、裁剪、拼装
   - 当前 `build_chapter_context()` 仍然偏一次性全包构建，而不是 cache-aware 组装

2. draft 偏离目标区间后触发串行 repair
   - 当前主链路是：
     - `draft`
     - `continuation` 或 `compress`
     - 仍不合规时再 `hard_compress`
   - 这意味着一次字数偏差，直接增加一次完整 provider RTT

3. provider 抖动 + prompt/context 形状抖动叠加
   - 已有 probe 说明：
     - 有些章节主要是 provider jitter
     - 有些章节同时有 prompt/context instability

因此本轮优化目标不是“继续调一点温度”，而是把系统从：

- 全量组装
- 单次大 draft
- 偏差后串行补救

推进到：

- cache-aware 分层组装
- focus 驱动增量刷新
- 更稳定的首稿命中率
- 后处理尽可能后台化

### 13.3 这一轮只做 6 件事

#### 13.3.1 章节生成接入 Context Spine

目标：

- 让 chapter generation 不再每次从零拼一个“完整 prompt context”
- 把章节生成上下文也变成 `FrozenPrefix / ProjectStablePrefix / FocusPack / HotBuffer / EphemeralScratch`

要做的事：

- 复用现有 `ContextSpine`
- 把章节生成输入拆层：
  - `FrozenPrefix`
    - system chapter-generation contract
    - fixed output protocol
  - `ProjectStablePrefix`
    - Story Contract
    - Author Style
    - long-term Canon / Promise short summaries
  - `FocusPack`
    - current chapter mission
    - result feedback
    - next beat
    - story impact radius
    - selected Project Brain evidence
  - `HotBuffer`
    - current user instruction
    - target existing text
    - explicit override summary

完成定义：

- 章节生成能产出 cache-aware spine artifact
- 稳定前缀与动态尾部可分别统计 chars / estimated tokens
- 章节生成能输出 cache stability report，而不是只输出 context source list

#### 13.3.2 章节生成接入 FocusPack 增量刷新

目标：

- 连续写作时，不重复重建整包上下文
- 只在真正切换 focus node 时刷新高波动部分

要做的事：

- 给 chapter generation 增加最小 `FocusState`
- 在以下情况下只 rebuild `FocusPack + HotBuffer`：
  - chapter switch
  - scene switch
  - selected evidence materially changed
  - result feedback / next beat changed
  - story impact radius materially changed

明确不做：

- 不在本轮把所有 agent 路径都强行统一
- 不在本轮把 chapter generation 改造成复杂多 agent orchestration

完成定义：

- 同卷连续章节生成时，stable prefix 不重复构建
- focus rebuild 次数、prefix churn 次数有独立 telemetry
- 新增 gate：
  - `chapter_generation_focus_pack_rebuild_only`
  - `chapter_generation_stable_prefix_reuse`

#### 13.3.3 上一章全文改为“结构化结果优先，按风险升级全文”

目标：

- 不把上一章全文读取当作默认成本
- 仍然保住跨章连续性与收尾语义

要做的事：

- 默认上一章输入优先使用：
  - `ChapterResultSummary`
  - `NextBeat`
  - `Promise last_seen / expected_payoff`
  - `reader_takeaway`
  - `settlement delta`
- 只有以下情况升级为“读取上一章全文”：
  - continuity risk 高
  - unresolved debt 密度高
  - target chapter 对上一章 closing image 强依赖
  - previous structured result 证据不足

明确不做：

- 不取消上一章全文 fallback
- 不为了快而只保留 outline summary

完成定义：

- 默认 chapter generation 不再无条件读取上一章全文
- 全文读取有显式升级理由和 telemetry
- 新增 gate：
  - `chapter_generation_previous_fulltext_upgrade_only_on_risk`

#### 13.3.4 CompiledInput 进入章节生成主链

目标：

- 把“为什么写、用哪些证据、遵守哪些规则”压缩为稳定工件
- 降低 prompt prose 漂移带来的质量抖动

要做的事：

- `CompiledInput { intent_text, selected_evidence, rule_stack, trace_hint }`
  进入 `BuiltChapterContext`
- chapter draft / continuation / compress prompt 优先消费 compiled input
- `selected_evidence` 不再只是 artifact，而是 prompt 一级输入

明确不做：

- 不把 compiled input 变成独立 truth file 让作者维护
- 不让 compiled input 旁路原有 Story Contract / Chapter Mission

完成定义：

- chapter generation prompt 中能看到 compact compiled input
- context prose 与 compiled input 的重复度明显下降
- `compiled_input.json` 成为默认 runtime artifact
- 新增 gate：
  - `chapter_generation_compiled_input_enters_prompt`
  - `chapter_generation_selected_evidence_stability`

#### 13.3.5 Story Impact Radius 成为章节证据选择前置层

目标：

- 不再先堆很多上下文，再让模型自己筛
- 让“本章真正会被影响的对象”决定召回与组装

要做的事：

- chapter generation 在 lore / RAG / previous chapter 选择前，先计算 `StoryImpactRadius`
- 召回改为：
  - coarse recall
  - story-impact scoped typed filter
  - small-set rerank
- 让 promise / relationship / knowledge / scene / story-time 相关节点开始进入 impact radius

完成定义：

- 章节上下文中的外部证据至少一部分由 story impact 驱动选出
- `selected_evidence` 中可区分：
  - focus-derived
  - impact-derived
  - fallback-derived
- 新增 gate：
  - `chapter_generation_story_impact_scoped_recall`
  - `chapter_generation_impact_budget_without_noise_expansion`

#### 13.3.6 把 preflight 升级成 generation strategy selector

目标：

- 不再让所有章节都走同一条慢路径
- 在不牺牲质量的前提下，让系统自动选“该重还是该轻”

要做的事：

- 在现有 preflight 基础上增加策略选择：
  - `interactive_fast_draft`
  - `interactive_safe_draft`
  - `background_long_chapter`
  - `repair_heavy_mode`
- 策略选择只依赖已有信号：
  - `story_contract_quality`
  - `story_impact_truncated`
  - `context_total_chars`
  - `provider_budget_decision`
  - `recent repair telemetry`
  - `focus pack churn`

明确不做：

- 不让策略选择隐藏真实风险
- 不让 interactive 模式偷偷跳过质量 gate

完成定义：

- preflight 报告新增 `generation_strategy`
- 不同策略映射到不同 chapter profile / context budget / fallback policy
- 新增 gate：
  - `chapter_generation_strategy_selection_consistency`

### 13.4 实施顺序

这 6 件事不能乱做，必须按下面顺序推进：

1. `CompiledInput` 主链接入
2. chapter generation 接入 `ContextSpine`
3. `FocusPack` 增量刷新
4. 上一章全文升级策略
5. `StoryImpactRadius` 前置证据选择
6. `preflight -> generation strategy selector`

原因：

- 没有 compiled input，就只是换一套 context 拼接方式，收益有限
- 没有 spine，就没有稳定前缀复用
- 没有 focus rebuild，就没有连续写作收益
- 没有 risk-upgrade 机制，就无法安全减少全文读取
- 没有 story impact 前置层，就会继续用大上下文堆质量
- 没有 strategy selector，就无法把不同章节分流到合适 profile

### 13.5 性能与质量共同红线

本轮必须同时满足两类红线：

性能红线：

- 默认 interactive chapter generation 平均延迟继续下降
- repair rate 不能上升
- context assembly 不因新结构显著变慢
- save path 不引入新的同步重活

质量红线：

- anchor hit / anchor carry 不下降
- continuation / compress 触发率下降或持平
- real-author smoke 不能因为压缩上下文而出现明显 continuity regression
- `Story Contract / Chapter Mission / Result Feedback / Promise Ledger` 命中率不能退化

### 13.6 必补 telemetry

这轮不允许“凭感觉调快”，必须补 telemetry：

- `draft_ttft_ms`
- `draft_total_ms`
- `continuation_count`
- `compress_count`
- `hard_compress_count`
- `repair_total_latency_ms`
- `stable_prefix_chars`
- `dynamic_tail_chars`
- `focus_pack_rebuild_count`
- `previous_fulltext_upgrade_count`
- `generation_strategy`

同时按章节类型分桶：

- dialogue-heavy
- action-heavy
- reveal-heavy
- transition / bridge

### 13.7 新增 gate

在原有 gate 之外，本轮新增以下约束 gate：

- `chapter_generation_compiled_input_enters_prompt`
- `chapter_generation_stable_prefix_reuse`
- `chapter_generation_focus_pack_rebuild_only`
- `chapter_generation_previous_fulltext_upgrade_only_on_risk`
- `chapter_generation_story_impact_scoped_recall`
- `chapter_generation_strategy_selection_consistency`
- `chapter_generation_repair_rate_does_not_regress`
- `chapter_generation_anchor_carry_does_not_regress`

### 13.8 本轮明确不做

- 不直接改成 streaming-first 生成架构
- 不在本轮引入复杂多 agent 章节编排
- 不把 settlement / diagnostics 全部改成异步 eventual-consistency
- 不把所有慢问题都归因于 provider，再停在“换模型试试”
- 不通过砍掉上下文、砍掉 contract、砍掉 quality gate 来换速度

### 13.9 最后一句话

这一轮优化的目标不是：

> 让 Forge 更像一个快一点的生成按钮。

而是：

> 让 Forge 在继续守住长篇正文质量的前提下，变成一个更会复用上下文、更少返工、更少串行补救、更稳定命中高质量初稿的长篇写作内核。
