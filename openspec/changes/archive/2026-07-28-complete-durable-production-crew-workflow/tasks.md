## 1. 数据库边界与持久化契约

- [x] 1.1 先在 `backend/tests/database_migrations.rs` 添加失败断言，覆盖 Full Crew 制作意图绑定、Run/Step、计划快照、package/Gate、资源预占、租约、媒体证据和领域关联所需的表、字段、外键、CHECK、唯一键与索引。
- [x] 1.2 新增只增不删的 SQL migration：扩展 `production_projects` 的 `project_id/topic_id` 与来源快照，保留既有未绑定数据为只读 `legacy_unbound`，并禁止其启动新 Run。
- [x] 1.3 在同一 migration 中创建持久化 `production_runs`、`production_steps`、不可变 package/Gate、资源用量、建议响应、媒体证据及真实外键 domain link 结构，并为 JSONB、枚举和约束补充业务注释。
- [x] 1.4 先添加 PostgreSQL repository contract tests，覆盖制作意图与完整 Run/Step 集合的原子创建、幂等重放、版本读取、状态更新和失败回滚。
- [x] 1.5 实现 Full Crew PostgreSQL repositories 与事务接口，确保流程查询完全从 PostgreSQL 重建且不依赖 Redis payload 或进程内状态。
- [x] 1.6 添加旧 schema 升级与全新数据库 replay 测试，验证历史 `production_projects` 可只读查询、现有脚本/作品数据不被复制或破坏，且 migration 可重复应用于项目测试基线。
- [x] 1.7 先添加数据库并发测试，覆盖同一 Topic 只能存在一个 active Full Crew 制作意图、每个制作意图只能有一个 Run、终态且未晋升时可释放 active-intent 锁，以及相同幂等 key 不同 request digest 冲突。
- [x] 1.8 扩展 migration 与 repository：增加 source fingerprint/lock、revision epoch、cancellation intent、稳定 actor、命令 request digest、active intent/单 Run 条件唯一约束，并确保 legacy/unbound 数据不参与新约束。
- [x] 1.9 先为 ContinuityLedger、TakeReview、RequiredTakeInventory 和 MediaEvidence 添加迁移失败测试，要求真实 run/step/attempt、WorkVersion、inventory/evidence、version/digest 外键与 append-only 唯一约束。
- [x] 1.10 迁移现有质量表为按 Run/WorkVersion/attempt 追加版本结构；历史无精确来源记录标记 `legacy_partial_audit` 且不得进入新 QualityPackage，不伪造 version、take 或媒体映射。

## 2. 固定计划、状态机与恢复

- [x] 2.1 先为 Full Crew v1 计划添加纯单元测试，覆盖固定 step key/type/DAG、可选 character critic、受限并行、各 package 最大修订次数、QC 最大返工次数、模型 binding 摘要和 canonical plan digest。
- [x] 2.2 实现版本化 Full Crew 计划注册表与 Run 计划冻结逻辑，使新版本只影响新 Run，既有 Run 始终按原计划快照推进。
- [x] 2.3 先为 Run/Step 纯状态机添加表驱动失败测试，覆盖 role、gate、domain_command、external_wait 的合法转换、依赖解锁、等待原因、终态及越级冲突。
- [x] 2.4 实现不产生 I/O 的状态机与可执行命令推导，并让所有推进命令统一校验当前状态、依赖、计划版本和幂等键。
- [x] 2.5 先添加并发数据库测试，证明同一 step 只能被一个 worker 原子 claim，重复唤醒不产生第二个 attempt，未产生外部副作用的过期租约才可被重新认领。
- [x] 2.6 实现 lease owner/expiry/attempt 的 claim、续租、释放与恢复扫描；对已发出但结果不确定的模型/provider 调用持久化 `attention_required`，禁止透明重试。
- [x] 2.7 添加 Redis 丢消息、重复消息以及 API/worker 重启后的集成测试，验证已完成步骤不重跑、queued 步骤可重新唤醒、等待审批和 external wait 状态可恢复。
- [x] 2.8 实现只携带 `run_id/step_id` 的 Redis 唤醒适配器和基于 PostgreSQL 的恢复调度器，不在 Redis 中保存任何权威流程状态。
- [x] 2.9 先扩展状态机表驱动测试，覆盖三类 package reject/revision epoch、脚本语义回流、修订上限、`cancelling/cancelled`、外部 `failed/waiting_manual/cancelled` 及删除/归档命令合法性。
- [x] 2.10 实现 append-only revision epoch 与固定影响图：reject 只重新打开 owner 和确定性后继，旧 step/package/Gate 不可回写或越级推进。
- [x] 2.11 实现 cancellation intent 与终止协调：停止解锁新 step，按副作用确定性释放或保留资源预占，调用既有 WorkGeneration 取消端口，并在不确定结果下保持 `attention_required`。
- [x] 2.12 先添加来源生命周期集成测试，覆盖 active 账号校验、活跃流程期间 Topic 编辑/状态/归属/软删除和账号归档被拒，以及安全失败/取消后 Topic 锁释放。
- [x] 2.13 在 Topic/Project Application 边界实现 source lock 协议；账号策略后续变化不覆盖 Run 快照，ScriptPackagePromotion 是锁定 Topic 唯一允许的 `approved -> scripted` 路径。

