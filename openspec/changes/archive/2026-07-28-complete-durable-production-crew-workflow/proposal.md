## Why

虚拟制作团队目前只能独立执行单个角色，`execute_flow`、流程查询和 Fast Lane 仍是内存骨架或 stub；现有单角色入口还能绕过多数 Gate，过程产物也没有与正式选题、脚本、分镜和作品生成链路形成一致的事务边界。现在需要把 Full Crew 建成可持久化、可恢复、不可绕过且能产出真实作品的后端正式流程，否则继续增加角色或界面只会固化第二套孤立业务数据和不可审计的付费执行路径。

## What Changes

- 建立以 PostgreSQL 为唯一状态事实源的 `ProductionRun`、版本化执行计划、`ProductionStep`、包级 `GateDecision` 和恢复协议；Redis 只承担唤醒与派发，不保存权威流程状态。
- **BREAKING**：Full Crew 创建必须绑定状态为 `active` 的账号及其 `approved` 选题；同一选题同时只允许一个 active Full Crew 制作意图，且一个制作意图只允许一个 Run。公开流程接口不再接受任意 `roles`、`auto_approve`、模型覆盖或未治理 context，单角色执行也必须服从同一计划、前置条件、幂等和 Gate 规则。
- 补齐 Gate reject、角色修订 epoch、显式 retry、取消、外部等待失败和终止协议；运行中的账号归档、选题修改/软删除和制作意图删除必须被阻断，不能让权威来源或流程在执行中消失。
- 将 `CreativeBrief`、`ScriptPackage`、`ProductionPackage` 和媒体质量评审建模为带精确产物版本与 digest 的不可变检查点；协作建议只能由产物 owner 通过新版本响应，不能直接覆盖已确认产物。
- 升级编剧输出契约，使 `ScriptDraft` 可确定性映射为正式 `Script/Scene`；批准整个 `ScriptPackage` 后，在同一事务内创建 `approved` 脚本和分镜并将选题更新为 `scripted`，不得二次审批或追加 LLM 格式转换。
- 将已批准 `ProductionPackage` 通过类型化 Application Service 接入既有画面生成、`WorkPlan` 和 `work_generation_run`；继续执行作品级一次人工确认、模型/参数快照、非金额资源用量、幂等与失败恢复规则，不建立第二套视频任务。
- 用 `ResourceSafetyGate` 替换金额估算型 `BudgetGate`，在每次模型或付费媒体调用前限制调用次数、token、视频任务数、总时长、TTS 字符、ASR 数量、并发和重试。
- 让 Editor/QC 读取真实成片及不可变媒体引用；以当前 WorkVersion 的确定性 take inventory 为覆盖基线，将 `ContinuityLedger`、`TakeReview` 和媒体证据按 Run/WorkVersion/attempt 追加版本化，缺少满足视觉/音频要求的模型、分析能力、最终成片或逐 take 评审时 fail-closed，QC 返工必须派生新的作品版本而不得覆盖既有版本。
- 补齐所有 Scene/Character/Shot/Take 的类型化真实引用和集合完整性、角色输出完整 schema 校验、协作建议持久化、多产物事务写入、`agent_runs` 终态、流程/步骤幂等、并发租约、稳定操作者身份和审计关联，并为角色 Prompt/输出契约建立零费用 contract/golden 评测；任何真实模型 EvalRun 继续要求显式预算确认，普通 ProductionRun 只允许固定已 `active` 的 Definition binding。

## Capabilities

### New Capabilities

- `durable-production-crew-workflow`: 定义 Full Crew 的持久化执行计划、步骤状态机、包级 Gate、角色协作、资源安全、恢复、真实媒体 QC 和现有领域集成契约。

### Modified Capabilities

- `content-topic-management`: 增加 Full Crew 只能从 active 项目下的 `approved` 选题启动、同一选题只能存在一个 active 制作意图、活跃流程期间禁止修改或软删除来源选题，并且只在 `ScriptPackage` 原子晋升成功后将选题更新为 `scripted` 的要求。
- `script-agent-mvp`: 增加 Full Crew 经包级审批确定性创建正式 `approved` 脚本与分镜、保存来源快照且不得二次审批或追加模型转换的要求。
- `work-generation`: 增加从已批准 `ProductionPackage` 创建或更新既有作品计划、将全部人工 override 纳入 fingerprint、继续由操作者确认非金额资源用量后启动现有作品运行、向 ProductionRun 传播运行终态，以及 QC 返工派生新版本的要求。

## Impact

- 数据库：扩展 `production_projects`，新增持久化流程、步骤、修订 epoch、Gate 决策、计划快照、取消、租约和领域晋升关联；为质量产物补充 Run/WorkVersion/attempt/version/digest 作用域，并增加 active intent、单 Run、真实引用和幂等约束，但不复制 `scripts/scenes/works/work_generation_*`。
- Rust：重点影响 `crates/novex-production-crew` 的 orchestrator、executor、state、gates 和角色 schema，以及 `backend` 的 production API、Application Service、脚本/选题事务和作品生成集成端口。
- Agent 定义：升级制作角色的 `AgentDefinition`、Prompt、Context Policy 引用和输出 schema；Editor/QC 增加真实媒体能力要求，继续复用 `PromptCompiler`、`AuditedModelExecutor`、`ModelCall` 和 Eval 治理。
- API：调整 `/api/v1/production/` 下 Full Crew 创建、启动、查询、审批、拒绝修订、取消、恢复和单角色命令语义；命令统一使用稳定 actor、作用域化 Idempotency-Key 和 payload fingerprint，现有 stub 响应、随机用户 ID、任意角色序列、任意 context 及逐步模型覆盖不再兼容。
- 不改变 Pi Runtime、通用 Tool Loop、现有作品 Worker 内部 DAG、发布运营规则或 Fast Lane；不在本 change 中新增 `apps/video-agent`/`admin` 页面，前端审批工作台需独立 Pencil 原型确认和 OpenSpec change。
