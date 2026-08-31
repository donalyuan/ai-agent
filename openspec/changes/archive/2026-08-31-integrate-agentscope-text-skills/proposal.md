## Why

## 文本输入与审核闭合

文本 Worker 启动只读取 Registry index 和 approved metadata；默认 `drama-mvp-a-default` 只绑定 approved `novel-writing` 与 `drama-skills`。八项 registry 的另六项为 `pending_provenance`/disabled，只有 node `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 全部满足时按需读取，不能成为启动或默认 Run 前置。

同一 Run、同项目、Schema-valid 且 hash/revision/scope 匹配的 provisional upstream candidate 可以构造完整候选图；accepted owner fact 也可作为输入。只有不满足这两类条件的输入才拒绝。完成后仅一次 immutable `TextReviewBatch`，以 successor/stale closure 和全有或全无 CAS 收口；文本 review action 只用 `accept|reject`，legacy/unknown `approve` validation 且零 accepted handoff/owner ack/媒体副作用。

阶段 0 已具备 `TextModelPort`、Mock Provider、显式 Local test/offline profile 和确定性 `SkillRegistry`/`SkillRouter`，但尚未装配 AgentScope、固定文本 skills 或可人工确认的结构化叙事生成。该切片让后续 Scene/Shot 工作流获得可审计、Schema-valid 的文本候选，而不触碰图片、视频或 Provider catalog 管理。

## What Changes

- AgentScope 2.x 作为 Agent Worker 的独立运行时依赖，由 Worker 的依赖清单与 lock 单独管理；不得放入 `third_party/skills/<name>/<commit>`。Git Skill 使用 `commit` 与内容 `digest` 固定来源；公开 Markdown Skill 使用 `archive URL`、获取时间、内容 `digest` 与 `license status` 固定来源。Worker 启动只读取 Registry index 与 approved metadata，路由确定后才按需读取对应固定快照的 `SKILL.md` 和 `references`，不执行第三方脚本。
- 补齐运行前 Skill 路由裁决：保存确定性过滤后的候选、淘汰/排序原因、lexical/可选 semantic score、policy 结果和 decision revision；并列或低置信时返回 `needs_human_selection`，由用户从当前候选中显式选择，随后 workflows/runs 才冻结最终 SkillRevision。设置页启停 Skill 不代替本次运行裁决。
- 以带精确 `episodeCount`/`scenesPerEpisode`/`shotsPerScene` 与每集 `episodeDurationSeconds` 的 CreativeBrief 输入 snapshot，通过 `TextModelPort` 生成项目级 StorySpec、每集 ScriptSpec、稳定 Scene/Shot、版本化 ShotSpec，以及这些输出实际引用的初始 AssetBible entry specs 的完整结构化候选图，并逐对象执行 JSON Schema 校验。
- 将 MVP-A 文本输出边界固定在上述结构化规格；即使 SourceMaterial 类型为 `novel`，也只解析来源并生成改编用 StorySpec/ScriptSpec/Scene/Shot/ShotSpec，不生成小说正文、章节正文或章节草稿。
- 冻结 `creationMode=original|adaptation`：original 消费 projects owner CreativeBrief 的六项创作语义、三个精确计数及 schema/revision，不要求 SourceMaterial；adaptation 以 `materialType=novel|synopsis|existing_script`、`inputMode=inline_text|uploaded_file` 的 SourceMaterial import/parse/validation/binding/recovery 生成。SourceMaterial 保留 immutable revision/contentHash、parse/validation 与 binding status/version；brief snapshot 冻结精确 project/source/brief IDs、revisions、content/payload hashes，Run snapshot 再增加 run ID/revision，上传文件才有 AssetVersion 引用。
- 对每个第三方 Skill 按来源类型固定 source identity、内容 digest、manifest、license 状态，并留下 network、subprocess、file、secret 访问审计证据；任何未授权访问或脚本执行在模型调用前拒绝。
- 新增 `TextReviewBatch`、一次必需的批量确认/拒绝、候选依赖 stale 传播、输入输出版本、模型/Skill/prompt/cost 审计与错误可见性；完整 accepted batch handoff 必含 candidate/source hashes、payload hash、expected revisions 与 correlation，Project/Episode/Scene/Shot/AssetBible 各 owner typed batch/orchestration command 的幂等 ack 齐备前不得启动付费媒体。
- 固定数据库与共享 Schema 的 `schema_version` 为结构化候选唯一版本事实；HTTP DTO 的 `schemaVersion` 只映射同一值，缺失、冲突或双独立赋值在 UoW 前失败且无写入。
- 默认测试使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；真实文本模型仅显式 opt-in，失败不静默 fallback 或切换 profile。
- 实现真实 `TextModelPort` 时固定为 OpenAI-compatible adapter：Codex 中转站是默认真实 profile，DeepSeek 是可选 opt-in profile；adapter 通过 catalog/security 的 credential resolver、`/v1/models` candidate diff、显式 model accept、结构化 request/response 解析、受限 retry 和 opt-in probe 接入，默认测试仍零网络。

## Capabilities

### New Capabilities
- `agentscope-text-skills`: AgentScope 固定 skills、TextModelPort 结构化文本候选、人工确认与审计。

### Modified Capabilities

- 无。

## Impact

后续实现将影响 Agent Worker、API domain/application/adapters/interfaces、contracts、skill manifests、Alembic 和测试。不会实现 catalog CRUD、真实图片/视频调用、素材二进制、Timeline 或工作流运行编排；上传本体仍由 StoragePort/AssetVersion owner 负责。

## 与总体计划的追溯与边界

- 本 change 落实 `plan-phase-one-drama-mvp-a` 的总体任务 **3.1**，并受共享工程任务 **5.1**、**5.3**、**5.4**、**5.5** 约束。
- 直接实施依赖是已完成的 scenes、workflows 与 catalog 切片所冻结的已确认 Scene/Shot 事实、WorkflowVersion/Run 标识和 Provider/Skill capability snapshot；实施开始前必须以真实代码、schema 和测试核验这些依赖，而不是把总体计划当作运行时输入。
- `plan-phase-one-drama-mvp-a` 只负责 OpenSpec 协调与验收，不是 Agent Worker、TextModelPort、SkillRegistry/SkillRouter、HTTP 或持久化组件的运行时代码依赖。
- 完整非目标是拥有 Provider/Profile/Model catalog CRUD、credential service、WorkflowRun/NodeRun/RunEvent/ProviderCall 状态或事件历史，创建、执行或推进 WorkflowRun，实现图片/视频/音频生成、媒体二进制入库或 Timeline；也不让未确认候选覆盖 Project/Episode/Scene/Shot，不默认调用真实 Provider，不把总体协调 change、未完成的 scenes/workflows/catalog 或真实 Provider 当作运行时依赖或静默 fallback。

## 默认工作流端口闭合

本 change 为默认 Workflow 的 `text.generate` 提供 project-scope CreativeBrief 输入和完整 `TextReviewBatch` 输出；只有精确 CAS accepted batch 才可向媒体端口交接 immutable storyboard/reference/version facts。它不启动媒体调用、不拥有 Timeline，默认 E2E 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。

## Successor 与 stale closure 合同

**DDD**：上游编辑产生 successor candidate，依赖闭包旧对象 immutable 且 stale。**BDD**：partial/stale/foreign/duplicate batch 被拒绝，CAS 接受全有或全无。**SDD**：`regenerateStaleClosure`/`regenerateTextCandidate` 绑定 run/brief/batch revision、source ids/hashes、expected revisions，并逐对象校验 schema/count/scope/hash。**TDD**：覆盖完整 closure 成功、每一类拒绝与零媒体副作用。

## Skill 路由人工裁决合同

**DDD**：catalog 拥有 SkillRevision，text/Agent runtime 拥有 SkillRouteDecision/人工 selection，workflows/runs 拥有最终 frozen Run snapshot。**BDD**：候选、淘汰/排序原因和歧义可见；未选择、过期或非候选选择不启动模型。**SDD**：decision/selection 绑定 project/node/launch、candidate SkillRevision ID/digest、router policy/version 与 expected revisions。**TDD**：覆盖确定性结果、并列/低置信、semantic adapter unavailable、candidate drift、刷新不自动选择和零 TextModel/Provider 副作用。

## SourceMaterial import boundary

**DDD**：projects owner 持有 Project、`creationMode`、CreativeBrief、项目设置和预算阈值；text owner 只消费 projects 返回的已校验 CreativeBrief snapshot，并拥有 adaptation `SourceMaterial`/其 revision、parse/validation 状态、文本候选和 TextReview binding。text owner 不创建或更新 Project、creationMode、CreativeBrief、项目设置或预算。original 无 SourceMaterial，adaptation 必须 valid source。uploaded_file 的二进制和 `AssetVersion` 仍由 Storage/Assets owner 拥有，inline_text 不创建 storage session、StoredObject 或 AssetVersion。**BDD**：适配输入可解析、校验和恢复；无效 enum/scope/revision/hash/status 或 adaptation source 失败不能创建或启动付费文本 Run。**SDD**：SourceMaterial DTO 冻结 `creationMode`、`materialType`、`inputMode`、`sourceId`、`revision`、`contentHash`、`parseStatus`、`validationStatus`、`bindingStatus`、`bindingVersion` 与仅 uploaded_file 的 `assetVersionId`；`CreativeBriefSourceBindingSnapshot` 精确冻结 `projectId`、`sourceMaterialId`、`sourceMaterialRevision`、`sourceContentHash`、`creativeBriefId`、`creativeBriefRevision`、`creativeBriefPayloadHash`、`parseStatus`、`validationStatus`、`bindingStatus`、`bindingVersion`；Run 创建后 `TextRunSourceBindingSnapshot` 再增加 `runId`、`runRevision`。**TDD**：覆盖 owner 边界、original、两种 adaptation input、invalid/foreign/stale、source parse recovery、同一 revision 重试无重复上传和 unknown submission reconciliation。