## 3. 不可变产物包、Gate 与协作所有权

- [x] 3.1 先为 BriefPackage、ScriptPackage、ProductionPackage、QualityPackage 添加 canonical fixture 测试，覆盖稳定排序、组成产物版本/content digest、来源 step 与 package digest。
- [x] 3.2 实现不可变 `ArtifactPackageSnapshot` 构建与持久化服务，拒绝缺失、跨 Run、schema 无效或引用不一致的组成产物。
- [x] 3.3 先为包级审批添加合同测试，覆盖当前 digest approve/reject、旧 digest `stale_package`、重复决策幂等、单产物 approved 不推进及决策审计字段。
- [x] 3.4 实现包级 GateDecision 服务与状态机联动，使 Gate 只解锁精确已批准 package 的后继步骤，新产物版本自动使旧决策失效。
- [x] 3.5 先为 `collaboration_suggestions` 添加 repository/领域测试，覆盖来源 ModelCall、目标 owner/产物版本、阻断级别、不可变接受或拒绝响应及非空拒绝理由。
- [x] 3.6 实现协作建议写入和 owner 修订义务：建议不得直接修改 owner 产物，接受后的新版本必须引用建议 ID，阻断建议未闭合时禁止批准 ProductionPackage。
- [x] 3.7 先为 Gate reject 添加合同测试，覆盖非空理由、受影响 owner、同 digest 同决策幂等、相反决策冲突、新 revision epoch、旧 approval 失效及修订上限。
- [x] 3.8 实现 Brief/Script/Production Package reject 服务，并将 optional character critic 建议纳入 ScriptPackage suggestion resolution；拒绝不得修改 Topic、正式 Script 或旧产物。

## 4. 非金额资源安全

- [x] 4.1 先为 `ResourceSafetyGate` 添加并发和边界测试，覆盖 role/model 调用数、输入/输出 token、role retry、返工环数、视频任务/总时长、TTS 字符、ASR 数量、并发与 provider retry。
- [x] 4.2 实现 Run 资源限制快照、调用前原子预占、终态实际用量结算和失败释放规则，并返回具体限制项、当前值与上限。
- [x] 4.3 添加 schema/API 回归断言，确保资源结构和审计响应不含价格、币种、金额上限、API Key、认证头或原始 provider 凭据。
- [x] 4.4 用 `ResourceSafetyGate` 替换 Full Crew 的 `BudgetGate` 注册与调用路径，删除固定单价/金额判断，且不得通过截断输入、换模型或部分提交绕过超限。
- [x] 4.5 添加编排层与既有 WorkGeneration 执行层双重资源保护的合同测试，证明任何超限都发生在新的模型或媒体副作用之前。

## 5. 角色输出契约与原子执行协议

