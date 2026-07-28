## Context

`establish-virtual-production-crew` 已建立 `production_projects`、10 类过程产物、协作建议、角色清单、Gate 和 HTTP 路由；`wire-production-crew-role-execution` 已让单角色调用进入 `PromptCompiler + AuditedModelExecutor + ModelCall`。但当前流程仍存在以下断点：

- `execute_flow` 只返回内存构造的状态，流程查询和 Fast Lane 查询仍是 stub，服务重启后无法恢复。
- 执行计划是可由客户端传入的扁平角色列表，Gate 的“角色前/后”索引语义不一致，单角色入口只在 producer 前运行一个 Gate。
- 多产物写入、项目阶段更新和 `agent_runs` 终态不在同一可靠执行协议内；协作角色的输出不会自动进入 `collaboration_suggestions`。
- 当前 `ScriptDraft` 与正式 `Script/Scene` 字段不兼容，`production_projects` 也未绑定 `projects/content_topics`。
- director 后没有连接真实画面/作品生成，Editor/QC 只读取文本产物，空 `TakeReview` 仍可能通过质量检查。
- 当前 `BudgetGate` 用固定金额估算，与作品生产“不得计算金额，只治理资源用量”的正式规则冲突。
- Gate API 虽支持 reject，但 Brief、Script 和 Production Package 被拒后的 owner 修订、回流和次数上限没有状态机语义；流程也没有取消、删除或外部作品失败传播协议。
- 当前选题在没有正式脚本引用时可以软删除，账号也可以归档；活跃 Full Crew 的来源锁定、同一 Topic 并发制作意图和一个制作意图的 Run 基数尚未治理。
- `PerformanceBrief`、`SoundPlan`、`ContinuityLedger` 和 `TakeReview` 仍依赖自由字符串引用，后两类产物还没有 Run/WorkVersion/version 作用域，无法组成真正不可变的 QualityPackage。

本 change 跨 `crates/novex-production-crew`、backend Application/API、PostgreSQL、Agent Definition Registry、脚本/选题事务和现有作品生成领域。Rust 继续拥有视频领域状态、Repository 和高风险 Gate；Pi Runtime、通用 Tool Loop 与作品 Worker 内部 DAG 均不迁移。

## Goals / Non-Goals

**Goals:**

- 建立可持久化、可恢复、可查询、可审计且不会并发重复推进的 Full Crew 执行协议。
- 用固定计划和包级 Gate 约束角色、人工审批、领域晋升、外部等待与返工，消除任意角色序列和 Gate 绕过。
- 让过程产物以确定性、事务化方式进入现有选题、脚本、分镜、画面和作品生成业务，不建立平行正式数据。
- 复用现有模型审计、Context 编译、作品计划、人工确认、资源限制、Worker 恢复和作品版本治理。
- 让 Editor/QC 基于真实媒体证据执行并在能力不足时 fail-closed。
- 定义 reject 修订、失败、取消、删除、来源失效和外部等待的完整生命周期，使任何终止路径都能释放未消费预占并保留不可变审计。
- 让所有过程产物和质量证据按 Run、revision epoch、正式领域实体及 WorkVersion 精确归属，禁止跨 Run 或自由字符串串联。

**Non-Goals:**

- 不实现 Fast Lane AI 集成，不把 Full Crew 的兼容逻辑复制给 Fast Lane。
- 不新增多个 Agent 自由群聊、自主 Planner、正式 Memory、通用 Tool Loop 或 Pi 执行路径。
- 不修改作品 Worker 的 Seedance/TTS/ASR/FFmpeg DAG 归属，不建设第二套生成任务。
- 不自动发布作品，也不改变现有人工发布交接规则。
- 不在本 change 中实现 `apps/video-agent` 或 `admin` 页面；前端审批工作台必须另行完成设计上下文、Pencil 原型和用户确认。
- 不在迁移、测试或评测中未经确认调用真实 LLM、视频、TTS、ASR 或其他收费服务。
- 不在 v1 支持同一 Topic 的并行竞案；未来若需要竞案，必须以显式 variant 聚合和独立 OpenSpec change 建模，不能通过并行普通制作意图隐式实现。
- 不建设通用登录、RBAC 或多租户身份系统；本地单用户命令仍必须使用稳定的 actor 标识，禁止每次请求生成随机 UUID 冒充操作者。

