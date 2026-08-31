## ADDED Requirements

### Requirement:文本批次包含实际引用的初始 AssetBible 规格
完整结构化候选图 SHALL 包含 StorySpec/ScriptSpec/Scene/Shot/ShotSpec 实际引用的初始 Character/Look/Location/SceneVisual/Prop/VisualStyle entry specs、稳定 IDs、typed relationships、`schema_version` 和 candidate hashes，并纳入同一个 TextReviewBatch 的依赖/stale/CAS 集合。接受后 text owner SHALL 通过 typed handoff 调用 AssetBible owner；AssetBible owner 落地 immutable entries/versions 并返回 resolved snapshot refs，Scene/Shot owner才可冻结这些 refs。text owner MUST NOT 写 AssetBible current/override/task 或复制 GenerationSpec prompt 正文/媒体。Project/Episode/Scene/Shot/AssetBible 全部 matching ack 前不得解锁付费媒体。

#### Scenario:接受含初始资产设定的完整文本批次
- **WHEN** batch 的叙事对象和其实际引用的初始 entry specs 全部 Schema-valid、non-stale、同项目且精确 hashes/revisions 匹配
- **THEN** text owner 追加一个 accepted handoff，AssetBible 与其他 aggregate owners 幂等落地并返回独立 ack；全部 ack 后媒体门才可打开

#### Scenario:资产设定缺失或 ack 失败时阻断
- **WHEN** ShotSpec 引用的 entry candidate 缺失/stale/foreign/hash mismatch，或 AssetBible owner ack 缺失/失败
- **THEN** batch accept/owner projection 保持 validation/pending/failed，零 image/video Provider submission，且 text owner 不直接补写 AssetBible

### Requirement:可审计且可人工裁决的 Skill 路由
系统 SHALL 对每次 launch/node skill resolution 按 `deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide` 产生 immutable `SkillRouteDecision`，保存 project/node/launch scope、router policy/version、input fingerprint、候选 SkillRevision ID/digest、淘汰/排序原因、lexical/semantic score/source、confidence/tie facts、decision revision 与 `status=selected|needs_human_selection|rejected`。只有 fixed/唯一合法或 policy 唯一且达到阈值的候选可自动 selected；semantic adapter unavailable、低置信或并列 MUST 返回 `needs_human_selection` 并保留确定性基础排序，MUST NOT 默认选择第一项。

#### Scenario:路由唯一候选并保存原因
- **WHEN** allowedSkills/requiredCapabilities/provenance/approval/enabled/schema/tool policy 过滤后只有一个合法 SkillRevision
- **THEN** 系统保存 selected decision、完整过滤/排序原因和 candidate digest，且只按需读取该 revision 正文

#### Scenario:歧义时等待人工选择
- **WHEN** 合法候选并列、低置信或 optional semantic adapter unavailable 且 policy 无法唯一裁决
- **THEN** 系统返回 `needs_human_selection` 和候选/原因，零 Run/NodeRun/TextModelPort/Provider 调用，不读取候选正文或默认取首项

### Requirement:人工 Skill selection 与最终冻结
`ResolveSkillRouteDecision` SHALL 只接受当前 candidate set 中仍 approved/enabled、ID/digest/revision 未漂移且满足 node allowedSkills/requiredCapabilities 的 SkillRevision，并绑定 decision expectedRevision、launch expectedRevision、project/node scope 和稳定 actor UUID。成功 MUST 追加 immutable `SkillRouteSelection`/audit ack，并由 workflows/runs 在 Run create/start 时冻结最终 SkillRevision；设置页的 create/edit/enable/disable MUST NOT 代替本次 route selection。相同 fingerprint 重试返回原 selection，不同选择或 stale candidate 返回 409 并要求 reroute。

#### Scenario:人工选择当前候选后启动
- **WHEN** 用户从 `needs_human_selection` 的当前候选中选择合法 SkillRevision 且 expected revisions 匹配
- **THEN** text owner 追加 selection/ack，workflows/runs 冻结该 SkillRevision 后才允许创建/启动 Run 和调用 TextModelPort

