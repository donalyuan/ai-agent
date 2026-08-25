## ADDED Requirements

### Requirement:Skill candidate 与 Provider runnable gate 的只读投影
系统 SHALL 投影八项 candidate 的 provenance、approval、enabled；`novel-writing` 和 `drama-skills` SHALL 是 `verified_snapshot`、`approved`、`enabled=true` 且为 `drama-mvp-a-default` 的唯一默认 binding，其他六项 SHALL 是 `pending_provenance`、`not_approved`、`enabled=false`。Worker 启动或默认 Run MUST NOT 依赖后六项；仅 node `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 匹配时才按需读取。

首次 connection-test/probe UI action SHALL 仅对已安装、`approval=approved`、`featureGate=MVP-A` 且用户提供 explicit live opt-in、已选 profile、可解析 credential、timeout 的 operation 可用，并在成功时冻结 capability snapshot，MUST NOT 要求既有 snapshot 或 `runnable=true`，也 MUST NOT 因 disabled-for-run 阻断。snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default、Run resolution 与 live invocation；后三者 SHALL 额外要求成功 snapshot 与 `runnable=true`。MVP-B candidate、uninstalled、not-approved 或缺 explicit opt-in/profile/credential/timeout MUST 零 probe/外部调用；TTS/ASR、MiniMax H3、Seedance 2.5、Agnes 未选中 mode 不可运行；默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），且 explicit live opt-in MUST 保持。

#### Scenario:设置页允许首次 explicit probe 但不允许未完整 runnable 的解析
- **WHEN** 用户以 explicit live opt-in/profile/credential/timeout 对已安装、approved、`featureGate=MVP-A` 且无 snapshot 或 `runnable=false` 的 operation 发起首次 connection-test/probe
- **THEN** UI 仅调用 owner probe command 并显示 pending/result；成功冻结 snapshot，未发生 enable/default/Run resolution

#### Scenario:设置页拒绝不具备对应 action 前置的 operation
- **WHEN** 用户对 MVP-B/uninstalled/not-approved candidate、或缺 explicit live opt-in/profile/credential/timeout 的 operation 请求首次 probe，或对 snapshot-missing/`runnable=false`/disabled-for-run 的 operation 请求默认绑定、Run resolution 或 live invocation
- **THEN** UI 可显示 owner state；前一类不发起连接测试或外部调用，后一类不默认绑定、不 resolve 或 invoke

### Requirement:Provider, model and Skill settings routes
系统 SHALL 在 `/settings/providers`、`/settings/providers/:providerId`、`/settings/skills` 及项目模型设置深链中显示 Provider、Profile、Model、SkillRevision 与 capability snapshot。UI MUST 以 owner ID/revision/schemaVersion/snapshot hash 为事实源，且不得将 catalog 复制为 Zustand 或本地持久化数据。

#### Scenario:inspect a provider profile
- **WHEN** 用户打开 Provider Profile
- **THEN** UI 显示 owner 返回的掩码 credential 状态、模型、SkillRevision、snapshot capturedAt 与 revision，不显示 plaintext secret

#### Scenario:receive an invalid catalog DTO
- **WHEN** 响应缺少 canonical version、含未知字段或包含 secret/plaintext credential
- **THEN** Zod boundary 拒绝响应、显示安全诊断并不写入 Query/store

### Requirement:Explicit catalog lifecycle commands
Provider/Profile/Model/Skill MUST 在 settings 中暴露显式 create、edit、enable 和 disable action。每项 mutation MUST 携带 `expectedRevision` 和 `If-Match`；Skill content 变更 MUST 追加新的 immutable `SkillRevision`，而 enable/disable MUST 保留历史 snapshot。`409 revision_conflict` MUST 刷新 owner state，且不执行 optimistic overwrite。

#### Scenario:create or edit a catalog resource
- **WHEN** 用户在设置页明确提交合法 Provider/Profile/Model/Skill payload 和当前 revision
- **THEN** UI 调用对应 owner command，显示新的 revision/state，并只失效受影响 query

#### Scenario:enable or disable without rewriting history
- **WHEN** 用户明确启用或停用资源
- **THEN** UI 提交 expectedRevision/If-Match，显示 owner state；既有 SkillRevision、CapabilitySnapshot 和历史 Run snapshot 保持不变

#### Scenario:reject a stale lifecycle mutation
- **WHEN** owner 返回 `409 revision_conflict` 或资源属于其他项目/已读历史 revision
- **THEN** UI 放弃乐观写入、刷新权威资源并显示原始诊断，不产生第二个 mutation 或覆盖历史

### Requirement:Dedicated StorageProfile settings lifecycle
系统 SHALL 在 `/settings/storage-profiles` 和 `/settings/storage-profiles/:storageProfileId` 提供 StorageProfile 专属列表、详情和表单。UI MUST 以 owner DTO 显示并编辑 `storageProfileId`、`schemaVersion`、`revision`、`name`、`adapterKey=tos`、`enabled`、`bucketBindingId`、`region`、`endpoint`、`privateBucket`、`credentialRef`、`credentialStatus`、connect/read/write timeout、presign max TTL 和 project scope；credential status 只能显示 `configured|unconfigured|rotating|failed|master_key_unavailable` 与 masked summary。create/edit/enable/disable MUST 调用 `/v1/storage-profiles` owner commands 并携带 `expectedRevision`/`If-Match`；页面加载、刷新、筛选和表单草稿 MUST 不产生 mutation。

#### Scenario:create or edit a StorageProfile
- **WHEN** 用户在 StorageProfile 页面明确提交 Bucket/Region/Endpoint/private policy、credential reference、timeout、TTL 和 project scope
- **THEN** UI 调用 owner command，显示新的 revision/enabled/credentialStatus，只失效该 profile query，不回显 secret、envelope、objectKey 或 workspace URI

#### Scenario:reject stale StorageProfile CRUD or toggle
- **WHEN** owner 返回 `409 storage_profile_revision_conflict`，或 profile/bucket 的 project scope 不匹配
- **THEN** UI 放弃乐观写入、显示 expected/current revision 与原始诊断、刷新权威 profile，且不创建第二 mutation、session、adapter 或 AssetVersion

#### Scenario:explicitly run a StorageProfile connection test
- **WHEN** 用户点击 connection-test 并确认当前 profile revision/snapshot
- **THEN** UI 发送一次带 timeout 和 `probeCorrelationId` 的 owner command，显示 pending 后的 `connected` 或 `unconfigured|validation|authentication|network|timeout` 脱敏结果；测试不改变 profile config revision、不创建对象/AssetVersion、不把失败切换为 Local

#### Scenario:preserve masked credential status on failure
- **WHEN** profile 未配置、disabled、认证失败、network timeout 或 master key 不可用
- **THEN** UI 显示 owner 的 masked credential status 和原始 redacted diagnostic（主密钥缺失为 `credential_master_key_unavailable`/503），不自动重试、启用、rotate 或报告 connection success

### Requirement:Masked credential replacement and rotation
系统 SHALL 只以一次性受控输入支持 credential replace/rotate，并在提交/取消/卸载后清空输入。API response、Query cache、错误、toast、审计显示和浏览器持久化 MUST NOT 包含 plaintext；设置页读取或刷新不得触发 rotate。

#### Scenario:replace a credential
- **WHEN** 用户在明确的 replace command 中输入新 credential 并提交
- **THEN** UI 只显示 owner 返回的 masked state/rotation status，清除输入并失效受影响 Profile cache

#### Scenario:inspect or fail a credential action
- **WHEN** 用户刷新页面或 owner 返回 validation/authentication/network error
- **THEN** UI 不回显输入 secret，不自动重试/rotate，并显示经 redaction 的可诊断错误

### Requirement:Manual model synchronization and capability acceptance
系统 SHALL 将模型同步和 capability discovery 显示为只读 diff/proposal；只有明确用户 accept 才可提交 owner mutation。路由进入、自动 refresh、取消 dialog 或未选择任何 diff 项 MUST NOT 接受、更改模型或触发同步/probe。

#### Scenario:review and accept a model diff
- **WHEN** 用户显式启动 sync、阅读新增/移除/能力/参数 diff 并确认选定项
- **THEN** UI 提交精确 diff proposal/revision，成功后刷新该 Provider 的模型与 snapshot

#### Scenario:abandon a sync proposal
- **WHEN** 用户关闭 diff、刷新页面或拒绝 proposal
- **THEN** UI 不发送 accept mutation，现有模型和 snapshot 保持 owner 原值

### Requirement:Overrides and parameter Schema form
系统 SHALL 显示 system、project、workflow 三层覆盖值、优先级、effective source 与 owner revision。参数编辑 MUST 由 Zod 和 owner 提供的 JSON Schema 驱动，未知参数、类型不符、未启用 capability 或过期 revision MUST 阻止保存，不创建独立参数版本源。

#### Scenario:view an effective project override
- **WHEN** 项目值覆盖 system 值且 workflow 未覆盖
- **THEN** UI 同时显示三层值、effective value 为 project 及其来源 revision

#### Scenario:submit invalid parameters
- **WHEN** 用户输入不符合 parameter Schema 的值或提交 stale revision
- **THEN** UI 显示字段错误或 revision conflict，且不发送有效配置替代写入

### Requirement:Explicit connection and capability probe
系统 SHALL 只在用户点击明确 command 后请求 connection test 或 capability probe，并显示目标 Profile、snapshot context、状态与 redacted 诊断。浏览设置、读取列表、表单校验或模型选择 MUST NOT 触发真实调用。

#### Scenario:explicitly probe a configured profile
- **WHEN** 用户点击 capability probe 并确认 Profile
- **THEN** UI 发出一次 owner probe command，显示 pending 和 owner 返回的 snapshot/失败状态

#### Scenario:encounter an unconfigured profile
- **WHEN** Profile 未配置或 probe 失败
- **THEN** UI 显示 unconfigured/原始 redacted failure，不自动 fallback、同步或报告 capability 可用

### Requirement:Cost confirmation, project threshold and retention controls
系统 SHALL 显示并允许有权限用户通过 projects owner 编辑项目文本费用阈值，并显示 threshold snapshot ID/revision/hash；catalog 只消费该 snapshot。图片/视频批量生成前 MUST 显示估算/币种/来源并要求明确确认，文本估算超阈值 MUST 显示 `waiting_review`，`cost=unknown` MUST 显示独立强确认。确认请求 MUST 绑定 owner 提供的 threshold snapshot、`runId + logicalOperation`、request fingerprint、稳定本地 `userUuid` 和当前 revision；重试、恢复、参数变化或其他 Run 不得复用。UI MUST 显示 `retention_policy/version/hold` 与至少 30 天诊断状态，但不得在客户端解除 hold 或伪造 retention。

#### Scenario:confirm an exact image or video batch
- **WHEN** 用户查看当前 Run 的批量 operation、费用来源和影响后明确确认
- **THEN** UI 只提交同一 `runId + logicalOperation`、fingerprint、revision 和稳定 userUuid；owner 成功后显示 confirmationId，刷新/重试不重复确认或收费

#### Scenario:require explicit confirmation for unknown cost
- **WHEN** owner 返回 `cost=unknown`，或文本估算超过项目阈值
- **THEN** UI 显示 unknown/over-threshold 原因并保持 `waiting_review`，在用户完成对应强确认前不触发付费 command

#### Scenario:reject stale or mismatched confirmation
- **WHEN** confirmation 属于其他 Run/logical operation、fingerprint/参数/revision 已变化，或 retention hold 状态非法
- **THEN** Zod/owner boundary 拒绝 mutation，UI 刷新 authoritative BudgetGate/retention 状态，不自动复用旧确认

### Requirement:设置页不代替运行级 Skill 路由选择
设置 UI SHALL 只管理 Skill/SkillRevision 的 create/edit/enable/disable、provenance/approval/capability 展示。它 MUST NOT 展示为某次 launch 的最终选择器，也不得在启用、刷新或保存后自动解决 `needs_human_selection`。运行级候选、过滤/排序原因和人工 selection SHALL 在 Workbench 从 text/Agent runtime owner 读取，最终选择由 workflows/runs snapshot 展示。

#### Scenario:启用 Skill 不自动解决 pending route
- **WHEN** 用户在设置页启用一个 Skill，而现有 launch route decision 等待人工选择
- **THEN** UI 只显示 catalog lifecycle 更新；该 route 仍待 Workbench 明确选择，零 Run/NodeRun/TextModel/Provider 调用

### Requirement:Settings verification boundary
系统 SHALL 以 component/state/Zod/Query contract 和 Playwright E2E 覆盖 catalog CRUD/enable/disable/CAS、StorageProfile/Bucket/Region/Endpoint 表单与 CRUD/启停/connection-test/409、redaction、rotate、diff acceptance、覆盖、参数、explicit probe、文本阈值、批量/unknown cost 确认、run/logical operation 绑定和 retention hold。默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），且验证访问/渲染不产生 Provider 或 Storage mutation，不创建或切换 profile。

#### Scenario:execute settings UI regression tests
- **WHEN** 维护者运行本 change 的验收命令
- **THEN** 测试能证明 StorageProfile 字段、CRUD/启停/connection-test、409 zero-write、masked status、无 plaintext/隐式调用并定位失败层级，且实现前 tasks 全部未勾选

### Requirement:Explicit live probe matrix boundary
系统 SHALL 将默认 browser E2E 保持在 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），且不得由设置页面自动触发 AgentScope/Provider/TOS/FFmpeg 或创建/切换 profile。live `1x1x1` 仅为显式 opt-in provider/storage/renderer probe（1 Episode x 1 Scene x 1 Shot）；未配置 MUST 记录 `unconfigured`，不得伪造成功。

#### Scenario:inspect an unconfigured live profile
- **WHEN** 未配置 account/credential/renderer 输入而请求 live probe
- **THEN** UI 显示 `unconfigured` 与 owner 原始诊断，不启动默认 E2E 或报告 capability 成功
### Requirement:Safe catalog and credential settings
Settings MUST 发送 expectedRevision/If-Match、展示 conflict 和 masked credential/rotation state，且绝不暴露 envelope、key material、object key 或 workspace URI。

#### Scenario:Master key is unavailable
- **WHEN** real profile 需要不可用 master key
- **THEN** UI 展示 `credential_master_key_unavailable`；不发生隐式 fallback 或 credential disclosure。

### Requirement:Operation policy、quota 与限流诊断 UI
设置页 SHALL 按 Provider/Profile operation 显示并允许有权限用户编辑 maxConcurrency、rate window/limit、bounded admission policy 和 429/`Retry-After` policy revision；quota SHALL 只读显示 owner 的 `known|unknown|exhausted`、native remaining/reset/source/capturedAt。UI MUST NOT 自行计算 active concurrency、推导 unknown quota 或因刷新触发 ProviderCall。

#### Scenario:编辑 operation policy 并观察 quota
- **WHEN** 用户以当前 expectedRevision 保存有效的并发/限流 policy，并刷新 quota status
- **THEN** UI 显示新 owner revision 和原生 quota observation；stale policy 返回 409 并 refetch，刷新本身零外部调用

#### Scenario:显示 429 或 unknown quota
- **WHEN** owner 返回 429/`Retry-After`、quota unknown 或 exhausted
- **THEN** UI 显示原始 redacted diagnostic、source 与可重试时间，不伪报可用、不自动换模型或重复发起 operation

### Requirement:历史引用 Model 的 disable-only UI
Model detail SHALL 读取 owner reference proof。存在 CapabilitySnapshot、ProviderCall、Run、project default、WorkflowVersion 历史引用或 proof unavailable 时，UI MUST 只提供 disable，不得提供可成功的 physical delete；disable MUST 保留 identity/snapshot/audit。

#### Scenario:被引用模型不能删除
- **WHEN** 用户查看或尝试删除 `model_in_use`/reference-proof-unavailable 的 Model
- **THEN** UI 显示引用保护与 disable action，拒绝 delete mutation或展示 owner 拒绝，且历史 Model/snapshot 仍可读取

### Requirement:Provider/Model/Skill 表格与动态参数表单
设置页 SHALL 使用 TanStack Table 展示 Provider、Model、Skill owner rows，并使用 React Hook Form + Zod 按 owner 参数 schema 渲染和校验动态参数表单；表格和表单 MUST 复用 `shared/ui`，不得引入第二套组件库、基础变体或页面级手写 CSS。提交 MUST 携带 owner expectedRevision/If-Match，读取/筛选/动态渲染 MUST NOT 触发 ProviderCall、probe 或 Run。

#### Scenario:编辑动态参数
- **WHEN** 用户在 Provider/Model/Skill 表格选择资源并提交有效或无效动态参数
- **THEN** TanStack Table 保持稳定列/排序，RHF + Zod 显示字段级错误与焦点，成功只提交一次 owner command，409 refetch 且不覆盖他人更新

#### Scenario:参数 schema 不可用
- **WHEN** owner 返回缺失、未知或不兼容参数 schema
- **THEN** 表单显示原始不可用诊断并禁用提交，不猜测字段、不发起 probe、不伪造默认值