## Decisions

### 1. 分离制作意图、流程执行、模型调用和作品任务

领域层级固定为：

```text
ProductionProject (制作意图，绑定 project/topic)
  └─ ProductionRun (v1 唯一的一次固定计划执行)
       ├─ ProductionStep (role/gate/domain_command/external_wait)
       │    └─ AgentRun + ModelCall（仅 role step）
       ├─ RevisionEpoch + GateDecision + ArtifactPackageSnapshot
       └─ ProductionDomainLink
            ├─ Script / Scene
            └─ Work / WorkVersion / WorkPlan / WorkGenerationRun
```

`production_projects` 保留现有表名以避免无意义重命名，但其语义收敛为制作意图：新增非空 `project_id`、`topic_id`、创建输入快照、source fingerprint 和约束，Full Crew 只接受 `active` 项目下的 `approved` 选题。同一 Topic 通过条件唯一约束同时只允许一个 active Full Crew 制作意图；v1 每个制作意图只允许一个 ProductionRun，恢复和重试使用 Step attempt，不创建第二个 Run。失败或取消后只有在 Topic 仍为 `approved` 且旧意图已终态时才能创建新的制作意图；已晋升为 `scripted` 后不能重开。

`production_runs` 保存不可变计划 key/version/digest、资源限制快照、每个 role step 的 active Definition/model binding 摘要和运行状态；`production_steps` 保存 step key/type、revision epoch、依赖、输入/输出 digest、状态、attempt、租约和错误。所有过程产物、建议、package 和质量证据都必须引用真实 `run_id/step_id/attempt`，不得仅按 ProductionProject 查询“最新产物”后跨 Run 混用。

`agent_runs/ModelCall` 不承担流程状态；`work_generation_runs/steps` 不承担角色和 Gate 状态。通过带真实外键的 domain link 关联这些聚合，禁止只把目标 ID 写入无约束 metadata。

替代方案是复用 `agent_runs` 或 `work_generation_runs` 作为总流程。前者无法表达人工 Gate 和领域命令，后者只属于媒体生成 DAG，都会混淆状态所有权，因此不采用。

### 2. PostgreSQL 是唯一流程事实源，Redis 只做唤醒

创建、推进、暂停、失败、取消、恢复和完成均先提交 PostgreSQL。Redis 消息只携带 `run_id/step_id` 唤醒 worker；消息丢失后可由 PostgreSQL 扫描可运行步骤恢复，重复消息通过 step 状态和幂等键消除。

step claim 使用数据库行锁和有限租约；只有依赖满足且状态可执行的 step 才能从 `queued` 进入 `running`。租约过期只允许重新认领尚未产生不确定外部副作用的 step。外部调用结果不确定时进入 `attention_required`，不得自动再次提交。

替代方案是把 Redis job payload 作为流程状态。它无法提供包级审批、事务晋升和长期审计所需的一致性，因此不采用。

### 3. 执行计划由版本化代码定义并在 Run 创建时冻结

Full Crew v1 计划不是客户端提供的角色数组，而是版本化 DAG：

