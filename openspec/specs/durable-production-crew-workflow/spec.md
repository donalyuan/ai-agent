# durable-production-crew-workflow Specification

## Purpose
TBD - created by archiving change complete-durable-production-crew-workflow. Update Purpose after archive.
## Requirements
### Requirement: Full Crew 必须绑定 active 账号和唯一 active 制作意图

系统 SHALL 仅为 `status=active` 的 `projects` 账号下状态为 `approved` 且未软删除的 `content_topics` 创建 Full Crew 制作意图，并 SHALL 保存账号、选题、创建输入及 source fingerprint；同一 Topic 同时 SHALL 只有一个 active Full Crew 制作意图，每个制作意图 SHALL 只有一个 ProductionRun。创建制作意图或 Run 本身 SHALL NOT 提前改变选题状态。

#### Scenario: 从有效选题创建制作意图

- **GIVEN** 当前账号下存在一条未软删除的 `approved` 选题
- **WHEN** 操作者以该账号和选题创建 Full Crew
- **THEN** 系统 SHALL 创建绑定真实 `project_id` 和 `topic_id` 的制作意图
- **AND** 系统 SHALL 保存不可变选题与账号策略来源快照
- **AND** 选题 SHALL 继续保持 `approved`
- **AND** 系统 SHALL 建立 Topic active-intent 锁和制作意图单 Run 约束

#### Scenario: 无效来源不能创建 Full Crew

- **GIVEN** 账号已归档，或选题不存在、已软删除、不是 `approved` 或不属于提交的账号
- **WHEN** 操作者请求创建 Full Crew
- **THEN** 系统 SHALL 返回稳定的来源校验错误
- **AND** 系统 SHALL NOT 创建制作意图或 Run
- **AND** 系统 SHALL NOT 修改账号、选题、脚本或作品数据

#### Scenario: 同一选题并发创建制作意图

- **GIVEN** 同一 approved Topic 当前没有 active Full Crew 制作意图
- **WHEN** 两个请求并发创建普通 Full Crew 制作意图
- **THEN** 只有一个请求 SHALL 成功并持有 active-intent 锁
- **AND** 另一个请求 SHALL 返回 `active_intent_conflict`
- **AND** 系统 SHALL NOT通过创建两个 Run 隐式支持竞案

#### Scenario: 同一制作意图重复创建 Run

- **GIVEN** ProductionProject 已经创建一个 ProductionRun
- **WHEN** 客户端再次请求创建或启动新 Run
- **THEN** 相同幂等命令 SHALL 返回原 Run，不同命令 SHALL 返回 `run_already_exists`
- **AND** 系统 SHALL NOT创建第二组 Step 或跨 Run 复用最新产物

### Requirement: Full Crew 执行计划必须版本化并在 Run 创建时冻结

系统 SHALL 从代码发布的版本化计划定义创建 Full Crew Run，并 SHALL 在 Run 中固定计划 key/version/digest、可选角色、步骤依赖、最大返工次数、资源限制和模型 binding 摘要；公开请求 SHALL NOT 接受任意角色序列或 `auto_approve`。

#### Scenario: 创建固定计划 Run

- **WHEN** 操作者启动一个有效 Full Crew 制作意图
- **THEN** 系统 SHALL 创建持久化 ProductionRun 和完整 ProductionStep 集合
- **AND** 每个 step SHALL 具有稳定 step key、类型、依赖和初始状态
- **AND** Run SHALL 保存不可变计划与资源限制快照

#### Scenario: 客户端试图改写计划

- **WHEN** 客户端提交自定义 `roles`、`auto_approve`、跳过 Gate 或运行中计划替换参数
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建或修改任何可执行步骤

#### Scenario: 发布新计划版本

- **GIVEN** 已存在使用旧计划版本的运行中或历史 Run
- **WHEN** 代码发布新的 Full Crew 计划版本
- **THEN** 新 Run SHALL 使用新计划版本
- **AND** 既有 Run SHALL 继续引用原计划快照
- **AND** 系统 SHALL NOT 静默迁移既有 Run

### Requirement: PostgreSQL 必须是流程状态唯一事实源

系统 SHALL 在 PostgreSQL 中持久化 Run、Step、ArtifactPackageSnapshot、GateDecision、资源用量、租约、错误和领域关联；Redis SHALL 只用于唤醒或派发，Redis 消息丢失或重复不得改变流程正确性。

