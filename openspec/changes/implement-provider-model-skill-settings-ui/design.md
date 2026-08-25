## Context

## UI operation gate

Skill registry 面板必须精确投影八项 candidate 的 provenance、approval、enabled：`novel-writing`/`drama-skills` 为 `verified_snapshot`/`approved`/`true` 并且是 `drama-mvp-a-default` 的唯一默认 binding；`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira` 均为 `pending_provenance`/`not_approved`/`false`。后六项不得被 UI 暗示为 Worker 启动或默认 Run 依赖，只有 `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 均匹配才按需读取。

设置 action 必须分别读取 `adapterInstalled`、catalog `approval`、capability snapshot、`runnable`、`featureGate`。首次 connection-test/probe 只需 installed、`approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout，成功时冻结 snapshot；它不要求 snapshot/`runnable=true`，也不因 disabled-for-run 被拒绝。snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default、Run resolve 与 live invocation，后者才要求 installed、approved、成功 snapshot、`runnable=true`、`featureGate=MVP-A`。MVP-B/uninstalled/not-approved 或缺 opt-in/profile/credential/timeout 的 operation 零 probe/外部调用；TTS/ASR、MiniMax H3、Seedance 2.5 与 Agnes 未选中 mode 保持不可运行，默认测试使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），explicit live opt-in 不得弱化。

Provider catalog change 拥有 Provider、Profile、Model、CapabilitySnapshot、SkillRevision、项目默认/覆盖、masked credential 与调用审计；当前 Web 仅为阶段 0 shell。本 change 定义桌面设置消费者，绝不将凭据、Provider SDK、同步或 probe 逻辑移动到浏览器。

## Goals / Non-Goals

**Goals:**

- 用 Provider/Profile/Model/SkillRevision/capability snapshot 页面展示可配置、可追溯且明确生效来源的设置。
- 用掩码 replace/rotate 流程、模型 diff 人工接受、参数 Schema 表单和显式 connection/probe 命令控制副作用。
- 用 projects owner 的项目文本阈值、批量/unknown cost BudgetGate confirmation、精确 run/logical operation 绑定和 retention/hold 视图控制付费操作与审计保留。
- 明确 React Router、Query、Zustand、Zod、shadcn/Radix/Tailwind/Lucide 与 `Mock Provider +` 显式 Local test/offline profile 测试边界。

**Non-Goals:**

- 不读取/缓存/回显 plaintext secret，不实现 credential storage、Provider adapter、模型同步算法、真实调用、费用结算或 Skill 执行。
- 不在浏览器计算权威费用、判定账单结算或生成确认 ID；UI 只通过 projects owner 编辑项目文本阈值，并向 catalog/workflows owner 提交精确 BudgetGate confirmation。
- 不在设置页执行运行级 Skill route selection；设置页只管理 catalog lifecycle，Workbench/text runtime 才显示和解决 `needs_human_selection`。
- 不因路由进入、列表刷新、草稿编辑或覆盖预览自动同步模型、探测能力或调用 Provider。

## Decisions

### 1. DDD state ownership and routes

路由为 `/settings/providers`、`/settings/providers/:providerId`、`/settings/storage-profiles`、`/settings/storage-profiles/:storageProfileId`、`/settings/skills`，项目级覆盖以 `/projects/:projectId/settings/models` 深链展示。Provider/Profile/Model/SkillRevision/capability snapshot/StorageProfile/BucketBinding/credential state 均是 owner Query 事实；Zustand 只保存已选 tab、筛选、参数表单 draft 与待确认 diff selection。浏览器只保存 credential 是否已配置、masked hint、last rotation timestamp/状态，永不保存原值或使用 localStorage 恢复 secret。

### 2. SDD: adapter, schemas and cache

`catalogSettingsApi` 对接 projects/catalog/workflows/storage owner 的 additive resources：项目文本费用阈值只经 projects owner 读取/写入；Provider/Profile/Model/SkillRevision/snapshot、StorageProfile/BucketBinding、masked credential replace/rotate、project/workflow binding、model sync proposal accept/reject、connection test/capability probe、BudgetGate estimate/actual/source/unknown、确认状态和 retention policy/version/hold 仍经各自 owner。StorageProfile request/response 必须逐字段验证 `storageProfileId`、`schemaVersion`、`revision`、`name`、`adapterKey=tos`、`enabled`、`bucketBindingId`、`region`、`endpoint`、`privateBucket`、`credentialRef`、`credentialStatus`、connect/read/write timeout、presign max TTL 与 project scope；credential status 只显示 `configured|unconfigured|rotating|failed|master_key_unavailable` 和 masked summary。Zod request/response schema 必须拒绝 `secret` 出现在 response/error/log DTO，验证 canonical schemaVersion、revision、capability snapshot ID/timestamp、parameter JSON Schema、覆盖层 `system|project|workflow` 与 effective source，以及 threshold/confirmation 的 owner IDs、revisions、hashes、`runId`、`logicalOperation`、request fingerprint、稳定本地 `userUuid`。Query key 以 owner IDs/revision/snapshot hash 分隔；mutation 成功只失效受影响资源，Probe/Sync 不使用自动 refetch 触发。