```text
validate_source
  -> producer -> brief_approval
       reject -> producer_revision(epoch + 1, bounded) -> brief_approval
  -> screenwriter -> character_critic(optional) -> character_suggestion_resolution
       -> script_package_approval
       reject -> screenwriter_revision(epoch + 1, bounded) -> script_package_approval
  -> promote_script
  -> director -> cinematographer -> suggestion_resolution
       -> director_revision(epoch + 1, bounded)
  -> performance_director + sound_director
  -> production_package_approval
       reject -> affected_owner_revision(epoch + 1, bounded) -> production_package_approval
  -> wait_scene_visual_manifest
  -> create_work_plan -> work_plan_confirmation
  -> wait_work_generation
       failed/cancelled/waiting_manual -> explicit operator command
  -> editor -> qc -> quality_gate
       -> create_rework_draft(loop, bounded) | completed
```

并行只允许出现在固定依赖明确的步骤，例如表演指导和声音指导；角色不能自由互相调用。可选角色、每类 package reject 最大修订次数、QC 最大返工次数和资源限制在 Run 创建前进入计划快照，运行中不得静默改变。每次 reject 保存原 GateDecision 并创建新的 revision epoch；只重新打开被拒 package 的 owner role 及确定性受影响后继，不覆盖旧 step、产物或决策。达到上限后 Run 进入 `attention_required`，只能取消或新建制作意图。

Director 在脚本晋升后提出语义变更时不能直接创建或覆盖 Script；系统创建受控 script revision epoch，回到 screenwriter 生成新 ScriptPackage、重新审批和确定性晋升为带 `parent_id` 的新 Script，然后使旧 ProductionPackage、SceneVisualManifest 关联和 WorkPlan 失效并从 Director 后继重新执行。脚本修订同样计入固定修订上限。

公开 API 删除 `roles` 和 `auto_approve`。单角色调试入口不再是生产旁路；正式调用只能执行当前计划中已解锁的 role step，并复用相同前置检查、幂等和审计。

### 4. Gate 审批整个不可变包，而不是单表状态

`ArtifactPackageSnapshot` 保存 package type/version、revision epoch、按稳定顺序排列的产物 ID/type/version/content digest、来源 step/attempt、生成时间和 canonical package digest。Gate 决策引用精确 package digest、稳定 actor、决策、备注和时间；reject 必须包含非空理由及受影响 owner，新产物版本产生后旧决策仍保留但不再满足当前 Gate。

包定义如下：

- `BriefPackage`：当前 `CreativeBrief` 精确版本。
- `ScriptPackage`：`StoryBible + CharacterBible[] + ScriptDraft` 的一致版本集合。
- `ProductionPackage`：`DirectorialTreatment + ShotContract[] + PerformanceBrief[] + SoundPlan` 以及已处理协作建议引用。
- `QualityPackage`：目标 `WorkVersion`、确定性 required take inventory、最终媒体引用、逐 take 媒体证据、版本化 `ContinuityLedger[] + TakeReview[]`。

`collaboration_suggestions` 仍是建议而不是跨角色写权限。接受建议只会要求 owner 生成新版本；存在未响应的阻断建议，或接受建议后尚无包含该建议引用的新 owner 版本时，包不得批准。

现有单产物 approve API 可保留为过程标记查询能力，但不能推进 Full Crew；正式推进只认包级 GateDecision。对同一 digest 重放相同决策返回原结果；同一 digest 提交相反决策返回冲突，任何 reject 后都不得继续使用原 approval 或从旧 epoch 越级推进。

### 5. ScriptPackage 先满足正式领域 schema，再做零模型调用晋升

screenwriter candidate 的输出 schema 必须补齐正式 `Script/Scene` 所需字段：`title`、`hook`，以及每个 scene 的 `sequence`、`narration`、`visual_description`、`emotion`、`duration_sec`。额外故事结构字段可以保留，但不能替代正式字段。

`ScriptPackagePromotionService` 在一个 PostgreSQL 事务中：

1. 锁定 ProductionProject、Topic、当前 package 和 GateDecision。
2. 验证项目仍为 `active`、Topic 仍为同项目 `approved`、未软删除且 source fingerprint 未变化、package digest 未变化且正式字段完整。
3. 通过确定性 mapper 构建 `Script(status=approved)` 和有序 `Scene[]`，保存 topic/production/package 来源快照。
4. 写入 script/scenes、将 topic 更新为 `scripted`、创建 production-domain link 并完成 promotion step。