#### Scenario: 服务重启后恢复流程

- **GIVEN** 某 Run 已完成部分步骤并处于可继续、等待审批或等待外部结果状态
- **WHEN** API、worker 或 Redis 重启
- **THEN** 系统 SHALL 从 PostgreSQL 重建当前流程状态
- **AND** 已完成步骤 SHALL NOT 重新执行
- **AND** 可执行步骤 SHALL 可被重新唤醒

#### Scenario: 收到重复唤醒消息

- **GIVEN** 同一 step 已被一个 worker 合法认领或已进入终态
- **WHEN** 另一个 worker 收到重复 Redis 消息
- **THEN** 系统 SHALL 通过数据库状态、租约和幂等约束拒绝重复执行
- **AND** 系统 SHALL NOT 创建第二次模型调用或领域写入

#### Scenario: Redis 消息丢失

- **GIVEN** PostgreSQL 中存在依赖已满足的 queued step
- **WHEN** 对应 Redis 消息丢失
- **THEN** 恢复扫描 SHALL 重新唤醒该 step
- **AND** Run SHALL NOT 因 Redis 丢失而永久停滞

### Requirement: Step 推进必须具备并发租约和不确定副作用语义

系统 SHALL 仅允许依赖满足的 step 被原子认领，并 SHALL 保存 lease owner、过期时间和 attempt；租约过期时只有尚未产生不确定外部副作用的 step 可以自动重新认领，结果不确定的模型或 provider 调用 MUST 进入 `attention_required` 而不得透明重试。

#### Scenario: 两个 worker 并发认领

- **GIVEN** 一个 step 当前可执行
- **WHEN** 两个 worker 同时尝试认领
- **THEN** 只有一个 worker SHALL 获得有效租约
- **AND** 另一个 worker SHALL 观察到冲突且不执行副作用

#### Scenario: 纯数据库步骤租约过期

- **GIVEN** 一个尚未提交任何外部副作用的数据库 step 因进程退出而租约过期
- **WHEN** 恢复扫描认领该 step
- **THEN** 系统 SHALL 使用原幂等键安全继续或返回原结果

#### Scenario: 外部调用结果不确定

- **GIVEN** step 已发出模型或 provider 请求但未能确定上游是否接收
- **WHEN** 请求超时或连接中断
- **THEN** step SHALL 进入 `attention_required`
- **AND** 系统 SHALL 保留 prepared 审计和已知请求证据
- **AND** 系统 SHALL NOT 自动创建新 attempt 或再次提交

### Requirement: 包级 Gate 必须绑定精确产物版本和 digest

系统 SHALL 将 BriefPackage、ScriptPackage、ProductionPackage 和 QualityPackage 保存为不可变 package snapshot，并 SHALL 只允许 GateDecision 审批提交时展示的精确 package digest；任何组成产物或媒体引用变化 SHALL 使旧决策不能推进当前流程。

#### Scenario: 批准当前 package

- **GIVEN** package 包含完整且通过 schema 校验的产物 ID、类型、版本和 digest
- **WHEN** 操作者批准该 package digest
- **THEN** 系统 SHALL 保存不可变 GateDecision、操作者身份、决策时间和备注
- **AND** 对应 Run SHALL 只解锁该 package 的下一步骤

#### Scenario: 使用过期 package digest 审批

- **GIVEN** package 展示后任一组成产物产生了新版本
- **WHEN** 客户端批准旧 package digest
- **THEN** 系统 SHALL 返回 `stale_package`
- **AND** 系统 SHALL NOT 推进 Run 或晋升旧产物

#### Scenario: 单产物 approved 不得替代包级审批

- **GIVEN** package 中一个或多个单独产物状态为 `approved`
- **WHEN** 尚不存在当前 package digest 的 GateDecision
- **THEN** Full Crew SHALL 继续停在对应 Gate
- **AND** 系统 SHALL NOT 根据单表状态自动推进

### Requirement: 角色协作不得越过产物所有权

协作角色 SHALL 将输出持久化为 `collaboration_suggestions`；接受建议 SHALL 只产生 owner 修订义务，建议本身 SHALL NOT 修改 owner 产物。阻断建议未响应，或接受后 owner 尚未生成引用该建议的新版本时，相关 package SHALL NOT 可批准。

#### Scenario: 摄影指导提出阻断建议

