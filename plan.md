# Forge Cursor-Style Writing Agent 完整开发计划

Last updated: 2026-05-05

## 0. 北极星

Forge 的产品不是“带 AI 功能的写作工具”，而是“Cursor 式小说写作 agent”：

- 编辑器只是工作台。
- Agent 才是产品主体。
- Agent 必须像作家的第二大脑、第二作者、写作伴侣一样，持续理解这本书、守住承诺、发现风险、提出可审查行动，并从作者反馈中学习。

一句话目标：

> 作者不是在和 AI 聊天，而是在和一个懂这本书、能一起想、一起写、一起复盘的创作伙伴并肩工作。

## 1. 当前事实基线

### 已经成立的地基

- `agent-harness-core` 已具备 provider abstraction、tool registry、tool executor、permissions、compaction、TaskPacket、trace 等通用 agent runtime 能力。
- `src-tauri/src/writer_agent` 已具备 Writer Agent Kernel，包括 observation、intent、context pack、diagnostics、typed operations、proposal queue、feedback、memory、trajectory export。
- Story Contract、Chapter Mission、Result Feedback Loop、Promise Ledger、Companion Panel quiet mode 已经成为活跃产品地基。
- `WriterOperation` 已覆盖正文、canon、promise、style、story contract、chapter mission、outline 等变更。
- 写入类操作已有 approval context 和 audit 记录。
- 关键保存风险已处理：dirty state、chapter switching、autosave、inline operation、accepted feedback、batch generation dirty protection。
- `ask_agent` 已不再在 command 层直接创建旧 `AgentLoop`，现在通过 Writer Agent Kernel 的 `prepare_task_run` / `run_task` 执行。
- Operation lifecycle 已进入 trace：proposed、approved、applied、durably_saved、feedback_recorded。
- Command boundary audit 已覆盖 57 个 Tauri commands，并进入 `npm run verify`。
- Tauri command handlers 已全部移入 `src-tauri/src/commands/*`；`src-tauri/src/lib.rs` 当前不再包含 `#[tauri::command]`。
- AppState、启动期 Hermes/Writer memory DB 打开、legacy DB migration、kernel seed 逻辑已抽入 `src-tauri/src/app_state.rs`。
- Semantic lint payload/event 和设定/诊断 lint 逻辑已抽入 `src-tauri/src/semantic_lint.rs`。
- Manual request context injection、用户画像读取、章节 embedding、近期技能抽取和 LLM memory candidate 生成已抽入 `src-tauri/src/memory_context.rs`。
- Agent/editor/manual observation payload 和 WriterObservation 转换逻辑已抽入 `src-tauri/src/observation_bridge.rs`。
- Editor realtime ghost rendering、ambient output 转发、editor prediction 清理、realtime cowrite 开关和 LLM ghost proposal flow 已抽入 `src-tauri/src/editor_realtime.rs`。
- API key 读取、路径 helper、事件常量、事件 payload、Agent status payload、项目写入审计、章节保存观察/canon refresh/context render helper 已分别抽入 `api_key.rs`、`app_paths.rs`、`events.rs`、`event_payloads.rs`、`agent_status.rs`、`project_audit.rs`、`writer_observer.rs`。
- 原 `lib.rs` 内联测试已抽入 `src-tauri/src/tests.rs`；`lib.rs` 已降为主要保留模块 wiring、Tauri setup 和 command registration 的 root glue，并由 `scripts/check-architecture-size.cjs` 持续约束体量预算。
- trajectory JSONL 已导出 `writer.product_metrics`，包含采纳率、忽略率、promise recall、canon false-positive、mission completion、durable save 和 save-to-feedback latency。
- 当前本轮验证基线由 `scripts/verification-baseline.cjs` 维护，`agent-harness-core` 为 88 tests passing，`agent-writer` 为 223 tests passing，完整 `agent-evals` 为 245/245 passing；`check:baseline` 现在除同步 README / project-status 外，还会解析真实 `cargo test -- --list`、`cargo run -p agent-evals`、command audit 和 architecture guard 输出，避免只验证文档等于脚本。新增真实 provider smoke tests 已接入 `src-tauri/src/api_integration_tests.rs`，但必须显式设置 `FORGE_REAL_API_TESTS=1` 且提供真实 `OPENAI_API_KEY` 才会执行，普通验证保持离线；`llm_runtime` 已把 general chat / JSON / 章节草稿 / ghost preview / 分析 / 平行草稿 / 手动改写 / 工具续写 / Project Brain stream 的 temperature 与 max_tokens 拆成受限环境参数，支持真实环境按功能低成本调参。Story Contract 质量门槛已进入 generation/rewrite/diagnosis task packet：低质量合同会在可回放 TaskPacket trace 中产生 `story_contract_quality_gate` belief 和 `StoryContractQuality` required context。元认知硬门禁 eval 覆盖写作 run blocking、正文写入 operation blocking、恢复性 mission calibration operation 放行、以及专用恢复 run 保持只读任务边界；`WriterMetacognitiveSnapshot` 会从 context pressure、failure bundle、post-write diagnostics、低置信 proposal、重复忽略率和 durable-save 健康度聚合风险等级与建议动作，并在 Inspector / trajectory 展示 `writer.metacognition`。Inspector metacognition 卡片已补恢复 CTA，可跳转 Review、诊断/保存、失败、上下文和 meta 视图，也可通过 `run_metacognitive_recovery` 触发只读 Planning Review / Continuity Diagnostic 恢复 run。
- Writer Agent context pack 的 Canon / Promise slice 已引入写作相关性排序，并输出 `WHY writing_relevance` 解释，避免只按文本相似或固定 ledger 顺序取材。
- P4 后端第一阶段已继续推进：WriterRunEventStore 可持久化回放，Planning / Review 只读模式有专用任务包/上下文/工具边界，章节生成已有 WriterTaskReceipt 和 failure evidence bundle，ContinuityDiagnostic 已有只读 receipt、diagnostic_report task artifact、trajectory 回放和 Inspector receipt/artifact 筛选；记忆候选反馈已有 correction / reinforcement 信号且纠错优先于强化，可审查记忆候选已记录 `writer.memory_candidate_created` run event 且明确不会直接写 ledger，WriterOperation 审批成功/拒绝已记录 `writer.approval_decided` run event，真实写作工作流的上下文组装已记录 `writer.context_pack_built` run event 且只存预算/来源摘要、不写入正文原文，章节生成 / Project Brain / manual request 在预算门禁通过、真实 provider call 启动前已记录 `writer.model_started` run event，manual AgentLoop 工具调用 start/end 已记录 `writer.tool_called` run event 且只存工具名、phase、参数 key、大小、耗时、成功/失败和 remediation code，Chapter Mission 状态机已支持 draft/active/completed/drifted/blocked/needs_review/retired 且保存结果迁移保留 Result Feedback 证据，Project Brain 已有 knowledge index / shared-keyword graph、chunk source/version metadata、source-history aggregation、active/archived revision 标记、read-only source revision compare、Graph 页 source history/compare 展示和 source revision 恢复第一阶段；该恢复只切换同一 `source_ref` 的 active/archived chunk，不回写章节正文或 Story Bible。Project Brain 也新增只读 cross-reference command 和显式作者批准的 external research 手动导入 command；导入路径被 command audit 分类为 Project Brain 写入，并要求 `author_approved` 与批准理由。Project Brain embedding 已有本地 provider registry / profile、模型维度、input limit、batch status、retry policy 和兼容回退状态的第一阶段边界，Research / Diagnostic 子任务已有隔离 artifact workspace、tool policy 和 evidence-only 结果边界，Research 子任务 start/completed 已能记录为 `writer.subtask_started` / `writer.subtask_completed` run event 并进入 Inspector subtask timeline，Research 子任务工具失败会生成带 subtask 证据的 failure bundle；Inspector timeline 有后端视图且 trajectory export 已带 redaction warning / local-only 标记，并可额外导出 Claude-Code-style / HF Agent Trace Viewer 兼容 JSONL；Provider budget 已能对超预算 provider call 输出 approval-required 决策和 remediation，章节草稿生成会在真实 provider call 前执行 budget preflight，Project Brain chat answer 会在 `stream_chat` 前执行 `project_brain_query` budget preflight，manual request 会在 AgentLoop 每一轮 provider call 前执行 `manual_request` provider budget guard，元认知恢复 run 会使用专用 `metacognitive_recovery` provider budget guard，external research subtask 已有 provider budget report / failure bundle helper，超预算会记录 `writer.provider_budget` 和 `writer.error`；Project Brain / manual request 已接入 Explore 审批卡和批准凭证重试，且 budget report 会进入 `writer.provider_budget` run event / trajectory；章节保存观察路径和 accepted inline/proposal durable-save 路径已记录 post-write diagnostic report，accepted operation 后写诊断已会把诊断结果注册为可审查 proposal / story debt，不自动改写正文；通用 ToolExecution 失败结果已带结构化 remediation，并已映射进 WriterFailureEvidenceBundle 与 Inspector failure event；Inspect failure 视图已有基于失败证据的恢复排查跳转入口；元认知第一阶段已把 trace-derived risk/action 接入 Inspector 和 trajectory，并已成为写作 run-loop / 正文写入 operation 的第一段硬门禁，同时保留 Planning Review、Continuity Diagnostic 和 mission calibration 等恢复路径；Inspector 侧已补 metacognitive block 恢复 CTA 和专用恢复 run。

### 当前剩余核心矛盾

- 前端仍保留聊天式 `AgentPanel`，容易把产品拉回“AI 聊天助手”心智。
- Story Contract 已有 quality/quality_gaps 字段并在 CompanionPanel 可视化；低质量 Story Contract 不进入 ProjectBrief context pack，但会进入核心 TaskPacket 风险信号，覆盖 ambient ghost 和 prepared run 两条路径；Chapter Mission 状态已在 OutlinePanel/EditorPanel 展示。前端强门禁审批卡仍未接入 generation/diagnosis/save 流程。
- `agent-evals/src/evals/product_scenarios.rs` 已集中承载 10 个长篇产品场景 eval、1 个合成 20 章连续写作 fixture，以及 1 个作者式 5 章长会话校准 fixture；这些 fixture 已把多章保存、伏笔、任务漂移、作者反馈和产品指标串成同一条可验证链路。下一步仍要继续引入真实作者项目数据对照，而不是只堆数量或合成场景。
- `src-tauri/src/lib.rs` command 层拆分、AppState 拆分、semantic lint 拆分、memory/context helper 拆分、observation bridge 拆分、editor realtime 拆分、root helper 拆分和测试拆分已完成；剩余主要是最终 app setup / command registration glue。当前 57 个 Tauri commands 都在 command audit 显式分类，新增读者补偿 review chain 命令已登记为 read-only。`writer_agent/kernel.rs` 的 P2 拆分已完成：TaskPacket/context trace、product metrics、product metrics trend、proposal lifecycle、ghost helper、memory feedback、memory candidate、run-loop、Context Spine run-event helper、feedback、operation execution、snapshot、trace recording 和测试都已进入 `src-tauri/src/writer_agent/kernel/` 职责模块。`agent-evals/src/evals.rs` 也已拆成职责单一的 eval 子模块。架构体量不再依赖手工维护的精确行数描述，改由 `npm run check:architecture` 检查 `lib.rs`、kernel facade、关键 kernel 子模块、eval facade、CompanionPanel 拆分预算、kernel facade 显式导出、trace recording no-`include!` 和 eval root isolation。

## 2. 总体原则

1. 先统一大脑，再加功能。
2. 先保证写作状态不会被污染，再追求自动化。
3. 所有 agent 行动必须 typed、reviewable、audited、feedback-fed。
4. 作家不需要看 agent 的全部运行过程，只需要看到与写作决策有关的少量高价值信号。
5. 评测不能只证明“代码没坏”，必须逐步证明“作者长期写作更稳、更快、更少返工”。

## 3. 目标架构

```text
Frontend
  EditorPanel
  CompanionPanel
  OutlinePanel
  Foundation surfaces
  Minimal manual request launcher

Tauri Commands
  Thin command bridge only
  No business-heavy orchestration in lib.rs

Writer Agent Kernel
  observe
  classify intent
  build task packet
  assemble context pack
  enforce story contract / mission / promise / canon / style policy
  run model/tool loop
  emit proposals
  approve typed operations
  execute durable writes
  record feedback and memory
  export trajectory

agent-harness-core
  provider
  tool registry
  tool executor
  permission
  compaction
  trace
  task packet primitives
```

核心闭环：

```text
Observation
  -> Intent
  -> TaskPacket
  -> ContextPack
  -> ToolPolicy
  -> Model/Tool Run
  -> AgentProposal
  -> WriterOperation
  -> Approval
  -> Durable Save
  -> Result Feedback
  -> Memory/Ledger Update
  -> Next Context
```

## 4. P0：统一 Writer Agent Kernel 大脑

当前状态：已完成。保留本节作为验收清单和防回归边界。

### P0.1 退役 `ask_agent` 旧执行层

目标：所有手动请求不再由 `src-tauri/src/lib.rs` 直接创建旧 `AgentLoop`，而是进入 Writer Agent Kernel 的统一 run loop。

任务：

- 新增 `WriterAgentRunRequest`：
  - `task`: `ManualRequest | InlineRewrite | GhostWriting | ChapterGeneration | ContinuityDiagnostic | CanonMaintenance | ProposalEvaluation`
  - `observation`
  - `user_instruction`
  - `frontend_state`
  - `approval_mode`
  - `stream_mode`
- 新增 `WriterAgentRunResult`：
  - `answer`
  - `proposals`
  - `operations`
  - `task_packet`
  - `context_pack_summary`
  - `tool_inventory`
  - `trace_refs`
- 在 `writer_agent/kernel.rs` 或新模块中实现：
  - `run_task(request) -> WriterAgentRunResult`
  - `run_manual_request`
  - `run_inline_rewrite`
  - `run_chapter_generation`
  - `run_continuity_diagnostic`
- 将 provider/tool loop 封装到 kernel 内部，`lib.rs` 只负责命令参数转换和事件转发。
- 删除或隔离 `ask_agent` 中的直接 `AgentLoop::new`。

验收标准：

- `rg "AgentLoop::new" src-tauri/src/lib.rs` 无结果。
- `ask_agent` 只调用 Writer Agent Kernel，不直接拼 system prompt、不直接组装 registry、不直接 run loop。
- 新增 eval：`writer_agent:manual_request_kernel_owns_run_loop`。
- 新增 eval：ManualRequest 仍不能暴露 approval-required write tools。
- `npm run verify` 通过。

### P0.2 统一 action lifecycle

目标：所有 agent 影响项目状态的动作都走同一生命周期。

标准生命周期：

```text
proposed -> approved -> applied -> durably_saved -> feedback_recorded
```

任务：

- 为 `WriterOperation` 增加或统一 action state tracking。
- 对正文操作、foundation 操作、promise 操作、canon 操作、outline 操作统一记录：
  - proposal id
  - operation kind
  - source task
  - approval source
  - affected chapter / entity / promise
  - save result
  - feedback result
- 所有写入必须在 durable save 成功后再写 accepted feedback。
- 保留 rejected / ignored feedback，防止重复打扰。

验收标准：

- 任意 accepted text operation 若保存失败，不会进入 writer memory 的 accepted preference。
- 任意 write-capable operation 缺少 approval context 时被拒绝。
- 新增 eval：`writer_agent:operation_feedback_requires_durable_save`。
- 新增 eval：`writer_agent:write_operation_lifecycle_trace`。

### P0.3 命令边界审计

目标：Tauri command 不再成为绕过 agent policy 的暗门。

任务：

- 建立 command inventory：
  - read-only
  - provider call
  - memory write
  - manuscript write
  - destructive write
- 每个 command 标注：
  - 是否需要 WriterOperation
  - 是否需要 approval
  - 是否需要 conflict check
  - 是否需要 audit
- 对 legacy direct save commands 继续压缩范围。
- 优先将可迁移写入改为 typed operations。

验收标准：

- 新增静态检查脚本：列出所有 `#[tauri::command]` 和风险等级。
- `save_chapter`、`save_outline_node`、`restore_backup`、`rename_chapter` 等写路径都有明确 audit 或 operation route。
- 文档更新 command policy matrix。

## 5. P1：把“信任合同”变成写作飞控律

### P1.1 Story Contract 强化

目标：Story Contract 不只是设置页内容，而是所有 agent 行动的书级约束。

当前状态：Story Contract 质量评估已落地。`StoryContractSummary` 新增 `quality`（missing/vague/usable/strong）和 `quality_gaps` 字段，`fill_quality()` 在构造/反序列化时自动计算。CompanionPanel 已在 Story Contract 行展示 `contractQuality` 和具体缺口列表，前端 protocol.ts 同步新增 `quality` / `qualityGaps`。kernel_ops.rs 的 `StoryContractUpsert` 操作路径也已填充质量字段。低质量 Story Contract 仍会被排除出 ProjectBrief context pack，但 generation / rewrite / diagnosis 相关 TaskPacket 会追加 `story_contract_quality_gate` belief 和必需的 `StoryContractQuality` required context，避免 trace 里看不见基础合同风险。

剩余：把低质量合同风险进一步接成前端审批/确认卡，并按真实作者项目校准是否需要阻断而不只是警告。

任务：

- 为 Story Contract 增加字段质量等级：
  - missing
  - vague
  - usable
  - strong
- 保存时给出具体缺口，不只报 invalid。
- 所有 generation / rewrite / diagnosis task packet 必须引用 Story Contract 状态。
- 当 Story Contract 缺失或质量弱时：
  - ghost writing 可以继续，但降低自信度。
  - chapter generation 必须提示风险。
  - large rewrite 必须要求确认。

验收标准：

- 新增 eval：低质量 Story Contract 不会污染 context pack。
- 新增 eval：chapter generation 会显式记录 Story Contract source。
- Companion Panel 显示 Story Contract 强度和最关键缺口。（已完成）

### P1.2 Chapter Mission 工作流升级

目标：每一章都有当前任务，agent 不能只看光标附近文本。

当前状态：前端 Chapter Mission UI 已落地两层：OutlinePanel 每个节点旁展示 mission 状态 badge（draft/active/completed/drifted/blocked/needs_review/retired，带颜色编码），EditorPanel 顶部新增 mission 状态栏（当前章 mission 摘要 + 状态圆点 + must_not 约束）。后台 `get_writer_agent_ledger` 已同时供给 OutlinePanel 和 EditorPanel 使用。

剩余：从大纲节点直接创建/编辑 Chapter Mission，mission 状态在保存时后自动结算与建议 UI。

任务：

- 在 OutlinePanel 中显示每章 mission 状态。（已完成）
- 支持从大纲节点直接创建 / 编辑 Chapter Mission。
- 保存章节后自动结算：
  - completed
  - drifted
  - needs_review
  - active
  - blocked / retired 由作者显式设置，不会被保存观察自动覆盖
- 生成或重写时必须检查：
  - must include
  - must not reveal
  - expected ending
  - relation advancement
  - promise payoff / deferral

验收标准：

- 新增 eval：违反 `must_not` 会生成 story debt。
- 新增 eval：完成 expected ending 会标记 mission completed。
- 新增 UI check：当前章 mission 在写作视图中始终可见但不喧宾夺主。（已完成：EditorPanel 顶部 mission 状态栏含状态圆点 + mission 摘要 + must_not）

### P1.3 Promise Ledger 变成主控账本

目标：长篇稳定性的核心不是普通 RAG，而是“承诺与兑现”。

任务：

- Promise Ledger 类型完善：
  - plot promise
  - emotional debt
  - object whereabouts
  - character commitment
  - mystery clue
  - relationship tension
- 每个 promise 记录：
  - introduced chapter
  - last seen chapter
  - expected payoff
  - current status
  - risk level
  - related entities
- 保存章节后自动抽取新增 / 兑现 / 延期 / 放弃候选。
- Companion Panel 只显示当前最重要 1-3 个 promise，不做账本噪音倾倒。

验收标准：

- 新增 eval：物件去向能跨章节进入 context pack。
- 新增 eval：过期 promise 会在 payoff 章节附近提示。
- 新增 eval：已 resolved promise 不重复打扰。

### P1.4 Result Feedback Loop 加固

目标：每章保存后自动回灌下一章上下文。

任务：

- 保存后生成结构化结果：
  - chapter summary
  - character state changes
  - promise updates
  - canon updates
  - conflict changes
  - next beat
  - open risks
- 对保存结果生成 memory candidates，但不自动污染 canon / promise，除非有明确规则或用户批准。
- 在下一章 context pack 中优先注入上章 result feedback 和 next beat。

验收标准：

- 新增 eval：Result Feedback 在 tight budget 下仍保留。
- 新增 eval：next beat 被下一章 generation 使用。
- 新增 eval：保存后的 memory candidate 未批准时不会写入永久 canon。

## 6. P1：Companion Panel 降噪和产品心智修正