事务任一步失败均不改变 Topic 或创建部分 Scene。相同 promotion idempotency key 和 payload digest 返回原 Script；相同 key 不同 payload、Topic 已被其他 Script 消费或来源锁失效时返回冲突。Gate 后不得调用 LLM 修补字段或转换格式。

现有 `ScriptRepository.save_script()` 与 Topic 更新分离，不能直接满足该事务；实现时增加归脚本/选题应用边界所有的事务化晋升端口，而不是让 Orchestrator 拼 SQL。

### 6. ProductionPackage 不覆盖脚本，只驱动画面和作品计划

脚本晋升后 Director 必须读取正式 Scene UUID；`ShotContract.scene_id` 改为引用正式 `scenes.id`，禁止自由字符串或只靠 `scene_number` 关联。`PerformanceBrief.character_id` 必须引用当前 CharacterBible 稳定身份，其情绪弧 Scene 引用以及 `SoundPlan` 的音乐、音效和对白 Scene 引用都必须解析为同一正式 Script 的 Scene UUID。ProductionPackage 必须证明每个 Scene 至少有一个 ShotContract、每个被脚本引用的角色都有 PerformanceBrief、所有 Shot/Scene/Character 引用闭合且时长与顺序满足领域约束。

导演或协作部门若要改变已批准脚本语义，必须进入受控 script revision epoch，派生新的 Script 版本并使现有下游 package/manifest/plan 失效，不能直接更新 Scene。只改变镜头语言、表演、声音或作品参数且不改变脚本语义时，生成对应 owner 的新过程产物版本，不重写 Script。

批准的 `ProductionPackage` 通过类型化端口：

- 向既有画面生成领域提供每个 Scene 的视觉约束和 Prompt 来源；实际图片生成、数量限制、主画面选择和 `SceneVisualManifest` 仍由现有领域负责。
- 等待现有 `SceneVisualManifest` 完整且 `input_version` 有效后，调用 `WorkGenerationService.plan()` 创建或更新既有 Work 草稿和 WorkPlan。
- 将导演、表演和声音方案转成可见的 `scene_prompts`、全片 Prompt、时间线/声音建议和来源引用，但模型、音色、时长、比例、分辨率等仍由操作者选择并重新校验。操作者对 Prompt、主画面、模型、音色、声音模式、字幕、时间线或输出参数的任何修改都生成显式 override diff 并进入 WorkVersion/WorkPlan input fingerprint；这些下游 override 不回写或伪装成已批准 ProductionPackage，Editor/QC 按实际 WorkGenerationRun 快照评审。
- 只有现有 WorkPlan 人工确认接口在显示模型、参数和非金额资源用量后，才能幂等创建 `work_generation_run`。

ProductionOrchestrator 只保存返回的 typed IDs/digests 并等待既有运行终态，不直接插入 `work_generation_steps` 或调用 provider。

### 7. ResourceSafetyGate 统一治理非金额资源

删除固定单价和金额预算逻辑。Run 资源限制快照至少包含 role/model 调用上限、输入/输出 token 上限、最大 role retry、最大返工环数；作品生成仍使用既有视频任务数、总秒数、TTS 字符、ASR 数量、并发和 provider retry 限制。

每个 role model call 前由 `ResourceSafetyGate` 基于已持久化用量原子预占额度，ModelCall 终态后结算实际用量；尚未产生外部副作用即失败或取消时释放预占，已有可信实际用量时按实际结算，结果不确定时保持预占并进入人工处理，禁止通过释放后重试突破上限。超限时 step 进入明确阻断状态，不得缩短请求、切换模型或跳过角色。作品生成的外部调用继续由既有 Worker 成本闸门二次检查，实现编排层与执行层双重保护。