- [x] 5.1 先为 producer、screenwriter、director、cinematographer、performance_director、sound_director、editor、qc 的完整 typed output 添加合法/非法 fixture 测试，覆盖必填字段、领域引用、数量、顺序与时长约束。
- [x] 5.2 扩展角色 schema 和验证器：screenwriter 输出正式脚本字段；ShotContract 引用真实 Scene UUID；PerformanceBrief 引用真实 Character/Scene；SoundPlan 引用真实 Scene；Editor/QC 输出引用真实 WorkVersion/take/inventory/evidence。
- [x] 5.3 先为 RoleExecutor prepare 阶段添加测试，覆盖 step/lease 校验、Run 创建时固定的 active Definition/model binding、精确 input package、Context 审计锚点、资源预占及 `agent_runs`/prepared ModelCall 创建，并证明 candidate 不能进入普通 Run。
- [x] 5.4 实现 prepare 阶段并移除“approved 优先、没有则 draft”的输入降级，任何缺少前置 package、能力或审计锚点的 role step 必须在模型调用前阻断。
- [x] 5.5 先为 execute/finalize 添加故障注入测试，覆盖模型失败、解析失败、schema 失败、数据库失败、多产物部分写入和相同 attempt 重复 finalize。
- [x] 5.6 实现 execute/finalize 事务：原子保存同一角色的全部产物或协作建议、用量、step 与 `agent_runs` 终态，并让重复 finalize 返回原结果而不创建新版本。
- [x] 5.7 添加审计闭合回归测试，确保成功、失败、阻断和 `attention_required` 均可由 ProductionStep 关联到 AgentRun、ModelCall、Context 与产物或错误证据，不残留无期限 `running`。
- [x] 5.8 先为 ProductionPackage 集合完整性添加 fixture/合同测试，覆盖每 Scene 至少一个 Shot、每被引用 Character 一个 PerformanceBrief、SoundPlan Scene 闭合、跨 Script/重复/自由字符串引用和时长顺序错误。
- [x] 5.9 实现类型化 Scene/Character/Shot 引用与 package cardinality 校验，所有过程产物必须携带 run/step/attempt/revision epoch，禁止按 ProductionProject 查询“最新产物”跨 Run 拼包。
- [x] 5.10 先添加动态输入拒绝测试，覆盖 `preferred_model_id`、任意 context、未声明 user_input；允许的修订指令必须保存 actor/trust/digest/epoch 并使受影响旧 package 失效。
- [x] 5.11 移除生产 API 的请求级模型覆盖和自由 context，受控用户指令只通过计划声明的修订命令进入 Context Compiler。

## 6. ScriptPackage 事务化晋升

- [x] 6.1 先为确定性 ScriptDraft mapper 添加单元测试，覆盖 `title/hook`、连续 scene sequence、narration、visual_description、emotion、duration_sec 以及 Story/Character 引用一致性。
- [x] 6.2 实现零模型调用 mapper，并在 schema 缺失、顺序错误、时长非法或引用不一致时拒绝整个 ScriptPackage，禁止额外 LLM 修补。
- [x] 6.3 先添加 ScriptPackagePromotion PostgreSQL 集成测试，覆盖同项目 `approved` 选题、精确 Gate digest、正式 `approved` Script/Scene、来源快照、domain link 与 Topic `scripted` 的同事务提交。
- [x] 6.4 添加晋升失败、陈旧 package、跨项目/软删除 Topic、Topic 已被消费、重复幂等键和并发晋升测试，证明不留下部分 Script/Scene 或虚假 `scripted` 状态。
- [x] 6.5 在脚本/选题 Application 边界实现 `ScriptPackagePromotionService` 事务端口，锁定 Topic 与 package，原子创建正式数据并让重放返回原 Script/Scene。
- [x] 6.6 先为正式脚本的后续引用和版本治理添加测试：Director 只能使用同一 Script 的真实 Scene ID，语义修改必须派生带 `parent_id` 的新 Script，并使旧 ProductionPackage/WorkPlan 失效。
- [x] 6.7 实现正式 Scene 引用校验与脚本新版本失效传播，保证 ProductionPackage 仅驱动画面/作品计划且不能原地覆盖已批准 Script/Scene。
- [x] 6.8 先添加脚本语义修订回流测试：Director 建议只能创建 screenwriter revision epoch，新 ScriptPackage 重新审批/晋升为带 `parent_id` 的 Script，旧 package/manifest/plan 失效且历史 WorkVersion 不覆盖。
- [x] 6.9 实现 script revision typed command 和确定性影响传播；仅制作表达变化只修订 owner 过程产物，不滥建 Script 版本。

