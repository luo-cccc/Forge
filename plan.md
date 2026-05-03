# Forge Cursor-Style Writing Agent 完整开发计划

Last updated: 2026-05-03

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
- Command boundary audit 已覆盖 47 个 Tauri commands，并进入 `npm run verify`。
- Tauri command handlers 已全部移入 `src-tauri/src/commands/*`；`src-tauri/src/lib.rs` 当前不再包含 `#[tauri::command]`。
- AppState、启动期 Hermes/Writer memory DB 打开、legacy DB migration、kernel seed 逻辑已抽入 `src-tauri/src/app_state.rs`。
- Semantic lint payload/event 和设定/诊断 lint 逻辑已抽入 `src-tauri/src/semantic_lint.rs`。
- Manual request context injection、用户画像读取、章节 embedding、近期技能抽取和 LLM memory candidate 生成已抽入 `src-tauri/src/memory_context.rs`。
- Agent/editor/manual observation payload 和 WriterObservation 转换逻辑已抽入 `src-tauri/src/observation_bridge.rs`。
- Editor realtime ghost rendering、ambient output 转发、editor prediction 清理、realtime cowrite 开关和 LLM ghost proposal flow 已抽入 `src-tauri/src/editor_realtime.rs`。
- API key 读取、路径 helper、事件常量、事件 payload、Agent status payload、项目写入审计、章节保存观察/canon refresh/context render helper 已分别抽入 `api_key.rs`、`app_paths.rs`、`events.rs`、`event_payloads.rs`、`agent_status.rs`、`project_audit.rs`、`writer_observer.rs`。
- 原 `lib.rs` 内联测试已抽入 `src-tauri/src/tests.rs`；`lib.rs` 当前约 170 行，主要保留模块 wiring、Tauri setup 和 command registration。
- trajectory JSONL 已导出 `writer.product_metrics`，包含采纳率、忽略率、promise recall、canon false-positive、mission completion、durable save 和 save-to-feedback latency。
- `npm run verify` 当前通过：lint、build、P2 checks、audit、Rust tests、89/89 writer evals。
- Writer Agent context pack 的 Canon / Promise slice 已引入写作相关性排序，并输出 `WHY writing_relevance` 解释，避免只按文本相似或固定 ledger 顺序取材。

### 当前剩余核心矛盾

- 前端仍保留聊天式 `AgentPanel`，容易把产品拉回“AI 聊天助手”心智。
- Story Contract / Chapter Mission 仍偏基础表单，还没有成为每次生成、诊断、保存的强门禁体验。
- `agent-evals/src/product_scenarios.rs` 已集中承载 10 个真实长篇产品场景 eval；下一步要继续提升场景真实性和失败解释质量，而不是只堆数量。
- `src-tauri/src/lib.rs` command 层拆分、AppState 拆分、semantic lint 拆分、memory/context helper 拆分、observation bridge 拆分、editor realtime 拆分、root helper 拆分和测试拆分已完成；剩余主要是最终 app setup / command registration glue。`writer_agent/kernel.rs` 的 P2 拆分已完成：TaskPacket/context trace、product metrics、proposal lifecycle、ghost helper、memory feedback、memory candidate、run-loop、feedback、operation execution、snapshot、trace recording 和测试都已进入职责模块，kernel facade 当前约 450 行。`agent-evals/src/evals.rs` 也已拆成职责单一的 eval 子模块。

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
- Companion Panel 显示 Story Contract 强度和最关键缺口。

### P1.2 Chapter Mission 工作流升级

目标：每一章都有当前任务，agent 不能只看光标附近文本。

任务：

- 在 OutlinePanel 中显示每章 mission 状态。
- 支持从大纲节点直接创建 / 编辑 Chapter Mission。
- 保存章节后自动结算：
  - completed
  - drifted
  - needs_review
  - unresolved
- 生成或重写时必须检查：
  - must include
  - must not reveal
  - expected ending
  - relation advancement
  - promise payoff / deferral

验收标准：

