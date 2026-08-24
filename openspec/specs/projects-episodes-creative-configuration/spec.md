# projects-episodes-creative-configuration Specification

## Purpose
TBD - created by archiving change extend-projects-episodes-creative-slice. Update Purpose after archive.
## Requirements
### Requirement:Project 创作事实的唯一 owner 与版本事实
系统 SHALL 扩展既有 Project/Episode owner，使 Project 唯一拥有 `creationMode`、CreativeBrief、项目创作设置、项目文本费用确认阈值和 accepted StorySpec current reference，使 Episode 唯一拥有 accepted ScriptSpec current reference。数据库与共享 Schema 的 `schema_version` SHALL 是 canonical 版本事实，HTTP `schemaVersion` MUST 只映射同一值。Provider/Profile/Model/Skill default/override MUST 继续由 catalog 拥有，CostConfirmation/provider usage 由 catalog 拥有，BudgetGate/Run 状态由 workflows/runs 拥有。

#### Scenario:拒绝 owner 泄漏和双版本事实
- **WHEN** projects command 尝试保存 Provider default、Provider usage、Run/BudgetGate 状态，或请求缺少/冲突 `schema_version` 与 `schemaVersion`
- **THEN** 系统在 UoW 前返回稳定 validation/architecture failure，且不写 Project、CreativeBrief、设置、Episode、audit 或 Outbox

### Requirement:不可变 CreativeBrief 与明确 creationMode
系统 SHALL 支持 `creationMode=original|adaptation`，并为每个 Project 使用稳定 `creativeBriefId` 和只追加 `CreativeBriefVersion`。每个 version MUST 精确包含 `subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeDurationSeconds`、`episodeCount`、`scenesPerEpisode`、`shotsPerScene`、`schema_version`、revision、payloadHash 和项目归属；时长表示每集目标时长，三个计数 MUST 为正整数。修改 MUST 创建 successor version，并仅通过 expected Project/Brief revision CAS 移动 current pointer。

#### Scenario:保存 original CreativeBrief
- **WHEN** 用户为同项目提交完整 canonical 字段、`creationMode=original` 和当前 expected revisions
- **THEN** projects owner 追加不可变 CreativeBriefVersion、移动 current pointer 并返回新 Project/Brief revision，且不要求或创建 SourceMaterial、Run、ProviderCall

#### Scenario:CreativeBrief 冲突不覆盖历史
- **WHEN** 请求缺少字段、计数/时长无效、跨项目、使用旧 revision 或尝试原地覆盖既有 version
- **THEN** 系统返回 422/403/409，current pointer 与全部历史 version、audit、Outbox 保持不变

### Requirement:精确 adaptation source binding snapshot
adaptation Project SHALL 只在 text owner 已验证的 handoff 上追加 `CreativeBriefSourceBindingSnapshot`，且快照 MUST 精确包含 `projectId`、`sourceMaterialId`、`sourceMaterialRevision`、`sourceContentHash`、`creativeBriefId`、`creativeBriefRevision`、`creativeBriefPayloadHash`、`parseStatus`、`validationStatus`、`bindingStatus`、`bindingVersion` 与 `schema_version`。original MUST NOT 保存或要求该快照。快照 MUST NOT 包含 SourceMaterial 正文、StoredObject、AssetVersion 内容或 parse diagnostics。

#### Scenario:绑定有效 adaptation source
- **WHEN** adaptation source 与 current CreativeBrief 同项目，revision/hash/status/version 全部匹配且 command 使用当前 expected revisions
- **THEN** projects owner 追加不可变 binding snapshot 并使其可供 Run freeze 读取，不重新上传、解析或复制 source 内容

#### Scenario:拒绝错误模式或不精确 source
- **WHEN** original 请求 source binding，或 adaptation handoff 为 foreign、stale、hash/revision/status/version mismatch
- **THEN** 系统返回稳定 validation/conflict，且不移动 brief/binding pointer、不调用 Storage/TextModel/Provider

### Requirement:版本化项目文本费用阈值
系统 SHALL 使用只追加 `ProjectCreativeSettingsVersion` 保存 `textCostConfirmationThreshold`；非空值 MUST 包含非负 decimal `amount` 和 ISO 4217 `currency`，`null` 表示不设置金额阈值。更新 MUST 使用 expected Project/settings revision CAS。catalog SHALL 只读取 projects owner 的 threshold snapshot，不得持久化第二份项目阈值；`cost=unknown` 的强确认规则不因 threshold 为 null 而取消。

#### Scenario:更新并读取项目阈值
- **WHEN** 用户以当前 expected revisions 保存合法 amount/currency
- **THEN** projects owner 追加 settings version、移动 current pointer，catalog/workflows 可读取同一 snapshot ID/revision/hash 进行费用门判断