设置页的 Provider/Profile/Model/Skill 生命周期使用 owner command `create|edit|enable|disable`，分别提交资源 ID（创建时为空）、payload、`expectedRevision` 与 `If-Match`。Provider/Profile/Model 的内容编辑更新当前资源 revision；Skill 内容或 manifest 变化只能追加新的 immutable `SkillRevision`，启停只改变可路由状态，不覆盖历史 revision 或已冻结 CapabilitySnapshot。owner 返回 `409 revision_conflict` 时 UI 必须刷新权威资源并放弃乐观写入；页面不得在客户端复制或合并 catalog 事实。

StorageProfile 页面使用同一 owner command 形态：`POST /v1/storage-profiles`、`GET/PATCH /v1/storage-profiles/{storageProfileId}`、`POST /v1/storage-profiles/{storageProfileId}/enable`、`POST /v1/storage-profiles/{storageProfileId}/disable`、`POST /v1/storage-profiles/{storageProfileId}/connection-test`。create/edit/enable/disable 提交 `expectedRevision`/`If-Match`；stale 返回 `409 storage_profile_revision_conflict`，含 expected/current revision 且 UI 零乐观覆盖。connection-test 提交 profile revision/snapshot、timeout 和 `probeCorrelationId`，只显示 `connected` 或 `unconfigured|validation|authentication|network|timeout` 脱敏状态，不改变 profile config revision、不创建对象/AssetVersion、不把失败切为 Local。

### 3. BDD: manual acceptance and side-effect controls

模型同步先产生只读 diff（新增、移除、字段/能力/参数变化），用户必须逐项/整体显式 accept；关闭、离开或刷新不接受。连接测试和 capability probe 使用带 provider/profile/snapshot context 的明确 button，显示 `unconfigured`、validation、authentication、network 或 owner error，原始密钥不在任何界面/日志显示。StorageProfile connection-test 必须单独显示目标 Bucket/Region/Endpoint 与 masked credential status，区分 profile 配置错误和远程连接失败。覆盖页面显示 system/project/workflow 的来源、优先级与回退结果，只有 owner 允许的 mutation 能保存。图片/视频批量、超阈值文本和 `cost=unknown` 显示各自 BudgetGate，确认前保持 waiting_review；确认必须展示并提交精确 run/logical operation/fingerprint/revision，参数变化或恢复后的失配确认不可复用。retention/hold 只显示和提交 owner 允许的策略命令，浏览器不自行过期或删除审计。

Provider/Profile/Model/Skill 的创建、编辑、启用和停用都必须由用户明确点击并确认；路由加载、刷新、筛选或表单草稿不产生 mutation。停用的资源不可成为新 Run 的解析结果，重新启用仍保留旧 revision/snapshot；跨项目、过期 revision、Skill 内容覆盖历史或未知字段均在 owner 与 UI contract boundary 拒绝。

### 4. TDD, accessibility and compatibility

先写 Zod redaction、adapter、Query mutation、secret input reset、diff accept、首次 probe（含 disabled-for-run/snapshot-missing/`runnable=false`）与失败 gate、覆盖优先级、预算确认/unknown cost/绑定冲突和 retention hold 组件/Store 测试，再实现界面；E2E 使用 `Mock Provider +` 显式 Local test/offline profile 并断言无隐式 mutation、无 profile 切换。使用 shadcn/Radix 的 form/dialog/tabs/menu/tooltip、Tailwind token 与 Lucide；敏感动作使用 icon+text 明确 command，密钥输入不以卡片嵌套呈现。当前未安装计划依赖，安装属于后续代码任务。

## Dependency DAG

```text
provider/model/skill catalog + provider integrations + storage boundary
                              |
            provider/model/skill settings UI
```

## Current / Defined / Todo

- **Current**：当前 Web 无设置页面、catalog/storage owner API 或安全凭据 UI。
- **Defined**：资源视图、StorageProfile 专属表单与 lifecycle/connection-test、掩码 credential、人工 diff 接受、覆盖展示、参数 Zod 表单、显式 probe、费用阈值/确认闸门与 retention 状态。
- **Todo**：在 owner contract 实现后完成 adapter/routes/components、`Mock Provider +` 显式 Local test/offline profile fixtures、accessibility/E2E 和依赖安装。