## 7. 现有画面与作品生成链路集成

- [x] 7.1 先用 fake port 为 ProductionPackage typed input 添加合同测试，覆盖 Script/Scene、ShotContract、PerformanceBrief、SoundPlan、已闭合建议及来源 package digest 的完整映射。
- [x] 7.2 定义并实现 ProductionOrchestrator 到 SceneVisualManifest/WorkGenerationService 的类型化 Application Port，Orchestrator 只保存返回的正式 ID/version/digest。
- [x] 7.3 先添加主画面 readiness 测试，覆盖缺失/陈旧/跨 Script manifest 的具体 blocker，并证明 blocker 存在时不创建可确认 WorkPlan、WorkGenerationRun 或 provider 任务。
- [x] 7.4 实现 `wait_scene_visual_manifest` external wait 与恢复逻辑，在正式 manifest 完整且 input version 有效后才允许规划作品。
- [x] 7.5 先添加 WorkPlan 集成测试，覆盖创建或更新同一 Script 的既有 Work 草稿/版本/计划、可见 Prompt/时间线/声音建议、来源引用及 ProductionPackage/Script/manifest/参数变化后的旧计划失效。
- [x] 7.6 扩展既有 WorkGenerationService 规划入口以接受 typed ProductionPackage 输入，复用现有 Work/WorkVersion/WorkPlan 表和 repository，不创建第二套作品或视频任务。
- [x] 7.7 添加既有人工确认接口的幂等测试，覆盖非金额资源展示、重复 Idempotency-Key 返回原 WorkGenerationRun、ProductionRun domain link 与 external wait 状态。
- [x] 7.8 实现确认结果和作品运行终态同步端口；ProductionOrchestrator 不得直接插入 `work_generation_steps`、调用 provider 或把 HTTP `202` 当成运行成功。
- [x] 7.9 先添加 Full Crew WorkPlan override/fingerprint 测试，覆盖 Prompt、主画面、模型、音色、声音模式、字幕、时间线和输出参数变化，要求保存 override diff、旧计划失效和重新确认。
- [x] 7.10 扩展 typed plan input 与 WorkPlan fingerprint，使人工 override 进入 WorkVersion/Plan/Run 快照但不回写 ProductionPackage。
- [x] 7.11 先添加外部终态映射测试，覆盖 WorkGeneration `failed/waiting_manual/unknown_submission/cancelling/cancelled/succeeded-without-evidence`，证明不会自动重试或提前运行 Editor/QC。
- [x] 7.12 实现外部状态观察与命令协调端口：保留 WorkGeneration 真实技术终态，retry/cancel 只调用既有正式接口并关联返回 ID。

## 8. 真实媒体证据、Editor/QC 与返工