真实模型 EvalRun 仍走现有预算确认协议；生产 Run 的资源确认与 EvalRun 成本确认是两个独立快照。

### 8. RoleExecutor 采用 prepare/execute/finalize 协议

role step 在调用模型前完成：计划/租约校验、Run 创建时固定的 active Definition/model binding 校验、输入 package 解析、完整 Context 编译、ResourceSafetyGate 预占、`agent_runs` 与 prepared ModelCall 审计锚点持久化。模型返回后先执行完整 typed schema 校验，再在一个事务内写入该角色全部产物或协作建议、更新 step/run 用量和 `agent_runs` 终态。candidate Definition 只能进入 Eval/dry-run，普通 ProductionRun 创建和执行均不得绑定。

模型失败、解析失败、schema 失败和数据库失败都必须让 `agent_runs`、ProductionStep 与 ModelCall 留下可关联终态；不得只停留在 `running`。多产物角色不得部分写入。重复 finalize 使用 step/attempt/model_call 唯一键返回原结果。

角色输入由计划和 Gate 决定：需要 approved package 的步骤不得再采用“approved 优先、没有则 draft”的隐式降级。公开命令不得提交任意 context 或逐步 `preferred_model_id`；计划明确允许的用户补充指令必须作为带 actor、来源、trust=`user_instruction`、digest 和 revision epoch 的输入保存，并在影响当前 package 时先使旧 package/Gate 失效。

### 9. Editor/QC 必须检查真实媒体证据

作品运行只有在既有规范确认必需步骤成功且 `final_video` 已登记后，才能进入 Editor。系统先从当前 WorkGenerationRun 中实际成功且被最终合成消费的视频 step/attempt/output asset 确定性生成不可变 `RequiredTakeInventorySnapshot`；每个 take 具有稳定 take ID，并关联 WorkVersion、generation step/attempt、asset 和 segment。由于现有 VideoSegment 可以覆盖一个或多个 Scene，inventory 必须保存有序非空 `scene_ids[]`，并为每个 Scene 保存从已批准 ProductionPackage 确定性解析的非空 `shot_contract_ids[]`；不得虚构 take 与 Shot 一对一关系。缺少 Scene 或适用 ShotContract 集合、存在跨 Script/重复引用时不能进入评审。

系统随后为同一 WorkVersion 和 take inventory 建立不可变 `MediaEvidenceSnapshot`，只保存自管 asset ID、版本/hash、MIME、时长、对应 work step/segment/scene/shot/take 关系、能力版本和脱敏分析结果，不保存 base64、长期签名 URL、凭据或原始请求头。

媒体证据由受控 `MediaEvidenceProvider` 读取真实成片和分段：视觉审查必须使用满足 vision 要求的模型或明确版本的视觉分析器；音频审查必须使用可证明读取真实音轨的音频分析/ASR 能力。临时媒体访问只存在于调用期，审计快照保存资产引用。

`ContinuityLedger` 和 `TakeReview` 改为 append-only 版本实体，精确引用 run、revision epoch、WorkVersion、take inventory 和 MediaEvidenceSnapshot；ContinuityLedger 覆盖 required shot，TakeReview 一对一覆盖 required take，每个 TakeReview 引用适用的 ledger 版本。缺少 final media、shot/segment/take 映射、适用能力或任一必需证据时，Editor/QC step 返回稳定阻断原因。`QualityGate` 只接受同一 WorkVersion、同一 inventory digest 的完整当前集合；空集合、跨版本或重复覆盖不能通过。rejected/needs_revision 通过既有 Work Library 版本治理创建 `edit` 或 `full_regeneration` 草稿、差异计划和新的人工确认，不覆盖当前 WorkVersion，也不自动再次调用 provider。

### 10. API 以命令和持久化状态为中心

后端提供以下语义，具体 URI 在实现时保持 `/api/v1/production/` 前缀一致：