- **WHEN** cinematographer 输出合法的高优先级建议
- **THEN** 系统 SHALL 原子保存建议、来源角色、目标角色、目标产物版本和来源 ModelCall
- **AND** ProductionPackage Gate SHALL 保持阻断

#### Scenario: 接受建议后生成 owner 新版本

- **GIVEN** 导演接受一条针对 ShotContract 的建议
- **WHEN** director 基于该建议完成新版本
- **THEN** 新版本 SHALL 引用建议 ID
- **AND** 原 ShotContract SHALL 保持不变并可审计
- **AND** 只有新版本进入当前 package 后建议才 SHALL 视为已落实

#### Scenario: 拒绝建议

- **WHEN** owner 拒绝建议并提交非空理由
- **THEN** 系统 SHALL 保存不可变响应记录
- **AND** 建议 SHALL NOT 修改任何产物
- **AND** Gate SHALL 按版本化政策判断该拒绝是否解除阻断

### Requirement: ResourceSafetyGate 必须治理非金额资源

正式 Full Crew SHALL 使用 `ResourceSafetyGate` 在每次 role model call 和作品生成领域命令前校验并原子预占资源；限制 SHALL 覆盖模型调用次数、输入/输出 token、role retry、返工次数、视频任务数、总时长、TTS 字符、ASR 数量、并发和 provider retry，并 SHALL NOT 计算或保存价格、币种或金额上限。

#### Scenario: 资源额度充足

- **WHEN** 当前 step 的预估资源与已用资源均在 Run 固定限制内
- **THEN** 系统 SHALL 原子记录资源预占并允许 step 继续
- **AND** step 终态后 SHALL 保存实际可得用量

#### Scenario: 资源限制超出

- **WHEN** 任一调用次数、token、时长、字符、任务、并发、重试或返工限制将被超出
- **THEN** 系统 SHALL 在外部调用前阻断 step
- **AND** 系统 SHALL 返回具体限制项和当前用量
- **AND** 系统 SHALL NOT 缩短请求、切换模型或部分提交

#### Scenario: 查询资源审计

- **WHEN** 操作者查询 Run 状态
- **THEN** 系统 SHALL 返回不含金额和凭据的限制、预占及实际用量摘要
- **AND** 响应 SHALL NOT 包含价格、币种、API Key 或认证信息

### Requirement: Role step 必须原子保存产物并闭合审计终态

每个 role step SHALL 使用 prepare/execute/finalize 协议：调用前持久化 step attempt、固定 Definition/model binding、Context 审计锚点和 `agent_runs`；调用后完成完整 typed schema 校验，并在同一事务内保存该角色全部产物或建议、step 用量及 `agent_runs` 终态。多产物角色 MUST 全部成功或全部不写入。

#### Scenario: 多产物角色执行成功

- **WHEN** screenwriter 返回完整合法的 StoryBible、CharacterBible 和 ScriptDraft
- **THEN** 系统 SHALL 在一个事务内保存所有产物及其共同来源 attempt/ModelCall
- **AND** step 和 `agent_runs` SHALL 进入成功终态
- **AND** package SHALL 引用同一次一致输出

#### Scenario: 输出 schema 不合法

- **WHEN** 角色输出缺字段、字段类型错误、引用不存在或违反完整 schema
- **THEN** 系统 SHALL 将 step、`agent_runs` 和 ModelCall 关联为明确失败
- **AND** 系统 SHALL NOT 保存任何部分产物
- **AND** 系统 SHALL NOT推进项目阶段或后续 Gate

#### Scenario: finalize 重复提交

- **GIVEN** 某 step attempt 已成功完成 finalize
- **WHEN** worker 因响应丢失再次提交相同 finalize
- **THEN** 系统 SHALL 返回原产物和终态
- **AND** 系统 SHALL NOT创建新版本或重复建议

### Requirement: 正式角色执行不得绕过计划与 Gate

生产环境中的单角色命令 SHALL 只执行当前 Run 中已解锁且与请求 role key 匹配的 role step，并 SHALL 复用同一租约、资源、输入 package、模型 binding 和审计规则；任意项目/角色直接执行不得成为旁路。

#### Scenario: 执行当前合法角色

- **GIVEN** 当前 Run 的 producer step 已解锁且未执行
- **WHEN** 操作者触发该 step
- **THEN** 系统 SHALL 通过 ProductionOrchestrator 执行对应 role step
- **AND** 结果 SHALL 关联当前 Run 和 Step