- 新增 eval：违反 `must_not` 会生成 story debt。
- 新增 eval：完成 expected ending 会标记 mission completed。
- 新增 UI check：当前章 mission 在写作视图中始终可见但不喧宾夺主。

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
- 新增 P2 UI check：Companion 默认区域不会展示 raw trace / raw chain / raw tool log。

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

- 新增 `agent-evals` 场景不少于 10 个。（已完成：`agent-evals/src/product_scenarios.rs` 当前集中 10 个长篇产品场景）
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

当前状态：工程第一版已完成，但产品验证不能按完成态理解。上述指标已从 Writer Agent trace 派生，并随 trajectory JSONL 以 `writer.product_metrics` 事件导出；Companion 写作模式会摘要采纳率和保存健康度。剩余工作是保留更长的 session 历史、提供 debug/inspector 趋势视图，并用真实连续写作场景证明这些指标与作者价值相关。

验收标准：

- 本地 trajectory export 可包含匿名化指标摘要。（已完成）
- Companion / debug view 能查看最近写作 session 的 agent 有用程度。（部分完成：Companion 已显示摘要，debug 趋势视图未完成）

## 8. P2：上下文、记忆、检索继续补强

### P2.1 Context Pack 质量升级

任务：

- 为不同任务定义 context budget profiles：
  - GhostWriting
  - InlineRewrite
  - ManualRequest
  - ChapterGeneration
  - ContinuityDiagnostic
  - ProposalEvaluation
- 必保来源：
  - Story Contract
  - current Chapter Mission
  - latest Result Feedback
  - relevant Promise Ledger slice
  - canon slice
  - cursor prefix/suffix
- 增加 context pack explainability：
  - 为什么选这些来源
  - 哪些来源被截断
  - 哪些来源因 budget 被丢弃

验收标准：

- tight budget 下必保来源不丢。
- context pack trace 可导出 JSONL。
- debug view 可查看 context source summary。

### P2.2 记忆写入质量门槛

当前状态：核心 Canon / Promise 候选门槛已接入真实 proposal 生成路径。本地保存抽取和 LLM memory candidate 都会过滤模糊、空泛、重复候选；同名 canon 的整实体 upsert 默认 dedupe，避免覆盖既有 attributes；与现有 canon kind 或关键 attributes 冲突的候选不会生成直接写入操作，而是生成高优先级 ContinuityWarning，要求作者明确确认后再处理。新增 eval 已覆盖 `vague_memory_candidate_rejected`、`duplicate_memory_candidate_deduped`、`conflicting_memory_candidate_requires_review`。

任务：

- 记忆候选分级：
  - observation
  - candidate
  - approved
  - rejected
  - superseded
- Canon / Promise / Style / Contract 写入分别定义质量规则。
- 模糊、空泛、重复、互相冲突的记忆不得进入长期记忆。

验收标准：

- 新增 eval：vague memory rejected。（已完成：覆盖 LLM memory proposal 路径）
- 新增 eval：duplicate memory deduped。（已完成：覆盖 canon/promise duplicate 不产生写操作）
- 新增 eval：conflicting memory requires explicit approval。（已完成：冲突 canon 不产生直接写入，转为 review proposal）

### P2.3 检索从“相似文本”升级为“写作相关性”

当前状态：已完成第一阶段。`src-tauri/src/writer_agent/context_relevance.rs` 已集中承载 Writer Agent context pack 的写作相关性评分；Canon / Promise slice 会综合当前 chapter mission、next beat、result feedback、recent decisions、cursor 附近正文和 open promises 排序，并在每条被选中的 canon / promise 前输出 `WHY writing_relevance`。本阶段覆盖 Writer Agent ledger context，后续如继续增强外部 project brain / vector DB，可在同一评分语义上接入 rerank。

任务：

- 检索排序引入：
  - 当前 chapter mission（已完成）
  - active entities（已完成：Canon slice 相关实体评分）
  - active promises（已完成：Promise slice 与 canon 关联评分）
  - recent decisions（已完成）
  - cursor scene type（部分完成：当前基于 cursor 附近正文、段落、选区和任务上下文抽取写作信号，未单独建模 scene type taxonomy）
- 不只返回 lore excerpt，还返回“为何相关”。（已完成：`WHY writing_relevance`）

验收标准：

