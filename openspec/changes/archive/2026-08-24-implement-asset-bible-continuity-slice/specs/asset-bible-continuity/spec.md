## ADDED Requirements

### Requirement:项目级 AssetBible 与稳定 typed entry
每个 Project SHALL 最多拥有一个稳定 AssetBible identity。AssetBible SHALL 使用稳定 UUID 管理 `entryType=character|look|location|scene_visual|prop|visual_style` 的 typed `AssetBibleEntry`；每个 entry 内容变化 MUST 追加不可变 `AssetBibleEntryVersion`，包含稳定 version ID、entry/project identity、单调 version、structured attributes、canonical payload hash、`schema_version`、revision 和 actor UUID。Look MUST 引用同项目 Character，SceneVisual MUST 引用同项目 Location；被历史引用的 entry/version 不得物理删除。

#### Scenario:创建并版本化资产设定条目
- **WHEN** 用户在同项目创建合法 Character/Look/Location/SceneVisual/Prop/VisualStyle 并以当前 expected revisions 修改其内容
- **THEN** 系统保存稳定 entry identity、追加 successor version 并移动 AssetBible current map，旧版本继续只读可解析

#### Scenario:拒绝不合法 typed relationship
- **WHEN** entry 使用未知类型、跨项目 reference、Look 缺 Character、SceneVisual 缺 Location、形成循环或尝试覆盖/删除历史 version
- **THEN** 系统返回 validation/conflict，且不写 entry/version/current pointer、audit 或 Outbox

### Requirement:AssetVersion 与 GenerationSpec 仅以 owner reference 关联
AssetBibleEntryVersion SHALL 只保存 `referenceAssetVersionRefs[]` 与 `generationSpecRefs[]` 的 owner ID/revision/hash/用途，不得保存媒体 bytes、objectKey、永久 URL、提示词正文或复制 AssetVersion/GenerationSpec metadata。owner query unavailable MUST 返回 partial/unavailable diagnostic，不得把引用解释为不存在或复制内容补偿。

#### Scenario:读取带媒体与生成规格引用的条目
- **WHEN** 调用方读取有权限的 entry version
- **THEN** AssetBible 返回稳定 owner references/hashes，并由相应 owner projection 提供可用摘要，读取不调用 Provider、不复制正文或媒体

### Requirement:显式分层 override 与确定性解析
系统 SHALL 只接受 `project -> episode -> scene -> shot` 四层 `AssetBibleOverrideAssignment`。assignment MUST 绑定 project/scope type/scope ID、entry/version、expected scope/entry revisions、assignment revision 和 `schema_version`；所有 scope MUST 属同一项目且 entry type 兼容。resolver SHALL 按从 project 到 shot、最具体 scope 胜出的固定规则生成 immutable `ResolvedContinuitySnapshot`，包含完整 override chain、resolved version refs、source revisions、target scope 与 canonical hash；相同输入必须得到相同排序与 hash。

#### Scenario:解析四层连续性覆盖
- **WHEN** 同一条目在 project、episode、scene、shot 存在合法 assignment
- **THEN** resolved snapshot 选择 shot version，同时保留四层来源链、revision 和确定性 hash 供 ShotSpec/Agent/Run 冻结

#### Scenario:拒绝歧义或错误归属的覆盖
- **WHEN** assignment 跨项目/父级、引用 stale/disabled version、类型不兼容、循环或同层存在无法裁决的重复值
- **THEN** resolver 返回稳定 validation/conflict，不生成或更新 snapshot、不标记下游 stale

### Requirement:只读且完整的连续性影响分析
系统 SHALL 提供 `PreviewAssetBibleRevisionImpact`，以 base entry version、candidate successor payload/hash、expected AssetBible/entry revision 和语义 scope 创建不可变 `ContinuityImpactAnalysis`。分析 MUST 通过 owner query 收集直接或经 resolved snapshot 引用该 version 的精确 Episode/Scene/Shot ID/revision 集合，并为每项保存 reason、current snapshot/hash 与建议动作。owner unavailable、分页不完整或 revision 漂移时分析状态 MUST 为 `incomplete`，不得接受。

#### Scenario:预览项目级条目修改影响
- **WHEN** 用户提交合法 candidate successor 进行预览且所有 owner projections 可验证
- **THEN** 系统返回按稳定 ID 排序的精确 Episode/Scene/Shot 引用集合、set hash 和原因，且不创建 successor、stale、task、ProviderCall 或媒体操作

#### Scenario:不完整影响集合不可接受
- **WHEN** 任一 owner projection unavailable、跨项目、分页遗漏、revision 漂移或无法证明完整集合
- **THEN** analysis 标记 `incomplete` 并保留 diagnostic，后续 accept 被拒绝且 current/历史版本不变

### Requirement:原子接受 successor 与 ContinuityRevisionTask
`AcceptAssetBibleRevision` SHALL 要求 analysis ID/revision/hash、candidate payload hash、完整实际 target reference set、expected AssetBible/entry/scope revisions、稳定 actor UUID 和语义范围。系统 MUST 在同一 UoW 重新校验 set/hash/revisions，并全有或全无地追加 successor entry version、移动 current pointer、追加 AcceptDecision/audit/Outbox，并为受影响 target 创建或去重 `ContinuityRevisionTask`。任务状态 SHALL 只取 `pending|acknowledged|resolved|superseded`，并保存 target ID/revision、old/new entry versions、old snapshot/hash、reason 和 correlation。