#### Scenario: 越级执行角色

- **GIVEN** ScriptPackage 尚未批准
- **WHEN** 客户端直接请求 director、editor 或 qc
- **THEN** 系统 SHALL 返回 `transition_conflict`
- **AND** 系统 SHALL NOT调用模型或写入产物

### Requirement: 作品生成必须作为现有领域外部等待步骤接入

ProductionOrchestrator SHALL 通过类型化 Application Port 创建或更新既有 SceneVisualManifest 和 WorkPlan，并 SHALL 保存返回的正式 ID、版本和 digest；只有既有 WorkPlan 人工确认接口可以创建 `work_generation_run`，Orchestrator SHALL NOT 直接插入作品 step 或调用媒体 provider。

#### Scenario: 等待主画面完整

- **GIVEN** ProductionPackage 已批准但任一 Scene 尚无有效主画面
- **WHEN** 流程到达 visual readiness step
- **THEN** step SHALL 进入明确等待状态并返回 blocker
- **AND** 系统 SHALL NOT创建可确认 WorkPlan 或视频任务

#### Scenario: 创建既有 WorkPlan

- **GIVEN** 当前 SceneVisualManifest 完整且 input version 有效
- **WHEN** Orchestrator 提交已批准 ProductionPackage 的类型化规划输入
- **THEN** 现有 WorkGenerationService SHALL 创建或更新同一生产意图的 Work 草稿和 WorkPlan
- **AND** ProductionRun SHALL 保存 Work、WorkVersion、WorkPlan 和来源 package digest 关联
- **AND** 系统 SHALL NOT创建第二套作品或视频任务表

#### Scenario: 等待作品运行终态

- **GIVEN** 操作者已通过既有确认接口创建 `work_generation_run`
- **WHEN** 作品 Worker 执行中
- **THEN** ProductionRun SHALL 处于 external wait
- **AND** Orchestrator SHALL 只读取正式运行状态和产物引用
- **AND** 重复唤醒 SHALL NOT重提 provider 任务

### Requirement: Editor 和 QC 必须基于真实媒体证据且 fail-closed

Editor/QC SHALL 仅在当前 WorkVersion 存在已登记 final media 和完整 MediaEvidenceSnapshot 后运行；视觉证据必须由满足 vision 要求的模型或版本化视觉分析器读取真实媒体，音频证据必须由可证明读取真实音轨的音频分析或 ASR 能力产生。审计只保存自管资产 ID、版本/hash、MIME、时长、映射和脱敏结果，不得保存 base64、长期签名 URL 或凭据。

#### Scenario: 使用完整媒体证据评审

- **GIVEN** 当前 WorkVersion 的 final media、分段/Scene/Shot 映射及视觉和音频证据完整
- **WHEN** Editor 和 QC 执行
- **THEN** 系统 SHALL 将不可变媒体引用作为受控输入
- **AND** 每个 required shot SHALL 有对应 ContinuityLedger，每个 required take SHALL 有唯一 TakeReview
- **AND** ModelCall SHALL 保存资产引用和能力证据而非媒体二进制

#### Scenario: 缺少媒体或能力

- **WHEN** final media、映射、视觉能力、音频能力或任一必需证据缺失
- **THEN** Editor/QC step SHALL 返回稳定 capability/evidence blocker
- **AND** QualityGate SHALL NOT通过
- **AND** 系统 SHALL NOT以文本产物、空 review 或推测结果降级继续

#### Scenario: 空评审集合

- **GIVEN** 当前 WorkVersion 存在一个或多个必需 take
- **WHEN** TakeReview 为空或未覆盖全部 take
- **THEN** QualityGate SHALL 拒绝通过
- **AND** ProductionRun SHALL NOT进入质量批准终态

### Requirement: QC 返工必须派生新作品版本并受次数限制

QualityGate 对 `rejected` 或 `needs_revision` 的结果 SHALL 通过现有 Work Library 版本治理创建 `edit` 或 `full_regeneration` 草稿和差异计划，并 SHALL 等待新的资源展示与人工确认；当前和历史 WorkVersion、媒体、运行及评审不得被覆盖。达到 Run 固定返工上限时 SHALL 明确终止自动推进。

#### Scenario: 局部 QC 返工