目标：作家不需要知道 agent 在干嘛；作家只需要知道 agent 正在守什么、哪里危险、下一步怎么写。

任务：

- 默认 Companion Panel 只显示 3-5 件事：
  - 当前章节目标
  - 最重要未兑现 promise
  - 最高风险 canon / continuity issue
  - 当前角色状态或情绪弧
  - 下一步建议
- Trajectory / tool inventory / task packet 进入 debug 或 inspector，不作为默认 UI。
- `AgentPanel` 从主聊天区降级为 Explore / Ask / Debug：
  - 不默认展示。
  - 不能替代 Companion Panel。
  - 手动 ask 结果默认转为 proposal / operation / note，而不是聊天流。
- 所有 agent 建议必须可忽略、可接受、可拒绝、可解释。

验收标准：

- 默认写作界面没有聊天框主导视觉。
- Agent 建议不超过固定数量，超出进入队列。
- 新增 P2 UI check：`npm run check:p2` 通过 AST 约束验证 Companion 默认区域不会展示 raw trace / raw chain / raw tool log；`npm run check:p2-render` 用 React server render 注入内部 trace fixture，验证 write mode 实际 DOM 不泄漏 task packet、operation lifecycle、run event 或 Inspector-only 文案。（已完成）

## 7. P1：作者价值评测

目标：从“代码能过”升级到“真实写作流程有价值”。

### P1.1 场景级 eval

新增 eval fixtures：

- 连续 5 章写作：
  - chapter 1 引入 promise
  - chapter 2 改变角色状态
  - chapter 3 插入误导线索
  - chapter 4 临近 payoff
  - chapter 5 检查兑现或延期
- 角色设定冲突：
  - lorebook 写明武器 / 关系 / 禁忌
  - 正文写反
  - agent 必须提示但不能强行改
- 风格连续性：
  - 作者接受某种句式偏好
  - 后续 ghost writing 应降低违背偏好的候选
- 章节任务漂移：
  - mission 要求推进关系
  - 正文完全变成风景铺陈
  - agent 必须提示 mission drift

验收标准：

- 新增 `agent-evals` 场景不少于 10 个。（已完成：`agent-evals/src/evals/product_scenarios.rs` 当前集中 10 个长篇产品场景）
- 每个 eval 都输出：
  - expected behavior
  - actual behavior
  - evidence source
  - pass/fail reason

### P1.2 产品指标

本地记录但不上传：

- proposal acceptance rate
- rejection reason distribution
- ignored repeated suggestion rate
- save-to-feedback latency
- promise recall hit rate
- canon false-positive rate
- chapter mission completion rate
- manual ask converted-to-operation rate
- context pressure coverage / truncated / dropped trend

当前状态：工程第一版已完成，但产品验证不能按真实作者项目完成态理解。上述指标已从 Writer Agent trace 派生，并随 trajectory JSONL 以 `writer.product_metrics` 事件导出；Companion 写作模式会摘要采纳率和保存健康度，Inspect 模式已展示 manual ask 转可执行 operation 率。多 session 第一阶段已完成：`WriterAgentTraceSnapshot.productMetricsTrend` 会从持久化 `writer_run_events` 按 session 聚合 proposal / feedback / operation lifecycle、manual ask 转 operation 率、最近 session 的 save-to-feedback 平均值、上一 session 对照、总体平均值和 delta；同时从持久化 `writer.context_pack_built` run events 聚合 context pack count、requested/provided chars、coverage、truncated/dropped source counts 和 recent-vs-previous coverage delta。Inspect 模式展示这些趋势，trajectory JSONL 额外导出 `writer.product_metrics_trend`。连续写作验证第一阶段已完成：`writer_agent:continuous_writing_fixture_20_chapters` 用合成 20 章长篇项目覆盖保存观察、任务漂移、伏笔召回、作者反馈和指标趋势。剩余工作是用真实作者项目数据证明这些指标与作者价值相关，并校准阈值。

验收标准：

- 本地 trajectory export 可包含匿名化指标摘要。（已完成）
- Companion / debug view 能查看最近写作 session 的 agent 有用程度。（已完成第一阶段：Companion 显示当前摘要，Inspect 显示多 session 趋势）
- manual ask converted-to-operation rate 进入 Inspect Run Health 和 per-session trend，并由 `writer_agent:product_metrics_manual_ask_conversion` / `writer_agent:product_metrics_manual_ask_conversion_trend` 覆盖。（已完成）
- context pressure coverage / truncated / dropped 进入 Inspect Session Trend 和 trajectory export，并由 `writer_agent:product_metrics_context_pressure_trend` 覆盖。（已完成）
- 连续 10-20 章 fixture 能把保存、反馈、伏笔、任务漂移、story debt 和 product metrics 串成同一条可回放证据链。（已完成第一阶段：合成 20 章 fixture）

## 8. P2：上下文、记忆、检索继续补强

### P2.1 Context Pack 质量升级

Status：核心已完成，Inspect mode 已有当前 trace 和跨 session context pressure 趋势视图；后续重点转向真实长 session 阈值校准。

Done：

- 已为 GhostWriting、InlineRewrite、ManualRequest、ChapterGeneration、ContinuityDiagnostic、ProposalEvaluation 定义 context budget profiles。
- Story Contract、current Chapter Mission、latest Result Feedback、relevant Promise Ledger slice、canon slice、cursor prefix/suffix 已作为核心 context sources 参与 pack。
- Context budget trace、source summary、截断信息和 selected source explanation 已进入 Writer Agent trace / eval 路径。
- `WriterAgentTraceSnapshot.context_source_trends` 已按最近 proposal 的 context budget reports 聚合 source appearances、provided/truncated/dropped 次数、总请求/提供字符、平均提供量和最后截断原因，作为 debug inspector 的后端趋势数据。
- Inspect mode 的 Context Pressure 区块已展示整体覆盖率、truncated / dropped 事件数、受压 source、每个 source 的覆盖率条、平均提供量和最近截断/丢弃原因。
- `writer_agent:context_source_trend_pressure` 已用紧预算长会话式 fixture 验证 dropped / truncated source pressure 和 budget-exhaustion reason 会被 trace/debug 路径暴露。
- `productMetricsTrend` 已从持久化 `writer.context_pack_built` run events 聚合每个 session 的 context pack count、requested/provided chars、coverage rate、truncated/dropped source count、overall/recent/previous coverage 和 delta；Inspect Session Trend 已展示 `ctx`、`ctx packs`、`trunc`、`drop`。
- `writer_agent:product_metrics_context_pressure_trend` 已覆盖跨 session context pressure trend 和 trajectory export 字段。

Partial：

- Context pressure 现在有当前 trace 和 persisted session trend 两层 Inspect 视图，但还未按真实作者项目建立阈值、告警或分章节趋势。
- Budget 被丢弃来源的解释已经进入 pack/report/Inspector 层，但还需要真实长 session 数据校准哪些 dropped source 是可接受压缩，哪些代表关键上下文缺失。

Remaining：

- 用真实长 session fixtures 继续验证 context source trends 是否能暴露预算挤压、关键来源缺失和截断异常。
- 继续校准 Inspector 中 coverage / dropped / truncated 的阈值和排序，让它能从 debug 面板升级为可执行的修复入口。

Verification：

- `writer_agent:context_budget_required_sources`
- `writer_agent:context_budget_trace`
- `writer_agent:context_source_trend`
- `writer_agent:context_source_trend_pressure`
- `writer_agent:product_metrics_context_pressure_trend`
- `writer_agent:result_feedback_survives_tight_budget`
- `writer_agent:context_pack_explainability`
- `npm run verify`

### P2.2 记忆写入质量门槛

Status：Canon / Promise 质量门槛已接入真实 proposal 生成路径；Story Contract / Chapter Mission foundation guard 已存在；Style memory validation 已覆盖显式 style operation、反馈派生 style ledger 写入、基础风格 taxonomy 冲突识别和 polarity-aware 同向合并；同名 canon entity 的缺失属性补充已使用窄 `canon.update_attribute` 审批操作，避免整实体覆盖。

Done：

- 记忆候选已通过 proposal lifecycle 和 memory feedback 路径覆盖 observation、candidate、approved、rejected、superseded 等状态。
- WriterMemory 已新增结构化 `memory_feedback_events` 第一阶段 schema，记录 memory candidate 的 slot、category、action、confidence_delta、source_error、proposal_id、reason 和 created_at。
- memory candidate feedback 仍保留旧 style preference 信号兼容层，但 `MemoryExtractionFeedback` 已优先读取结构化 feedback；correction 会覆盖 reinforcement 并压制同 slot 后续候选。
- Ledger snapshot 已新增 `memoryReliability` 聚合视图，按 slot 汇总 trusted / needs_review / unproven、可靠性分数、reinforcement/correction 次数、net confidence delta 和最近 source error。
- Companion Audit 页已展示 Memory Reliability，能把需要复核的纠错 slot 放在作者可见位置。
- 本地保存抽取和 LLM memory candidate 会过滤模糊、空泛、重复候选。
- Canon / Promise 候选已有 dedupe 和冲突拦截；与现有 canon kind 或关键 attributes 冲突的候选不会直接写入长期记忆，而是生成高优先级 ContinuityWarning。
- Story Contract / Chapter Mission 写入已有 foundation quality gate，低质量 foundation 不会进入有效 context。
- Style preference 写入已有质量门槛：空泛、重复、同 key 反向冲突和同 taxonomy slot 反向冲突的偏好不会污染 style ledger；反馈派生的 style preference 只有足够具体时才会写入。
- Style preference 已有轻量 taxonomy slot：dialogue.subtext、prose.sentence_length、exposition.density、description.sensory_detail、pov.distance、action.clarity、structure.hook、tone.voice；不同 key 但落在同一 slot 的冲突偏好会被拦截。
- Style preference 已有 polarity-aware merge：同一 taxonomy slot 且方向一致的后续偏好会写入归一化 `style:<slot>` key 并合并文本；方向相反的偏好仍会作为 conflict 拦截，避免把“留白”和“解释情绪”同时写成作者偏好。
- 同名 canon entity 的非冲突缺失属性会生成可审批的 `canon.update_attribute` 窄操作，不再用整实体 upsert 覆盖既有 attributes。

Partial：

- Style taxonomy 仍是轻量关键词规则，polarity-aware merge 也仍基于关键词方向判断，尚未做作者可编辑 taxonomy、可视化合并审阅或更细粒度风格维度。

Remaining：

- 把 Contract / Mission 的质量门槛在文档和 eval 名称上继续独立维护，避免被 Canon / Promise 覆盖情况掩盖。
- 后续如继续扩展 Style，可把 taxonomy slot 做成作者可审阅/可编辑的偏好模型，而不是继续增加散落关键词。

Verification：

- `writer_agent:vague_memory_candidate_rejected`
- `writer_agent:duplicate_memory_candidate_deduped`
- `writer_agent:conflicting_memory_candidate_requires_review`
- `writer_agent:style_memory_validation`
- `writer_agent:same_entity_attribute_merge`
- `writer_agent:memory_feedback_schema_records_quality_signals`
- `writer_agent:memory_reliability_snapshot`
- `writer_agent:foundation_write_validation`
- `writer_agent:story_contract_quality_nominal`
- `npm run verify`

### P2.3 检索从“相似文本”升级为“写作相关性”

Status：部分完成。当前已完成 Writer Agent ledger context ranking，并已把轻量 writing relevance rerank 接入 project brain / vector DB 结果、standalone `query_project_brain` 和章节生成 RAG chunks；scene type taxonomy 和 avoid-term 约束已作为显式评分和解释信号接入，剩余缺口是继续用更真实的长会话检索数据证明普通语义段落干扰能稳定被压制。

Done：

- `src-tauri/src/writer_agent/context_relevance.rs` 已集中承载 Canon / Promise ledger slice 的写作相关性评分。
- Canon / Promise slice 会综合当前 chapter mission、next beat、result feedback、recent decisions、cursor 附近正文和 open promises 排序。
- 被选中的 canon / promise 会输出 `WHY writing_relevance`，说明当前写作相关性来源。
- `query_project_brain` 和 chapter generation 的 Project Brain / RAG chunk 会在 hybrid/vector/关键词初筛后按 writing relevance rerank，并输出 `WHY writing_relevance` 解释。
- `writer_agent:project_brain_writing_relevance_rerank` 已覆盖普通语义相似段落干扰：即使干扰段落初始相似分更高，mission/promise 相关段落也会排到前面。
- Scene type taxonomy 已覆盖 dialogue、action、description、emotional beat、conflict escalation、reveal、setup/payoff、exposition、transition；ledger 和 Project Brain rerank 都会把 scene type match 作为显式加分并写入 `WHY writing_relevance`。
- `writer_agent:scene_type_relevance_signal` 已证明 reveal / setup-payoff 场景信号能把揭示真相段落排在表层相似的描写段落之前。
- standalone `query_project_brain` 已注入 WriterMemory focus：active chapter mission、recent chapter result feedback、next beat 和 recent decisions 会参与初筛文本与 rerank focus，避免只依赖用户 query。
- `writer_agent:project_brain_writer_memory_focus` 已覆盖“用户 query 表层指向旧门传闻，但当前章节任务指向寒玉戒指下落”时，WriterMemory focus 能把任务相关 chunk 提到首位。
- Writing relevance 会把 `不要` / `不得` / `禁止` / `避免` / `不能` 后的短语识别为 avoid terms，避免 mission 的禁止事项在 Project Brain rerank 中反向抬高干扰段落，同时不把可回收的旧伏笔一概压制。
- `writer_agent:project_brain_long_session_candidate_recall` 已用多章节 Project Brain fixture 覆盖 query-only top5 漏召回、focus-aware 检索召回当前任务 chunk、并通过 avoid-term rerank 压制旧门传闻噪声。
- `writer_agent:project_brain_avoid_terms_preserve_payoff` 已覆盖 `不要被旧门传闻稀释主线` 不会压掉 `旧门钥匙` 这类正在回收的伏笔段落。
- `writer_agent:project_brain_must_not_boundary` 已覆盖 `不得让旧门传闻盖过寒玉戒指下落` 这类复杂 must_not 边界句式，避免把边界后的正向目标误识别为 avoid term。
- Scene type relevance 解释会优先输出 setup/payoff 和 reveal，再输出 action/description 等泛场景信号，让 `WHY writing_relevance` 更贴近检索决策。
- Writing relevance 已复用 Project Brain 关键词抽取并补充短语边界拆分，让 `霜铃塔钥`、`潮汐祭账` 这类作者项目专有词能进入 rerank 和 `WHY writing_relevance`，同时过滤 `Chapter-*` / `rev-*` 等结构标签噪声。
- `writer_agent:project_brain_author_fixture_rerank` 已用更接近真实作者项目的多章节 Project Brain fixture 覆盖 query-only top10 漏召回、口语化 `别再让...抢走...` must_not、作者项目专有词召回和噪声段落压制。

Partial：

- Scene type taxonomy 仍是轻量关键词规则，尚未做作者项目级自定义、LLM 校准或真实语料回归分析。
- Project Brain 初筛候选池已扩大并使用 query + WriterMemory focus；rerank 现在还会在 active chapter 已知时给当前/相邻/近邻章节 chunk 轻量加权，并把 `chapter proximity` 写入 `WHY writing_relevance`，避免同题远古 archive 在长会话里压过相邻章节线索。底层 VectorDB hybrid search 已把 BM25 查询分词、每次搜索的文档频率和文档长度预计算为单次搜索成本，避免每个 chunk 重复扫描全库；新增 harness test 覆盖词法排序行为。已有合成长会话和作者式长会话 fixture 覆盖候选召回、avoid-term 噪声压制、旧伏笔保留、复杂 must_not 边界、项目专有词解释和相邻章节优先，仍需要真实作者项目数据验证候选池倍率、召回稳定性、章节邻近权重和噪声段落压制边界。

Remaining：

- 用真实连续章节的 Project Brain fixtures 继续扩展 rerank eval，覆盖更多普通语义相似干扰、多章节召回、作者项目特有词汇和跨书名词碰撞。
- 基于真实项目数据继续校准 Project Brain candidate multiplier 和 avoid-term 负向权重，尤其验证更多口语化/长句式 must_not、需要回收的旧线索和作者自造名词共存时的表现。

Verification：

- `writer_agent:current_plot_relevance_prioritizes_same_name_entity`（已覆盖 ledger Canon slice）
- `writer_agent:promise_relevance_beats_plain_similarity`（已覆盖 ledger Promise slice）
- `writer_agent:project_brain_writing_relevance_rerank`（已覆盖 Project Brain / vector chunk rerank 的普通语义干扰）
- `writer_agent:scene_type_relevance_signal`
- `writer_agent:project_brain_writer_memory_focus`
- `writer_agent:project_brain_long_session_candidate_recall`
- `writer_agent:project_brain_avoid_terms_preserve_payoff`
- `writer_agent:project_brain_must_not_boundary`
- `writer_agent:project_brain_author_fixture_rerank`
- `npm run verify`

## 9. P2：架构拆分和可维护性（P2.4-P2.6）

### P2.4 拆分 `src-tauri/src/lib.rs`

目标：`lib.rs` 只保留 app setup、command registration 和少量跨模块 glue。

当前状态：已完成。command handler 拆分已完成；`lib.rs` 当前有 0 个 `#[tauri::command]`，所有 57 个 Tauri commands 都在 `src-tauri/src/commands/*` 下，并由 `npm run check:audit` 要求显式风险分类。`src-tauri/src/app_state.rs` 已承接 AppState、锁 helper、memory DB 初始化、legacy DB migration 和 Writer Kernel seed。`src-tauri/src/semantic_lint.rs` 已承接 SemanticLint payload/event、设定冲突 lint 和 Writer Agent diagnostic lint。`src-tauri/src/memory_context.rs` 已承接 manual request context injection、用户画像读取、章节 embedding、近期技能抽取和 LLM memory candidate 生成。`src-tauri/src/observation_bridge.rs` 已承接 Agent/editor/manual observation payload 和 WriterObservation 转换逻辑。`src-tauri/src/editor_realtime.rs` 已承接 editor ghost rendering、ambient output 转发、editor prediction 清理、realtime cowrite 开关和 LLM ghost proposal flow。`api_key.rs`、`app_paths.rs`、`events.rs`、`event_payloads.rs`、`agent_status.rs`、`project_audit.rs`、`writer_observer.rs` 已承接原先散落在 root 的通用 helper 和写作保存观察 helper。`src-tauri/src/tests.rs` 已承接原 `lib.rs` 内联测试。`lib.rs` 只保留模块 wiring、Tauri setup 和 command registration，并纳入 `npm run check:architecture` 的 root glue 预算。

建议模块：

```text
src-tauri/src/commands/
  mod.rs
  settings.rs
  chapters.rs
  outline.rs
  lore.rs
  writer_agent.rs
  generation.rs
  backups.rs
  diagnostics.rs

src-tauri/src/events.rs
src-tauri/src/app_state.rs
src-tauri/src/api_key.rs
src-tauri/src/app_paths.rs
src-tauri/src/event_payloads.rs
src-tauri/src/agent_status.rs
src-tauri/src/project_audit.rs
src-tauri/src/semantic_lint.rs
src-tauri/src/memory_context.rs
src-tauri/src/observation_bridge.rs
src-tauri/src/editor_realtime.rs
src-tauri/src/writer_observer.rs
src-tauri/src/tests.rs
src-tauri/src/context_injection.rs
src-tauri/src/manual_agent.rs
```

验收标准：

- `lib.rs` 行数继续下降，且不再承载业务重的 helper。
- 所有 command handler 有对应模块。（已完成）
- AppState / startup DB 初始化有独立模块。（已完成）
- Semantic lint 有独立模块。（已完成）
- Context / memory helper 有独立模块。（已完成）
- Observation bridge 有独立模块。（已完成）
- Editor realtime helper 有独立模块。（已完成）
- Root utility / event / audit / writer observation helper 有独立模块。（已完成）
- Root tests 有独立模块。（已完成）
- `cargo test -p agent-writer` 通过。
- `npm run check:architecture` 通过，防止 root glue、kernel facade、关键 kernel 子模块和 eval facade 重新膨胀。
- `npm run check:architecture` 同时检查 CompanionPanel 的 proposal / contract / brain helper 模块不引入 React、JSX、hook-like 调用或副作用 API，检查 `writer_agent/kernel.rs` 不恢复 wildcard facade export，并检查 `writer_agent/kernel/trace_recording.rs` 不恢复 `include!`。（已完成）

### P2.5 拆分 `writer_agent/kernel.rs`

当前状态：已完成。`writer_agent/kernel.rs` 保留 facade、状态结构、公开类型、`new()` 和少量共享转换 helper；对外 `writer_agent::kernel::*` 路径保持稳定，但 facade 导出已从通配 re-export 收敛为显式列表。章节、helper、operation、prompt、review、TaskPacket、metrics、metrics trend、proposal lifecycle、ghost、memory feedback、memory candidate、run-loop、run-loop Context Spine helper 类型和 stateful impl 均已落入 `src-tauri/src/writer_agent/kernel/` 下的职责模块。`trace_recording` 已从 `.in.rs include!` 迁成正常 Rust 子模块：`trace_recording/event_recording.rs`、`trace_recording/task_packet.rs`、`trace_recording/helpers.rs`。kernel facade、关键子模块、explicit export 和 trace no-`include!` 由 `npm run check:architecture` 持续守住。