- [x] 8.1 先为 `RequiredTakeInventorySnapshot` 与 `MediaEvidenceSnapshot` 添加合同测试，覆盖 final compose 实际消费的 generation step/attempt/output asset、work/version hash、MIME、时长、segment 的有序 Scene 集合、每 Scene 的确定性 ShotContract 集合、视觉/音频能力版本与脱敏分析结果，并证明不虚构 take/Shot 一对一关系。
- [x] 8.2 实现受控 `MediaEvidenceProvider` 和不可变快照持久化，临时媒体访问只存在于调用期，数据库不得保存 base64、长期签名 URL、凭据或原始请求头。
- [x] 8.3 先添加 Editor/QC capability 测试，覆盖缺 final media、映射、vision、音频/ASR 或任一必需证据时的稳定 fail-closed blocker，并证明不会退化为文本猜测。
- [x] 8.4 将媒体能力校验、真实资产读取证据和 QualityPackage 接入 Editor/QC role step，只有完整快照可进入模型/分析器调用。
- [x] 8.5 先为 QualityGate 添加覆盖性测试，要求当前 WorkVersion 每个 required shot 有 ContinuityLedger、每个 required take 有唯一 TakeReview，空集合、陈旧版本、重复或部分覆盖均不得通过。
- [x] 8.6 实现 QualityGate 的媒体版本/digest/全 take 覆盖检查，并区分作品技术终态与 ProductionRun 的质量批准状态。
- [x] 8.7 先为局部 `edit`、全局 `full_regeneration` 和返工上限添加 Work Library 集成测试，验证原 WorkVersion、媒体、运行与 QC 证据不可变，差异计划仍需新一次人工确认。
- [x] 8.8 实现 WorkVersionRework typed port、差异计划与受限返工环；达到上限时进入明确人工终止/新制作意图状态，不自动再次调用 provider。
- [x] 8.9 实现确定性 required take inventory 构建器，只纳入 final compose 实际消费的成功输出；按 WorkPlan segment 保存有序 Scene 集合并从 ProductionPackage 解析每 Scene 的 ShotContract 集合，缺失、跨 Script、重复或歧义映射时 fail-closed。
- [x] 8.10 先添加质量产物版本隔离测试，覆盖跨 WorkVersion、旧 inventory digest、重复当前 review、旧 QC 决策和返工新版本的独立 inventory/evidence/ledger/review 集合。
- [x] 8.11 实现 append-only ContinuityLedger/TakeReview repository 与 QualityPackage builder：ledger 覆盖 required shot，review 一对一覆盖 required take，禁止读取 legacy/跨版本记录推进 Gate。

## 9. Production API 与旁路收敛

- [x] 9.1 先添加 API 合同测试，覆盖同账号 `approved` Topic 创建 Full Crew、创建/启动 Run、查询 Run/Step/Package/Gate/DomainLink、approve/reject、resume 和显式 retry。
- [x] 9.2 添加拒绝测试，覆盖跨账号、软删除或非 approved Topic，以及请求中的任意 `roles`、`auto_approve`、跳过 Gate、计划替换和越级单角色执行。
- [x] 9.3 重构 `/api/v1/production/` DTO、handler 与 Application Service，删除内存/stub flow 响应和客户端角色序列，所有命令先持久化再通过 Orchestrator 推进。
- [x] 9.4 将生产单角色命令收敛为当前 Run/Step 命令，复用相同 lease、package、资源、active binding 和审计；保留 Fast Lane 非目标边界且不复制兼容流程。
- [x] 9.5 先添加状态响应测试，覆盖 `waiting_approval`、`external_wait`、resource/capability blocker、`attention_required`、领域冲突、可重试性、允许命令和相关审计 ID。
- [x] 9.6 实现稳定错误码与 HTTP 语义：异步命令接受返回 `202` 和持久化 ID，陈旧 package 返回冲突，查询不得泄露凭据、未脱敏响应或金额字段。
- [x] 9.7 添加路由级重启恢复和重复命令测试，证明查询状态来自 PostgreSQL，重复 start/resume/retry 不会重复调用模型、晋升脚本或创建作品运行。
- [x] 9.8 先添加 actor/幂等 API 测试，证明服务端使用稳定 local_operator、拒绝客户端 user_id，同作用域同 key 同 digest 重放、同 key 异 digest 冲突，且 key 不跨 command/aggregate 命中。
- [x] 9.9 实现统一 ProductionCommandStore 与 canonical request digest，覆盖 create/start/approve/reject/resume/retry/cancel/promotion/rework 命令。
- [x] 9.10 先添加取消、删除和归档 API 合同测试：空白制作意图可删除，有 Run/产物/关联的制作意图只能归档；取消结果不确定不得返回 cancelled。
- [x] 9.11 实现 cancel/archive 命令和状态响应，移除随机 UUID 用户占位；有历史的 ProductionProject 及其审计保持可查询。

## 10. Candidate 角色版本与零费用评测门禁

