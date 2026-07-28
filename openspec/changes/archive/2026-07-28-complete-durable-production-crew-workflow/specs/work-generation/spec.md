## ADDED Requirements

### Requirement: 已批准 ProductionPackage 必须复用现有作品计划链路

系统 SHALL 将当前已批准 ProductionPackage 通过类型化 Application Service 转换为现有画面生成和 WorkPlan 输入，并 SHALL 复用既有 Work、WorkVersion、WorkPlan、WorkGenerationRun 和 Worker DAG；ProductionOrchestrator SHALL NOT直接插入作品生成步骤、调用媒体 provider 或建立第二套视频任务。

#### Scenario: 主画面不完整时等待

- **GIVEN** ProductionPackage 已批准但正式 Script 的任一 Scene 缺少有效主画面
- **WHEN** Full Crew 请求创建作品计划
- **THEN** 现有 SceneVisualManifest 校验 SHALL 返回具体 blocker
- **AND** 系统 SHALL NOT创建可确认 WorkPlan、WorkGenerationRun 或 provider 任务

#### Scenario: 从 ProductionPackage 创建 WorkPlan

- **GIVEN** SceneVisualManifest 完整且 input version 有效
- **WHEN** Full Crew 提交当前 ProductionPackage 的 typed plan input
- **THEN** WorkGenerationService SHALL 创建或更新同一 Script 的既有 Work 草稿和 WorkPlan
- **AND** WorkPlan SHALL 保存 ProductionRun、ProductionPackage digest、Script/Scene、主画面和相关产物来源引用
- **AND** 导演、表演和声音方案 SHALL 进入可见 Prompt、时间线或声音建议快照
- **AND** 系统 SHALL NOT自动确认计划

#### Scenario: ProductionPackage 变化使旧计划失效

- **GIVEN** WorkPlan 引用了某个已批准 ProductionPackage digest
- **WHEN** 当前 ProductionPackage、Script、SceneVisualManifest、Prompt、主画面、模型、音色、声音模式、字幕、时间线或输出参数发生变化
- **THEN** 旧 WorkPlan SHALL 失效且不得确认
- **AND** 系统 SHALL 基于新输入创建或更新合法计划修订

#### Scenario: 操作者修改 Full Crew 下游方案

- **GIVEN** WorkPlan 已从已批准 ProductionPackage 创建
- **WHEN** 操作者修改 Prompt、模型、音色、声音模式、字幕、时间线或输出参数
- **THEN** 系统 SHALL 保存相对 ProductionPackage 的显式 override diff
- **AND** 全部修改 SHALL 进入 WorkVersion 快照和 WorkPlan input fingerprint
- **AND** 系统 SHALL NOT回写 ProductionPackage 或把人工修改伪装成原 Gate 已批准内容
- **AND** 旧计划 SHALL 失效并要求重新规划、展示资源和确认

### Requirement: Full Crew 作品运行必须继续人工确认非金额资源

Full Crew 创建的 WorkPlan SHALL 继续展示模型、音色、时长、比例、分辨率、声音模式、字幕配置、视频任务数、视频总秒数、TTS 字符数和 ASR 时长；只有操作者通过现有幂等确认接口后，系统才 SHALL 创建 WorkGenerationRun，并 SHALL NOT计算或展示金额。

#### Scenario: 确认 Full Crew 作品计划

- **GIVEN** WorkPlan 当前有效且全部能力和输入校验通过
- **WHEN** 操作者查看并确认模型、参数和非金额资源用量
- **THEN** 现有确认接口 SHALL 幂等创建一个 WorkGenerationRun
- **AND** ProductionRun SHALL 保存正式 run ID 并进入外部等待状态
- **AND** ProductionOrchestrator SHALL NOT绕过该确认

#### Scenario: 相同确认重复提交

- **GIVEN** 相同 WorkPlan 和 Idempotency-Key 已创建 WorkGenerationRun
- **WHEN** Full Crew resume 或客户端重试确认
- **THEN** 系统 SHALL 返回原 WorkGenerationRun
- **AND** 系统 SHALL NOT创建第二组视频、TTS、ASR 或合成任务

#### Scenario: 资源限制不满足

- **WHEN** WorkPlan 超出视频任务数、总时长、TTS 字符、ASR 数量、并发或重试限制
- **THEN** 系统 SHALL 在创建 WorkGenerationRun 前阻断
- **AND** 系统 SHALL 返回具体非金额限制项
- **AND** 系统 SHALL NOT通过缩短输入、切换模型或部分提交继续

### Requirement: Full Crew QC 返工必须遵守作品版本治理