建议模块：

```text
writer_agent/
  kernel.rs              // facade / state owner
  kernel/                // stateful WriterAgentKernel impl blocks（已完成）
  kernel/run_loop_ext.rs  // unified task execution types / prepared run（已完成）
  kernel/ghost.rs         // ghost proposal helpers（已完成）
  kernel/memory_feedback.rs // memory feedback / slot helpers（已完成）
  kernel/memory_candidates.rs + kernel/memory_candidates/ // memory candidate extraction / validation（已完成）
  kernel/task_packet.rs   // TaskPacket / context trace helpers（已完成）
  kernel/metrics.rs       // trace-derived product metrics（已完成）
  kernel/proposals_ext.rs // proposal lifecycle helpers（已完成）
  kernel/operations.rs    // operation execution impl（已完成）
  kernel/proposal_creation.rs // proposal creation / registration impl（已完成）
  kernel/feedback.rs      // feedback impl（已完成）
  kernel/snapshots.rs     // ledger snapshot impl（已完成）
  kernel/run_loop.rs      // run_task / prepared-run impl（已完成）
  kernel/trace_recording.rs + kernel/trace_recording/ // run-event trace recording（已完成）
```

验收标准：

- kernel facade 保持清晰 API。
- kernel facade 不恢复通配 re-export。
- operation execution、task packet、feedback、policy 分离。
- trace recording 不恢复 `include!` facade。
- eval 不降级。
- `npm run check:architecture` 通过。

### P2.6 拆分 `agent-evals/src/evals.rs`

当前状态：已完成。`agent-evals/src/evals.rs` 只保留共享 imports、`EvalToolHandler`、`eval_llm_message` 和子模块 re-export；root-level `evals_extra.rs` / `evals_extra2.rs` 已清除，原遗留 eval 已按职责归档进 promise、canon、context、story_debt、trajectory、task_packet 和新增 `memory_quality` 模块，`product_scenarios` 也已移入 `agent-evals/src/evals/` 并由 facade 统一导出；Reader Compensation 和 Context Spine eval 也已进入职责模块。`main.rs` 现在只挂载 `evals` 与 `fixtures`，不再直接依赖 legacy eval 文件；`cargo run -p agent-evals` 仍输出同一报告格式；当前完整 eval 基线由 `scripts/verification-baseline.cjs` 维护，为 245/245 passing。

建议模块：

```text
agent-evals/src/
  main.rs
  fixtures.rs
  evals/
    intent.rs
    context.rs
    canon.rs
    promise.rs
    mission.rs
    ghost_feedback.rs
    run_loop.rs
    task_packet.rs
    foundation.rs
    story_debt.rs
    trajectory.rs
    tool_policy.rs
    product_scenarios.rs
```

验收标准：

- 每个 eval 文件职责单一。
- 新增 eval 不需要继续扩大单文件。
- `cargo run -p agent-evals` 仍输出同一报告格式。

## 10. P3：高级写作伙伴能力

当前状态：P3.1-P3.3 第一阶段全部完成。Multi-Ghost 已有 per-branch evidence grounding，Ambient Lore 空闲实体检测管道已激活，Parallel Drafts 已注入 mission context 并路由至 InlinePreview 生命周期。

这些能力必须在 P0/P1 完成后推进，否则会变成花活。

### P3.1 Multi-Ghost 情境接力

目标：不是 Copilot 式补一两句，而是根据上下文给出多个创作方向。

任务：

- 停顿触发 3 个分支：
  - direct continuation
  - emotional subtext
  - conflict escalation
- 每个分支标注：
  - 依据的角色状态
  - 依据的 chapter mission
  - 可能触碰的 promise
- Tab 接受当前分支，方向键切换分支。（已完成）

当前状态：`ProposalAlternative` 已新增 `evidence` 字段，`per_branch_evidence()` 按 Canon/Mission/Promise 为分支 A/B/C 分配依据。Ghost decoration badge 显示 grounding source（如 `[B 言语试探 · 2/3 · ChapterMission]`）。方向键切换、Tab 接受均已就绪。

剩余：接受/拒绝反馈影响后续分支排序（需跨 session 的 proposal feedback 统计）。

验收标准：

- 已有 multi-ghost eval 扩展到 mission / promise / style grounding。（已完成：`multi_ghost_branches` eval）
- 接受 / 拒绝反馈影响后续分支排序。

### P3.2 Ambient Lore

目标：作家不主动问，agent 也能守住设定。

任务：

- 实时实体锚点。
- canon conflict 微提示。
- hover 显示简短证据，不塞大段 lore。
- 高风险冲突进入 Companion queue。（已完成：已有 story debt → Companion queue 管道）

当前状态：`get_ambient_entity_hints` Tauri 命令已上线，EditorPanel 4s 空闲定时器提取当前段落中的 Canon 实体名，后端查 canon_facts 返回摘要。已有 `EntityAnchor` 扩展自动装饰关键词，`EntityHoverCard` 弹窗显示 "关键词 · 章节 · Canon fact"。高风险冲突已通过 story debt 进入 Companion queue。

剩余：大文本 DOM decoration rebuild 性能验证（当前仅覆盖光标附近 ±200 chars），canon conflict 微提示的实时推送。

验收标准：

- DOM decoration 不复制完整 lore 内容。（已完成：仅展示 facts[:3] 摘要，不塞原文）
- 大文本下 decoration rebuild 不明显卡顿。（部分完成：限制 ±200 chars 扫描窗口）
- 错误提示可拒绝并学习。

### P3.3 Parallel Drafts

目标：重头戏时 agent 提供可拼接的平行草稿，而不是一次性替作者写完。

任务：

- 同一 scene goal 生成 3 个版本：（已完成：A 顺势推进、B 冲突加压、C 情绪转折）
- 允许句段级采纳。（已完成：段落级 splitDraft + 点击插入）
- 每段采纳都进入 operation lifecycle。（已完成：handleInsertParallelDraft 改为 setInlinePreview → accept/reject → feedback）

当前状态：`ParallelDraftPayload` 已新增 `mission_context` 字段，prompt 注入章节任务约束。段级采纳改为路由到 InlinePreview（而非直接 insertContent），接受后走完整 operation lifecycle → durable save → feedback 闭环。`generate_parallel_drafts` 命令已有 mission/promise 上下文注入。

剩余：agent 主动触发 parallel drafts（当前仅用户手动触发），eval 覆盖。

验收标准：

- 不直接覆盖正文。
- 每个 draft 标明与 mission / promise 的关系。
- 采纳后保存成功才写入 feedback。

## 11. P4：外部 Agent 项目借鉴专项计划

本节来自对 `C:/Users/Msi/Desktop/agent` 下 8 个本地 agent 项目的定向审查，其中 `code-review-graph-main` 对应 GitHub 来源 `https://github.com/tirth8205/code-review-graph.git`；另包含对 `C:/Users/Msi/Desktop/竞品` 下 18 个传统 AI 写作工具项目的垂直化竞品抽样。目标不是把 Forge 改造成通用 agent 平台或传统 AI 写作 IDE，而是在证据可追溯的前提下，把其他项目中已经证明有价值的机制裁剪到小说写作 agent 的五个核心方面。

### 11.0 证据边界

已审查项目：

- `claw-code-main`
- `CowAgent-2.0.7`
- `deer-flow-main`
- `hermes-agent-2026.4.30`
- `ml-intern-main`
- `openclaw-main`
- `opencode-1.14.30`
- `code-review-graph-main`（本地路径：`C:/Users/Msi/Desktop/agent/code-review-graph-main`；GitHub 来源：`https://github.com/tirth8205/code-review-graph.git`）

已审查传统 AI 写作竞品项目：

- `91Writing-main`
- `AI_NovelGenerator-main`
- `ai-novel-lab-master`
- `AI-Novel-Writing-Assistant-main`
- `AINovel-main`
- `De-AI-Prompt-Enhancer-Writer-Booster-SKILL-main`
- `dramatica-flow-main`
- `inkos-master`
- `kimi-book-writer-main`
- `Morpheus-main`
- `novel_ai_writer`
- `novel-pro-main`
- `novelvel-master`
- `Openwrite-main`
- `PlotPilot-2-main`
- `vela-master`
- `webnovel-writer-master`
- `WenShape-main`

证据纪律：

- 只把 README 或源码中已经看到的机制写成依据。
- 对 `openclaw-main`、`opencode-1.14.30`、`hermes-agent-2026.4.30` 这类大仓，只按已审查源码得出局部结论，不声明“已完整审计”。
- README 声称但未进入源码验证的能力，不能作为高置信实现依据。
- `code-review-graph-main` 当前本地目录不是 git checkout，不从该目录推断 commit hash；只把已核对到本地 README / source function 的机制写入计划，benchmark 数字只能作为该项目自报证据，不能直接外推到 Forge。
- `OpenHarness-main` 审查只覆盖 harness 相关源码和 README：QueryEngine、tool registry、permission checker、compaction、dry-run preview、swarm/autopilot 局部模块；结论限定为 harness 架构借鉴，不声明已完整审计整个项目。
- `C:/Users/Msi/Desktop/竞品` 审查覆盖 18 个项目的 README / architecture 文档和少量源码抽样：只用于识别传统 AI 写作工具的共同产品形态和可迁移机制，不把各项目 README 中的营销表述等同为真实生产质量。
- 没发现致命问题时必须坦承，不为了显得尖锐而硬批。

### 11.1 可借鉴证据清单