- [x] 10.1 为受影响角色新增 candidate Definition、Prompt、Context Policy/output schema 版本，并先添加 registry/schema/Prompt 编译测试，禁止原地修改 active 版本或历史快照。
- [x] 10.2 添加结构化输出 fixture、历史 snapshot dry-run、package mapper、fake media 和 fake workflow golden tests，覆盖所有角色在新固定计划中的输入输出契约。
- [x] 10.3 添加发布门禁测试，证明缺少符合治理要求的不可变 EvalReport 时 candidate 不能用于普通 ProductionRun，新 Run 只固定已 active 版本，既有 Run 不随发布漂移。
- [x] 10.4 建立真实 EvalRun 授权分支：先生成 case 数、模型 fingerprint、token、重试和成本上限清单；仅在用户明确确认后运行并按报告决定激活，未获确认时验证零次真实调用并保持 candidate inactive。
- [x] 10.5 对任何真实视频、TTS、ASR 或媒体分析验收应用同一授权原则；未获成本确认时只运行 fake/fixture 测试，并将 provider 能力不足保留为可查询的 fail-closed 部署阻断。

## 11. 端到端回归与交付验证

- [x] 11.1 在容器内运行 production crew crate 单元/集成测试以及 backend 直接相关的 migration、production、topic、script、asset、work generation、work library、model call 和 eval 测试。
- [x] 11.2 添加并运行零费用 Full Crew 端到端测试：从 approved Topic 到 package Gate、脚本晋升、fake manifest/WorkPlan、人工确认、fake generation、媒体 QC、完成与受限返工。
- [x] 11.3 运行并发、幂等、故障注入和重启恢复测试矩阵，覆盖 active intent/单 Run 竞争、来源锁、三类 Gate reject、脚本修订 epoch、取消、数据库提交前后、Redis 丢失/重复、模型/provider 结果不确定及全部 WorkGeneration external terminal 恢复。
- [x] 11.4 运行 `cargo fmt --check`、workspace build/test 和数据库 migration replay，修复全部直接或上下游回归，并确认 Pi Runtime、Fast Lane、既有 Worker DAG 与发布运营行为未被本 change 改写。
- [x] 11.5 复核 OpenSpec scenarios 与实现测试的追踪关系，更新必要的开发/API 文档和本 change 的任务勾选；真实付费验证若未授权，必须明确记录 candidate 未激活和部署阻断，不得宣称生产可用。
- [x] 11.6 增加逐 Requirement/Scenario 追踪表，特别证明 candidate 零次生产执行、旧 revision/旧 WorkVersion 不串用、任意 context/model override 被拒、actor/幂等冲突和删除审计保留均有自动化证据。

## 12. 代码审查整改与生产闭环

- [x] 12.1 添加真实进程级 Full Crew Runner 合同测试，并实现 PostgreSQL 恢复扫描、Redis 最小唤醒消费、step claim/执行/finalize、Gate/领域命令/external wait 调度和优雅停机；不得再由测试直接改 SQL 模拟 E2E 推进。
- [x] 12.2 添加 active Definition 与 durable typed output 的兼容性预检和回归测试；普通 Run 创建前必须证明每个 active Prompt output schema 满足角色强类型契约，未通过 Eval 的 candidate 继续禁止进入生产。
- [x] 12.3 添加真实执行资源计量测试并补齐 role retry、quality rework、视频任务/时长、TTS、ASR、并发和 provider retry 的编排层原子预占、结算或释放，且继续保留 WorkGeneration Worker 二次保护。
- [x] 12.4 添加跨默认模型/active Definition 变化的 `start_run` 重放测试，确保相同客户端请求和 Idempotency-Key 始终返回原 Run，只有客户端请求 payload 变化才返回 `idempotency_conflict`。
- [x] 12.5 修正 Context Compiler 对 `required_sources` 的存在性校验和 candidate Context Policy：初始执行不得要求不存在的修订指令，修订执行必须将受控指令作为 required candidate；补齐缺失、过期和初始/修订场景测试。
- [x] 12.6 实现并在生产 AppState 注入版本化、受控的真实 `MediaEvidenceProvider`，只通过调用期临时访问读取自管媒体，输出视觉/音频能力证据和脱敏分析；缺少能力时在副作用前稳定 fail-closed。
- [x] 12.7 为后续新增的脚本/package 失效事实和 TakeReview-Ledger 精确版本表补充数据库级 append-only trigger 与 migration replay/直接 mutation 拒绝测试，并重跑跨服务回归、更新追踪表和交付验证。