- **GIVEN** QC 只拒绝部分可独立重生成的 take
- **WHEN** 操作者接受返工建议
- **THEN** 系统 SHALL 从当前 WorkVersion 派生 `edit` 草稿和差异计划
- **AND** 未受影响成功素材 SHALL 可按现有规则复用
- **AND** 新调用 SHALL 在人工确认后进入新 WorkGenerationRun

#### Scenario: 全局 QC 返工

- **GIVEN** QC 问题影响全局风格、比例、分辨率或完整叙事
- **WHEN** 操作者接受返工建议
- **THEN** 系统 SHALL 创建 `full_regeneration` 草稿
- **AND** 原完成版本及全部审计 SHALL 保持不变

#### Scenario: 达到返工上限

- **WHEN** Run 已达到计划快照中的最大返工次数
- **THEN** 系统 SHALL 将 Run 标记为需要人工终止或新建制作意图
- **AND** 系统 SHALL NOT自动创建下一版或再次调用 provider

### Requirement: 流程 API 必须返回可操作状态和审计关联

Full Crew API SHALL 提供 ProductionRun、Step、当前 package、Gate、等待原因、可执行命令、资源摘要、领域关联和 AgentRun/ModelCall 引用；命令接受 SHALL 与运行完成严格区分。

#### Scenario: 查询等待审批的 Run

- **WHEN** Run 停在 ScriptPackage Gate
- **THEN** API SHALL 返回 `waiting_approval`、当前 package digest、组成产物版本和允许的 approve/reject 命令
- **AND** API SHALL NOT将其表示为 running 或 completed

#### Scenario: 异步命令被接受

- **WHEN** 启动、resume 或 retry 命令已持久化但尚未执行完成
- **THEN** API SHALL 返回 `202` 和持久化 run/step ID
- **AND** API SHALL NOT声称角色、Gate 或作品任务已经成功

#### Scenario: 查询失败或注意状态

- **WHEN** step 因 schema、资源、能力、外部不确定结果或领域冲突停止
- **THEN** API SHALL 返回稳定 error code、可重试性、所需人工动作和相关审计 ID
- **AND** API SHALL NOT泄露凭据、原始请求头或未脱敏 provider 响应

### Requirement: 行为变化角色必须通过版本化评测门禁

本 change 修改的角色 Definition、Prompt、Context Policy 引用和输出 schema SHALL 发布为 candidate 新版本，并 SHALL 在激活前通过 registry/schema、Prompt 编译、结构化输出 fixture、历史 snapshot dry-run、包映射和 fake media/workflow golden tests；任何真实模型 EvalRun SHALL 继续要求显式预算确认。

#### Scenario: 只完成零费用验证

- **GIVEN** 尚未取得真实模型 EvalRun 的预算确认
- **WHEN** 实现完成静态、dry-run、fixture 和 golden 验证
- **THEN** candidate SHALL 保持不可用于普通生产 Run
- **AND** 系统 SHALL NOT调用真实模型或静默激活 candidate

#### Scenario: candidate 通过完整门禁

- **GIVEN** 操作者已明确确认真实评测 case、模型 fingerprint、token、重试和成本上限
- **WHEN** candidate 完成全部必需评测且生成通过的不可变 EvalReport
- **THEN** 后续代码发布 SHALL 可将 candidate 标记为 active
- **AND** 新 ProductionRun SHALL 固定该 active 版本
- **AND** 既有 Run 和历史快照 SHALL 保持原版本

### Requirement: 活跃 Full Crew 必须锁定来源生命周期

ProductionProject 进入 active 后，系统 SHALL 阻止来源 Topic 的内容、归属、状态和软删除变更，并 SHALL 阻止其账号归档；账号策略等可变配置后续更新 SHALL NOT覆盖 Run 创建时保存的快照。只有 ScriptPackagePromotion 可以把锁定 Topic 从 `approved` 原子更新为 `scripted`。失败或取消只有在不存在成功或不确定领域晋升时才能释放 active-intent 锁。

#### Scenario: 活跃流程期间修改或删除选题

- **GIVEN** approved Topic 已绑定 active Full Crew 制作意图
- **WHEN** 操作者尝试编辑、归档、改变归属或软删除该 Topic
- **THEN** 系统 SHALL 返回 `source_locked`
- **AND** Topic 及 ProductionRun SHALL 保持不变