#### Scenario:接受完整且未过期的影响集合
- **WHEN** 用户明确接受，analysis 完整且 candidate/set/hash/expected revisions 与当前 owner facts 完全一致
- **THEN** 系统原子创建一个 successor、移动 current，并为精确目标集合创建任务；旧版本和旧 snapshot 保持不变

#### Scenario:任一 revision 过期时全有或全无失败
- **WHEN** analysis、entry、AssetBible、scope 或任一实际 target revision/hash 已变化，或提交集合缺失/重复/foreign
- **THEN** 整个接受返回 409/validation，零 successor、pointer、AcceptDecision、audit、Outbox 或 task 写入

### Requirement:stale 传播不自动改写下游
ContinuityRevisionTask SHALL 只声明对应 Episode/Scene/Shot 及其已冻结 snapshot 需要重新评估。AssetBible owner MUST NOT 自动修改 StorySpec、ScriptSpec、Scene/Shot/ShotSpec、TextReview candidate、GenerationSpec、AssetVersion、storyboard current、Timeline 或 Run，也 MUST NOT 自动调用 Provider。对应 owner 只有在显式 typed command/ack 后才能发布 successor 或解决任务。

#### Scenario:接受设定修改后下游保持可审计 stale
- **WHEN** 已被 ShotSpec 和当前媒体引用的 entry version 被 successor 取代
- **THEN** 旧 ShotSpec/AssetVersion/current reference 仍指向原 snapshot，任务显示 pending，且没有隐藏重生成或 current 替换

### Requirement:跨 owner typed handoff 与精确 projection
TextReview accepted handoff 对叙事候选实际引用的初始 AssetBible entry/version SHALL 发出创建/引用请求，AssetBible owner MUST 以 typed command 校验 project/stable IDs/hash/revision 并返回独立幂等 ack；不被叙事候选引用的条目不得由 handoff 隐式创建。Scene/Shot owner SHALL 只提交/读取 assignment 与 resolved snapshot reference；GPT Image、Agent context、Workflow/Run SHALL 只读消费 accepted snapshot ID/hash、GenerationSpec refs 和 AssetVersion refs。任何 consumer MUST NOT 直接写 AssetBible tables 或把 Provider result 自动设为 entry reference/current。

#### Scenario:幂等应用初始 TextReview handoff
- **WHEN** 相同 accepted handoff ID/fingerprint 因 Worker 重启被重放
- **THEN** AssetBible owner 返回原 ack 和相同 entry/version refs，不重复创建版本、任务、audit/Outbox，也不解锁其他 owner 尚未 ack 的媒体门

#### Scenario:拒绝 consumer 越权写入
- **WHEN** Provider、Agent、Workflow、Scene/Shot 或 UI 尝试直接改 AssetBible current/version 或自动绑定生成结果
- **THEN** 架构/contract boundary 拒绝请求，AssetBible 和下游 current 均不变

### Requirement:CAS HTTP、读取与持久化约束
系统 SHALL 提供 project-scoped camelCase entry/version/assignment/resolution/impact/task APIs，mutation 使用同值 `expectedRevision` 与 `If-Match`，冲突返回 409。数据库/shared Schema 的 `schema_version` SHALL 是唯一版本事实，HTTP `schemaVersion` 仅作 alias。列表、筛选、resolved query 与 impact preview MUST 为零 Provider/Worker/Run/Timeline mutation；migration SHALL additive，并以 FK、唯一、类型、hash 与项目归属约束保护事实。

#### Scenario:读取连续性投影无副作用
- **WHEN** Workbench、Review、GPT Image preflight 或 Agent context 查询同项目 entry/resolved/task projection
- **THEN** API 返回精确 IDs/revisions/hashes/status 或 partial diagnostic，且不创建 snapshot/task/ProviderCall/RunEvent/AssetVersion

#### Scenario:CAS 或 schema 冲突时零写入
- **WHEN** `If-Match` 缺失/过期/与 body 不同，或 `schema_version`/`schemaVersion` 缺失或冲突
- **THEN** API 在 UoW 前返回 409/422，entry/current/assignment/analysis/task/audit/Outbox 不变

### Requirement:BDD/TDD 与阶段一连续性证据
实现 SHALL 包含 domain/application/repository/HTTP/contract/migration/architecture tests，覆盖 typed relationship、override priority、deterministic hash、impact completeness、atomic accept、idempotent ack、历史不可变、owner leakage 与零 Provider side effect。`E2E-MVPA-001` MUST 在文本接受到 ShotCard/image/Agent handoff 间记录 AssetBible owner ack、resolved snapshot、影响任务和 focused failure；默认 fixture MUST 使用 Mock Provider 与显式 Local test/offline profile。

#### Scenario:2x2x3 fixture 保持跨集连续性可追溯
- **WHEN** Mock E2E 为 2 Episodes x 2 Scenes/Episode x 3 Shots/Scene 创建并修改共享 Character/Look/Prop entry
- **THEN** report 可读取精确 resolved chains、受影响 target set/hash、显式 accept/task 和 no-auto-regeneration 证据，所有失败路径保持零越权写入
