## Context

## 文本输入与 review verb

Worker 只加载 Registry index/approved metadata；`drama-mvp-a-default` 固定绑定 approved `novel-writing`、`drama-skills`。`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira` 均保持 `pending_provenance`/disabled，不是启动 lock 或默认 Run 前置，只有 `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 都满足才按需读取。

同一 Run、同项目、Schema-valid、hash/revision/scope 匹配的 provisional upstream candidate 可用于完整候选图；accepted owner fact 同样可读。仅不满足上述任一输入条件时拒绝。成员齐备后只建一次 immutable `TextReviewBatch`，successor/stale closure 与 CAS 均为全有或全无，不增加第二审核。candidate/batch 的正向 review verb 和 audit event 统一为 `accept`，拒绝为 `reject`；legacy/unknown `approve` validation 且零 handoff/ack/current/媒体副作用。

现有 `TextModelPort` 是同步端口，`DeterministicMockProvider`、LocalWorkspaceAdapter、SkillRegistry/SkillRouter 已具备，但没有 AgentScope 或外部 skill 包运行时。当前 Scene/Shot Schema 仅是基础身份模型，Story/Script 的完整字段尚未冻结。真实 Provider 和密钥管理不在本切片范围。

## 与总体计划的实施追溯

本设计直接落实 `plan-phase-one-drama-mvp-a` 的任务 **3.1**，并执行共享任务 **5.1**（UoW/审计/Outbox）、**5.3**（版本、scope 与跨项目拒绝）、**5.4**（`Mock Provider +` 显式 Local test/offline profile 和 opt-in 原始错误）、**5.7**（逐 change OpenSpec/status/strict）与 **5.8**（全量质量门）。直接实施依赖是 scenes、workflows、catalog 三个已实现切片提供的已确认事实、版本化 Run 标识和 capability snapshot；总体 change 仅说明依赖 DAG，不被导入、读取或调用为运行时代码。若任一直接依赖尚未实现或其契约未冻结，本 change 应停在依赖核验而非凭总体文档推断接口。

## Goals / Non-Goals

**Goals:**

- 以 Worker 依赖清单与 lock 单独引入可复现的 AgentScope 2.x runtime；Git Skill 固定 commit/digest，公开 Markdown Skill 固定 archive URL/获取时间/digest/license status，并验证 manifest/哈希/许可与 Registry 准入。
- 以已校验的 CreativeBrief 输入 snapshot，通过 TextModelPort 生成 StorySpec -> ScriptSpec -> Episode/Scene/Shot/ShotSpec 的完整结构化候选图；每个对象经过对应 JSON Schema 校验。
- 将候选与人工确认、输入输出版本、模型选择、Skill revision、prompt digest、token/cost 与错误审计持久化。
- 默认测试使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；真实调用只能由明确配置启用，未配置、Schema 无效或 provider 失败均保持可见且不得切换 profile。

**Non-Goals:**

- 不拥有 Provider/Profile/Model catalog CRUD、credential service、WorkflowRun/NodeRun/RunEvent/ProviderCall 状态或事件历史，不创建、执行或推进 WorkflowRun，也不实现图片/视频/音频生成、媒体二进制入库或 Timeline。
- 不让未确认候选覆盖 Project/Episode/Scene/Shot，不默认调用真实 Provider，不把总体协调 change、未完成的 scenes/workflows/catalog 或真实 Provider 当作运行时依赖或静默 fallback。
- 不创作或交付小说正文、章节正文、章节草稿或其他长篇 prose；`materialType=novel` 只表示可解析的改编来源类型，不扩大阶段一输出域。

## Decisions

### 固定依赖和技能装配

AgentScope 2.x 是 Agent Worker 的独立 runtime dependency，由 Worker 的依赖清单与 lock 管理，不属于 Skill vendor 目录。Skill 先按来源类型审计并固定 source identity：Git Skill 使用 commit/digest，公开 Markdown Skill 使用 archive URL、获取时间、digest 和 license status；固定内容快照只作为 Skill revision 的受控读取源。Worker 启动阶段只读取 Registry index 与 approved metadata，不读取候选正文；SkillRouter 完成确定性路由后，才按需读取选中 revision 的 `SKILL.md` 与必要 `references`。任何第三方脚本仍禁止执行。未匹配 Skill、manifest 损坏、来源身份或 digest 不符、许可未批准、未授权网络/子进程/文件/密钥访问均明确失败，不回退到未审计 prompt。

SkillRouter 继续使用阶段 0 已定顺序 `deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide`，但每次 launch/node resolution 必须持久化 immutable `SkillRouteDecision`：project/node/launch scope、router policy/version、输入 fingerprint、全部过滤前 identity refs、通过候选 SkillRevision ID/digest、每个淘汰/排序原因、lexical/semantic score/source、confidence/tie facts、状态 `selected|needs_human_selection|rejected` 和 decision revision。fixed 且唯一合法候选可直接 selected；inherit/多候选只有 policy 能唯一且达到阈值时可 selected。semantic adapter unavailable、低置信或并列必须保留确定性基础排序并返回 `needs_human_selection`，不能默认取第一项。

`ResolveSkillRouteDecision` 只能选择当前 decision candidate set 中仍 approved/enabled、digest/revision 未漂移且满足 allowedSkills/requiredCapabilities 的 SkillRevision，并绑定 decision expectedRevision 与 launch expectedRevision。成功追加 `SkillRouteSelection`/actor/audit ack，随后 workflows/runs 创建/启动 Run 并冻结最终 SkillRevision；在此之前不得读取候选正文、创建 NodeRun、调用 TextModelPort/Provider。相同 fingerprint 重试返回原 selection，不同选择或候选 drift 返回 409 并要求 reroute。设置页只改变 catalog 生命周期，不能替用户确认既有或新 launch 的 route。

### 结构化生成边界

application 定义 `StructuredTextGeneration` Command，输入为 projects owner 已校验的 CreativeBrief snapshot、typed scope union 和可选 adaptation SourceMaterial snapshot；初始创作只需 `projectId` scope，Episode/Scene/Shot 是候选输出的显式 target scope；adapter 将封装 prompt 后调用 TextModelPort。每一输出先以 Draft 2020-12 Schema 验证，再作为 immutable candidate 保存；同一 Run 内已校验的上游 candidate 可作为 provisional input，但不得投影为已确认领域事实或回写 projects owner。CreativeBrief 必含 `subject`、`genre`、`audience`、`characterPremise`、`style`、精确 `episodeCount`、`scenesPerEpisode`、`shotsPerScene` 与每集目标 `episodeDurationSeconds`；StorySpec 为项目级且含 logline/characters/conflict/beats/continuity，ScriptSpec 为每集级且含 episode goal/conflict/scene order，SceneSpec 含地点/时间/角色/道具/目标/情绪/对白/shot order，ShotSpec 含 durationFrames/framing/camera/action/dialogue/first-last-frame/audio/continuity。Scene/Shot 均有稳定 ID，ShotSpec 版本化并保存 AssetBible project -> episode -> scene -> shot 的 resolved references。所有对象均有 `schema_version`、来源 candidate/version；不合格 JSON、额外字段、跨项目引用、精确计数不符或不满足顺序/归属约束一律拒绝。

该 Command 的合法输出类型是 StorySpec、ScriptSpec、Episode、Scene、Shot 与 ShotSpec；不得接受 `novel_body`、`chapter`、`chapter_draft` 或语义等价 prose artifact。来源解析可以读取小说内容，但不得把来源类型误作小说创作 capability。

`creationMode` 与 CreativeBrief 由 projects owner 冻结。`SourceMaterial` 冻结 `creationMode=original|adaptation`，由 text owner 仅在 adaptation 下拥有。original 使用 projects owner snapshot 的六项创作语义、三个精确计数及 schema/revision，不要求 SourceMaterial 或 source snapshot；adaptation 必须为 `materialType=novel|synopsis|existing_script`、`inputMode=inline_text|uploaded_file` 的 SourceMaterial 保存 immutable revision/contentHash、parse/validation state 和 binding status/version。保存 source reference 时形成精确 `CreativeBriefSourceBindingSnapshot={projectId, sourceMaterialId, sourceMaterialRevision, sourceContentHash, creativeBriefId, creativeBriefRevision, creativeBriefPayloadHash, parseStatus, validationStatus, bindingStatus, bindingVersion}`；Run 创建后形成 `TextRunSourceBindingSnapshot` 并增加 `runId`、`runRevision`。uploaded_file 只经 StoragePort/Assets owner 交接 verified `assetVersionId`；inline_text 不创建 storage session、StoredObject 或 AssetVersion。invalid enum/scope/revision/hash/status、adaptation parse/validation 失败时不得创建或启动付费文本 Run，且不得改写 projects owner state。恢复只重用相同 source/brief revision，不隐式换源。

### 初始 AssetBible handoff

完整文本候选图还必须包含 Story/Script/Scene/Shot/ShotSpec 实际引用的初始 AssetBible entry specs，只允许 Character/Look/Location/SceneVisual/Prop/VisualStyle stable IDs、typed relationships 和 structured attributes。text owner 不解析 override、不保存 AssetBible current，也不复制 GenerationSpec prompt 正文或媒体；batch accepted 后先由 AssetBible owner typed command 落地 entries/versions 并返回 resolved snapshot refs，再由 Scene/Shot owner 校验并冻结这些 refs。Project/Episode/Scene/Shot/AssetBible 任一 ack 缺失都保持媒体门关闭。

### OpenAI-compatible TextModelPort adapter

真实 `TextModelPort` 由本 change 的 Agent worker adapter owner 实现，catalog 继续只拥有 Profile/Model/CapabilitySnapshot、credential envelope、model candidate diff 与 explicit accept。`OpenAICompatibleTextModelAdapter` 从 enabled profile 读取 base URL、endpoint path、timeout 和受限 CredentialResolver，以 adapter-bound Bearer/API-key authentication 发送结构化 generation request；Codex relay 为默认真实 profile，DeepSeek 仅为明确 opt-in profile，二者不得硬编码为业务 model。`GET /v1/models` 只产生 catalog candidate diff，手工 model 保留且只有 catalog accept command 能改变 enabled record。adapter 解析 request-id、native usage、JSON/structured payload；只对 transport/429/5xx/timeout 进行有上限 retry，未知提交先 reconciliation，authentication/validation 4xx、malformed response 或 schema failure 均不重试且不写 candidate。任何真实 request/probe 需要 enabled profile、有效 credential/master key 和明确 command；默认测试使用 `Mock Provider +` 显式 Local test/offline profile，零网络且运行开始后冻结 Adapter/Profile，日志/audit 不保存 secret。

### 人工确认和审计

candidate 状态为 `generated`、`pending_review`、`accepted`、`rejected`、`stale` 或 `superseded`；`TextReviewBatch` 状态为 `building`、`pending_review`、`accepted`、`rejected` 或 `superseded`。一次 Run 先生成并校验完整候选依赖图，再以精确 candidate IDs/hashes、source hashes、依赖边、payload hash 和 expected revisions 冻结一个 batch；逐对象 Schema 校验不插入人工暂停。用户在批次内修改上游候选时必须生成新候选并把所有依赖成员标记 `stale`，只有补齐并重新校验完整集合后 batch 才能回到 `pending_review`。唯一的 `AcceptTextReviewBatch`/`RejectTextReviewBatch` Command 校验 batch revision、scope 和完整成员集合；接受只在文本 owner 的 UoW 中追加 TextReview accepted handoff、StorySpec/ScriptSpec facts 与审计/Outbox。owner-defined orchestration 随后以 handoff id/revision/correlation、candidate/source hashes、payload hash 和 expected revisions 幂等调用 Project/Episode/Scene/Shot 各自的 typed batch command，并记录 matching owner ack；任一 owner command 失败时保持 projection pending/failed 与原始诊断，不能解锁付费媒体。文本 owner 不直接越过这些 aggregate ownership，拒绝不产生 handoff 或领域版本。审计记录审核人、时间、基础/输出版本、脱敏 prompt digest、input/output schema/version/hash、model/provider snapshot、Skill source identity、usage/cost；不保存密钥或隐私原始 prompt。Agent Worker 的 restart/reconcile、temporary cleanup 或日志维护不得删除、覆盖或静默压缩关联的 `RunEvent`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和仍被引用的 `AssetVersion`；它只验证 workflows/runs、catalog 与 Assets owner 的长期引用，不复制这些事实或接管其 GC policy。

### DB/API/测试

新增 skill_route_decision/selection、text_generation_candidate、text_review_batch/member、text_generation_audit、confirmation/outbox 和 owner-handoff/ack 表及外键/项目归属/状态/唯一 hash 约束；Project/Episode/Scene/Shot 只通过各自 owner 的 typed batch command 创建或更新，文本 migration/repository 不接触其表。数据库与共享 Schema 的 `schema_version` 是 SkillRouteDecision、CreativeBrief、StorySpec、ScriptSpec 与 TextReviewBatch 的唯一文本版本事实；owner handoff 只携带各 aggregate owner 定义的 target id、expected revision、payload hash 和 command schema version，不复制其版本事实。HTTP camelCase DTO 的 `schemaVersion` 只映射对应 canonical 值。请求缺少必需版本、同时给出冲突的 `schema_version`/`schemaVersion` 或实现双独立赋值时，必须在 UoW 与模型调用前返回稳定 validation error，不写 route decision/selection、candidate、batch、generation audit、confirmation、owner handoff/ack、领域事实或 Outbox，也不创建 RunEvent/ProviderCall。HTTP 使用显式 route resolve/select、scope 和 batch confirm/reject endpoint；默认 provider 为 deterministic mock。TDD 覆盖 domain、application、registry/AgentScope adapter、Schema、HTTP/BDD、DB migration、审计、owner command/ack、版本映射、未配置真实调用，以及超过诊断窗口、不同 hold 和 restart/reconcile 后的跨 owner no-GC fixtures。

## Risks / Trade-offs

- [外部 skills 漂移或供应链替换] -> AgentScope runtime 依赖使用独立 lock；Skill 按来源类型固定 source identity/digest、manifest 验证、network/subprocess/file/secret 访问证据及审计保存 revision；未授权能力 fail closed。
- [模型返回格式不可靠] -> 强制结构化 Schema 验证，保留原始脱敏错误并不产生候选事实。
- [候选被自动采纳] -> 所有下游变更必须经人工 Confirm Command、expected revision 和审计。
- [真实调用成本或不可用] -> 默认 `Mock Provider +` 显式 Local test/offline profile、显式 opt-in、usage/cost 审计与无 fallback 失败。

## Migration Plan

1. 定义 AgentScope runtime dependency lock、按来源类型区分的 Skill provenance manifest、结构化 Schema 与失败测试，先确认 Git commit/digest 或公开 archive URL/获取时间/digest/license status 可获取。
2. 添加 additive Alembic revision，建立 candidate/audit/confirmation/outbox 事实表和项目/版本/状态约束。
3. 实现 AgentScope adapter、TextModelPort composition、Commands/Queries/HTTP；默认测试注入 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），真实 Worker adapter 仅显式 opt-in。
4. rollback 禁用新路由/Worker 注册；候选和审计为只追加记录，不用迁移脚本伪造确认结果。

## 待实现取证

- AgentScope runtime dependency 与两个 Skill 的实际审计结果必须分开记录：AgentScope 记录依赖版本与 lock digest；Git Skill 记录 commit/digest，公开 Markdown Skill 记录 archive URL/获取时间/digest/license status；所有 Skill 另记录 manifest、network/subprocess/file/secret access matrix 和脚本入口。未通过审计或包含未授权能力/脚本执行要求时不得进入 approved Registry。
- native model usage 字段和审核人身份来源按 adapter/身份边界验证；不得改变已冻结的最小 Story/Script/Scene/Shot 字段或一次批量文本审核边界。

## DDD / BDD / SDD / TDD

- **DDD**：候选、TextReviewBatch、确认和审计是追加事实；逐对象 Schema 校验后只经过一次批量人工审核。
- **BDD**：覆盖精确计数、Schema 无效、Skill drift、未审核付费暂停和确认冲突。
- **SDD**：固定 vendor 目录、最小 Spec 字段、AssetBible 覆盖和 Mock/opt-in 边界。
- **TDD**：先写 schema/candidate/registry 失败测试，再验证 adapter、HTTP、migration 与 BDD。

## Current / Defined / Todo

- **Current**：TextModelPort、Mock Provider、显式 Local test/offline profile 和确定性 SkillRegistry/Router 已有，AgentScope/text chain 未实现。
- **Defined**：审计后 vendor、完整结构化候选图、一次批量审核、stale 传播和付费暂停。
- **Todo**：完成此 change 的未勾选任务和依赖取证。

## 默认 Workflow 端口契约

**DDD**：TextReviewBatch 是文本端口的 immutable accepted handoff，不改变 Run/Timeline 所有权。**BDD**：partial/stale/foreign batch 不可 handoff。**SDD**：端口 payload 仅含 owner-defined IDs/hashes/revisions/reference facts。**TDD**：先覆盖 accepted/foreign/stale handoff 和零媒体副作用；兼容既有 candidate CAS 与 `Mock Provider +` 显式 Local test/offline profile，非目标是 UI 端口推断和真实 Provider fallback。验收沿用本 change 的 strict、定向测试与 `pnpm run check`。

编辑上游 Text candidate 必须 append successor，并标记可达依赖闭包 stale。regenerate command 必须冻结 run/brief/batch revision、source candidate ids/hashes 与 expected revisions，逐对象通过 schema/count/scope/hash 后才 append successor candidates 和新的 immutable TextReviewBatch；旧对象不改写，批次 CAS 为全有或全无。