#### Scenario: 活跃流程期间归档账号

- **GIVEN** active 项目下存在未终态 Full Crew ProductionRun
- **WHEN** 操作者尝试归档该项目
- **THEN** 系统 SHALL 拒绝归档并返回 active ProductionProject/Run 引用
- **AND** 系统 SHALL NOT让 Run 继续依赖已归档账号

#### Scenario: 终止后释放选题锁

- **GIVEN** ProductionRun 已确定失败或取消且从未成功或不确定地晋升脚本
- **WHEN** 终止事务完成
- **THEN** 系统 SHALL 释放 Topic active-intent 锁
- **AND** Topic SHALL 保持 `approved` 并可创建新的制作意图

### Requirement: Gate reject 必须创建有界修订 epoch

BriefPackage、ScriptPackage 或 ProductionPackage 被 reject 时，系统 SHALL 保留原 Package 和 GateDecision，记录非空理由和受影响 owner，并 SHALL 创建新的 revision epoch，只重新打开 owner role 及确定性受影响的后继步骤。每类 package 的最大修订次数 SHALL 固定在 Run 计划快照中；旧 epoch 的 step、产物、approval 和 audit SHALL NOT被覆盖或再次推进。

#### Scenario: BriefPackage 被拒绝

- **WHEN** 操作者拒绝当前 BriefPackage 并提交非空理由
- **THEN** 系统 SHALL 创建新的 producer revision step
- **AND** screenwriter 及其后继 SHALL 保持未解锁
- **AND** 新 CreativeBrief SHALL 形成新 package digest 并重新等待审批

#### Scenario: ScriptPackage 被拒绝

- **WHEN** 操作者拒绝当前 ScriptPackage
- **THEN** 系统 SHALL 重新打开 screenwriter revision，并按计划处理 character critic 建议
- **AND** 系统 SHALL NOT晋升旧 package、创建正式 Script 或修改 Topic

#### Scenario: ProductionPackage 被拒绝

- **WHEN** 操作者指出 Director、Performance 或 Sound 产物需要修订
- **THEN** 系统 SHALL 只重新打开被点名 owner 及受影响的 package 汇合步骤
- **AND** 已批准 Script SHALL 保持不变
- **AND** 旧 SceneVisualManifest/WorkPlan SHALL NOT被当前流程消费

#### Scenario: 达到 package 修订上限

- **WHEN** 当前 package 已达到计划快照中的最大修订次数
- **THEN** Run SHALL 进入 `attention_required`
- **AND** 允许命令 SHALL 仅包含取消或结束后新建制作意图
- **AND** 系统 SHALL NOT自动再次调用模型

### Requirement: 脚本语义修订必须回流到新的 ScriptPackage

正式 Script 晋升后，任何旁白、Scene 顺序、Scene 结构或核心叙事语义变化 SHALL 创建 script revision epoch，由 screenwriter 生成并重新审批新的 ScriptPackage，再确定性晋升为带 `parent_id` 的新 Script；系统 SHALL 使旧 ProductionPackage、SceneVisualManifest 关联和 WorkPlan 失效，并从新 Script 的 Director 后继重新执行。

#### Scenario: Director 提出脚本语义变更

- **GIVEN** 当前 Script 已晋升且 Director 提议修改旁白或 Scene 结构
- **WHEN** 操作者接受该语义变更建议
- **THEN** 系统 SHALL 创建 screenwriter revision step 而不是允许 Director 写 Script
- **AND** 新 ScriptPackage SHALL 重新经过包级审批和事务化晋升
- **AND** 新 Script SHALL 通过 `parent_id` 引用原 Script

#### Scenario: 仅修改制作表达

- **GIVEN** 修改只涉及镜头语言、表演、声音或作品参数且不改变 Script 语义
- **WHEN** owner 生成修订产物
- **THEN** 系统 SHALL 创建对应过程产物新版本
- **AND** 系统 SHALL NOT创建不必要的新 Script 版本

### Requirement: Run 取消和 ProductionProject 删除必须保留审计真实性

ProductionRun SHALL 支持持久化 cancellation intent 和 `cancelling/cancelled` 终态。取消后 SHALL NOT解锁新 step 或发起新模型/provider 调用；尚未产生外部副作用的预占 SHALL 释放，可信实际用量 SHALL 结算，结果不确定的调用 SHALL 保持预占并进入 `attention_required`。只有从未创建 Run、过程产物或领域关联的 ProductionProject 可以删除；其余只能归档展示并保留全部审计。