- 创建 Full Crew ProductionProject：必须提交真实 `project_id/topic_id` 和创建幂等键。
- 创建/启动 ProductionRun：返回持久化 `run_id`、计划版本、资源限制和首个状态，不接收角色数组。
- 查询 Run/Step/Package/Gate/DomainLink：返回当前状态、可执行命令、等待原因和审计引用。
- 对精确 package digest 做 approve/reject；旧 digest 返回冲突。
- 在人工 Gate 或外部领域状态满足后 resume；resume 仅唤醒，不直接越级推进。
- 对 `attention_required` 或可重试失败创建显式新 attempt；任何再次调用模型/provider 的动作继续要求相应资源确认。
- 显式 cancel 先持久化 cancellation intent；只在所有未完成 step 已停止且既有 WorkGenerationRun 的取消结果确定后进入 `cancelled`，结果不确定时保持 `attention_required`。
- ProductionProject 只有在从未创建 Run、没有过程产物和领域关联时才可删除；有历史 Run 时只能归档展示，不能删除或隐藏审计。

HTTP `202` 只表示持久化命令已接受，不表示运行已完成。错误码区分 source invalid、source locked、active intent conflict、stale package、transition conflict、resource limit、capability mismatch、external wait、attention required、cancellation pending 和领域集成失败。

所有命令从服务端稳定解析本地 actor（首版 `actor_type=local_operator`），不信任客户端自报 `user_id`。幂等身份固定为 `(actor, command_type, aggregate_id, idempotency_key)` 并保存 canonical request digest：同 key 同 digest 返回原命令结果，同 key 不同 digest 返回 `idempotency_conflict`；key 不得跨命令或聚合误命中。Gate、reject、retry、cancel 和领域晋升都服从此规则。

### 11. 外部作品状态必须显式传播

`wait_work_generation` 只观察既有 WorkGenerationRun，不重写其技术终态：`queued/running` 保持 external wait；`succeeded` 还必须验证 final media 和 required take inventory 后才解锁 Editor；`failed` 映射为带原错误和可重试性的 Production blocker；`waiting_manual/unknown_submission` 映射为 `attention_required`；`cancelling/cancelled` 只有在 ProductionRun 已请求取消时才能推进取消，否则显示外部取消冲突。任何外部 retry 都继续通过既有 WorkGeneration 接口和人工确认，Orchestrator 只关联返回的新正式 ID。

### 12. 来源实体在活跃流程中锁定

创建制作意图时必须验证 `projects.status=active`、Topic 同项目、未软删除且为 `approved`，并锁定 project/topic source fingerprint。active ProductionProject 存在期间，Topic 的内容、状态、归属和软删除均被拒绝；脚本晋升事务是唯一允许将其改为 `scripted` 的路径。账号策略等非身份配置后续变化不覆盖运行快照，但账号归档必须在存在 active Full Crew 时被阻断。失败或取消释放 Topic active-intent 锁，前提是没有成功脚本晋升或不确定领域提交。

### 13. Prompt 与 schema 以 candidate 版本迁移

screenwriter、director、cinematographer、performance_director、sound_director、editor 和 qc 的行为/输出契约变化发布新 candidate Definition/Prompt/Context Policy 引用，不原地修改 active 版本。先执行 registry/schema、Prompt 编译、结构化输出 fixture、历史 snapshot dry-run、包映射和 fake media/workflow golden tests。

行为变化版本只有在现有 Eval 治理要求满足后才能 active。任何真实模型评测必须先明确 case 数、最大 token、重试和成本上限并取得用户确认；未确认时 change 可以完成代码和零费用证据，但运行时不得激活或执行未获准 candidate。创建 ProductionRun 时必须一次解析所有 role step 的 active binding 和能力要求；任一角色无可用 active 版本或模型能力不满足时 Run 不得创建，禁止回退旧 schema、candidate 或请求级模型覆盖。

## Risks / Trade-offs