| 项目 | 已确认机制 | 证据位置 | 对 Forge 的意义 |
| --- | --- | --- | --- |
| CowAgent | 工作区分层：`AGENT.md` / `USER.md` / `RULE.md` / `MEMORY.md` / `memory/` / `knowledge/`；`MEMORY.md` 有上下文截断提示；KnowledgeService 支持 list/read/graph，并防路径逃逸。 | `C:/Users/Msi/Desktop/agent/CowAgent-2.0.7/agent/prompt/workspace.py:16`, `C:/Users/Msi/Desktop/agent/CowAgent-2.0.7/agent/prompt/workspace.py:158`, `C:/Users/Msi/Desktop/agent/CowAgent-2.0.7/agent/knowledge/service.py:115`, `C:/Users/Msi/Desktop/agent/CowAgent-2.0.7/agent/knowledge/service.py:142` | 可借鉴 Project Brain 的 knowledge index / graph；不能照搬自动沉淀长期记忆。 |
| DeerFlow | Lead agent 以 middleware 组合 summarization、todo、memory、loop detection、clarification 等；MemoryUpdater 支持 correction / reinforcement hint、fact category、confidence、dedupe、max facts；ACP 子代理使用 per-thread workspace。 | `C:/Users/Msi/Desktop/agent/deer-flow-main/backend/packages/harness/deerflow/agents/lead_agent/agent.py:7`, `C:/Users/Msi/Desktop/agent/deer-flow-main/backend/packages/harness/deerflow/agents/memory/updater.py:293`, `C:/Users/Msi/Desktop/agent/deer-flow-main/backend/packages/harness/deerflow/agents/memory/updater.py:547`, `C:/Users/Msi/Desktop/agent/deer-flow-main/backend/packages/harness/deerflow/tools/builtins/invoke_acp_agent_tool.py:20` | 可借鉴阶段化 agent pipeline、记忆纠错/强化、隔离子任务工作区。 |
| Claw Code | Worker boot 有明确状态、TaskReceipt、startup evidence bundle、prompt misdelivery / tool permission / trust gate 分类。 | `C:/Users/Msi/Desktop/agent/claw-code-main/rust/crates/runtime/src/worker_boot.rs:28`, `C:/Users/Msi/Desktop/agent/claw-code-main/rust/crates/runtime/src/worker_boot.rs:125`, `C:/Users/Msi/Desktop/agent/claw-code-main/rust/crates/runtime/src/worker_boot.rs:197` | 可借鉴长任务启动、生成和失败解释的证据包，而不是只返回字符串错误。 |
| ML Intern | Session event append、trace message append、Claude Code JSONL 转换、上传前 scrub、数据卡明确 redaction 风险。 | `C:/Users/Msi/Desktop/agent/ml-intern-main/agent/core/session.py:153`, `C:/Users/Msi/Desktop/agent/ml-intern-main/agent/core/session.py:179`, `C:/Users/Msi/Desktop/agent/ml-intern-main/agent/core/session_uploader.py:136`, `C:/Users/Msi/Desktop/agent/ml-intern-main/agent/core/session_uploader.py:395` | 可借鉴 append-only WriterRunEventStore、trace inspector、导出格式和隐私警告。 |
| OpenCode | 内置 `build` 和只读 `plan` agent，`plan` 禁止 edit；`explore` 子代理只允许搜索/读取等探索工具；写文件后返回 LSP diagnostics。 | `C:/Users/Msi/Desktop/agent/opencode-1.14.30/packages/opencode/src/agent/agent.ts:123`, `C:/Users/Msi/Desktop/agent/opencode-1.14.30/packages/opencode/src/agent/agent.ts:160`, `C:/Users/Msi/Desktop/agent/opencode-1.14.30/packages/opencode/src/tool/write.ts:73` | 可借鉴只读规划/审稿模式、探索子代理权限边界、写后诊断反馈。 |
| OpenClaw | Memory embedding SDK 暴露 provider registry、batch helpers、input limit、multimodal path 分类；ACP persistent binding tests 覆盖 session key、cwd mismatch reinit、error-state reinit。 | `C:/Users/Msi/Desktop/agent/openclaw-main/packages/memory-host-sdk/src/engine-embeddings.ts:3`, `C:/Users/Msi/Desktop/agent/openclaw-main/packages/memory-host-sdk/src/engine-embeddings.ts:28`, `C:/Users/Msi/Desktop/agent/openclaw-main/src/acp/persistent-bindings.test.ts:887` | 可借鉴 Project Brain embedding provider 抽象和长会话绑定恢复；当前不建议照搬多渠道 gateway。 |
| Hermes Agent | Cron job 运行时禁用部分 toolsets，并设置 `skip_memory=True`，注释说明 cron system prompts 会污染 user representations；skill usage sidecar 只允许 curator 处理 agent-created skills。 | `C:/Users/Msi/Desktop/agent/hermes-agent-2026.4.30/cron/scheduler.py:1044`, `C:/Users/Msi/Desktop/agent/hermes-agent-2026.4.30/cron/scheduler.py:1051`, `C:/Users/Msi/Desktop/agent/hermes-agent-2026.4.30/tools/skill_usage.py:1`, `C:/Users/Msi/Desktop/agent/hermes-agent-2026.4.30/tools/skill_usage.py:151` | 这是反向边界证据：后台自动任务、技能自改、长期记忆必须强约束，不能污染写作项目。 |
| code-review-graph | Tree-sitter AST / SQLite graph / MCP tools 组合出 minimal context、impact radius、review context、graph traversal；README 自报 token reduction，同时承认小型单文件变更可能更贵、impact precision 会保守过报。 | `C:/Users/Msi/Desktop/agent/code-review-graph-main/README.md:77`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/README.md:126`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/README.md:141`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/README.md:146`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/README.md:181`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/code_review_graph/tools/context.py:37`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/code_review_graph/tools/review.py:24`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/code_review_graph/changes.py:275`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/code_review_graph/graph.py:625`, `C:/Users/Msi/Desktop/agent/code-review-graph-main/code_review_graph/tools/query.py:573` | 可借鉴 graph-shaped context assembly、minimal context first、blast-radius discipline 和预算化遍历；不能照搬代码 AST/call graph 到小说写作域。 |
| OpenHarness | QueryEngine 注入 provider、tool registry、permission checker、hook、context window、auto-compact threshold 和 usage tracker；dry-run preview 静态解析 auth/provider/skills/tools/commands/MCP/system prompt 并输出 readiness；compaction 有 microcompact、boundary marker、checkpoint metadata；permission checker 支持敏感路径、path rule、command deny pattern；多 tool call 使用并发执行并保证每个 tool_use 有结果。 | `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/engine/query_engine.py:19`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/cli.py:333`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/cli.py:396`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/services/compact/__init__.py:99`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/services/compact/__init__.py:187`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/services/compact/__init__.py:808`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/permissions/checker.py:18`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/permissions/checker.py:75`, `C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/engine/query.py:815` | 可借鉴 writer run preflight、compaction 可观测性、调用级权限上下文和只读工具并发；不建议照搬通用 swarm/autopilot/plugin 平台。 |

Forge 当前不是空白 agent 框架。现有事实基线已经包括 `agent-harness-core`、Writer Agent Kernel、TaskPacket、typed WriterOperation、approval/audit、operation lifecycle、trajectory export、Story Contract、Chapter Mission、Promise Ledger、Project Brain rerank。P4 的任务是补齐控制面和反馈土壤，不是重复建设通用 agent runtime。

### 11.2 核心大脑：推理与规划能力

目标：把 Forge 的“想清楚再写”做成一等工作流，而不是继续让手动聊天承担规划职责。

可借鉴机制：

- DeerFlow 的 middleware 链证明 agent 大脑可以拆成 summarization、planning、memory、loop detection、clarification 等可组合阶段。
- OpenCode 的 `plan` agent 证明只读规划和执行 agent 应该分权。
- Claw Code 的 `WorkerTaskReceipt` 证明长任务需要明确“收到的是什么任务、要产出什么、失败时证据是什么”。

任务：

1. 增加 Writer Planning / Review 模式。（第一阶段已完成）
   - 已新增 `WriterAgentTask::PlanningReview` / `AgentTask::PlanningReview`。
   - 已使用专用 context budget profile，优先 Chapter Mission、Project Brief、Result Feedback、NextBeat、PromiseSlice、CanonSlice、DecisionSlice、OutlineSlice、AuthorStyle、SelectedText、CursorPrefix、RagExcerpt。
   - 已在 task packet 中限制为 Chapter scope + AnalyzeText intent。
   - 已在 tool policy 中限制为 read-only project tools，不允许 provider-call 生成、approval-required 写入、正文写入或记忆写入。
   - 已新增专用系统提示词，输出目标理解、证据、风险、候选行动、需要作者确认的问题。
   - 已新增 eval：`writer_agent:planning_mode_denies_writes`、`writer_agent:planning_mode_uses_story_foundation`。
2. 将 planning / diagnosis / generation 的前置阶段整理成 Writer Kernel phase pipeline。
   - 保持现有 `WriterAgentKernel.prepare_task_run()` 作为统一入口。
   - 当前完成度：PlanningReview / ManualRequest / ChapterGeneration 已通过统一 task packet 和 trace 入口；真实写作工作流中的 context pack 组装已进入 `writer.context_pack_built` run event；章节生成 / Project Brain / manual request 的 provider call 启动已进入 `writer.model_started` run event；manual AgentLoop 工具调用 start/end 已进入 `writer.tool_called` run event。`agent-harness-core::ToolExecutor` 现在有可选 audit sink，Tauri 侧已有 `writer_tool_audit_sink` helper，manual AgentLoop 已迁到 executor 层记录；这补上了非 AgentLoop 直接 executor 调用的第一阶段能力，但还需要把未来真实外部工具入口逐个挂上该 sink。
3. 为 ChapterGeneration、BatchGeneration、长诊断增加 WriterTaskReceipt。（第一阶段已完成）
   - 已新增 `src-tauri/src/writer_agent/task_receipt.rs`。
   - `WriterTaskReceipt` 字段包括 task id、task kind、chapter、objective、required evidence、expected artifacts、must_not、source_refs、base_revision、created_at。
   - `BuiltChapterContext` 现在携带 receipt；章节生成保存前校验 task id、chapter、base revision、expected artifact 和 required evidence。
   - receipt mismatch 会阻断写入，并产出结构化错误证据。
   - ContinuityDiagnostic 第一阶段已接入只读 diagnostic receipt：准备诊断任务时记录 required evidence、expected diagnostic artifacts、must_not 写入类 artifact，并以 `writer.task_receipt` run event 进入 trace / trajectory；Inspector timeline 已将其作为 `task_receipt` 一等事件展示，Inspect 面板可单独筛选 receipt 并查看 evidence / artifact / guard 摘要。
   - ContinuityDiagnostic 完整 run 成功后会将模型诊断答案登记为 `diagnostic_report` task artifact，写入 `writer.task_artifact` run event，进入 trajectory 和 Inspector `task_artifact` 筛选；artifact 登记前复用 receipt 校验，不能伪装成 saved_chapter / memory_write。
   - PlanningReview 完整 run 成功后会将模型规划审查答案登记为 `planning_review_report` task artifact，写入 `writer.task_artifact` run event，进入 trajectory 和 Inspector `task_artifact` 筛选；artifact 登记前复用 receipt 校验，仍保持只读、不可伪装成 saved_chapter / memory_write。
   - 边界：当前长诊断 / 规划审查 artifact 是 run event 中的本地可回放 artifact；独立文件级 artifact browser、真实长诊断/规划报告库、更多失败分类接入 Inspector 仍未完成。
   - 已新增 eval：`writer_agent:chapter_generation_task_receipt_required`、`writer_agent:continuity_diagnostic_task_receipt`、`writer_agent:continuity_diagnostic_artifact_recorded`、`writer_agent:planning_review_artifact_recorded`、`writer_agent:task_receipt_mismatch_blocks_write`。
4. 增加失败证据包。（第一阶段已完成）
   - 已新增 `WriterFailureEvidenceBundle`，分类包括 context_missing、tool_denied、provider_failed、receipt_mismatch、save_failed、feedback_blocked。
   - Provider timeout/rate limit/config/error、save conflict、receipt mismatch 可映射为结构化 failure bundle。
   - `WriterAgentKernel.record_failure_evidence_bundle()` 会将失败包写入 `writer.error` run event，并随 trajectory 以 `writer.run_event` 导出。
   - 已新增 eval：`writer_agent:run_failure_evidence_bundle`。

验收：

- `writer_agent:planning_mode_denies_writes`（已完成）
- `writer_agent:planning_mode_uses_story_foundation`（已完成）
- `writer_agent:chapter_generation_task_receipt_required`（已完成）
- `writer_agent:continuity_diagnostic_task_receipt`（已完成第一阶段：只读诊断 receipt + run event / trajectory / Inspector receipt event）
- `writer_agent:continuity_diagnostic_artifact_recorded`（已完成第一阶段：诊断报告 artifact run event + trajectory / Inspector artifact event）
- `writer_agent:planning_review_artifact_recorded`（已完成第一阶段：规划审查 artifact run event + trajectory / Inspector artifact event）
- `writer_agent:task_receipt_mismatch_blocks_write`（已完成）
- `writer_agent:run_failure_evidence_bundle`（已完成）

### 11.3 记忆系统：世界与自我的感知

目标：让 Forge 的记忆不仅“存事实”，还要知道事实的来源、置信度、纠错历史和适用边界。

可借鉴机制：

- CowAgent 的 `knowledge/` + `index.md` + graph 适合作为 Project Brain 的可视化知识索引参考。
- DeerFlow 的 `category`、`confidence`、correction / reinforcement hint、dedupe、max facts 适合迁移到写作记忆质量层。
- OpenClaw 的 embedding provider registry / batch helpers / input limits 适合迁移到 Project Brain 的向量能力边界。

任务：

1. 为 WriterMemory 增加 memory confidence / category / source error 语义。（第一阶段已完成）
   - Canon、Promise、Style、Contract、Mission 分别保留现有 typed schema。
   - 不把所有记忆压平成通用 fact。
   - 已新增 `memory_feedback_events` 结构化反馈表：slot、category、action、confidence_delta、source_error、proposal_id、reason、created_at。
   - memory candidate 的 accepted/rejected feedback 会分别写入 reinforcement/correction；accepted 记录正向 confidence delta，rejected 记录负向 confidence delta 和作者纠错原因作为 source error。
   - 已新增 ledger snapshot `memoryReliability`：按 slot 聚合 trusted / needs_review / unproven、可靠性分数、reinforcement/correction 次数、net confidence delta 和最近 source error。
   - Companion Audit 页已展示 Memory Reliability，让作者能看到哪些记忆槽位被强化、哪些因 source error 需要复核。
   - 边界：这还不是把 confidence/source error 下沉到 Canon/Promise/Style/Contract/Mission 每一条 ledger row 的完整模型，也未覆盖作者撤销或手工纠错入口。
2. 增加作者纠错和正反馈校准。（第一阶段已完成）
   - 已复用现有 `proposal_feedback`、`memory_audit_events`、style preference 计数和新的 `memory_feedback_events` 作为反馈持久化底座。
   - accepted memory candidate 会写入 `memory_reinforcement:<slot>` 和 `memory_extract:<slot>` reinforcement 信号。
   - rejected / edited memory candidate 会写入 `memory_correction:<slot>` 和 `memory_extract:<slot>` correction 信号，并进入 memory audit。
   - `MemoryExtractionFeedback` 现在优先读取结构化 memory feedback，并将 correction 视为强压制信号：即使同 slot 已有 reinforcement，纠错也优先，后续同 slot 候选默认不再出现；旧 style preference 信号仅作为兼容兜底。
   - 已新增 eval：`writer_agent:memory_correction_overrides_reinforcement`、`writer_agent:accepted_feedback_reinforces_style_memory`、`writer_agent:rejected_proposal_records_correction_signal`、`writer_agent:memory_feedback_schema_records_quality_signals`、`writer_agent:memory_reliability_snapshot`。
   - 剩余：覆盖作者撤销/手工纠错入口，并把 reliability 视图从只读摘要推进到可操作审阅。
3. Project Brain 增加 knowledge index / reference graph。（第一阶段已完成）
   - 已新增 `ProjectBrainKnowledgeIndex` / node / edge schema。
   - 已从 Project Brain vector chunks、outline、lorebook 构建索引节点，节点带 kind、label、source_ref、keywords、summary。
   - 已基于 shared keyword 生成 reference graph edge，保留 evidence_ref。
   - 已新增 `knowledge_index.json` rebuild / load / save helper。
   - 已新增 knowledge index 文件读取路径守卫，拒绝 absolute path 和 `..` 逃逸。
   - 已新增 read-only Tauri command：`get_project_brain_knowledge_graph`。
   - Graph 页已新增 Brain 模式，可查看 Project Brain knowledge nodes / shared-keyword edges / source refs / keywords，并保留 Ask Brain 入口。
   - Graph 页已新增第一层节点类型过滤、来源/关键词/摘要/关系/revision 搜索、选中节点邻接高亮、source kind / revision / chunk index 详情、source history 摘要、read-only source revision compare、reference / back-reference 列表和一键跳转到相邻节点。
   - Project Brain vector chunks 已新增 `source_ref`、`source_revision`、`source_kind`、`chunk_index`、`archived`，章节 embedding 会记录来源章节和内容 revision，并把旧 revision 标为 archived；默认 Project Brain 检索只看 active chunk，source compare 才读取 archived history；knowledge node 保留 `kind=chunk`，并附带 source metadata；knowledge index 会按 source_ref 聚合 revision history、active 标记、node/chunk counts 和 chunk indexes。
   - Source/version 回滚第一阶段已完成：新增 `restore_project_brain_source_revision`，Graph 页可从 archived revision 执行恢复；后端只把同一 `source_ref` 下目标 revision 的 chunk 设为 active，并把其他 revision 设为 archived，随后重建 knowledge index 和记录 `project_brain` 写审计。
   - 该回滚不是章节正文恢复，不会改写 chapter markdown、outline 或 lorebook；作者若要恢复正文，仍需要走现有 file backup/chapter restore 路径。
   - 已新增 eval：`writer_agent:project_brain_knowledge_index_graph`、`writer_agent:project_brain_knowledge_index_path_guard`、`writer_agent:project_brain_chunk_source_version`、`writer_agent:project_brain_source_revision_restore`。
   - 剩余：更深的 cross reference / back reference 操作、更多真实 Story Bible / 章节结果来源、跨来源冲突/重复关系校准，以及把 source revision 恢复和真实正文/Story Bible 恢复流程做成一致的作者确认体验。
4. Project Brain embedding provider 抽象。（第一阶段已完成）
   - 已新增 `ProjectBrainEmbeddingProviderRegistry` / provider spec / model spec，OpenAI、OpenRouter 和 local OpenAI-compatible provider 走本地 registry 解析。
   - 已新增 `ProjectBrainEmbeddingProviderProfile`，明确 provider id、model、维度、input limit、batch limit、retry limit、provider registry status 和 model registry status。
   - 未知 provider / model 会显式标记为 compatibility fallback，不伪装成已校准 provider。
   - `LlmSettings` 已支持 `OPENAI_EMBEDDING_INPUT_LIMIT_CHARS`，默认 8000 字符，并拒绝过小配置。
   - `embed_chapter` 和 Project Brain query embedding 已走统一 wrapper：输入会按 profile 截断，embedding 维度会校验，失败会按 retry policy 重试。
   - 已新增 `ProjectBrainEmbeddingBatchReport` / `ProjectBrainEmbeddingBatchStatus`，记录 requested、embedded、skipped、truncated、errors 和 complete / partial / empty 状态，避免批量入库静默误报。
   - 已新增 eval：`writer_agent:project_brain_embedding_provider_limits`、`writer_agent:project_brain_embedding_provider_registry`。
   - 剩余：provider-specific embedding 质量校准，以及真实项目 embedding 召回质量验证。
5. 记忆写入继续保持 reviewable。
   - 不允许后台自动任务绕过 WriterOperation。
   - 不允许 LLM 直接写永久 Canon / Promise / Style。
   - 已新增 eval：`writer_agent:memory_auto_write_cannot_bypass_review`，覆盖保存观察触发 Canon / Promise 候选时只产生 proposal 和 `writer.memory_candidate_created`，不会直接写入 Canon / Promise ledger。
6. Story Impact Radius Context Pack。（第一阶段已完成：内部类型、种子提取、故事图构建、双向 BFS 遍历、同层候选按故事价值排序的预算分配、真实 reached-drop 预算报告、`writer.story_impact_radius_built` run event 关联 observation、TaskPacket 摘要接入、默认写作 prompt ContextPack Sources 接入和 12 个 eval；借鉴 `code-review-graph` 的上下文组装纪律）
   - 证据判断：`code-review-graph` 的可迁移部分不是 Tree-sitter，也不是代码 call graph，而是“先最小上下文、再按变更/任务计算影响半径、再按预算抽取相关证据、最后暴露风险和截断”的流程。
   - Forge 不能照搬函数、类、调用者、测试覆盖这些代码语义；小说域需要转译为角色、设定、伏笔、章节任务、result feedback、Project Brain chunk、source revision 和 Story Contract / Mission / Canon / Promise 的证据关系。
   - 新增内部类型：
     - `WriterStoryGraphNode { id, kind, label, source_ref, source_revision, chapter, confidence, summary }`
     - `WriterStoryGraphEdge { from, to, kind, evidence_ref, confidence }`
     - `WriterStoryImpactRadius { seed_nodes, impacted_nodes, impacted_sources, edges, risk, truncated, reasons }`
   - Seed 来源：
     - active chapter / selected text / cursor prefix
     - accepted text operation / proposal durable save observation
     - manual request objective
     - Chapter Mission clauses / must_not / expected ending
     - open promise / canon entity / Project Brain chunk source refs
     - post-write diagnostic report / result feedback
   - Edge kind 第一版：
     - `mentions_entity`
     - `updates_promise`
     - `supports_mission`
     - `contradicts_canon`
     - `depends_on_result`
     - `same_source_revision`
     - `shared_keyword`
   - Context pack 流程：
     - 先生成 minimal writer context：当前任务、章节、mission、关键 canon/promise、context pressure、风险等级、建议下一步 diagnostic/tool。
     - 把当前任务或已接受 operation 映射到 seed story nodes。
     - 在 Project Brain knowledge index、WriterMemory ledger、mission/result feedback 上做 max depth + char budget 遍历。
     - 综合 writing relevance、graph distance、risk、source confidence 排序；同一 BFS 层内会优先保留 PlotPromise、ChapterMission、StoryContract、CanonRule/Entity 和高价值 edge，避免低价值 Project Brain chunk 在紧预算下抢占关键故事节点。
     - 输出 budget report：requested/provided chars、真实被遍历到但因预算丢弃的 truncated nodes、dropped high-risk sources、为什么纳入/丢弃；不会把无关孤立节点误算进预算压力。
     - 已选择并实现 `writer.story_impact_radius_built` run event；事件只记录预算、节点类型、source refs 和 reasons，不记录正文原文。
     - 已把 Story Impact 风险/预算摘要接入默认写作 TaskPacket 的 belief 与 required context，下一步才是把摘要以受预算约束的形式渲染进模型 prompt 的 ContextPack Sources。
   - 写作版 guidance：
     - 受影响 promise 没有 payoff target。
     - 受影响 canon 证据不足或来源 revision 过旧。
     - operation 触碰了 Chapter Mission `must_not`。
     - Project Brain chunk 与 Canon / Mission 互相冲突。
     - context pressure 导致高风险来源被截断。
   - 第一阶段 eval 必须先行：
     - `writer_agent:story_impact_radius_includes_impacted_promise_under_budget`
     - `writer_agent:story_impact_radius_excludes_semantic_distractor`
     - `writer_agent:story_impact_radius_reports_truncated_sources`
     - `writer_agent:story_impact_radius_maps_operation_to_story_nodes`
     - `writer_agent:story_impact_radius_traverses_reverse_edges`
     - `writer_agent:story_impact_radius_memory_seed_ids_align`
     - `writer_agent:story_impact_radius_run_event_links_observation`
     - `writer_agent:story_impact_radius_small_change_stays_minimal`
   - 产品验收不看“图更复杂”，只看长篇写作效果：关键伏笔召回率、设定误报率、mission drift 发现率、作者采纳率、context pressure 下的高风险遗漏数是否改善。

验收：

- `writer_agent:memory_correction_overrides_reinforcement`（已完成）
- `writer_agent:accepted_feedback_reinforces_style_memory`（已完成）
- `writer_agent:rejected_proposal_records_correction_signal`（已完成）
- `writer_agent:project_brain_knowledge_index_graph`（已完成）
- `writer_agent:project_brain_knowledge_index_path_guard`（已完成）
- `writer_agent:project_brain_chunk_source_version`（已完成）
- `writer_agent:project_brain_source_revision_restore`（已完成）
- `writer_agent:project_brain_embedding_provider_limits`（已完成）
- `writer_agent:project_brain_embedding_provider_registry`（已完成）
- `writer_agent:memory_auto_write_cannot_bypass_review`（已完成）
- `writer_agent:story_impact_radius_includes_impacted_promise_under_budget`（已完成）
- `writer_agent:story_impact_radius_excludes_semantic_distractor`（已完成）
- `writer_agent:story_impact_radius_reports_truncated_sources`（已完成）
- `writer_agent:story_impact_radius_maps_operation_to_story_nodes`（已完成）
- `writer_agent:story_impact_radius_traverses_reverse_edges`（已完成）
- `writer_agent:story_impact_radius_memory_seed_ids_align`（已完成）
- `writer_agent:story_impact_radius_run_event_links_observation`（已完成）
- `writer_agent:story_impact_radius_enters_task_packet`（已完成）
- `writer_agent:story_impact_radius_enters_prompt_context`（已完成）
- `writer_agent:story_impact_radius_small_change_stays_minimal`（已完成）

### 11.4 行动闭环：工具调用与外部互动

目标：让 Forge 能调用外部能力，但所有外部行动都必须隔离、可审计、可回收、不能直接污染正文和记忆。

可借鉴机制：

- DeerFlow 的 ACP 子代理 per-thread workspace。
- OpenCode 的 `explore` 子代理权限边界。
- OpenCode 的写后 LSP diagnostics 可类比为 Forge 的写后 continuity / mission / save diagnostics。
- ML Intern 的 cost / approval / event queue 思路可迁移到长生成和 provider 调用预算。

任务：

1. 增加 Research / Diagnostic 子任务工作区。（第一阶段已完成）
   - 已新增 `src-tauri/src/writer_agent/research_subtask.rs`。
   - 每个子任务可创建 `agent_subtasks/<subtask_id>/artifacts/` 隔离目录。
   - artifact 路径要求相对路径，拒绝 absolute path、`..`、非法 subtask id 和 workspace 逃逸。
   - 子任务结果只包含 objective、summary、evidence_refs、artifact_refs、blocked_operation_kinds，不携带可直接执行的 `WriterOperation`。
   - 尝试写正文、Canon、Promise、Style、Story Contract、Chapter Mission、Outline 等操作时，只记录为 blocked operation kind。
   - 子任务 started / completed payload 会记录 tool policy、evidence/artifact refs、blocked operation kinds 和计数，不记录原始 evidence snippet 或 artifact 绝对路径。
   - Kernel 已能把子任务 started / completed 写成 `writer.subtask_started` / `writer.subtask_completed` run event，并随 trajectory 的 `writer.run_event` 导出。
   - Inspector timeline 已新增 `subtask` 类型，前端 Inspect 模式可单独筛选 Subtasks。
   - 已新增 eval：`writer_agent:research_subtask_uses_isolated_workspace`、`writer_agent:research_subtask_outputs_evidence_only`、`writer_agent:research_subtask_run_events`。
   - 剩余：把子任务自动调度接进真实 run loop，并把外部公开资料检索做成可审计 provider/tool 调用。
2. 增加子任务 tool policy。（第一阶段已完成）
   - research：project read/search/rag，允许 Project Brain provider-call，但不允许 approval-required write。
   - diagnostic：project read/context/analyze only，阻断 Project Brain provider-call、generation preview、chapter draft write 和 internal trace tool。
   - drafting：只允许 generation preview，不允许保存。
   - 已新增 eval：`writer_agent:diagnostic_subtask_denies_writes`。
3. 写后诊断闭环。（保存观察路径第一阶段已完成）
   - 已新增 `src-tauri/src/writer_agent/post_write_diagnostics.rs`。
   - `observe(Save)` 会复用真实 `DiagnosticsEngine` 结果，生成 `WriterPostWriteDiagnosticReport`，包含 severity/category 计数、诊断条目、evidence refs 和 remediation。
   - 报告已写入 `writer.post_write_diagnostics` run event，并进入 `WriterAgentTraceSnapshot.post_write_diagnostics` 与 trajectory JSONL。
   - `record_writer_operation_durable_save` 已支持保存后正文 / 章节 / revision 参数；accepted inline / proposal text operation 成功持久化后，会用操作影响窗口复跑 diagnostics，并在 report source refs 中保留 proposal / operation / affected scope。
   - Companion Audit 页已展示最近 post-write diagnostic reports，包含错误/警告/信息计数、诊断消息、fix suggestion、revision 和 source refs。
   - Inspect 模式已展示最新 post-write diagnostic report，可在专用调试面板里查看错误/警告/信息计数、诊断条目、remediation、最近 save_completed 事件 source refs、save_completed 专用筛选、当前 save-to-feedback latency 和多 session save-to-feedback 趋势。
   - 已新增 eval：`writer_agent:post_write_diagnostics_recorded`、`writer_agent:post_write_diagnostics_after_accepted_operation`。
   - 已新增 `writer.save_completed` run event，把 save result、proposal / operation source refs、post-write report id 与诊断计数串进同一条可回放事件。
   - 已新增 `WriterProductMetricsTrend`，从持久化 run events 聚合最近 session 的 proposal / feedback / durable save / save-to-feedback latency，并在 Inspect 模式和 trajectory 中展示。
   - 剩余：继续用真实连续写作 fixture 校准趋势是否真的暴露 agent 有用性变化，而不是只证明聚合可运行。
4. Provider call budget。（第一阶段已完成）
   - 已新增 `src-tauri/src/writer_agent/provider_budget.rs`。
   - 长章节生成、批量生成、Project Brain 重建、外部研究、手动请求和 ghost preview 都有默认 token/cost 阈值。
   - 超预算且未批准时输出 `ApprovalRequired`，已批准超预算请求降级为 `Warn`，空请求 `Blocked`，报告包含 reasons 和 remediation。
   - 章节草稿生成已在 `llm_runtime::chat_text` 前执行 provider budget preflight；超预算会返回 `PROVIDER_BUDGET_APPROVAL_REQUIRED`，并携带 `WriterFailureEvidenceBundle`。
   - 章节生成 provider budget report 已记录为 `writer.provider_budget` run event，并随 trajectory 的 `writer.run_event` 导出。
   - Project Brain answer generation 已在 `llm_runtime::stream_chat` 前执行 `project_brain_query` provider budget preflight；超预算会记录 `writer.provider_budget` run event 和 `PROJECT_BRAIN_PROVIDER_BUDGET_APPROVAL_REQUIRED` failure bundle，然后阻断真实 chat provider call。
   - Manual request 已在 AgentLoop 每一轮 provider call 前执行 `manual_request` provider budget guard；估算范围包括 system prompt、manual history、当前消息历史、4096 输出上限和工具 schema 保守开销。超预算会记录 `writer.provider_budget` run event 和 `MANUAL_REQUEST_PROVIDER_BUDGET_APPROVAL_REQUIRED` failure bundle，然后阻断对应轮次的真实 provider call。
   - External research subtask 已有 `ExternalResearch` provider budget helper，估算 objective、query、context chars、tool overhead 和输出上限；超预算可转成 `RESEARCH_SUBTASK_PROVIDER_BUDGET_APPROVAL_REQUIRED` failure bundle，并记录 `writer.provider_budget` / `writer.error`，不记录原始 query 文本。
   - 预算门禁通过后，章节生成、Project Brain 和 manual request 会在真实 provider call 前记录 `writer.model_started` run event，包含 task、model、provider、stream、估算 token/cost 和 budgetDecision；该事件不记录 prompt 或模型输出。
   - Inspect 模式已读取 `writer.provider_budget` run event，展示 decision、approval_required、估算 token/cost、reasons 和 remediation。
   - Explore 模式章节生成失败于 `PROVIDER_BUDGET_APPROVAL_REQUIRED`、Project Brain 失败于 `PROJECT_BRAIN_PROVIDER_BUDGET_APPROVAL_REQUIRED`、manual request 失败于 `MANUAL_REQUEST_PROVIDER_BUDGET_APPROVAL_REQUIRED` 时，都会展示 provider budget 审批卡；作者批准后以前端批准凭证重试。
   - 后端只在批准凭证覆盖同一 task、model、estimated_total_tokens 和 estimated_cost_micros 时，把 `ApprovalRequired` 降级为 `Warn`，避免小预算批准误放行更大请求。
   - 已新增 eval：`writer_agent:provider_budget_requires_approval`、`writer_agent:chapter_generation_provider_budget_preflight`、`writer_agent:provider_budget_records_run_event`、`writer_agent:model_started_run_event`、`writer_agent:project_brain_provider_budget_preflight`、`writer_agent:project_brain_provider_budget_approval`、`writer_agent:manual_request_provider_budget_preflight`、`writer_agent:manual_request_provider_budget_approval`、`writer_agent:manual_request_multi_round_provider_budget`、`writer_agent:research_subtask_provider_budget`。
   - 剩余：把 external research budget helper 接进真实外部检索工具调用，并加入更细的批准历史展示。
5. 外部工具错误可恢复。（第一阶段已完成）
   - `agent-harness-core::ToolExecution` 已增加结构化 `remediation`。
   - unregistered tool、approval/permission denied、missing binary/resource、workspace unavailable、unknown tool/agent、doom loop 和普通 handler failure 都会给出机器可读 code 与恢复建议。
   - ToolExecution 失败可映射为 `WriterFailureEvidenceBundle`，记录为 `writer.error` run event；Inspector timeline 会把 `writer.error` 渲染为 `failure` 事件，并在 summary/detail 中保留 remediation。
   - Inspect 模式已有 failure 筛选和最新 failure 摘要，可查看 code、category、recoverable 和 remediation。
   - Research 子任务中的 provider/tool 失败可带 subtask id、kind、objective、artifact refs 和原始 tool execution 进入 failure bundle，并由 Inspector failure event 展示。
   - 已新增 eval：`writer_agent:external_tool_error_has_remediation`、`writer_agent:tool_remediation_records_failure_bundle`、`writer_agent:research_subtask_tool_failure_records_bundle`。
   - 剩余：把前端 inspector failure 视图从只读摘要升级为可操作恢复入口，并接入真实外部公开资料 provider/tool。

验收：

- `writer_agent:research_subtask_uses_isolated_workspace`（已完成）
- `writer_agent:research_subtask_outputs_evidence_only`（已完成）
- `writer_agent:diagnostic_subtask_denies_writes`（已完成）
- `writer_agent:post_write_diagnostics_recorded`（已完成）
- `writer_agent:post_write_diagnostics_after_accepted_operation`（已完成）
- `writer_agent:save_completed_links_post_write_diagnostics`（已完成）
- `writer_agent:provider_budget_requires_approval`（已完成）
- `writer_agent:chapter_generation_provider_budget_preflight`（已完成）
- `writer_agent:model_started_run_event`（已完成）
- `writer_agent:external_tool_error_has_remediation`（已完成）
- `writer_agent:tool_remediation_records_failure_bundle`（已完成）
- `writer_agent:research_subtask_tool_failure_records_bundle`（已完成）

### 11.4A OpenHarness harness 架构借鉴计划

结论：未发现 Forge 当前 harness 有必须推翻重写的致命弱点。Forge 已有 `agent-harness-core`、Writer Agent Kernel、`prepare_task_run`、TaskPacket、tool inventory、provider budget、context window guard、compaction、permission policy 和 run trace。OpenHarness 的价值在于补强运行前预检、compaction 可观测性、调用级权限上下文和只读工具并发，而不是把 Forge 改成通用 agent runner。

证据依据：

- OpenHarness `QueryEngine` 负责把 provider、tool registry、permission checker、hook、context window、auto-compact threshold 和 usage tracker 注入运行循环：`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/engine/query_engine.py:19`。
- OpenHarness dry-run 在不执行模型和工具的前提下解析配置、auth、skills、tools、commands、MCP 和 system prompt，并输出 `ready / warning / blocked`：`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/cli.py:333`、`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/cli.py:396`。
- OpenHarness compaction 结果包含 boundary marker、attachments、hook results、compact metadata，并记录 checkpoint；microcompact 会先清旧 tool result 节省 tokens：`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/services/compact/__init__.py:99`、`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/services/compact/__init__.py:187`、`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/services/compact/__init__.py:808`。
- OpenHarness permission checker 除工具名和读写外，还检查敏感路径、path rules、command deny patterns：`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/permissions/checker.py:18`、`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/permissions/checker.py:75`。
- OpenHarness 多 tool call 使用并发执行，同时用 `return_exceptions=True` 避免部分 tool_use 没有对应 tool_result：`C:/Users/Msi/Desktop/agent/OpenHarness-main/src/openharness/engine/query.py:815`。
- Forge 对应能力：`prepare_task_run` 已组装 metacognition gate、context pack、Story Impact、TaskPacket、Story Contract quality gate、tool inventory 和 system prompt：`C:/Users/Msi/Desktop/Forge/src-tauri/src/writer_agent/kernel/run_loop.rs:38`；`PermissionPolicy::authorize` 当前主要按 tool name、side effect 和 approval 判断：`C:/Users/Msi/Desktop/Forge/agent-harness-core/src/permission.rs:43`；`AgentLoop` 当前顺序执行 tool call：`C:/Users/Msi/Desktop/Forge/agent-harness-core/src/agent_loop.rs:307`；`CompactionResult` 当前只记录 summary / counts / token before-after：`C:/Users/Msi/Desktop/Forge/agent-harness-core/src/compaction.rs:27`。

任务：

1. 增加 `WriterRunPreflightReport`。（P1，已完成）
   - 基于现有 `WriterAgentKernel.prepare_task_run()`，新增不调用 provider、不执行 tool 的预检报告。
   - 输出 readiness：`ready | warning | blocked`。
   - blocked 条件第一版：metacognitive write gate 阻断、TaskPacket validate 失败、provider budget blocked、context window hard block、write-sensitive task 无审批凭证。
   - warning 条件第一版：provider budget approval required、context pressure 高、Story Contract quality gaps、Story Impact 高风险来源被截断、tool inventory 中关键工具被权限挡住。
   - report 字段：task、observation id、context pack summary、Story Impact budget summary、TaskPacket id/objective、Story Contract quality gate、tool inventory allowed/blocked counts、first-round provider budget、estimated input/output tokens、source refs、next actions。
   - 前端只在 Inspect / Explore 显示完整报告；Companion 默认只显示最少量“能否开始、为什么、下一步”。
   - 验收 eval：`writer_agent:run_preflight_ready_for_safe_planning`、`writer_agent:run_preflight_blocks_metacognitive_write`、`writer_agent:run_preflight_warns_provider_budget_approval`、`writer_agent:run_preflight_reports_story_impact_truncation`。
2. 增强 `agent-harness-core` compaction 可观测性。（P2，已完成：CompactionResult 扩展 kind/checkpoints/tokens_saved_by_tool_truncation/boundary_summary/recovery_level，新增 microcompact_tool_results()，3 tests）
   - 扩展 `CompactionResult`：`kind`、`checkpoints`、`tokens_saved_by_tool_truncation`、`boundary_summary`、`recovery_level`。
   - 在 full compaction 前增加 cheap microcompact：仅压缩旧 tool result 内容，保留最近 tool pair 和 tool_call_id，不调用 provider。
   - `AgentLoopEvent::Compaction` 携带 kind/checkpoint/token saved，不只输出 before/after/count。
   - Tauri run trace 只记录 token/count/phase，不记录正文或 tool output 原文。
   - 验收 tests：`agent_harness:microcompact_preserves_recent_tool_pairs`、`agent_harness:compaction_result_records_checkpoints`、`writer_agent:compaction_event_is_redacted`。
3. 增强调用级权限上下文。（P2，已完成：ToolInvocationContext + SENSITIVE_PATH_PATTERNS + DANGEROUS_COMMAND_PATTERNS + authorize_with_context()，4 tests）
   - 新增 `ToolInvocationContext`：tool name、side effect、requires approval、resolved path、command preview、source refs、task id。
   - `ToolExecutor` 在权限检查前从常见字段解析 `path` / `file_path` / `root` / `command`。
   - 内置 deny：credential path、workspace escape、危险 shell pattern；deny 优先级高于 allow/full access。
   - 保持 Tauri command audit 的显式 command 分类；这是 command 层，不替代 harness tool 调用级检查。
   - 验收 tests：`agent_harness:permission_denies_sensitive_paths`、`agent_harness:permission_denies_dangerous_commands`、`agent_harness:permission_context_overrides_full_access_for_credentials`、`writer_agent:tool_failure_bundle_includes_permission_context`。
4. 只读工具并发执行。（P3，未完成 — 计划下一轮落地）
   - 仅当同一轮多个 tool call 全部为 `ToolSideEffectLevel::Read` 或无共享写状态的 `ProviderCall` 时并发。
   - `Write`、`External`、approval-required、Project Brain 写入、正文写入、memory/canon/promise 相关工具继续串行。
   - 每个 tool_call 必须生成对应 tool result；单个工具失败不能取消同轮其他只读工具结果。
   - 事件顺序需可回放：start 可批量，end 按 tool_call 原始顺序归并。
   - 验收 tests：`agent_harness:read_only_tools_can_execute_concurrently`、`agent_harness:parallel_tool_failure_preserves_all_results`、`agent_harness:write_tools_remain_serial`。

不建议照搬：

- OpenHarness swarm / autopilot / plugin 全套不进入近期主路径。Forge 的产品瓶颈是长篇写作上下文质量、证据链路、作者审批、记忆边界和真实写作指标，不是通用多代理自动化规模。
- OpenHarness 的 dry-run 是 CLI/runtime 预览；Forge 应转译为 writer run preflight，不把普通作者暴露在 provider/MCP/plugin 细节里。
- OpenHarness 的 command-line permission hint 不直接搬到 UI；Forge UI 只显示与写作决策相关的下一步。

### 11.4B 传统 AI 写作竞品垂直化借鉴计划

结论：未发现 Forge 北极星的致命弱点。`C:/Users/Msi/Desktop/竞品` 下 18 个项目反而共同指向一个事实：长篇 AI 写作的核心难点不是“生成一章”，而是长期维护故事状态、伏笔承诺、作者声音、上下文证据和作者控制权。Forge 的方向应继续坚持 Cursor 式小说写作 agent，而不是退回“功能很多的 AI 写作 IDE”。

证据依据：

- `91Writing-main` 把“智能大纲生成与章节管理”列为核心能力：`C:/Users/Msi/Desktop/竞品/91Writing-main/README.md:38`。
- `AI_NovelGenerator-main` 同时列出多阶段章节生成、状态追踪、语义检索、知识库集成和自动审校：`C:/Users/Msi/Desktop/竞品/AI_NovelGenerator-main/README_zh-CN.md:16`、`C:/Users/Msi/Desktop/竞品/AI_NovelGenerator-main/README_zh-CN.md:17`、`C:/Users/Msi/Desktop/竞品/AI_NovelGenerator-main/README_zh-CN.md:18`、`C:/Users/Msi/Desktop/竞品/AI_NovelGenerator-main/README_zh-CN.md:20`。
- `ai-novel-lab-master` 自述完成 100 章、428,000+ 字，并强调大纲驱动和一致性维护：`C:/Users/Msi/Desktop/竞品/ai-novel-lab-master/README.md:38`、`C:/Users/Msi/Desktop/竞品/ai-novel-lab-master/README.md:41`、`C:/Users/Msi/Desktop/竞品/ai-novel-lab-master/README.md:43`、`C:/Users/Msi/Desktop/竞品/ai-novel-lab-master/README.md:51`。
- `AI-Novel-Writing-Assistant-main` 把知识与设定 RAG、写作风格控制、叙事一致性和完整章节/整本生成放在同一条链路：`C:/Users/Msi/Desktop/竞品/AI-Novel-Writing-Assistant-main/README.md:23`、`C:/Users/Msi/Desktop/竞品/AI-Novel-Writing-Assistant-main/README.md:24`、`C:/Users/Msi/Desktop/竞品/AI-Novel-Writing-Assistant-main/README.md:25`。
- `AINovel-main` 明确“灵感 -> 大纲 -> 逐章创作”，并维护世界线、人物线、感情线等全局线：`C:/Users/Msi/Desktop/竞品/AINovel-main/README.md:3`。
- `De-AI-Prompt-Enhancer-Writer-Booster-SKILL-main` 从真实作者文章提炼写作 DNA，并提供 AI 痕迹检测、量化风格约束和风格审计脚本：`C:/Users/Msi/Desktop/竞品/De-AI-Prompt-Enhancer-Writer-Booster-SKILL-main/README.md:15`、`C:/Users/Msi/Desktop/竞品/De-AI-Prompt-Enhancer-Writer-Booster-SKILL-main/README.md:23`。
- `dramatica-flow-main` 把因果链、情感弧线、伏笔系统和关系网络作为可追踪、可审计的工程流程：`C:/Users/Msi/Desktop/竞品/dramatica-flow-main/README.md:40`。
- `inkos-master` 强调自主写、审、改，但也声明人工审核门控确保作者掌控：`C:/Users/Msi/Desktop/竞品/inkos-master/README.md:22`。
- `kimi-book-writer-main` 以长上下文、多步推理生成长书，并保留 outline phase / chapter phase / checkpoint-resume 形态：`C:/Users/Msi/Desktop/竞品/kimi-book-writer-main/README.md:15`。
- `Morpheus-main` 明确不是“给一段文本”的助手，而是 AI-native writing studio；强调 multi-agent、chapter-first workflow、memory-aware writing、reviewable and traceable：`C:/Users/Msi/Desktop/竞品/Morpheus-main/README.md:5`、`C:/Users/Msi/Desktop/竞品/Morpheus-main/README.md:11`、`C:/Users/Msi/Desktop/竞品/Morpheus-main/README.md:12`、`C:/Users/Msi/Desktop/竞品/Morpheus-main/README.md:13`、`C:/Users/Msi/Desktop/竞品/Morpheus-main/README.md:14`。
- `novel_ai_writer` 把 200 万字长篇、三层记忆、伏笔网络图、角色管理、质量检测作为一体能力：`C:/Users/Msi/Desktop/竞品/novel_ai_writer/README.md:3`、`C:/Users/Msi/Desktop/竞品/novel_ai_writer/README.md:9`、`C:/Users/Msi/Desktop/竞品/novel_ai_writer/README.md:10`、`C:/Users/Msi/Desktop/竞品/novel_ai_writer/README.md:11`、`C:/Users/Msi/Desktop/竞品/novel_ai_writer/README.md:12`。
- `novel-pro-main` 的核心定义是可分流、可接管、可审计、可结算的小说工作流系统，并点名伏笔回收和写后结算问题：`C:/Users/Msi/Desktop/竞品/novel-pro-main/README.md:12`、`C:/Users/Msi/Desktop/竞品/novel-pro-main/README.md:41`、`C:/Users/Msi/Desktop/竞品/novel-pro-main/README.md:43`。
- `novelvel-master` 源码显示 pipeline 为 outline / write / review / polish，并有伏笔债务与回收功能入口：`C:/Users/Msi/Desktop/竞品/novelvel-master/orchestrator/pipeline_dsl.py:90`、`C:/Users/Msi/Desktop/竞品/novelvel-master/orchestrator/pipeline_dsl.py:91`、`C:/Users/Msi/Desktop/竞品/novelvel-master/orchestrator/pipeline_dsl.py:92`、`C:/Users/Msi/Desktop/竞品/novelvel-master/orchestrator/pipeline_dsl.py:97`、`C:/Users/Msi/Desktop/竞品/novelvel-master/app.py:165`。
- `Openwrite-main` 明确“长篇小说不是一次性 prompt”，而是立项、设定、滚动大纲、章节写作、审查、真相文件和 workflow 的长期生产线：`C:/Users/Msi/Desktop/竞品/Openwrite-main/README.md:19`、`C:/Users/Msi/Desktop/竞品/Openwrite-main/README.md:24`。
- `PlotPilot-2-main` 把自动驾驶生成、知识图谱管理、风格分析绑定到长篇创作平台：`C:/Users/Msi/Desktop/竞品/PlotPilot-2-main/README.md:7`。
- `vela-master` 定位为本地优先 AI 写作 IDE，覆盖 worldbuilding、outline、draft、rewrite、review 和本地 RAG：`C:/Users/Msi/Desktop/竞品/vela-master/README.md:20`。
- `webnovel-writer-master` 架构文档把“大纲即法律”和“发明需识别”写成系统规则，并拆出 Context / Data / 多维 Checker：`C:/Users/Msi/Desktop/竞品/webnovel-writer-master/docs/architecture.md:9`、`C:/Users/Msi/Desktop/竞品/webnovel-writer-master/docs/architecture.md:11`、`C:/Users/Msi/Desktop/竞品/webnovel-writer-master/docs/architecture.md:41`。
- `WenShape-main` 明确关注一致性、回溯性与可维护性：`C:/Users/Msi/Desktop/竞品/WenShape-main/README.md:24`。

产品判断：

- 这些竞品覆盖 outline / chapter generation / RAG / story bible / foreshadowing / style / workflow / audit / autopilot。Forge 不应该照抄功能矩阵；这些功能已经是传统 AI 写作工具的公共地板，不是长期护城河。
- Forge 的差异化应落在“agent 能维护一本书的活状态，并把每次行动变成可审查、可回放、可纠错的写作协作”。
- 竞品中“全自动写整本书”的方向只能作为压力测试参考，不能成为 Forge 默认体验。Forge 的北极星要求作者仍然掌控正文、记忆和承诺变更。

任务：

1. 旧稿接管向导。（P1）
   - 目标：让作者导入已有长篇稿件后，Forge 先读懂而不是马上改写。
   - 输入：章节文件、现有设定、人物表、作者样章。
   - 输出：只读 `Project Intake Report`，包含章节地图、主要角色、已识别 canon、open promises、style fingerprint、冲突/缺口、可信度和 evidence refs。
   - 边界：第一轮只生成候选和报告，不自动写 Story Bible / Canon / Promise Ledger。
   - UI：Explore / Inspect 显示完整报告；Companion 只显示“是否可开始协作、最危险的 3 个缺口、建议下一步”。
   - 验收 eval：`writer_agent:project_intake_reports_sources`、`writer_agent:project_intake_does_not_auto_write_memory`、`writer_agent:project_intake_flags_open_promises`。
2. 章节结算队列。（P1）
   - 目标：把保存后自动分析变成作者可处理的结算工作流，吸收 `novel-pro-main` / `novelvel-master` 中“写后审查/结算”的公共需求，但保持 Forge 的 approval 边界。
   - 输入：保存后的章节正文、Chapter Mission、Story Contract、Promise Ledger、Canon、post-write diagnostics、Story Impact Radius。
   - 输出：`Chapter Settlement Queue`，按 Canon update、Promise progress、Mission status suggestion、Style drift、Continuity risk 分组。
   - 边界：默认只生成 proposals；Canon/Promise/Mission 更新必须作者批准。
   - UI：Companion 默认只显示最高风险 3 项；Inspect 可看完整 evidence / source refs / diagnostics。
   - 验收 eval：`writer_agent:chapter_settlement_creates_reviewable_updates`、`writer_agent:chapter_settlement_requires_approval_for_ledger_writes`、`writer_agent:chapter_settlement_prioritizes_high_risk_promises`。
3. 改写影响预览。（P1）
   - 目标：作者接受改写前先看到“这次改动会影响哪些故事承诺和设定”。
   - 复用：Story Impact Radius、Context Pack budget summary、post-write diagnostics、operation lifecycle。
   - 输出：`Rewrite Impact Preview`，列出 impacted canon / promise / mission / style signals、风险等级、会被截断的证据、建议是否先 Planning Review。
   - 边界：preview 只读；不能因为 impact preview 自动修改正文或 ledger。
   - 验收 eval：`writer_agent:rewrite_impact_preview_is_read_only`、`writer_agent:rewrite_impact_preview_includes_bidirectional_story_edges`、`writer_agent:rewrite_impact_preview_warns_on_truncated_high_risk_sources`。
4. 作者声音守护。（P2）
   - 目标：把竞品常见“去 AI 味 / 风格检测”升级为 Forge 的 author voice guard，避免把风格变成通用模板。
   - 输入：作者样章、已采纳正文、被作者拒绝/编辑的 proposals、style memory feedback。
   - 输出：`AuthorVoiceSnapshot`，包含 rhythm、diction、pov、dialogue texture、sentence shape、taboo phrases、confidence、sample refs。
   - 行动：生成/改写前作为 context source；保存后输出 style drift diagnostics；只提出建议，不把“AI 味检测”当作唯一质量真相。
   - 验收 eval：`writer_agent:author_voice_guard_uses_author_samples`、`writer_agent:author_voice_guard_records_feedback_corrections`、`writer_agent:style_drift_diagnostic_links_evidence`。
5. 受监督章节冲刺。（P2）
   - 目标：允许作者批量推进章节，但每章都必须经过任务收据、预算预检、写后诊断和结算队列，避免传统 autopilot 失控。
   - 流程：select chapters -> preflight -> generate draft proposal -> author review -> durable save -> settlement queue -> next chapter。
   - 边界：默认不自动保存批量生成正文；没有作者批准不得进入下一章的 durable write。
   - 验收 eval：`writer_agent:supervised_sprint_stops_before_unapproved_save`、`writer_agent:supervised_sprint_carries_forward_settlement_feedback`、`writer_agent:supervised_sprint_records_receipts_per_chapter`。

不建议照搬：

- 不做传统功能矩阵补课：outline、character、world、RAG、audit、autopilot 不应该各自长成孤岛功能。
- 不把 chat box 重新变成主入口；任何新能力都应进入 Writer Kernel / Companion / Review / Inspect 的协作闭环。
- 不把自动抽取事实直接写入 Canon / Promise / Memory；旧稿接管和章节结算都先产出候选和证据。
- 不默认后台整本生成。批量生成只能是受监督章节冲刺，且必须保留 receipt、budget、approval、diagnostics、settlement。
- 不把“AI 味”当作单一审美裁判。风格守护必须以作者样章和作者反馈为来源，输出可解释证据。

### 11.4C 读者情绪补偿模型落地计划

来源：`C:/Users/Msi/Desktop/写作逻辑.md`。该文件提出网文的第一层底层链条是“现实缺憾 -> 代入主角 -> 制造压迫 -> 释放补偿 -> 获得爽感 -> 形成追读”，并进一步说明写作起点应先判断目标读者的心理缺口，而不是先写世界观；缺憾要通过主角承载、场景冲突化、爽点回收和递进升级来维持追读。文件也明确提醒这只能作为第一层逻辑，长篇稳定性还要叠加人物成长、世界观好奇、关系张力、节奏控制和平台生态判断。

结论：这套逻辑适合成为 Forge 的一层 `Reader Compensation Model`，不适合变成硬编码爽文模板。它应该帮助 agent 判断“本章为什么让读者想看下去”，而不是替代 Story Contract、Chapter Mission、Promise Ledger、Author Voice 或作者审美。

补充判断：用户提出的“制造情绪债务，然后兑现”可以作为 11.4C 的结构化升级，但不应照搬其“信用卡、赌场、抖音推送没有本质区别”的操控性表述。对 Forge 有价值的是“情绪期待的建立、延迟、累积、兑现和升级”这一生命周期模型；产品表达必须改写为作者可审查的连载节奏工具，而不是成瘾机制或硬套爽文套路。

当前状态：第一阶段已落地。Reader Compensation / Emotional Debt Lifecycle / Chapter Mission 情绪补偿字段 / Emotional Debt Ledger / Payoff Diagnostic / Planning Review / Context Pack / product metrics 的 schema、只读 review chain、context source、trajectory/product metric seed 和 24 个相关 eval 已进入 `agent-evals`，新增 `get_reader_compensation_review_chain` Tauri command 已按 read-only 分类。当前完成度应理解为“结构与护栏可验证”，不是“真实作者价值已证明”。

剩余：继续把 Reader Compensation 接入前端审查体验和真实写作 workflow 的可见 UI；用真实作者长会话校准 pressure/payoff 阈值、overdue risk、relationship soil coverage 和 interest mechanism repetition；避免 eval 只验证 schema 字段存在而不验证章节正文质量；所有长期 profile / ledger 写入仍保持 proposal + approval。

证据依据：

- 核心链条：`现实缺憾 -> 代入主角 -> 制造压迫 -> 释放补偿 -> 获得爽感 -> 形成追读`：`C:/Users/Msi/Desktop/写作逻辑.md:5`。
- 读者缺口包括缺钱、缺爱、缺尊严、缺权力、缺安全感、缺被认可、缺改变命运机会：`C:/Users/Msi/Desktop/写作逻辑.md:9`、`C:/Users/Msi/Desktop/写作逻辑.md:10`。
- 主角要承载读者心理投影，前期弱势/屈辱/贫穷/被误解/被背叛/被压制不是单纯卖惨，而是让读者识别熟悉情绪：`C:/Users/Msi/Desktop/写作逻辑.md:18`、`C:/Users/Msi/Desktop/写作逻辑.md:21`、`C:/Users/Msi/Desktop/写作逻辑.md:23`。
- 缺憾必须戏剧化、场景化、冲突化，而不是背景介绍：`C:/Users/Msi/Desktop/写作逻辑.md:25`、`C:/Users/Msi/Desktop/写作逻辑.md:32`。
- 爽点不是主角单纯赢了，而是回收前面缺憾、替读者讨回不甘：`C:/Users/Msi/Desktop/写作逻辑.md:34`、`C:/Users/Msi/Desktop/写作逻辑.md:37`、`C:/Users/Msi/Desktop/写作逻辑.md:46`。
- 补偿要递进，前期生存，中期尊严/地位/能力，后期权力/秩序/命运/世界级缺憾；每次胜利解决旧问题并打开更大问题：`C:/Users/Msi/Desktop/写作逻辑.md:48`、`C:/Users/Msi/Desktop/写作逻辑.md:53`、`C:/Users/Msi/Desktop/写作逻辑.md:56`。
- 作品差异来自补偿路径：正面逆袭、反套路逆袭、碾压、智斗、经营成长、改变规则、稳定关系等：`C:/Users/Msi/Desktop/写作逻辑.md:58`、`C:/Users/Msi/Desktop/写作逻辑.md:61`、`C:/Users/Msi/Desktop/写作逻辑.md:63`、`C:/Users/Msi/Desktop/写作逻辑.md:68`。
- 该逻辑边界：它只是第一层，还要叠加人物成长、世界观好奇、关系张力、节奏控制和平台生态判断：`C:/Users/Msi/Desktop/写作逻辑.md:83`。

产品判断：

- Forge 现有 Story Contract 更偏“作品承诺”；Chapter Mission 更偏“本章任务”；Promise Ledger 更偏“故事债务”。读者情绪补偿模型应补上“读者为什么在这一章获得心理回收”的判断层。
- 这不是让 Forge 只写爽文，而是让 agent 对商业网文、类型文、连载文增加读者心理视角；严肃文学、慢热文、强作者表达项目可以降低该模型权重。
- 该模型必须可解释、可关闭、可按项目类型调权重；不能在后台强行把所有作品改成打脸/暴富/升级模板。
- “人物关系是催生债务的土壤”值得吸收：情绪债务往往来自师徒认可、亲密误解、敌对羞辱、家族压迫、同伴背叛、群体轻视、自我亏欠等关系结构，而不只是抽象的缺钱、缺爱、缺尊严。
- “套路是债务的计息方式”只能转译为 `interest_mechanism`：误解延迟、压迫升级、公开羞辱、资源稀缺、承诺延后、反转遮蔽等机制会放大期待；Forge 只能识别与提示风险，不能用套路模板替作者决定剧情。

任务：

1. 新增 `ReaderCompensationProfile`。（P1）
   - 字段：`target_reader`、`primary_lack`、`secondary_lacks`、`protagonist_proxy_state`、`dominant_relationship_soil`、`pressure_mode`、`payoff_mode`、`payoff_path`、`escalation_ladder`、`forbidden_shortcuts`、`confidence`、`source_refs`。
   - 来源：Story Contract、作者输入、旧稿接管报告、章节保存结果、作者反馈。
   - 边界：只作为 story foundation candidate；写入项目长期设定必须经过作者 approval。
   - 验收 eval：`writer_agent:reader_compensation_profile_extracts_lack`、`writer_agent:reader_compensation_profile_requires_author_approval`、`writer_agent:reader_compensation_profile_preserves_project_tone`。
2. 新增 `EmotionalDebtLifecycle`。（P1）
   - 生命周期状态：`introduced`、`escalating`、`partially_paid`、`paid`、`rolled_over`、`abandoned`。
   - 字段：`debt_id`、`debt_kind`、`relationship_soil`、`introduced_by_scene`、`interest_mechanism`、`payoff_contract`、`payoff_window`、`current_state`、`overdue_risk`、`rollover_target`、`source_refs`。
   - 目标：把“压迫 -> 兑现”从单章二元判断升级成跨章节生命周期，支持小兑现、延迟兑现、旧债滚入新债和过期风险。
   - 边界：该生命周期只描述读者期待和证据链，不把作者表达压缩成留存算法；不得自动生成操控性文案，不自动改写正文。
   - 验收 eval：`writer_agent:emotional_debt_lifecycle_tracks_partial_payoff`、`writer_agent:emotional_debt_lifecycle_rolls_over_after_payoff`、`writer_agent:emotional_debt_lifecycle_flags_overdue_without_autowrite`。
3. 扩展 Chapter Mission 的情绪补偿字段。（P1）
   - 新增或派生：`reader_lack_this_chapter`、`relationship_soil_this_chapter`、`pressure_scene`、`interest_mechanism`、`payoff_target`、`payoff_path`、`next_lack_opened`。
   - 目标：每章不只知道“要发生什么”，还知道“压迫了什么缺口、回收什么不甘、结尾打开什么更大缺口”。
   - 边界：这些字段作为 Planning Review / Chapter Generation 的辅助 context，不自动覆盖作者原有 mission 文案。
   - 验收 eval：`writer_agent:chapter_mission_tracks_pressure_and_payoff`、`writer_agent:chapter_mission_opens_next_reader_lack`、`writer_agent:chapter_mission_tracks_relationship_soil`。
4. 新增 `Emotional Debt Ledger`。（P1）
   - 类型第一版：`dignity_debt`、`scarcity_debt`、`powerlessness_debt`、`safety_debt`、`affection_debt`、`recognition_debt`、`fate_debt`。
   - 字段：debt id、kind、introduced_at、relationship_soil、pressure_evidence、interest_mechanism、payoff_contract、payoff_status、expected_payoff_window、payoff_path、overdue_risk、rollover_target、risk、source_refs。
   - 与 Promise Ledger 的关系：Promise Ledger 记录故事承诺；Emotional Debt Ledger 记录读者心理承诺。两者可交叉引用，但不混成一个表。
   - 验收 eval：`writer_agent:emotional_debt_created_from_pressure_scene`、`writer_agent:emotional_debt_payoff_closes_with_evidence`、`writer_agent:emotional_debt_does_not_autowrite_promise`、`writer_agent:emotional_debt_tracks_interest_mechanism`。
5. 新增 `Payoff Diagnostic` 写后诊断。（P2）
   - 保存后检查：是否有压迫无补偿、补偿无前置压迫、爽点没有回收对应缺憾、关系土壤不足导致债务悬空、计息机制过度重复、一次性填满导致追读断点、补偿路径与项目气质冲突。
   - 输出：只读 diagnostic report + 可审查 story debt proposal，不自动改正文、不自动改 ledger。
   - 与 11.4B `Chapter Settlement Queue` 对接：Payoff Diagnostic 的高风险项进入 settlement queue 的 emotional / continuity 分组。
   - 验收 eval：`writer_agent:payoff_diagnostic_flags_pressure_without_payoff`、`writer_agent:payoff_diagnostic_flags_unearned_payoff`、`writer_agent:payoff_diagnostic_flags_overfilled_lack`、`writer_agent:payoff_diagnostic_flags_repetitive_interest_mechanism`。
6. Planning Review 增加读者补偿链审查。（P2）
   - Planning Review 输出：目标读者缺口、主角承载方式、人物关系土壤、本章压迫场景、计息机制、本章补偿点、补偿路径、下一层缺口、风险。
   - 该审查只读，不生成正文、不修改 Canon / Promise / Emotional Debt。
   - 验收 eval：`writer_agent:planning_review_reports_reader_compensation_chain`、`writer_agent:planning_review_keeps_compensation_read_only`、`writer_agent:planning_review_reports_emotional_debt_lifecycle`。
7. Context Pack 增加受预算约束的 `ReaderCompensation` source。（P2）
   - 优先级：ChapterGeneration / PlanningReview / InlineRewrite 高；ContinuityDiagnostic 中等；Project Brain 问答默认低。
   - 内容必须短：当前主缺口、关系土壤、当前章压迫/补偿目标、最近未回收 emotional debt、当前 lifecycle state、下一层缺口。
   - 边界：不要把完整读者心理分析塞进 prompt；只给模型当前行动必要的少量信号。
   - 验收 eval：`writer_agent:reader_compensation_enters_chapter_generation_context`、`writer_agent:reader_compensation_budget_stays_compact`。
8. 产品指标增加读者补偿链指标。（P3）
   - 指标：`pressure_to_payoff_ratio`、`unearned_payoff_count`、`open_emotional_debt_count`、`overdue_emotional_debt_count`、`payoff_path_diversity`、`interest_mechanism_repetition`、`relationship_soil_coverage`、`next_lack_handoff_rate`。
   - 用途：只用于 Inspector / trajectory / eval 校准，不向普通 Companion 默认暴露复杂指标。
   - 验收 eval：`writer_agent:product_metrics_tracks_payoff_chain`、`writer_agent:trajectory_exports_reader_compensation_metrics`。

不建议照搬：

- 不把“读者缺什么”变成唯一创作起点；作者表达、人物成长、世界观好奇和关系张力仍是并列基础。
- 不把所有项目默认判定为打脸、暴富、升级、复仇、甜宠；这些只是文件举例，不是 Forge 的固定类型枚举上限。
- 不把“爽点”做成机械打分器。Forge 应输出证据和风险，而不是替作者裁决审美。
- 不把“情绪债务”当成操控读者的成瘾机制；Forge 只能帮助作者管理期待和兑现节奏，不能把创作目标改写为最大化留存刺激。
- 不把套路库当成债务计息引擎；`interest_mechanism` 必须来自当前作品证据、人物关系和作者选择，而不是默认套模板。
- 不允许模型为了补偿链自动改写 Canon / Promise / Story Contract / Chapter Mission；所有长期记忆和正文写入仍走 proposal + approval。

### 11.4D Cache-Aware Context Spine 落地计划

来源：`C:/Users/Msi/Desktop/方案设想.md`。该文件提出把发往云端模型的上下文拆成“极寒层”静态前缀和“高热层”动态 buffer，用图谱节点承载长期记忆，用事件触发的后台压缩维持长会话一致性，并把 Rust 核心作为独立执行引擎。该方向和 Forge 当前的 Writer Kernel / Context Pack / Project Brain / Provider Budget / Compaction / Inspector trace 有明显交集，但必须从“缓存友好上下文脊柱”角度落地，不能包装成不可验证的底层推理黑箱。

结论：可以作为 Forge 的性能与上下文质量升级，但不应重建一套脱离 Writer Kernel 的新 agent runtime。11.4D 的目标是让每次模型调用更稳定地复用静态前缀、更清楚地区分项目长期事实和当前任务热信息，并把缓存命中、TTFT、token 成本和压缩事件纳入可回放 trace。

当前状态：第一阶段已落地。`ContextSpineLayer`、Context Spine fingerprint / stability report、FocusPack 切换、prompt-cache redacted event seed、provider usage `cached_tokens` 解析、event-driven compaction report、BYOK cache policy guard 和 cache stability product metric seed 已有后端结构与 eval 覆盖；`metrics.rs` 的 trend 聚合和 `run_loop.rs` 的 Context Spine run-event helper 已拆出子模块，`npm run check:architecture` 回到 14/14 files within budget。

剩余：真实 provider 的 streaming usage / TTFT 仍需更多端到端观测校准；Inspector/Companion 的 cache miss UI 还需要和真实前端交互打通；不得默认后台 keepalive；cache 优化不得压过 Story Contract、Chapter Mission、Canon、Promise、Reader Compensation 等 required context。

证据依据：

- OpenAI Prompt Caching 官方文档明确说明：缓存命中依赖 prompt 的 exact prefix matches，并建议把静态或重复内容放在 prompt 开头，把动态、用户特定内容放在末尾；缓存从 1024 tokens 以上请求开始生效，`usage.prompt_tokens_details.cached_tokens` 可用于观测命中量。来源：https://platform.openai.com/docs/guides/prompt-caching。
- OpenAI 文档还说明 in-memory prompt cache 通常在 5-10 分钟无活动后过期，最长可到约 1 小时；扩展缓存可配置更长保留，但不是所有模型都支持，且有数据保留边界。因此 `方案设想.md` 中“近乎零重复调用”和“保活机制”只能作为优化假设，不能作为默认产品承诺。
- Forge 现有 `agent-harness-core/src/context_pack.rs` 已有 `ContextPacker`、source budget、truncation report；`src-tauri/src/writer_agent/kernel/run_loop.rs` 已在准备 run 时组装 Context Pack、Story Impact、TaskPacket、Provider Budget 和 run event；`agent-harness-core/src/compaction/core.in.rs` 已有 compact / microcompact / overflow recovery 的基础报告。11.4D 应复用这些链路。
- Forge 当前 `agent-harness-core/src/prompt_cache.rs` 只有本地 fingerprint / hit-miss 统计，并未接入真实 provider response 的 `cached_tokens`、TTFT 或 cache key；`src-tauri/src/llm_runtime.rs` 的 chat/stream 路径也尚未记录 provider usage。这里是实际缺口。

产品判断：

- “极寒层 / 高热层”可以落地为 `ContextSpineLayer`，不是物理复制两套 prompt。目标是稳定 prefix 顺序、降低无意义前缀抖动、让动态内容靠后进入模型。
- 图谱节点状态机适合 Forge，但节点应绑定写作域：Book Contract、Chapter Mission、Canon entity、Promise、Emotional Debt、Author Voice、Research Source、Project Brain chunk，而不是泛化成“业务节点”。
- BYOK 成本防御要靠可观测预算和作者授权，不靠默认后台 keepalive。任何为了缓存保活而主动发起 provider call 的行为都必须显示审批、预算上限、可关闭，并记录 `writer.prompt_cache` / `writer.provider_budget` run event。
- 事件驱动压缩值得吸收，但“影子调用”必须是可审计 maintenance task；不能静默改写长期记忆，不能把未验证摘要写入 Canon / Promise / Reader Compensation / Project Brain。

任务：

1. 新增 `ContextSpineLayer` 与稳定 prompt 顺序。（P1）
   - 层级第一版：`FrozenPrefix`、`ProjectStablePrefix`、`FocusPack`、`HotBuffer`、`EphemeralScratch`。
   - `FrozenPrefix`：系统写作协议、工具边界、输出格式、北极星约束；要求同一 task/profile 下尽量字节稳定。
   - `ProjectStablePrefix`：Story Contract、Book Contract、项目风格、长期 Canon/Promise/Author Voice 的短摘要；只在作者批准长期设定变化后刷新。
   - `FocusPack`：当前章节相关 Project Brain / Story Impact / Chapter Mission / Reader Compensation slice；跟随焦点节点变更。
   - `HotBuffer`：当前用户指令、选中文本、cursor prefix/suffix、本轮草稿和最近反馈。
   - `EphemeralScratch`：工具试错日志、临时推理产物、失败诊断细节；默认不进入稳定前缀。
   - 验收 eval：`writer_agent:context_spine_keeps_static_prefix_first`、`writer_agent:context_spine_moves_hot_buffer_last`、`writer_agent:context_spine_does_not_drop_required_sources`。
2. 增加 prefix fingerprint 与 cache observability。（P1）
   - 为每次 provider call 记录：`frozen_prefix_fingerprint`、`project_stable_fingerprint`、`focus_pack_fingerprint`、`dynamic_tail_fingerprint`、`prompt_cache_key`、`estimated_prefix_tokens`、`estimated_dynamic_tokens`。
   - 在 OpenAI/OpenAI-compatible provider usage 可用时解析 `cached_tokens`、prompt tokens、completion tokens、total tokens；stream 路径至少记录 TTFT、duration、estimated tokens 和 provider 是否返回 usage。
   - 新增 `writer.prompt_cache` run event，只记录统计、hash 和 source refs，不记录原始 prompt。
   - 验收 eval：`writer_agent:prompt_cache_event_records_prefix_hashes`、`writer_agent:prompt_cache_event_redacts_prompt_text`、`writer_agent:provider_usage_parses_cached_tokens`。
3. 给 Context Pack 增加 cache stability report。（P1）
   - 在现有 context budget report 上追加：每个 source 所属 spine layer、是否会破坏稳定前缀、刷新原因、上次 fingerprint、当前 fingerprint。
   - Inspector 显示“为什么这次没有命中缓存”：静态前缀变化、ProjectStablePrefix 刷新、工具 schema 变化、focus 节点切换、dynamic tail 过大、provider 未返回 usage。
   - Companion 默认只显示成本/延迟风险摘要，不展示 hash、raw trace 或 provider 细节。
   - 验收 eval：`writer_agent:context_spine_reports_prefix_churn`、`writer_agent:inspector_shows_cache_miss_reason`、`writer_agent:companion_hides_prompt_cache_internals`。
4. 将 Project Brain 焦点节点接入 `FocusPack` 状态机。（P2）
   - 焦点节点类型：chapter、scene、canon entity、promise、emotional debt、research source、author voice sample。
   - 焦点切换只重建 `FocusPack` 和 `HotBuffer`，不刷新 `FrozenPrefix`；只有 Story Contract / 长期设定 approval 才刷新 `ProjectStablePrefix`。
   - 与 Story Impact Radius 对接：从焦点节点出发取双向影响半径，预算内输出短摘要和 source refs。
   - 验收 eval：`writer_agent:focus_shift_rebuilds_focus_pack_only`、`writer_agent:focus_pack_uses_story_impact_sources`、`writer_agent:project_stable_prefix_changes_only_after_approval`。
5. 把 compaction 从纯水位触发升级为事件触发。（P2）
   - 触发条件第一版：章节保存成功并完成 post-write diagnostics、Planning Review artifact 完成、Continuity Diagnostic artifact 完成、焦点节点强制切换、provider context pressure 高水位、tool failure recovery 完成。
   - 输出 `ContextSpineCompactionReport`：触发事件、输入 source refs、压缩目标层、摘要置信度、验证状态、是否允许进入 ProjectStablePrefix。
   - 压缩结果默认进入 `FocusPack` 或 `EphemeralScratch`；进入长期记忆仍走 proposal + approval。
   - 验收 eval：`agent_harness:event_driven_compaction_after_verified_save`、`writer_agent:compaction_report_keeps_source_refs`、`writer_agent:compaction_does_not_autowrite_long_term_memory`。
6. 建立 BYOK cache / budget policy。（P2）
   - 禁止默认后台 keepalive provider call。
   - 可选维护任务必须具备：作者显式开启、单次/每日 token 与金额上限、provider budget preflight、取消入口、run event、Inspector 可见。
   - 对 OpenAI extended prompt cache 这类保留策略，必须在 UI 文案中说明数据保留差异；默认使用 provider 的普通 in-memory 缓存行为。
   - 验收 eval：`writer_agent:cache_keepalive_requires_author_approval`、`writer_agent:cache_maintenance_uses_provider_budget`、`writer_agent:extended_cache_requires_explicit_policy`。
7. 性能指标进入产品质量闭环。（P3）
   - 指标：`cache_hit_token_ratio`、`static_prefix_churn_rate`、`focus_pack_rebuild_count`、`ttft_ms`、`provider_duration_ms`、`estimated_cost_saved`、`cache_miss_reason_count`。
   - Inspector 可按 session 展示趋势；trajectory export 输出 redacted JSONL；README / project-status 只写真实可测基线。
   - 用途：只用于性能和上下文稳定性诊断，不作为写作质量评分。
   - 验收 eval：`writer_agent:trajectory_exports_prompt_cache_metrics`、`writer_agent:product_metrics_tracks_cache_stability`。

不建议照搬：

- 不宣称 Forge 已实现“Causal Autoregressive Buffer”学术算法；文档中只能说借鉴静态前缀与动态尾部的工程思想。
- 不承诺“近乎零成本”或“近乎零重复调用”；只能以真实 `cached_tokens`、TTFT、token usage 和 provider 账单差异为准。
- 不默认发起保活调用；BYOK 下任何额外 provider 消耗都必须作者批准。
- 不把压缩摘要当成事实来源。摘要只是二级证据，长期记忆写入仍要指向原始章节、诊断 artifact 或作者批准记录。
- 不把 cache 优化凌驾于写作上下文质量。Story Contract、Chapter Mission、Canon、Promise、Reader Compensation 等 required context 不能为了命中缓存被省略。

### 11.5 目标与信念：自主性的灵魂

目标：Forge 的自主性来自“它知道这本书要守什么、当前章节要完成什么、哪些承诺不能忘”，不是来自泛用人格或聊天式 persona。

现有优势：

- Story Contract 已有 Missing / Vague / Usable / Strong。
- Chapter Mission 已有 mission、must_include、must_not、expected_ending、status。
- Promise Ledger 已有 plot promise、emotional debt、object whereabouts、character commitment、mystery clue、relationship tension。
- TaskPacket 已把 context sources 转成 beliefs 和 required context。

任务：

1. Chapter Mission 状态机升级。
   - 第一阶段已完成：状态机已支持 draft、active、completed、drifted、blocked、needs_review、retired；旧 `in_progress` 会兼容归一为 `active`。
   - 保存章节后会基于 Result Feedback 将 draft/active/needs_review 迁移为 completed、active、drifted 或 needs_review，并把 `source_ref` 指向对应 `chapter_save` 结果；迁移会写入 creative decision 作为证据记录。
   - blocked / retired 是作者显式状态，不会被保存观察自动覆盖，也不会继续生成普通 mission save-gap proposal；写入 blocked 必须提供具体 `blocked_reason`，写入 retired 必须提供 `retired_history`，Companion 保存前也会前置校验。
   - Companion Chapter Mission 下拉已暴露 draft / active / review / done / drift / blocked / retired。
   - 剩余：把自动迁移改成可审查 suggestion/approval UI，而不是当前后端直接校准。
2. Belief conflict explanation。
   - 当 Story Contract、Mission、Canon、Promise、Project Brain 互相冲突时，必须说明冲突来源和置信度。
   - 不能静默选择一个来源覆盖另一个来源。
   - 第一阶段已完成：新增 `writer_agent::belief_conflict` 后端解释器，可从 WriterMemory 的 Story Contract / Chapter Mission / Canon / Promise Ledger 和 Project Brain chunk evidence 聚合来源标注、reference、snippet、confidence、signals，并输出 `ForbiddenReveal` 与 `FactContradiction` 两类解释。
   - `writer_agent:belief_conflict_explains_sources` 已覆盖“合同/任务/伏笔要求延后揭示寒玉戒指来源，但 Project Brain chunk 说已揭示，同时 Canon 仍记录来源未知”的场景；eval 要求 guard conflict 同时带 Story Contract / Chapter Mission / Promise Ledger / Project Brain 证据，fact conflict 同时带 Canon / Project Brain 证据。
   - 边界：当前只解释冲突并给 resolution hint，不自动改写正文、不自动修改 ledger、不在 UI 中仲裁哪个来源胜出；后续需要接入 planning/review surface 和作者审批流。
3. Promise payoff planner。
   - 在章节规划阶段提示本章附近应回收、延后或避免打扰的 promise。
   - 已 resolved promise 继续保持 quiet。
   - 第一阶段已完成：新增 `writer_agent::promise_planner` 后端规划器，按 current chapter / expected payoff / mission overlap / local draft overlap / ledger priority / promise kind 排序 open promises，并输出 `PayoffNow`、`PreparePayoff`、`Defer`、`AvoidDisturbing` 四类只读建议。
   - Planning / Review prompt 的 Ledger 区已追加 `Promise Payoff Plan`，用于提示当前章附近该回收、准备、延后或避免打扰的 promise；该计划只读，不生成 typed write operation，也不修改 Promise Ledger。
   - `writer_agent:promise_payoff_planner_prioritizes_nearby_debts` 已覆盖当前章 payoff 应压过远期高优先级 promise、mission `must_not` 命中的 promise 应标为 AvoidDisturbing、远期 promise 应 Defer、resolved promise 不进入 planner。
   - 边界：当前 planner 只进入 planning/review 上下文和 eval；尚未做前端专用卡片、作者一键 resolve/defer/abandon 操作，也未接入自动章节生成前的硬门禁。
4. Goal drift detector。
   - 章节生成或重写后检测是否偏离 mission / must_not。
   - 偏离时输出 story debt proposal，而不是自动改写。
   - 第一阶段已完成：accepted inline/proposal operation 在 durable-save 后会复用 post-write diagnostics，把 mission / must_not 偏离转换为可审查 `ChapterMission` proposal，并进入 story debt snapshot；该路径只记录诊断和 annotation operation，不回滚、不自动改写已保存正文、不直接改 Chapter Mission。
   - `writer_agent:goal_drift_creates_story_debt` 已覆盖“作者接受 ghost/inline 操作后保存，新增正文触碰本章 must_not，post-write report 记录 ChapterMissionViolation，pending proposal 和 mission story debt 同步出现”的场景。
   - 边界：当前完成的是 accepted operation durable-save/post-write 链路；章节生成 provider 输出写入前硬门禁、批量生成保存后的 drift UI、以及前端专用审批卡仍未完成。
5. 不引入泛用 persona 系统。
   - 所谓“自我”只落到写作目标、作者偏好、项目记忆、反馈历史。
   - 第一阶段已完成：TaskPacket belief source 会保留 `ProjectBrief`、`ChapterMission`、`PromiseSlice`、`CanonSlice`、`DecisionSlice`、`AuthorStyle` 等写作基础来源，generic identity 不作为 foundation 字段进入任务包。
   - `writer_agent:generic_persona_not_used_as_foundation` 已覆盖 PlanningReview 任务包的 beliefs / required context 均来自写作基础来源，并拒绝 `persona` / `personality` / `chatbot identity` / `agent_identity` 这类泛用 persona 基座。
   - 边界：系统提示仍会说明 Forge 是写作 Agent / 创作伙伴，这是产品角色语义；禁止的是把泛用人格当成记忆、目标和信念的基础来源。

验收：

- `writer_agent:mission_state_transition_requires_evidence`（已完成）
- `writer_agent:mission_blocked_retired_not_auto_calibrated`（已完成）
- `writer_agent:belief_conflict_explains_sources`（已完成第一阶段：后端解释 + eval）
- `writer_agent:promise_payoff_planner_prioritizes_nearby_debts`（已完成第一阶段：只读 planner + Planning/Review prompt + eval）
- `writer_agent:goal_drift_creates_story_debt`（已完成第一阶段：accepted operation durable-save 后写诊断进入 proposal / story debt）
- `writer_agent:generic_persona_not_used_as_foundation`（已完成第一阶段：TaskPacket foundation 来源化 + eval）

### 11.6 环境与反馈：赖以生长的土壤

目标：把 Forge 的运行过程从“最近 snapshot”升级为“可回放、可解释、可调参”的写作实验环境。

可借鉴机制：

- ML Intern 的 event append 和 trace message append。
- ML Intern 的 Claude Code JSONL trace viewer 格式和 redaction warning。
- Claw Code 的 failure evidence bundle。
- CowAgent 的 conversation store 将 tool result 合并到 tool_use 展示，避免 UI 噪音。

任务：

1. 实现 append-only WriterRunEventStore。
   - 当前状态：第一阶段已完成。`writer_run_events` 已持久化 observation、context_pack_built、model_started、tool_called、task_packet_created、proposal_created、operation_lifecycle、approval_decided、feedback_recorded、error、save_completed、memory_candidate_created，并以 `writer.run_event` 进入 trajectory JSONL。
   - 每条事件已有 seq、ts、project_id、session_id、task_id、source_refs。
   - `writer.memory_candidate_created` 只记录已经进入 proposal queue 的可审查记忆候选，包含 slot、operationKinds、evidenceCount、requiresAuthorReview=true、writesLedgerImmediately=false；它不改变长期记忆写入门禁。
   - `writer.approval_decided` 记录 WriterOperation 审批成功/拒绝、operationKind、affectedScope、approvalSource、actor、surfacedToUser 和 reason，用于回放写操作为什么被允许或拒绝。
   - `writer.context_pack_built` 记录真实 writer 工作流和章节生成上下文组装的 task、sourceCount、totalChars、budgetLimit、wasted、truncatedSourceCount、sourceReports、required 标记和来源 refs；事件只存预算/来源摘要，不写入正文上下文原文。
   - `writer.model_started` 记录章节生成、Project Brain 和 manual request 在预算门禁通过后、真实 provider call 前的 task、model、provider、stream、估算 token/cost、budgetDecision 和来源 refs；事件不记录 prompt 或模型输出。
   - `writer.tool_called` 记录 manual AgentLoop 工具调用 start/end 的 toolName、phase、success、durationMs、inputKeys、inputBytes、outputBytes、error 和 remediationCodes；事件不写入参数值或工具输出原文。`ToolExecutor` 已新增可选 audit sink，Tauri `writer_tool_audit_sink` 可把直接 executor 调用映射为同一类 run event，已有 eval 证明直接 executor 调用不会泄露 raw args / output；后续是把真实外部研究等新入口挂接到该 sink。
2. Inspector timeline。
   - 当前状态：第一阶段已完成，前端只读 Inspect 切片已完成。
   - 已新增 `src-tauri/src/writer_agent/inspector.rs`，从 `WriterAgentTraceSnapshot` 派生 Inspector timeline 和 Companion-safe summary。
   - 默认 Companion summary 不显示 task packet、raw run event、operation lifecycle 等内部 trace，只显示产品健康摘要和少量 proposal 摘要。
   - Debug/inspector 后端视图可查看 observation、task packet、task receipt、task artifact、proposal、proposal context budget、feedback、operation lifecycle、run event、context recall、product metrics。
   - 已新增 Tauri read-only commands：`get_writer_agent_inspector_timeline`、`get_writer_agent_companion_timeline_summary`。
   - 已新增前端 `src/components/WriterInspectorPanel.tsx` 和 `Inspect` 模式，读取 `get_writer_agent_inspector_timeline` / `get_writer_agent_trace`，支持 failure、save_completed、subtask、task_receipt、task_artifact、run_event、task_packet、operation_lifecycle、context_recall、product_metrics 筛选，并在 proposal timeline card 展开 context budget sourceReports。
   - Inspect 模式右侧摘要已覆盖 provider budget、latest failure、latest save、latest receipt、latest artifact、save-to-feedback latency、post-write diagnostics、proposal context budgets 和 context source trends；Context Pressure 已展示整体 coverage、truncated/dropped 事件数、受压 source、每源覆盖率条和最近截断/丢弃原因。
   - Inspect failure card 和 Latest Failure 摘要已根据 failure code/category/remediation/detail 提供 `Review budget`、`Review save`、`Review receipt`、`Review task packet`、`Review run events`、`Review context`、`Show failures` / `Open failures` 排查跳转；这是只读导航入口，不会自动重试 provider call 或执行写操作。
   - 已新增 eval：`writer_agent:inspector_timeline_hides_from_default_companion`。
   - 已新增静态检查：`npm run check:p2` 验证默认 write Companion 不读取 inspector timeline，内部时间轴、save_completed、task_receipt、task_artifact、save-to-feedback latency、proposal context budget drilldown 和 failure recovery action chips 只进入 dedicated Inspect mode。
   - 剩余：更细的 receipt mismatch / provider / tool 失败恢复动作仍需接真实操作入口；外部 trace viewer compatible export 已完成第一阶段。
3. 轨迹导出升级。
   - 保留当前 Forge writer trajectory schema。
   - 当前状态：redaction warning / local-only 第一阶段已完成。
   - `WriterTrajectoryExport` 已增加 `redaction_warning` 和 `local_only` 字段，明确导出可能包含 manuscript text、project memory、author feedback、prompts、tool results、internal reasoning metadata。
   - Claude Code / HF Agent Trace Viewer compatible JSONL 第一阶段已完成：保留 Forge 原 JSONL schema，同时派生 `traceViewerJsonl`，每行带 `type`、`message`、`uuid`、`parentUuid`、`sessionId`、`timestamp`、`forgeEventType` 和原始 Forge event bridge；Settings 可分别导出 Forge trace 与 Trace Viewer JSONL，本地导出不上传。
   - 导出前必须有 redaction warning；默认本地，不默认上传。（已完成）
   - 已新增 eval：`writer_agent:trajectory_export_has_redaction_warning`。
   - 已新增 eval：`writer_agent:trajectory_trace_viewer_export`。
   - `writer.metacognition` 已进入 Forge trajectory JSONL，记录风险等级、建议动作、置信度、原因和 remediation；用于回放 agent 为什么应继续、降级、澄清、诊断或阻断写入。（已完成第一阶段）
4. Product metrics 趋势。
   - 当前已有 acceptance rate、ignored suggestion rate、promise recall、canon false-positive、mission completion、durable save、save-to-feedback latency。
   - 多 session 第一阶段已完成：`productMetricsTrend` 从持久化 `writer_run_events` 按 session 聚合最近/上一 session 的 save-to-feedback latency、总体平均值、delta、采纳率、durable save 成功率和 manual ask 转 operation 率；同时从 `writer.context_pack_built` run events 聚合 context pack count、requested/provided chars、coverage、truncated/dropped source counts、overall/recent/previous coverage 和 delta。Inspect Session Trend 展示 `latency`、`ctx`、`ctx packs`、`trunc`、`drop` 等趋势，trajectory 导出 `writer.product_metrics_trend`。
   - 连续写作第一阶段已完成：`writer_agent:continuous_writing_fixture_20_chapters` 使用两段 session / 临时 SQLite 持久化 run event，验证 20 章保存观察、作者反馈、durable save、story debt 和 `writer.product_metrics_trend` 能在同一条轨迹中回放。
   - `promise_recall_hit_rate` 已修正为同时识别 context recall 里的 `PromiseLedger` 和旧 `PromiseSlice` 来源，避免当前证据映射下 promise recall 被错误计为 0。
   - 下一步增加真实作者项目 fixture 对照和更长历史窗口校准。
5. Continuous writing fixture。
   - 当前状态：第一阶段已完成。新增 `writer_agent:continuous_writing_fixture_20_chapters`，不只测单函数输出，而是通过 `WriterAgentKernel::observe`、`create_llm_ghost_proposal`、durable save、`apply_feedback`、trace snapshot 和 trajectory export 走真实链路。
   - 覆盖连续 20 章的设定、伏笔、物件、角色关系、任务漂移、作者采纳/编辑/拒绝反馈。
   - 边界：当前仍是合成作者项目 fixture；它证明工程链路能跨章节回放和计算指标，不证明真实作者数据上的阈值、召回率和误报率已经达标。

验收：

- `writer_agent:append_only_run_event_store`（已覆盖单次 run event append、SQLite replay、trajectory export）
- `writer_agent:run_failure_evidence_bundle`（已覆盖 writer.error run event 和 trajectory export）
- `writer_agent:run_event_store_replays_timeline`
- `writer_agent:inspector_timeline_hides_from_default_companion`（已完成）
- `writer_agent:trajectory_export_has_redaction_warning`（已完成）
- `writer_agent:trajectory_trace_viewer_export`（已完成）
- `writer_agent:memory_candidate_created_run_event`（已完成）
- `writer_agent:operation_approval_decided_run_event`（已完成）
- `writer_agent:context_pack_built_run_event`（已完成）
- `writer_agent:model_started_run_event`（已完成）
- `writer_agent:tool_called_run_event`（已完成）
- `writer_agent:tool_executor_audit_records_tool_called`（已完成）
- `writer_agent:product_metrics_multi_session_trend`（已完成）
- `writer_agent:product_metrics_context_pressure_trend`（已完成）
- `writer_agent:continuous_writing_fixture_20_chapters`（已完成第一阶段：合成 20 章连续写作 product fixture）
- `writer_agent:metacognitive_snapshot`（已完成第一阶段：trace-derived risk/action、Inspector event、trajectory export）
- `writer_agent:metacognitive_gate_blocks_write_run`（已完成第一阶段：高风险 trace 下阻断 GhostWriting/InlineRewrite/ChapterGeneration）
- `writer_agent:metacognitive_gate_blocks_approved_operation`（已完成第一阶段：高风险 trace 下阻断已审批正文写入 operation）
- `writer_agent:metacognitive_gate_allows_recovery_operation`（已完成第一阶段：高风险 trace 下保留 mission calibration 等恢复性 ledger 操作）
- `writer_agent:metacognitive_recovery_run_uses_read_only_task`（已完成第一阶段：专用恢复 run 只能走只读 Planning Review / Continuity Diagnostic 边界，并使用 `metacognitive_recovery` provider budget 分类）

### 11.7 不建议照搬的机制

明确不做：

- 不把 Forge 主路径做成多渠道聊天 gateway。
- 不把 Hermes / OpenClaw 的 always-on gateway / cron 直接接进写作主循环。
- 不允许后台定时任务自动写永久记忆。
- 不允许子代理直接写正文、Canon、Promise、Style、Story Contract、Chapter Mission。
- 不用泛用 agent persona 替代 Story Contract / Chapter Mission。
- 不把 Project Brain 变成没有来源、没有版本、没有质量门槛的大杂烩向量库。
- 不把 `code-review-graph` 的 Tree-sitter、函数调用图、测试覆盖缺口硬搬到小说写作；Forge 只能借鉴 impact-radius 上下文组装纪律，并转译为故事证据图。

原因：

- Forge 的产品价值是小说共同作者，不是万能自动化入口。
- Hermes cron 的 `skip_memory=True` 注释已经说明后台 system prompt 可能污染长期表示；这对 Forge 是强风险信号。
- Forge 当前最大风险仍是聊天面板心智、记忆污染和 eval 自我安慰，而不是工具不够多。

### 11.8 P4 推荐执行顺序

1. WriterRunEventStore。（第一阶段已完成）
   - 已有 append-only event store、SQLite replay 和 trajectory export，因为它是后续 inspector、失败证据、真实 eval 的底座。
2. Planning / Review 只读模式。（第一阶段已完成）
   - 已把“想清楚”从聊天主路径剥离出后端任务类型，具备只读工具边界和 Story Foundation 上下文。
3. WriterTaskReceipt + failure evidence bundle。（第一阶段已完成）
   - ChapterGeneration / BatchGeneration 已有 receipt 和 failure bundle；ContinuityDiagnostic 已有只读 receipt、`writer.task_receipt` run event、diagnostic_report `writer.task_artifact`、trajectory 回放和 Inspector `task_receipt` / `task_artifact` 事件展示；PlanningReview 已有只读 receipt、planning_review_report `writer.task_artifact`、trajectory 回放和 Inspector `task_artifact` 事件展示；更多失败分类接入和可操作恢复入口仍未完成。
4. Memory correction / reinforcement。（第一阶段已完成）
   - 已让作者对记忆候选的采纳/拒绝改变后续同 slot 候选行为；纠错优先于强化。
5. Project Brain knowledge index / graph。（第一阶段已完成）
   - 已先做来源可解释：index / node / shared-keyword graph / path guard / eval / Graph 页 Brain 模式已落地；第一层 graph filtering、search、source detail、source history、read-only source revision compare、source revision restore 和 reference/back-reference navigation 已落地；embedding provider registry / profile / input limit / batch status / retry policy / compatibility fallback 第一阶段已落地；chunk source_ref / source_revision / source_kind / chunk_index / archived metadata 已落地。
   - 剩余：provider-specific embedding 质量校准、真实 Story Bible / 章节数据召回质量验证，以及把索引 revision 恢复和正文/Story Bible 恢复整合到同一可审查恢复体验。
6. Story Impact Radius Context Pack。（第一阶段已完成：类型 + 故事图 + 双向遍历 + 同层高价值节点优先预算排序 + 真实 reached-drop 截断统计 + run event 关联 + TaskPacket 摘要 + prompt ContextPack Source + 12 eval）
   - 已把 impact radius 作为只读、受预算约束的 `StoryImpactRadius` ContextSource 接入默认写作 prompt；目标是让 Forge 知道“这次写作动作会影响哪些故事事实”，而不是只按相似文本和固定 ledger 顺序塞上下文。
   - 当前仍不自动写 Canon / Promise / Mission。剩余重点转为真实作者长会话校准：impact source 是否提升伏笔召回、降低 canon false-positive、减少 mission drift 漏报。
7. Isolated research / diagnostic subtask workspace。（第一阶段已完成）
   - 已建立只读/隔离/evidence-only 后端边界；subtask started/completed run event、trajectory export 和 Inspect Subtasks 筛选已落地；真实 run loop 自动调度和外部检索工具仍未完成。
8. Inspector timeline + trajectory export upgrade。（第一阶段已完成）
   - 已有后端 Inspector timeline / Companion-safe summary / redaction warning / local-only export 标记；前端 Inspect 模式已覆盖只读 timeline 筛选、failure、task_receipt、task_artifact、provider budget、save_completed、save-to-feedback latency、multi-session metric trend、proposal context budget drilldown、post-write diagnostics、当前 context pressure、持久化 per-session context pressure trend 和 failure recovery 排查跳转；Forge trajectory 已可额外导出 Claude-Code-style / HF Agent Trace Viewer compatible JSONL。
9. Provider call budget。（第一阶段已完成）
   - 已有 token/cost estimation、approval-required/warn/blocked 决策和 remediation；章节生成 provider call 前置门禁、`writer.provider_budget` run event、Explore 审批卡、已批准 budget 前端传递和后端覆盖校验已接入；Project Brain chat answer provider call 已有后端 preflight / run event / failure bundle；manual request AgentLoop 每轮 provider call 已有后端 budget guard / run event / failure bundle；external research subtask 已有 provider budget report / failure bundle helper 和 run event 覆盖；Project Brain/manual retry UI 和后端批准凭证覆盖校验已接入；真实外部检索工具调用接入和 external research 审批 UI 仍未完成。
10. Post-write diagnostics。（保存观察 + accepted operation 路径第一阶段已完成）
   - 保存观察会生成 post-write diagnostic report，写入 run event、trace snapshot 和 trajectory；accepted inline/proposal text operation durable-save 路径也会带保存后正文复跑 diagnostics，并输出 proposal / operation 级 source refs；Companion Audit 页已能查看最近报告；Inspect 模式已有最近 post-write diagnostics 摘要、save_completed 专用筛选、save-to-feedback latency 和多 session latency 趋势；`writer.save_completed` 已串联 save result、post-write report id 和诊断计数。剩余是真实连续写作 fixture 校准。
11. External tool remediation。（第一阶段已完成）
   - ToolExecution 失败结果已有结构化 remediation，并已映射进 `WriterFailureEvidenceBundle` / `writer.error` run event / Inspector `failure` event；Research 子任务 tool/provider 失败已有 subtask 证据包覆盖；真实外部公开资料 provider/tool 集成仍未完成。
12. OpenHarness harness 借鉴落地。（第一阶段完成：P1 WriterRunPreflightReport + P2 compaction 可观测性 + P2 调用级权限上下文；P3 只读工具并发未完成）
   - 第一优先级：`WriterRunPreflightReport`（已完成：Kernel::preflight() 覆盖 metacognitive gate、context budget、Story Impact、contract quality、tool inventory、token estimate → ready/warning/blocked，4 eval）。
   - 第二优先级：compaction metadata / microcompact（已完成：CompactionResult 扩展 + microcompact_tool_results()，3 tests）。
   - 第三优先级：调用级权限上下文（已完成：ToolInvocationContext + SENSITIVE_PATH_PATTERNS / DANGEROUS_COMMAND_PATTERNS + authorize_with_context()，4 tests；ToolExecutor 集成待下一轮）。
   - 第四优先级：只读工具并发（未完成 — 计划下一轮落地）。

### 11.9 P4 完成定义

短期完成：

- 只读规划/审稿模式可用，且权限测试证明不能写正文和记忆。
- ChapterGeneration / BatchGeneration 长生成有 TaskReceipt；ContinuityDiagnostic 长诊断有只读 receipt、diagnostic_report artifact run event、写入类 artifact guard、trajectory 回放和 Inspector receipt/artifact 筛选。
- 关键章节生成失败不是字符串，而是可分类的证据包；更多工具/反馈失败路径仍需接入。
- RunEventStore 可以回放一次 writer run。
- 作者对记忆候选的纠错会压制后续同 slot 抽取，采纳会强化同 slot 候选；记忆候选进入 review queue 时会写 `writer.memory_candidate_created` run event，但不会绕过作者确认直接写 ledger。
- Project Brain knowledge index / graph 有后端 schema、构建函数、路径守卫和 Graph 页 Brain 模式；第一层节点类型过滤、来源/关键词/摘要/关系/revision 搜索、source detail、source history、read-only source revision compare、source revision restore、邻接高亮和 reference/back-reference 导航已完成。当前 source revision restore 只恢复 Project Brain 索引 active/archived 状态，不回写正文；更深的跨引用操作、真实 Story Bible/章节结果来源校准和统一正文恢复体验仍未完成。
- Research / Diagnostic 子任务有隔离 artifact workspace、tool policy、evidence-only 结果边界、started/completed run event、Inspector subtask timeline 和 Research tool/provider 失败证据包；真实外部公开资料 provider/tool 调度仍未完成。
- Inspector timeline 有后端视图和前端 Inspect 只读调试面板，默认 Companion summary 已证明不暴露内部 trace；Inspect 已有 failure 恢复排查跳转、task_receipt/task_artifact 专用筛选、save_completed 专用筛选和 save-to-feedback latency 摘要；trajectory export 有 redaction warning、local-only 标记和 Trace Viewer compatible JSONL 导出。
- Provider budget 有后端估算、approval-required 决策和 remediation，且章节生成、Project Brain chat answer、manual request AgentLoop 每轮 provider call 都已有前置门禁、Explore UI approval surface 和批准凭证覆盖校验；ExternalResearch 已有 subtask provider budget report / failure bundle helper 和 run event 覆盖。Project Brain / manual request 预算失败会展示审批卡并用前端批准凭证重试。尚未强制接入所有真实 provider call，且真实外部检索工具调用和 external research 审批 UI 仍未完成。
- 保存观察路径和 accepted operation durable-save 路径已有 post-write diagnostic report、run event 和 trajectory export；Companion Audit UI 和 Inspect 模式已展示最近报告；`writer.save_completed` 已把保存结果与 post-write report 串联，并在 Inspect 中有专用筛选/摘要。
- 通用 ToolExecution 失败已有 remediation，并已映射到 WriterFailureEvidenceBundle 和 inspector failure surface；Inspect 模式已有 failure 筛选/摘要和恢复排查跳转入口；Research 子任务失败路径已有后端证据包，真实外部公开资料 provider/tool 集成仍未完成。
- OpenHarness harness 借鉴短期完成定义（3/4）：Writer run 能在不调用 provider / tool 的情况下产出 readiness report ✅；compaction event 有 checkpoint 和 token savings ✅；PermissionPolicy 能基于调用参数拒绝敏感路径和危险命令 ✅（ToolExecutor 集成待下一轮）；只读工具并发只在可证明无写副作用时启用 ❌。

中期完成：

- 作者纠错和采纳能改变记忆置信度。
- Project Brain 有可视化 index / graph 和来源解释。
- 子任务研究结果只能作为 proposal evidence 进入主循环，并在 inspector 中可回放其 artifact 与 tool boundary。
- Inspector 能解释为什么 agent 这么写、引用了什么、错在哪里，并提供可用前端调试面板。

长期完成：

- Forge 能在连续真实写作项目中证明：更少遗忘伏笔、更少设定污染、更少无效打扰、更高采纳率、更短返工路径。
- 如果这些产品指标没有改善，P4 不算完成。

## 12. 风险清单

### 风险 1：功能越做越多，但 agent 不像一个统一主体

控制：

- P0 必须先完成统一 run loop。
- 禁止新增绕过 kernel 的 AI command。

### 风险 2：右侧面板重新变成噪音中心

控制：

- 默认只显示 3-5 件最高价值事项。
- trace / tool / debug 信息隐藏到 inspector。

### 风险 3：记忆污染

控制：

- 长期记忆写入必须有质量门槛。
- Canon / Promise 变更必须 approval 或强证据。
- 保存结果先变 candidate，不默认永久化。

### 风险 4：eval 变成自我安慰

控制：

- 增加多章场景 eval。
- 增加误报、采纳率、重复打扰率等产品指标。
- eval fixture 必须模拟真实长篇问题，不只测函数输出。

### 风险 5：架构文件继续膨胀

控制：

- P2 拆分必须排期。
- 新功能进入新模块，不继续堆进 `lib.rs` 和 `kernel.rs`。

### 风险 6：借鉴外部 agent 后产品变成通用自动化平台

控制：

- P4 的所有借鉴都必须映射到五个写作 agent 方面。
- 外部子代理默认只读、隔离、只产出 evidence。
- 后台任务不得绕过 WriterOperation / approval / memory gate。
- 多渠道 gateway、cron、技能自改暂不进入主路径。

## 13. 推荐执行顺序

### 第一轮：P0 大脑统一

1. 设计 `WriterAgentRunRequest` / `WriterAgentRunResult`。
2. 在 kernel 内实现 `run_task`。
3. 改造 `ask_agent`，删除 lib.rs 内直接 `AgentLoop::new`。
4. 补 manual request run-loop eval。
5. 跑 `npm run verify`。

### 第二轮：P0 行动生命周期

1. 统一 operation lifecycle trace。
2. 写入 durable-save-before-feedback guard。
3. command inventory 静态检查。
4. 补 operation lifecycle eval。
5. 更新 docs/project-status.md。

### 第三轮：P1 信任合同

1. Story Contract quality gate。（已完成：`fill_quality()` + CompanionPanel 展示 quality/gaps）
2. Chapter Mission save settlement。（部分完成：状态机已升级，OutlinerPanel/EditorPanel UI 已上线，自动结算建议 UI 未接）
3. Promise Ledger 类型和优先级。（已完成）
4. Companion Panel 只显示最高价值 3-5 项。（已完成第一阶段）
5. 补 story/mission/promise scenario eval。（已完成）

### 第四轮：P1 作者价值评测

1. 新增连续 5 章 fixture。
2. 新增 product metrics 本地记录。
3. trajectory export 增加指标摘要。
4. 建立每轮开发必须通过的 product scenario eval。

### 第五轮：P2.4-P2.6 架构拆分

1. 拆 `lib.rs` command modules。（已完成）
2. 抽出 `app_state.rs` 和启动期状态初始化。（已完成）
3. 抽出 `semantic_lint.rs`。（已完成）
4. 抽出 `memory_context.rs`。（已完成）
5. 抽出 `observation_bridge.rs`。（已完成）
6. 抽出 `editor_realtime.rs`。（已完成）
7. 抽出 root utility / event / audit / writer observation helper。（已完成）
8. 抽出 root tests。（已完成）
9. 抽出 `writer_agent/kernel/task_packet.rs`。（已完成）
10. 抽出 `writer_agent/kernel/metrics.rs`。（已完成）
11. 抽出 `writer_agent/kernel/proposals_ext.rs`。（已完成）
12. 抽出 `writer_agent/kernel/ghost.rs`。（已完成）
13. 抽出 `writer_agent/kernel/memory_feedback.rs`。（已完成）
14. 抽出 `writer_agent/kernel/memory_candidates.rs` 与 `writer_agent/kernel/memory_candidates/` 子模块。（已完成）
15. 抽出 `writer_agent/kernel/` stateful impl 子模块：observation、run-loop、feedback、operation、snapshot、trace、tests，并把 `trace_recording` 从 `.in.rs include!` 迁成正常 Rust 子模块。（已完成）
16. 拆 `agent-evals/src/evals.rs`。（已完成：root-level legacy eval 模块已清除，`main.rs` 只通过 facade 调用职责模块）
17. 保持 public protocol 稳定。（已完成）

### 第六轮：P4 外部 Agent 项目借鉴落地

1. 实现 WriterRunEventStore。（第一阶段已完成：append-only / SQLite replay / trajectory export）
2. 增加 Planning / Review 只读模式。（第一阶段已完成：专用 task/context/prompt/tool policy/eval）
3. 增加 WriterTaskReceipt 和 failure evidence bundle。（第一阶段已完成：章节生成 receipt、长诊断 diagnostic_report artifact、Planning Review planning_review_report artifact、保存前校验、writer.error / writer.task_artifact run event、trajectory export）
4. 增加 memory correction / reinforcement。（第一阶段已完成：reviewed memory candidates 的 correction/reinforcement signal）
5. 增加 Project Brain knowledge index / graph。（第一阶段已完成：index / node / edge / path guard / chunk source-version metadata / source history / read-only revision compare / source revision restore / eval / Graph 页 Brain 模式）
6. 增加 Story Impact Radius Context Pack。（第一阶段已完成：内部类型 WriterStoryGraphNode / WriterStoryGraphEdge / WriterStoryImpactRadius / StoryImpactBudgetReport，种子提取 extract_seed_nodes，故事图构建 build_story_graph，双向 BFS 遍历 compute_story_impact_radius，promise seed id 与 graph node id 对齐，紧预算下同层候选按故事价值优先保留 PlotPromise / ChapterMission / Canon 等关键节点，预算报告只统计真实 reached drop、不把无关孤立节点算作截断，`writer.story_impact_radius_built` run event 关联 observation，Story Impact 风险/预算摘要进入 TaskPacket belief / required context，并以受预算约束的 `StoryImpactRadius` ContextSource 渲染进默认写作 prompt，12 个 eval 全部通过；剩余：真实作者项目校准 impact source 的召回收益、误报成本和上下文预算占用，不照搬代码 AST/call graph）
7. 增加 isolated research / diagnostic subtask workspace。（第一阶段已完成：artifact workspace / tool policy / evidence-only result / eval）
8. 增加 inspector timeline 和 trajectory export upgrade。（第一阶段已完成：backend timeline / companion-safe summary / task_receipt + task_artifact filter / save_completed filter / save-to-feedback latency / proposal context budget drilldown / redaction warning / local-only export marker / Trace Viewer compatible JSONL）
9. 增加 provider call budget。（第一阶段已完成：token/cost estimation / approval-required decision / remediation / chapter-generation preflight / eval）
10. 增加 post-write diagnostics。（保存观察 + accepted operation + Audit UI + save_completed link 第一阶段已完成：report / run event / trace snapshot / trajectory export / eval / UI summary / save_completed inspector filter）
11. 增加 external tool error remediation。（第一阶段已完成：ToolExecution remediation / missing tool / permission denied / handler failure eval / failure bundle 映射 / Inspector failure event）
12. 增加 metacognitive gate。（第一阶段已完成：`WriterMetacognitiveSnapshot` 从 trace 聚合 context pressure、failure bundle、post-write diagnostics、低置信 proposal、重复忽略率和 durable-save 健康度，输出风险等级和建议动作；Inspector / trajectory / eval 已接入；写作 run-loop 会在高风险时阻断 GhostWriting / InlineRewrite / ChapterGeneration，operation 层会阻断正文写入，同时保留 Planning Review、Continuity Diagnostic 和 mission calibration 等恢复路径；Inspector 元认知卡片已补恢复 CTA，可跳转 Review、诊断/保存、失败、上下文和 meta 视图，也可触发专用 `run_metacognitive_recovery` 只读恢复命令。Planning Review 结果已进一步持久化为可回放 `planning_review_report` artifact；剩余转向真实作者项目阈值校准。）
13. 补齐 P4 eval。（当前 P4 新增 eval 已覆盖 run event、planning mode、task receipt、task artifact、Planning Review artifact、failure evidence、memory correction/reinforcement、memory candidate run event、memory auto-write review boundary、operation approval decision run event、context pack built run event、model started run event、tool called run event、Project Brain knowledge index/path guard/chunk source/version metadata/source history/revision compare/source revision restore、Project Brain embedding provider profile/input limit/batch status/provider registry fallback、isolated research/diagnostic subtask workspace、research subtask started/completed run event、inspector timeline、trajectory redaction、Trace Viewer compatible export、provider budget、chapter-generation provider preflight、Project Brain provider preflight/approval retry、manual request provider preflight/approval retry、metacognitive recovery provider budget/read-only boundary、research subtask provider budget、provider budget approval coverage、provider budget run event、post-write diagnostics、accepted-operation post-write diagnostics、save_completed/post-write linkage、product metrics multi-session trend、metacognitive snapshot、metacognitive write gate、metacognitive recovery boundary、external tool remediation、tool remediation failure bundle 和 research subtask failure bundle；新增 Story Impact Radius eval 后才允许把它接入默认写作 context pack；P2 static guard 已覆盖 metacognitive recovery chips 只存在 Inspect；后续重点转向真实作者长会话校准和更丰富的恢复 artifact。）

## 14. 完成定义

短期完成定义：

- 手动 ask、inline、ghost、chapter generation 都由 Writer Agent Kernel 统一接管。
- 所有写入都有 typed operation、approval/audit、durable save、feedback 回灌。
- Story Contract / Chapter Mission / Promise Ledger 进入每次核心 agent action。
- Companion Panel 默认只显示作家真正需要知道的少量信号。
- `npm run verify` 通过。

中期完成定义：

- 连续多章 scenario eval 通过，并至少覆盖 10 个长篇产品场景。
- Agent 能稳定追踪承诺、角色状态、物件去向、章节任务。
- 作者可以相信它不会乱改设定、乱污染记忆、乱覆盖正文。
- 手动聊天不再是主路径。

长期完成定义：

- Forge 不再像“编辑器 + AI 面板”，而像一个有长期记忆、有行动边界、有反馈学习能力的小说共同作者。

## 15. 最薄弱的一根弦

只要作者价值没有被连续真实写作场景证明，工程 eval 再漂亮也只能说明系统没坏，不能说明产品成立。

只要 Companion 的默认体验仍让作者感觉像在操作一个工具，而不是被一个可靠的第二作家托住，Forge 就还没有真正进入 Cursor 式写作 agent。