#### Scenario: 在纯等待步骤取消

- **GIVEN** Run 正在等待人工审批且没有进行中的外部调用
- **WHEN** 操作者提交 cancel 命令
- **THEN** 未完成 step SHALL 进入 cancelled
- **AND** Run SHALL 确定进入 `cancelled`
- **AND** 系统 SHALL 释放未消费预占且保留既有产物和 GateDecision

#### Scenario: 外部调用期间取消

- **GIVEN** Run 关联的模型或 WorkGeneration 调用正在进行
- **WHEN** 操作者提交 cancel 命令
- **THEN** 系统 SHALL 先保存 cancellation intent 并调用对应领域已有取消端口
- **AND** 在结果确定前 Run SHALL 保持 `cancelling` 或 `attention_required`
- **AND** 系统 SHALL NOT虚假声称上游已取消

#### Scenario: 删除有历史的制作意图

- **GIVEN** ProductionProject 已存在 Run、产物、Gate 或领域关联
- **WHEN** 客户端请求删除
- **THEN** 系统 SHALL 拒绝物理或软删除并提供归档命令
- **AND** 查询审计 SHALL 继续可达

### Requirement: ProductionPackage 必须使用真实引用并满足集合完整性

ProductionPackage 中的 Scene、Character、Shot 和后续 Take 关系 SHALL 使用类型化稳定身份：ShotContract SHALL 引用当前正式 Script 的 Scene UUID，PerformanceBrief SHALL 引用当前 CharacterBible 稳定身份且其 Scene 引用必须属于同一 Script，SoundPlan 的全部 Scene 引用也必须解析到同一 Script。Package SHALL 证明每个 Scene 至少有一个 ShotContract、每个被脚本引用的 Character 有 PerformanceBrief，且不存在孤儿、跨 Script、重复身份或不合法顺序/时长。

#### Scenario: ProductionPackage 引用完整

- **WHEN** 系统构建 ProductionPackage
- **THEN** 所有 Scene/Character/Shot 引用 SHALL 解析到当前 Run 和当前 Script
- **AND** package snapshot SHALL 保存真实 ID、版本和 digest
- **AND** 稳定排序和集合基数 SHALL 进入 package digest

#### Scenario: 自由字符串或跨 Script 引用

- **WHEN** 任一角色输出未知 character_id、scene_number 字符串、跨 Script Scene UUID 或重复 shot identity
- **THEN** role finalize SHALL 以 schema/reference 错误原子失败
- **AND** 系统 SHALL NOT保存部分产物或构建 ProductionPackage

#### Scenario: Package 集合覆盖不完整

- **WHEN** 任一 Scene 没有 ShotContract、被引用 Character 没有 PerformanceBrief或 SoundPlan 含孤儿 Scene
- **THEN** ProductionPackage Gate SHALL 保持阻断并返回具体缺失项

### Requirement: QualityPackage 必须基于确定性 take inventory 和追加版本

系统 SHALL 从当前 WorkGenerationRun 中实际成功且被 final compose 消费的 generation step、attempt 和 output asset 确定性建立不可变 RequiredTakeInventorySnapshot；每个 required take SHALL 关联当前 WorkVersion、asset、segment、有序非空 `scene_ids[]`，并为每个 Scene 关联从已批准 ProductionPackage 解析的非空 `shot_contract_ids[]`。现有 segment 可以覆盖多个 Scene，系统 SHALL NOT虚构 take 与 Shot 一对一关系。ContinuityLedger 与 TakeReview SHALL 作为 append-only 版本实体引用 ProductionRun、revision epoch、WorkVersion、inventory digest 和 MediaEvidenceSnapshot；TakeReview SHALL 一对一覆盖 required take 并记录其适用 ShotContract 集合的检查结果，ContinuityLedger SHALL 覆盖 required shot，禁止按 ProductionProject 查询最新自由字符串记录组成 QualityPackage。

#### Scenario: 建立 required take inventory

- **GIVEN** WorkGenerationRun 技术成功且 final media 已登记
- **WHEN** 系统准备媒体评审
- **THEN** inventory SHALL 只包含实际被最终合成消费的成功输出
- **AND** 每个 take SHALL 有稳定 ID、唯一 asset/segment 来源、有序 Scene 集合和每个 Scene 的确定性 ShotContract 集合
- **AND** 缺失、跨 Script、重复或歧义映射 SHALL 返回 evidence blocker