## Risks / Trade-offs

- [secret 泄漏至 cache/error] -> response schema redaction、never-persist policy 和测试扫描。
- [自动同步造成意外调用/配置漂移] -> 只有显式 command，diff 未接受前不写。
- [能力 snapshot 已陈旧] -> 显示 capturedAt/hash/stale 状态，要求显式 probe。
- [覆盖来源误解] -> 同时展示 system/project/workflow 值、effective value 和 owner revision。
- [客户端误算并发或 quota] -> UI 只显示 owner policy/quota snapshot，不从列表项、定时器或 429 次数推导剩余额度。
- [删除模型破坏历史] -> delete affordance 由 owner reference proof gate；被引用或 proof unavailable 时只提供 disable，并保持历史 identity/snapshot 可查。

## Migration Plan

先创建 `Mock Provider +` 显式 Local test/offline profile settings route，不迁移 credential 或 catalog 数据；owner API 可用后 additive 接入。回滚移除 UI/cache，不轮换/删除 owner secrets、snapshots 或模型。

## Open Questions

- generic catalog owner 的其他 resource path、同步 diff/接受批次 DTO 和未配置 error codes；StorageProfile 的 owner path、字段和 error matrix 已由 TOS child 冻结。
- workflow 覆盖的发布版本/草稿可写性与参数 JSON Schema 支持的 UI 控件范围。

## Acceptance Commands

`openspec validate implement-provider-model-skill-settings-ui --strict --json`、`pnpm --filter @video-agent/web test`、`pnpm --filter @video-agent/web typecheck`、`pnpm --filter @video-agent/web lint`、`pnpm --filter @video-agent/web format:check`、`git diff --check -- openspec/changes/implement-provider-model-skill-settings-ui`。

## Explicit probe matrix

**DDD**：profile/config/StorageProfile remains catalog/storage owner；settings only dispatches explicit lifecycle/probe commands and displays owner state。**BDD**：通用 CRUD/enable/disable conflict、StorageProfile 字段/connection-test、unconfigured live probe 均可见且 never turn default E2E green。**SDD**：lifecycle DTOs carry expectedRevision/If-Match、StorageProfile/BucketBinding fields、credentialStatus and SkillRevision/snapshot references；`1x1x1` is explicit provider/storage/renderer capability request while browser suite remains `Mock Provider +` explicit Local test/offline profile（adapter identity=`local_workspace`）。**TDD**：first assert route load/no configured input makes zero external request and reports `unconfigured`, then cover StorageProfile lifecycle success/409/disabled/connection-test outcomes; preserve credential redaction, frozen Adapter/Profile selection and no fallback. Non-goals are catalog/storage ownership, harness ownership and real browser FFmpeg oracle; existing acceptance commands apply。

Provider/Profile/Model/Skill UI command 必须携带 expectedRevision/If-Match；409 显示 conflict 不乐观覆盖。Credential 只显示掩码/rotation state，主密钥缺失显示 `credential_master_key_unavailable`，不得传回或记录 algorithm ciphertext nonce authTag/keyVersion/AAD 的敏感值。

Provider/Profile detail 还必须按 operation 显示 owner 返回的 `maxConcurrency`、rate window/limit、policy revision、quota `known|unknown|exhausted`、native remaining/reset/source/capturedAt 和 429/`Retry-After` diagnostic。编辑 policy 使用 expectedRevision；页面加载和 quota refresh 不产生 ProviderCall。Model detail 根据 owner reference proof 显示 delete 或 disable-only，禁止用前端缓存推断“无引用”。

## Shared UI、Table 与 Form 分层

Provider/Model/Skill 列表统一使用 shared/ui 的 DataTable 外壳和 TanStack Table 的 column/sort/filter model；动态参数统一由 React Hook Form + Zod schema resolver 生成。UI 只显示 owner 返回的 schema、effective value、revision 和 redacted diagnostic，不复制 catalog/credential facts，也不在客户端计算 quota 或 operation lease。表单提交沿用 owner CAS，409 只 refetch 并保留原始错误。

组件测试须覆盖列定义稳定性、空/unknown 状态、动态字段增删、字段级错误聚焦、键盘/ARIA、masked secret 和 read-only load zero-call；浏览器验证须证明表格筛选、参数校验、candidate diff、explicit probe 和 disable-only 均不引入第二组件库或隐式外部调用。