- 新增 eval：同名实体优先返回当前剧情相关实体。（已完成）
- 新增 eval：promise 相关检索优先于普通语义相似段落。（已完成）

## 9. P2：架构拆分和可维护性

### P2.1 拆分 `src-tauri/src/lib.rs`

目标：`lib.rs` 只保留 app setup、command registration 和少量跨模块 glue。

当前状态：已完成。command handler 拆分已完成；`lib.rs` 当前有 0 个 `#[tauri::command]`，所有 47 个 Tauri commands 都在 `src-tauri/src/commands/*` 下。`src-tauri/src/app_state.rs` 已承接 AppState、锁 helper、memory DB 初始化、legacy DB migration 和 Writer Kernel seed。`src-tauri/src/semantic_lint.rs` 已承接 SemanticLint payload/event、设定冲突 lint 和 Writer Agent diagnostic lint。`src-tauri/src/memory_context.rs` 已承接 manual request context injection、用户画像读取、章节 embedding、近期技能抽取和 LLM memory candidate 生成。`src-tauri/src/observation_bridge.rs` 已承接 Agent/editor/manual observation payload 和 WriterObservation 转换逻辑。`src-tauri/src/editor_realtime.rs` 已承接 editor ghost rendering、ambient output 转发、editor prediction 清理、realtime cowrite 开关和 LLM ghost proposal flow。`api_key.rs`、`app_paths.rs`、`events.rs`、`event_payloads.rs`、`agent_status.rs`、`project_audit.rs`、`writer_observer.rs` 已承接原先散落在 root 的通用 helper 和写作保存观察 helper。`src-tauri/src/tests.rs` 已承接原 `lib.rs` 内联测试。`lib.rs` 当前约 170 行，只保留模块 wiring、Tauri setup 和 command registration。

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

### P2.2 拆分 `writer_agent/kernel.rs`

当前状态：已完成。`writer_agent/kernel.rs` 当前约 450 行，保留 facade、状态结构、公开类型、`new()` 和少量共享转换 helper；对外 `writer_agent::kernel::*` 路径保持稳定。既有 `kernel_chapters.rs`、`kernel_helpers.rs`、`kernel_ops.rs`、`kernel_prompts.rs`、`kernel_review.rs` 继续承接章节、helper、operation、prompt、review 逻辑。`writer_agent/kernel_task_packet.rs` 已承接 TaskPacket 构建、context budget trace 和 trace state expiry helper。`writer_agent/kernel_metrics.rs` 已承接 `WriterProductMetrics` 和 trace-derived product metrics 计算。`writer_agent/kernel_proposals.rs` 已承接 proposal 替换、优先级权重和过期判断 helper。`writer_agent/kernel_ghost.rs` 已承接 ghost 续写草稿、三分支候选、continuation 清理和 context evidence 映射。`writer_agent/kernel_memory_feedback.rs` 已承接 proposal slot、suppression slot、memory extraction feedback、memory audit/feedback helper。`writer_agent/kernel_memory_candidates.rs` 已承接 memory candidate extraction、LLM candidate parsing、canon/promise candidate proposal construction、dedupe、sentence splitting 和 quality validation。`writer_agent/kernel_run_loop.rs` 已承接 run-loop 类型和 `WriterAgentPreparedRun`。`writer_agent/kernel/` 下的子模块已承接 observation handling、context pack accessors、run-loop methods、proposal creation/registration、feedback、operation execution、snapshot、trace recording 和 kernel tests。

建议模块：

```text
writer_agent/
  kernel.rs              // facade / state owner
  kernel_run_loop.rs     // unified task execution types / prepared run（已完成）
  kernel/                // stateful WriterAgentKernel impl blocks（已完成）
  kernel_ghost.rs        // ghost proposal helpers（已开始）
  kernel_memory_feedback.rs // memory feedback / slot helpers（已完成）
  kernel_memory_candidates.rs // memory candidate extraction / validation（已完成）
  kernel_task_packet.rs   // TaskPacket / context trace helpers（已开始）
  kernel_metrics.rs       // trace-derived product metrics（已开始）
  kernel_proposals.rs     // proposal lifecycle helpers（已开始）
  operation_executor.rs
  proposal_engine.rs
  feedback_loop.rs
  ledger_snapshot.rs
  foundation_guard.rs
  policy.rs
```