#### Scenario: 质量产物跨版本或重复覆盖

- **WHEN** TakeReview 引用其他 WorkVersion、旧 inventory digest，或同一 required take 出现多个当前 review
- **THEN** QualityPackage SHALL 拒绝构建
- **AND** QualityGate SHALL NOT选择任意一条记录继续

#### Scenario: QC 返工后的新作品版本

- **GIVEN** QC 返工产生新 WorkVersion 和 WorkGenerationRun
- **WHEN** 新作品进入评审
- **THEN** 系统 SHALL 创建新的 inventory、evidence、ledger 和 review 版本集合
- **AND** 旧 WorkVersion 的 QualityPackage 和决策 SHALL 保持可审计但不得批准新版本

### Requirement: WorkGeneration 外部终态必须显式映射

ProductionOrchestrator SHALL 只观察并关联既有 WorkGenerationRun 的真实技术状态：`queued/running` 映射为 external wait；`succeeded` 还需 final media 和 take inventory 完整才可解锁 Editor；`failed` 映射为带原错误和可重试性的 blocker；`waiting_manual/unknown_submission` 映射为 `attention_required`；`cancelling/cancelled` 不得伪装为成功或普通失败。任何重试或取消 SHALL 继续通过 WorkGeneration 正式端口。

#### Scenario: 作品运行失败

- **WHEN** 关联 WorkGenerationRun 进入 `failed`
- **THEN** ProductionRun SHALL 保存外部 run ID、错误分类和可重试性并停止推进
- **AND** 系统 SHALL NOT运行 Editor/QC 或自动创建新作品运行

#### Scenario: 作品提交结果不确定

- **WHEN** WorkGenerationRun 进入 `waiting_manual` 且原因是 `unknown_submission`
- **THEN** ProductionRun SHALL 进入 `attention_required`
- **AND** resume SHALL NOT重提 provider 请求

#### Scenario: 作品运行被独立取消

- **GIVEN** ProductionRun 未请求取消
- **WHEN** 关联 WorkGenerationRun 进入 `cancelled`
- **THEN** ProductionRun SHALL 显示 `external_cancel_conflict`
- **AND** 操作者 SHALL 只能显式取消 ProductionRun 或重新规划并再次确认

### Requirement: Production 命令必须治理 actor、幂等和动态输入

服务端 SHALL 为本地单用户解析稳定 `actor_type/actor_id`，不得接受客户端自报 user_id 或每次生成随机 UUID。所有改变状态的命令 SHALL 使用 `(actor, command_type, aggregate_id, idempotency_key)` 和 canonical request digest；同 key 同 digest 返回原结果，同 key 不同 digest返回 `idempotency_conflict`。公开 role/step 命令 SHALL NOT接受任意 context、`preferred_model_id` 或未在计划中声明的用户输入。

#### Scenario: 相同命令幂等重放

- **WHEN** 客户端以相同作用域、Idempotency-Key 和 request digest 重放命令
- **THEN** 系统 SHALL 返回原 command/run/step 结果
- **AND** 系统 SHALL NOT重复模型调用、GateDecision 或领域写入

#### Scenario: 相同 key 提交不同 payload

- **WHEN** 客户端在同一命令作用域复用 key 但修改 package digest、理由、资源限制或其他 payload
- **THEN** 系统 SHALL 返回 `idempotency_conflict`
- **AND** 系统 SHALL NOT把旧结果应用到新请求

#### Scenario: 请求覆盖模型或注入任意 context

- **WHEN** 客户端提交 `preferred_model_id`、自由 context、`roles` 或 `auto_approve`
- **THEN** 系统 SHALL 拒绝请求
- **AND** 普通 ProductionRun SHALL 继续使用创建时固定的 active binding 和计划输入

#### Scenario: 计划允许用户补充指令

- **WHEN** 操作者在明确允许的修订命令中提交补充指令
- **THEN** 系统 SHALL 保存 actor、来源、`user_instruction` trust、digest 和 revision epoch
- **AND** 指令影响当前 package 时 SHALL 先使旧 package/Gate 失效
- **AND** 指令 SHALL NOT改写 Definition、绕过 Gate 或直接覆盖正式领域数据