#### Scenario:拒绝 stale 或非候选选择
- **WHEN** decision/launch revision 过期、candidate digest 漂移、Skill disabled/unapproved/不在 candidate set，或请求用设置页状态冒充选择
- **THEN** 系统返回 409/validation，零 selection/Run/NodeRun/TextModel/Provider 副作用并要求重新路由

### Requirement:默认 Skill binding、provisional 输入与 canonical review action
Worker 启动 SHALL 只读取 Registry index/approved metadata，`drama-mvp-a-default` SHALL 只绑定 approved `novel-writing` 和 `drama-skills`。其余六项 candidate 为 `pending_provenance`/disabled，MUST NOT 成为启动或默认 Run 前置，只有 node `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 都满足才按需读取。

同一 Run、同项目、Schema-valid、hash/revision/scope 匹配的 provisional upstream candidate SHALL 可构造完整候选图；accepted owner fact SHALL 可作为等价输入。只有输入既非 accepted owner fact 且不满足这组 provisional 条件时才拒绝。完整候选图只可产生一次 immutable `TextReviewBatch`，以 successor/stale closure 与全有或全无 CAS 收口，不增加第二审核。文本 domain/HTTP/audit action 仅允许 `accept|reject`；legacy/unknown `approve` MUST validation 且零 accepted handoff、owner ack、current 或媒体副作用。

#### Scenario:允许匹配 provisional，拒绝其余上游
- **WHEN** 文本节点接收同一 Run/项目、Schema-valid、hash/revision/scope 匹配的 provisional candidate，或接收其他 candidate
- **THEN** 前者可构造完整图并最终只产生一次 batch；后者除 accepted owner fact 外均在写入前拒绝

### Requirement:阶段一追溯与依赖边界
系统 SHALL 将本 capability 追溯到 `plan-phase-one-drama-mvp-a` 的任务 **3.1**，并遵守共享任务 **5.1**、**5.3**、**5.4**、**5.5**。实现 MUST 直接依赖经代码/schema/测试核验的 scenes、workflows、catalog 契约；总体协调 change MUST NOT 成为运行时代码依赖。完整非目标是拥有 Provider/Profile/Model catalog CRUD、credential service、WorkflowRun/NodeRun/RunEvent/ProviderCall 状态或事件历史，创建、执行或推进 WorkflowRun，实现图片/视频/音频生成、媒体二进制入库或 Timeline；本 capability MUST NOT 让未确认候选覆盖领域事实、默认调用真实 Provider，或把总体协调 change、未完成依赖和真实 Provider 当作运行时依赖或静默 fallback。

#### Scenario:只消费已验证的直接依赖
- **WHEN** 文本生成 Command 引用 Scene/Shot、WorkflowVersion/Run 或 capability snapshot
- **THEN** 系统只使用对应已实现切片的已确认、版本化事实，且不读取总体协调文档作为运行时配置

#### Scenario:拒绝所有权和依赖泄漏
- **WHEN** 实施尝试创建/推进 WorkflowRun、NodeRun、RunEvent、ProviderCall，或从未实现的依赖与总体协调文档推断运行时字段
- **THEN** 架构依赖/合同测试失败，且不生成候选、不改写运行事件或调用真实 Provider

#### Scenario:拒绝非目标职责泄漏
- **WHEN** 文本切片尝试承担任一列明的非目标、自动采纳候选或默认调用真实 Provider
- **THEN** 架构依赖/契约测试失败，且不写 candidate、generation audit、confirmation、领域事实或 Outbox

### Requirement:结构化候选的 canonical schema 版本映射
系统 SHALL 以数据库与共享 Schema 的 `schema_version` 作为 CreativeBrief、StorySpec、ScriptSpec、Episode、Scene 与 Shot 结构化候选的唯一版本事实。HTTP DTO 的 `schemaVersion` MUST 只映射同一个 canonical 值，且实现 MUST NOT 独立持久化或推导第二个版本事实。

#### Scenario:将 canonical 候选版本映射到 HTTP
- **WHEN** API 序列化或反序列化有效的结构化候选 DTO
- **THEN** `schemaVersion` 与 canonical `schema_version` 值相同，且持久化层只保存一个版本事实

#### Scenario:候选版本缺失或冲突时无写入拒绝
- **WHEN** 请求缺少必需版本、同时提供冲突的 `schema_version` 与 `schemaVersion`，或实现尝试分别赋值
- **THEN** API 在 UoW 与模型调用前返回稳定 validation error，且不写 candidate、generation audit、confirmation、领域事实或 Outbox，也不创建 RunEvent/ProviderCall

### Requirement:独立 AgentScope runtime 与按来源固定的 Skill
Agent Worker SHALL 将 AgentScope 2.x 作为独立 runtime dependency 由依赖清单与 lock 管理，AgentScope MUST NOT 被放入 `third_party/skills/<name>/<commit>`。Git Skill SHALL 使用 `commit` 与内容 `digest` 固定来源；公开 Markdown Skill SHALL 使用 `archive URL`、获取时间、内容 `digest` 与 `license status` 固定来源。每个 Skill MUST 经过 manifest/hash/metadata/schema/许可准入，并记录 network、subprocess、file、secret access matrix 与静态审计证据；运行时 MUST NOT 执行任何第三方 Skill 脚本。

#### Scenario:启动只加载 Registry metadata
- **WHEN** Worker 启动且 AgentScope runtime lock 有效、Registry index 存在，两个默认 Skill 的 approved metadata 可用
- **THEN** Worker 只加载 AgentScope runtime 与 Registry index/approved metadata，不读取任一 Skill 的 `SKILL.md`、`references` 或 disabled/pending candidate 正文

#### Scenario:路由后按需加载已批准 Skill
- **WHEN** SkillRouter 根据 node `allowedSkills`、`requiredCapabilities` 与 `selectionMode` 选中 approved Git 或公开 Markdown Skill revision，且其 source identity/digest/manifest/license 均匹配
- **THEN** 系统只读取该 revision 的固定快照、`SKILL.md` 和必要 `references`，并将 source identity 与选中 revision 写入生成审计中

#### Scenario:拒绝缺失或漂移的 Skill
- **WHEN** AgentScope runtime 未锁定，或 Skill source identity/digest/manifest 不匹配、许可未获准，或路由需要人工选择而调用方未选择
- **THEN** 系统返回稳定配置/validation 错误，不调用 TextModelPort 且不生成候选

#### Scenario:拒绝未授权 Skill 访问或脚本执行
- **WHEN** Skill 请求 manifest/policy 未允许的 network、subprocess、file、secret 访问，或包含需要运行的第三方脚本入口
- **THEN** registry/router/adapter 在任何模型或外部调用前拒绝，记录 redacted audit evidence，不生成候选、不写 ProviderCall 且不访问密钥

### Requirement:通过 Schema 校验的结构化文本候选
系统 SHALL 以 projects owner 已校验的 CreativeBrief 输入 snapshot、typed scope union 和可选 adaptation SourceMaterial snapshot，通过 `TextModelPort` 和明确 scope 生成项目级 StorySpec、每集 ScriptSpec、Episode、Scene、Shot 与 ShotSpec 的完整结构化候选图。初始创作 command 只需 `projectId` scope；Episode/Scene/Shot 是候选输出的显式 target scope。CreativeBrief MUST 精确包含六项创作语义 `subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeDurationSeconds`、三个计数 `episodeCount`、`scenesPerEpisode`、`shotsPerScene`、canonical `schema_version` 与 revision；Story/Script/Scene/Shot MUST 具有设计冻结的最小内容字段、稳定 ID、`schema_version` 与版本化 ShotSpec，且 AssetBible 覆盖 MUST 按 project -> episode -> scene -> shot 固定到 ShotSpec。每个输出 MUST 在持久化前通过对应 Draft 2020-12 JSON Schema，并关联同项目的 accepted owner fact 或同一 Run、同项目、Schema-valid、hash/revision/scope 匹配的 provisional upstream candidate；provisional candidate MUST NOT 被投影为已确认领域事实或回写 projects owner。

MVP-A 的合法文本输出 MUST 仅为上述结构化候选。`materialType=novel` 只用于 SourceMaterial import/parse，系统 MUST NOT 生成或持久化小说正文、章节正文、章节草稿或语义等价的 prose artifact。

#### Scenario:生成有效的下游候选
- **WHEN** 调用方提交已校验 CreativeBrief snapshot、与当前生成对象匹配的 typed scope 和已选择的 pinned Skill/model；初始创作 command 仅提交 project scope，后续 Episode/Scene/Shot candidate 使用同一 Run 中已校验的显式 target scope 与 provisional upstream candidates
- **THEN** 系统验证结构化输出并保存 immutable pending_review candidate、输入/输出 version/hash 与来源链

#### Scenario:拒绝无效输出或跨项目输入
- **WHEN** 模型输出不是合法 JSON、违反 Schema、包含额外字段、引用其他项目，或上游输入既不是 accepted owner fact、也不是同一 Run/项目且 Schema-valid、hash/revision/scope 匹配的 provisional candidate
- **THEN** 系统保存可诊断失败审计但不保存候选，也不改写领域事实

#### Scenario:拒绝阶段一小说正文输出
- **WHEN** Skill、模型或调用方请求 `novel_body`、章节正文、章节草稿或其他非结构化小说产物
- **THEN** 系统在 TextModelPort 调用或 candidate 持久化前返回 `unsupported_output_type`，不创建候选、ProviderCall、RunEvent 或下游媒体副作用

### Requirement:SourceMaterial 导入与绑定来源的生成
系统 SHALL 冻结 `creationMode=original|adaptation`，其 canonical owner 为 projects。original MUST 只使用 projects owner CreativeBrief 的六项创作语义 `subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeDurationSeconds`、三个精确计数 `episodeCount`、`scenesPerEpisode`、`shotsPerScene`、schema/revision，MUST NOT 要求、创建或冻结 SourceMaterial；缺少 SourceMaterial 不得阻断有效 original 文本生成。adaptation MUST 使用 `materialType=novel|synopsis|existing_script` 与 `inputMode=inline_text|uploaded_file` 的 valid SourceMaterial，且 text owner 仅拥有 adaptation SourceMaterial reference、immutable revision/contentHash、parse/validation、binding status/version 和文本候选。`CreativeBriefSourceBindingSnapshot` MUST 精确冻结 `projectId`、`sourceMaterialId`、`sourceMaterialRevision`、`sourceContentHash`、`creativeBriefId`、`creativeBriefRevision`、`creativeBriefPayloadHash`、`parseStatus`、`validationStatus`、`bindingStatus`、`bindingVersion`；创建 Run 后的 `TextRunSourceBindingSnapshot` MUST 再增加 `runId`、`runRevision`。inline_text MUST NOT 创建 storage session、StoredObject 或 AssetVersion；uploaded_file MUST 由 StoragePort/Assets owner 交接 verified AssetVersion。invalid enum、scope、revision、hash 或 status/version 与无效 adaptation source MUST 在 TextModel、付费 Provider 或 Storage mutation 前失败，且 text owner MUST NOT 写 projects owner state；recovery 只复用同一 source/brief revision。

#### Scenario:解析并绑定改编来源
- **WHEN** 用户选择 adaptation 并提交有效 `novel|synopsis|existing_script` SourceMaterial，解析和校验均成功
- **THEN** inline_text 追加 SourceMaterial revision/contentHash/binding status/version 且无 storage session/StoredObject/AssetVersion；projects owner 保存包含精确 project/source/brief IDs、revisions、content/payload hashes、parse/validation/binding status/version 的 `CreativeBriefSourceBindingSnapshot`，Run 创建后 text owner 再增加 run ID/revision 形成 `TextRunSourceBindingSnapshot`；uploaded_file 在 verified AssetVersion binding 后同样冻结这些字段，再生成结构化候选

#### Scenario:拒绝无效、跨项目或过期来源
- **WHEN** creation/material/input enum、scope 或 revision 无效，source parse/validation failed or invalid、AssetVersion 未验证/跨项目、revision/hash 不匹配，或 adaptation 没有 valid source
- **THEN** 系统保留原始 diagnostic，不调用 TextModelPort/付费 Provider，不创建/启动 TextRun，不隐式换源；original 或 inline_text 的 upload intent 不创建 storage session、StoredObject 或 AssetVersion

#### Scenario:恢复绑定来源的生成
- **WHEN** SourceMaterial parse 或绑定 TextRun 失败、状态未知、API/Worker 重启
- **THEN** recovery 重用相同 source revision/contentHash 和 `run_id + logical_operation`，先 reconciliation unknown state，不重复上传或收费提交

### Requirement:单次显式文本批量审核
系统 SHALL 在一次 Run 生成并逐对象校验完整候选图后，冻结一个包含精确 candidate IDs/hashes、source hashes、依赖边、scope、payload hash 与 expected revisions 的 `TextReviewBatch`，并只要求一次必需的人工 Confirm/Reject。系统 MUST NOT 在 StorySpec、ScriptSpec、Episode、Scene、Shot 或 ShotSpec 层之间插入必需人工门。批次内上游候选修改 MUST 生成新候选并把所有依赖成员标记 `stale`；存在 stale、缺失或 Schema-invalid 成员时批次 MUST NOT 被接受。Confirm MUST 对完整 batch 执行 CAS，并在文本 owner 的一个 UoW 中全有或全无地追加 TextReview accepted handoff、StorySpec/ScriptSpec facts 与审计/Outbox；Project/Episode/Scene/Shot 落地 MUST 由各 owner 的 typed batch/orchestration command 在其 own contract 内完成，文本 owner MUST NOT 直接写入这些 aggregate。owner handoff MUST 携带 handoff id/revision/correlation、target id、candidate/source hashes、expected revision 和 payload hash，重试幂等；任一 owner ack 缺失/失败时媒体门保持关闭。已确认事实 MUST NOT 被候选原地覆盖。

#### Scenario:一次确认完整文本审核批次
- **WHEN** 审核人以当前 batch revision 确认同项目、成员完整且全部 Schema-valid 的 pending_review batch，并提交精确 candidate hashes 与 target expected revisions
- **THEN** 文本 owner 在一个事务追加确认、accepted handoff、StorySpec/ScriptSpec facts 与审计/Outbox；Project/Episode/Scene/Shot owner 分别以 typed batch command 幂等落地并返回匹配 ack，全部 ack 完整后才解除付费媒体门

#### Scenario:拒绝过期、不完整或已决定的批次
- **WHEN** batch 含 stale/缺失/Schema-invalid 成员、candidate hash 或 scope 不匹配、expected revision 过期，或 batch 已被决定/替代
- **THEN** 系统返回稳定 validation/conflict，保留既有事实和候选状态，不写 accepted handoff、部分 owner command/ack 或领域版本，且不解除付费媒体门

### Requirement:文本生成审计与显式启用 Provider 边界
系统 SHALL 对每次生成或确认持久化模型/provider snapshot、Skill source identity、脱敏 prompt digest、输入输出版本/hash、Schema 结果、token/cost 和错误。默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；真实文本模型 MUST 由显式配置启用，且未配置或失败时 MUST NOT 静默 fallback 或切换 profile。Agent Worker 的 restart/reconcile、temporary cleanup 或日志维护 MUST NOT 删除、覆盖或静默压缩与文本操作关联的 `RunEvent`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要或仍被引用的 `AssetVersion`；这些事实仍分别由 workflows/runs、catalog 与 Assets owner 持有，本 change 只验证引用和清理边界，不复制其事实。

#### Scenario:默认 Mock 与 Local profile 生成
- **WHEN** 本地默认配置执行结构化生成
- **THEN** 系统仅调用 DeterministicMockProvider，并记录可复现的 mock/provider 审计而不访问真实服务

#### Scenario:真实 Provider 不可用
- **WHEN** 真实文本模型未明确启用、凭据缺失、超时或返回 provider 错误
- **THEN** 系统保存脱敏失败审计和稳定错误，不扣写候选或改写确认事实

#### Scenario:Worker 清理不得破坏跨 owner 长期事实
- **WHEN** Agent Worker 在超过诊断窗口、不同 hold、restart/reconcile 后执行 temporary cleanup 或日志维护
- **THEN** 关联的 `RunEvent`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和仍被引用的 `AssetVersion` 保持可读取、append-only 且不被删除、覆盖或静默压缩；Worker 不创建这些事实的副本

### Requirement:兼容 OpenAI 的 TextModelPort adapter
系统 SHALL 由文本 change 的 Agent worker adapter 实现 `OpenAICompatibleTextModelAdapter`，消费 catalog/security owner 的 enabled Profile/Model/CapabilitySnapshot 和受限 CredentialResolver。Codex relay SHALL 是默认真实 profile，DeepSeek SHALL 仅在显式 opt-in profile 下可用；adapter MUST 使用 profile-derived base URL/endpoint、Bearer/API-key auth、结构化 request、request-id/native usage/JSON payload parsing。`GET /v1/models` MUST 只生成 catalog candidate diff，手工 model 保留且只有显式 catalog accept 可更新 enabled model。

#### Scenario:同步模型但不隐式改变 catalog
- **WHEN** enabled profile 明确执行 `/v1/models` sync/probe
- **THEN** adapter 返回只读 candidate diff 给 catalog，既有 selection 和手工 model 不变，且没有 generation request

#### Scenario:只重试可重试的文本结果
- **WHEN** generation 遇到 transport、429、5xx 或 timeout
- **THEN** adapter 仅按 bounded policy retry，unknown submit 先 reconciliation；4xx authentication/validation、malformed response 和 schema failure 不重试、不写 candidate、不静默 fallback

#### Scenario:拒绝未配置的真实文本操作
- **WHEN** profile disabled/missing、credential/master key 缺失，或调用方未明确 opt-in command/probe
- **THEN** 返回原始 redacted diagnostic，默认 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）零网络，日志/audit 不泄露 authorization secret

### Requirement:默认工作流文本端口合同
系统 SHALL 为默认 Workflow 的 `text.generate` port 产生 project-scope CreativeBrief 输入与完整 `TextReviewBatch` 输出；只有 accepted batch 才可成为媒体节点的 immutable storyboard/reference input。任何 partial/stale/foreign batch MUST 不产生媒体 handoff，且默认 E2E 仅使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。

#### Scenario:交接已接受的文本批次
- **WHEN** 当前项目的完整 batch 以精确 CAS 被接受
- **THEN** 输出 port 只包含 owner 定义的 immutable candidate/reference/version facts，供下游读取而不启动媒体调用
### Requirement:stale 文本闭包再生成
编辑上游文本 candidate MUST 追加 successor，并将其依赖闭包标记为 stale。再生成 command 在追加 successor `TextReviewBatch` 前，MUST 校验已绑定的 run/brief/batch revision、source id/hash 以及每个对象的 schema/count/scope/hash。

#### Scenario:不接受不完整闭包
- **WHEN** 闭包 partial、stale、foreign、duplicate，或未通过 expected revision/hash
- **THEN** 不接受新的 batch，已有 candidate/batch 保持 immutable。