验收标准：

- kernel facade 保持清晰 API。
- operation execution、task packet、feedback、policy 分离。
- eval 不降级。

### P2.3 拆分 `agent-evals/src/evals.rs`

当前状态：已完成。`agent-evals/src/evals.rs` 当前约 64 行，只保留共享 imports、`EvalToolHandler`、`eval_llm_message` 和子模块 re-export；原大型 eval 函数已按职责拆入 `agent-evals/src/evals/` 下的 intent、canon、ghost_feedback、context、tool_policy、run_loop、task_packet、foundation、mission、promise、story_debt、trajectory 模块。`cargo run -p agent-evals` 仍输出同一报告格式，当前 89/89 passing。

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
- Tab 接受当前分支，方向键切换分支。

验收标准：

- 已有 multi-ghost eval 扩展到 mission / promise / style grounding。
- 接受 / 拒绝反馈影响后续分支排序。

### P3.2 Ambient Lore

目标：作家不主动问，agent 也能守住设定。

任务：

- 实时实体锚点。
- canon conflict 微提示。
- hover 显示简短证据，不塞大段 lore。
- 高风险冲突进入 Companion queue。

验收标准：

- DOM decoration 不复制完整 lore 内容。
- 大文本下 decoration rebuild 不明显卡顿。
- 错误提示可拒绝并学习。

### P3.3 Parallel Drafts

目标：重头戏时 agent 提供可拼接的平行草稿，而不是一次性替作者写完。

任务：

- 同一 scene goal 生成 3 个版本：
  - 保守推进
  - 情绪加压
  - 外部打断
- 允许句段级采纳。
- 每段采纳都进入 operation lifecycle。

验收标准：

- 不直接覆盖正文。
- 每个 draft 标明与 mission / promise 的关系。
- 采纳后保存成功才写入 feedback。

## 11. 风险清单

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

## 12. 推荐执行顺序

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

1. Story Contract quality gate。
2. Chapter Mission save settlement。
3. Promise Ledger 类型和优先级。
4. Companion Panel 只显示最高价值 3-5 项。
5. 补 story/mission/promise scenario eval。

### 第四轮：P1 作者价值评测

1. 新增连续 5 章 fixture。
2. 新增 product metrics 本地记录。
3. trajectory export 增加指标摘要。
4. 建立每轮开发必须通过的 product scenario eval。

### 第五轮：P2 架构拆分

1. 拆 `lib.rs` command modules。（已完成）
2. 抽出 `app_state.rs` 和启动期状态初始化。（已完成）
3. 抽出 `semantic_lint.rs`。（已完成）
4. 抽出 `memory_context.rs`。（已完成）
5. 抽出 `observation_bridge.rs`。（已完成）
6. 抽出 `editor_realtime.rs`。（已完成）
7. 抽出 root utility / event / audit / writer observation helper。（已完成）
8. 抽出 root tests。（已完成）
9. 抽出 `kernel_task_packet.rs`。（已完成）
10. 抽出 `kernel_metrics.rs`。（已完成）
11. 抽出 `kernel_proposals.rs`。（已完成）
12. 抽出 `kernel_ghost.rs`。（已完成）
13. 抽出 `kernel_memory_feedback.rs`。（已完成）
14. 抽出 `kernel_memory_candidates.rs`。（已完成）
15. 抽出 `writer_agent/kernel/` stateful impl 子模块：observation、run-loop、feedback、operation、snapshot、trace、tests。（已完成）
16. 拆 `agent-evals/src/evals.rs`。（已完成）
17. 保持 public protocol 稳定。（已完成）

## 13. 完成定义

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

## 14. 最薄弱的一根弦

只要作者价值没有被连续真实写作场景证明，工程 eval 再漂亮也只能说明系统没坏，不能说明产品成立。

只要 Companion 的默认体验仍让作者感觉像在操作一个工具，而不是被一个可靠的第二作家托住，Forge 就还没有真正进入 Cursor 式写作 agent。