#### Scenario:拒绝无效或重复 owner 阈值
- **WHEN** amount 为负数、currency 无效、revision 过期，或 catalog 尝试保存独立阈值副本
- **THEN** 系统返回 validation/409/architecture failure，既有 settings current 与 BudgetGate/CostConfirmation 均不变

### Requirement:Project/Episode accepted 文本 handoff 原子落地
projects application SHALL 提供 `ApplyProjectEpisodeTextHandoff` typed batch command，只接受 accepted immutable TextReview handoff。command MUST 包含 batch/handoff ID/revision、projectId、StorySpec candidate/source IDs/hashes 与 immutable reference、按 number 排序的完整 Episode stable IDs 和每集 ScriptSpec immutable references、payloadHash、correlationId、expected Project/existing Episode revisions 和 `schema_version`。系统 MUST 在一个 UoW 中全有或全无地移动 Project StorySpec reference、创建/更新 Episode 及其 ScriptSpec reference，并追加 audit、Outbox 和 `ProjectEpisodeHandoffAck`。

#### Scenario:原子应用完整 accepted handoff
- **WHEN** 完整 handoff 的 accepted provenance、项目归属、成员集合、hash/revision 和 expected revisions 全部有效
- **THEN** Project/Episode owner 在一个事务落地 StorySpec、Episode 顺序和每集 ScriptSpec reference，并返回包含相同 correlation/fingerprint 的 ack

#### Scenario:任一成员冲突时全批失败
- **WHEN** 任一 Episode/ScriptSpec 缺失、重复、foreign、stale、hash/revision mismatch，或 handoff 未 accepted
- **THEN** 整个 command 返回 validation/409，零 Project/StorySpec/Episode/ScriptSpec pointer、audit、Outbox 或 ack 写入，媒体门保持关闭

### Requirement:Handoff 幂等与 owner ack 边界
相同 handoff ID 与相同 canonical fingerprint 的重试 SHALL 返回原 `ProjectEpisodeHandoffAck`，不得重复创建 Episode、版本、audit 或 Outbox；相同 ID 不同 fingerprint MUST 返回 conflict。Project/Episode ack MUST NOT 被解释为 Scene/Shot、AssetBible 或其他 owner ack，workflows/runs MUST 等待总体合同要求的全部 owner ack 后才能进入付费媒体。

#### Scenario:重试返回既有 ack
- **WHEN** Worker 重启或 Activity retry 以相同 handoff ID/fingerprint 重放已成功 command
- **THEN** 系统返回原 ack 和相同 owner revisions，Episode 数量、audit/Outbox 数量及媒体 operation 数量不变

### Requirement:CAS HTTP 与只读创作投影
系统 SHALL 提供 project-scoped camelCase HTTP command/query，以 `expectedRevision` 和 `If-Match` 同值保护 creationMode、CreativeBrief、settings、binding 和 handoff mutation；revision conflict 返回 409。读取 projection SHALL 暴露 current/historical version identity、revision/hash 和 owner references，MUST NOT 返回 SourceMaterial/StorySpec/ScriptSpec 正文。GET、页面加载、select 和 light manifest projection MUST 为零 mutation。

#### Scenario:读取创作配置不触发业务副作用
- **WHEN** Workbench、text input、Run freeze 或 light export 查询同项目 current/historical projection
- **THEN** API 返回 owner IDs/revisions/hashes 与合法字段，且不移动 pointer、不创建 Episode/Run/ProviderCall/audit/Outbox

#### Scenario:If-Match 与 body revision 冲突
- **WHEN** command 的 `If-Match` 缺失、过期或与 `expectedRevision` 不同
- **THEN** API 返回 409/422 和 current revision diagnostic，所有 creative facts 保持不变

### Requirement:迁移、审计与阶段一证据
实现 SHALL 使用 additive migration、Repository/UoW、同事务 audit/Outbox、共享 contracts、domain/application/adapter/HTTP BDD/TDD，并保持既有 Project/Episode API 兼容。`E2E-MVPA-001` 的 `S02`/`S04` MUST 记录 projects owner 的 Brief/settings/source snapshot、accepted handoff/ack、focused failure 和 no-side-effect evidence；默认 fixture MUST 使用 Mock Provider 与显式 Local test/offline profile。

#### Scenario:迁移并验证 zero-episode 项目
- **WHEN** 既有 Project/Episode 数据 upgrade 到新 revision，并以 original/adaptation 正反 fixtures 执行 E2E
- **THEN** 旧身份/状态/revision 可读，未配置 creative facts 不被伪造，成功证据和冲突零写入证据均可追溯