Full Crew QualityGate 产生的局部或全局返工 SHALL 通过现有 Work Library 版本治理从被评审 WorkVersion 派生 `edit` 或 `full_regeneration` 草稿、差异计划和新的人工确认；系统 SHALL 保留原 WorkVersion、WorkGenerationRun、成功媒体和 QC 证据，不得原地覆盖或自动再次调用 provider。

#### Scenario: 局部返工派生 edit 版本

- **GIVEN** QC 只拒绝部分可独立重生成的 take
- **WHEN** 操作者接受局部返工建议
- **THEN** 系统 SHALL 创建或复用来源 WorkVersion 对应的 `edit` 草稿
- **AND** 差异计划 SHALL 标明受影响任务、可复用素材和非金额资源用量
- **AND** 只有再次人工确认后系统才 SHALL 创建新运行

#### Scenario: 全局返工派生 full regeneration 版本

- **GIVEN** QC 问题影响全局视觉、比例、分辨率、完整叙事或全部媒体
- **WHEN** 操作者接受整体返工建议
- **THEN** 系统 SHALL 创建或复用 `full_regeneration` 草稿和完整差异计划
- **AND** 原完成版本及其媒体、运行和审计 SHALL 保持不变

#### Scenario: QC 不通过不得伪装作品生成失败或成功批准

- **GIVEN** WorkGenerationRun 已技术成功并登记 final media，但 Full Crew QC 未通过
- **WHEN** 系统展示作品和 ProductionRun 状态
- **THEN** WorkGenerationRun SHALL 保持真实技术终态
- **AND** ProductionRun SHALL 显示质量未批准或等待返工
- **AND** 系统 SHALL NOT把技术成功等同于 Full Crew 质量批准

### Requirement: WorkGenerationRun 技术终态必须真实传播到 Full Crew

Full Crew SHALL 只通过既有 WorkGeneration Application Service 查询、重试或取消作品运行，并 SHALL 保留其真实 `queued/running/succeeded/failed/waiting_manual/cancelling/cancelled` 技术状态。ProductionRun 只有在作品运行 succeeded、final media 已登记且 required take inventory 完整后才能进入 Editor/QC；其他终态 SHALL 映射为明确等待、阻断、注意或取消状态。

#### Scenario: 作品运行失败

- **WHEN** WorkGenerationRun 进入 `failed`
- **THEN** ProductionRun SHALL 保存原 run ID、失败分类和可重试性并停止推进
- **AND** ProductionOrchestrator SHALL NOT自动重试、创建第二个运行或执行 Editor/QC

#### Scenario: 作品运行需要人工处理

- **WHEN** WorkGenerationRun 因上游提交结果不确定进入 `waiting_manual`
- **THEN** ProductionRun SHALL 进入 `attention_required`
- **AND** resume SHALL NOT重复提交 provider 请求

#### Scenario: 作品运行成功但成片证据不完整

- **GIVEN** WorkGenerationRun 状态为 `succeeded`
- **WHEN** final media、compose 消费关系或 take inventory 任一缺失
- **THEN** ProductionRun SHALL 保持 evidence blocker
- **AND** 系统 SHALL NOT把技术成功等同于可进行质量评审

#### Scenario: Full Crew 请求取消作品运行

- **GIVEN** ProductionRun 已保存 cancellation intent 且 WorkGenerationRun 仍在执行
- **WHEN** Orchestrator 调用既有取消端口
- **THEN** WorkGenerationRun SHALL 按原有 provider 取消协议进入真实 `cancelling/cancelled/waiting_manual` 状态
- **AND** ProductionRun SHALL 在结果确定前保持 `cancelling` 或 `attention_required`
- **AND** 系统 SHALL NOT直接更新作品运行为 cancelled

### Requirement: Full Crew 作品幂等必须校验请求内容

Full Crew 使用现有作品确认、重试和取消接口时，幂等记录 SHALL 同时绑定命令作用域、WorkPlan/Run ID 和 canonical request digest；同 key 同 digest SHALL 返回原结果，同 key 不同 digest SHALL 返回冲突，防止把旧作品运行误绑定到变化后的计划。

#### Scenario: 相同 key 确认不同计划修订

- **GIVEN** Idempotency-Key 已为某 WorkPlan revision 创建 WorkGenerationRun
- **WHEN** 客户端以相同 key 确认新的 plan ID、plan version 或 input fingerprint
- **THEN** 系统 SHALL 返回 `idempotency_conflict`
- **AND** 系统 SHALL NOT返回旧运行作为新计划的执行结果

#### Scenario: 相同 key 和相同计划重放

- **WHEN** 客户端以相同 key、plan ID、plan version 和 request digest 重放确认
- **THEN** 系统 SHALL 返回原 WorkGenerationRun
- **AND** 系统 SHALL NOT创建第二组外部任务