- [Risk] 一次 change 跨多个领域，事务和状态边界复杂。 → 以 typed Application Port 隔离 ScriptPromotion、SceneVisualManifest、WorkPlan 和 WorkVersionRework，并按任务分阶段集成，每阶段先写合同测试。
- [Risk] PostgreSQL 不能保证外部 provider exactly-once。 → 调用前持久化 attempt/idempotency，结果不确定时进入 `attention_required`，禁止透明重试；作品 provider 继续复用既有上游任务恢复规则。
- [Risk] 包级审批与现有单产物 approved 状态并存可能造成误解。 → Gate 只认可 package digest；单产物状态不再驱动流程，API 和审计明确返回 package decision。
- [Risk] 编剧 schema 变化会影响已有生产 Prompt。 → 新版本 candidate、golden/dry-run/Eval 门禁和显式激活，不修改历史快照。
- [Risk] 当前环境可能没有满足真实媒体 QC 的模型或分析器。 → fail-closed 并报告 capability blocker，不用文本或空评审降级；能力接入完成前流程可停在 media review 等待态。
- [Risk] Full Crew 多次模型调用和返工可能消耗大量资源。 → Run 固定资源限制、每步预占、返工次数上限、显式人工 Gate 和现有作品生成二次限制。
- [Risk] reject、脚本语义修订和 QC 返工会形成不同回流路径。 → 使用 append-only revision epoch 和固定影响图，只重新打开 owner 与确定性后继，不回写旧 step。
- [Risk] 取消发生在外部调用结果不确定时无法证明副作用已停止。 → 保持 `attention_required` 和资源预占，不虚假标记 cancelled，也不自动重试。
- [Trade-off] v1 禁止同一 Topic 并行竞案。 → 优先保证唯一正式脚本与资源安全；未来竞案必须显式建模 variant 聚合和晋升选择。
- [Trade-off] 本 change 不提供前端审批页面。 → 后端流程和 API 可完整验证，但正式操作者体验必须通过后续 Pencil 原型 change 接入；不得把临时 Admin 表单当作正式界面。

## Migration Plan

1. 先添加只增不删的数据库 migration、状态枚举、active intent/单 Run 条件唯一约束、真实引用、actor/幂等、取消、revision epoch 和 repository contract tests；已有未绑定 `production_projects` 标记为 `legacy_unbound`，只读保留，不能启动新 Run。
2. 新增持久化 Run/Step/RevisionEpoch/Package/Gate/DomainLink repository 和包含 reject/cancel/external terminal 的纯状态机，暂不接外部模型或作品任务。
3. 实现 ScriptPackage candidate schema、事务化 promotion 与正式 topic/script/scene 关联；完成零费用 fixture/golden 验证。
4. 将 RoleExecutor 切换到 step prepare/execute/finalize 协议，补齐协作建议、完整 schema、事务和审计终态；删除旧生产旁路和 stub flow 响应。
5. 接入 SceneVisualManifest 与 WorkGeneration typed port，验证只创建既有 WorkPlan/Run，且人工确认、幂等和资源规则保持不变。
6. 接入 RequiredTakeInventory、MediaEvidenceProvider、版本化 ContinuityLedger/TakeReview、Editor/QC 和 WorkVersion 返工端口；缺少映射或能力时验证 fail-closed。
7. 运行容器内 crate/backend 全量测试、migration replay、API 合同、重启恢复和跨领域回归；真实模型或真实媒体调用另行取得明确预算后执行。
8. candidate 通过规定评测后随代码发布激活；只有 active binding 可创建普通 ProductionRun。回滚只重新发布旧 supported Definition/二进制并停止新 Run；数据库保持向前兼容，已创建 Run/Step/审计不删除，未完成新 Run 标记为 migration/rollback blocked。

## Open Questions

无。具体可用视觉/音频 provider 和真实评测预算属于部署时显式配置与用户授权，不改变本设计的 fail-closed 契约。
