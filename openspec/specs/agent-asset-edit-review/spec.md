# agent-asset-edit-review Specification

## Purpose
TBD - created by archiving change implement-agent-asset-edit-review. Update Purpose after archive.
## Requirements
### Requirement:AssetEdit 单一 result AssetVersion 与 canonical accept
verified Provider terminal success SHALL 是 result `AssetVersion` 唯一 append 时点，retry/reconcile MUST 返回同一 version/candidate。review Schema/HTTP DTO/UI action/audit event 仅允许 `accept|reject|retake`；legacy/unknown `approve` MUST validation 且零 current/retake side effect。AssetEdit `accept` SHALL 只追加 AcceptDecision/audit，并以 exact CAS 对同一既有 version 写 scenes current eligibility；MUST NOT 复制 bytes/object/ref 或 append 第二 AssetVersion。reject、stale、foreign accept MUST 零 AssetVersion 数/current/Timeline mutation。

#### Scenario:retry 与 accept 复用唯一 result version
- **WHEN** Provider retry/reconcile 或用户对既有 candidate accept/reject
- **THEN** retry/reconcile 返回同一 version/candidate，accept 仅写 decision/audit/CAS，reject 不改变 AssetVersion 数

### Requirement:总体计划追溯和协调边界
本 change SHALL 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `1.2`、直接实施任务 `3.4` 和共享任务 `5.1`--`5.5`。实施 MUST 以总体任务 `2.1`、`2.2`、`2.3` 的交付及已归档 AssetVersion 契约为可核验前置；总体 plan 仅协调 change 顺序、范围和验收，MUST NOT 成为运行时代码依赖。完整非目标是实现或拥有 Provider Adapter、提示词生成、图片/视频编辑、自动接受、跨项目复制、历史/已发布版本重写，改变 AssetVersion 的 objectKey/hash/append-only/现有 HTTP 契约，以及实现 Timeline、音频、FFmpeg、MP4/SRT、`ProjectPackage`、`exportProfile` 或 `portable` payload；本 change MUST NOT 承担这些职责。

#### Scenario:前置或范围不满足时不伪造运行时依赖
- **WHEN** 实施取证发现 scenes、workflows、catalog 或 AssetVersion 前置尚未可用，或请求将 timeline/export 能力并入编辑审查
- **THEN** 实施保持阻塞或拒绝范围扩大，记录缺失前置；不得导入总体 plan 作为代码模块、伪造 resolver 或产生 `ProjectPackage`

#### Scenario:拒绝完整非目标职责泄漏
- **WHEN** 编辑审查切片尝试承担任一列明的非目标或改变既有 AssetVersion 契约
- **THEN** 架构依赖/契约测试失败，且不写 session、plan、candidate、decision、impact、AssetVersion 或 Outbox

### Requirement:Asset edit session 与通过 Schema 校验的 plan
系统 SHALL 为存在的项目创建带稳定 ID、canonical `schema_version`、revision 和审计状态的 `AssetEditSession`。每个 `AssetEditPlan` MUST 通过共享 Draft 2020-12 Schema 校验，并持久化其基础 AssetVersion、目标范围、影响声明和不可变内容 hash。数据库与共享 Schema 的 `schema_version` MUST 是唯一版本事实源；HTTP DTO 的 `schemaVersion` MUST 只映射同一个值，不得独立赋值。

#### Scenario:提交有效 plan
- **WHEN** 已存在项目中的 open session 收到引用同项目 AssetVersion 且满足 `AssetEditPlan` Schema 的计划
- **THEN** 系统在一个 UoW 中保存计划和审计记录，并返回 plan ID 与 revision

#### Scenario:拒绝无效或 foreign plan
- **WHEN** 计划不满足 Schema、缺少基础版本，或引用其他项目的资产/目标
- **THEN** 系统返回稳定 validation 或 not_found/forbidden 错误，且不写入计划

#### Scenario:拒绝冲突的 schema version 映射
- **WHEN** 数据库或共享 Schema 的 `schema_version` 缺失，HTTP DTO `schemaVersion` 与其不一致，或调用方试图分别提供两个版本值
- **THEN** 系统返回稳定 validation 错误，且不写入 session、plan、candidate、decision、impact 或 Outbox

### Requirement:Conversation 和 turn 所有权
系统 SHALL 将 Agent conversation、message 与 turn 作为 `AssetEditSession` 内的追加事实，由稳定 `conversationId`、`turnId`、`sequence`、`role`（`user|agent`）、状态、scope、`schema_version` 和 correlation 组成。用户 message 只能由显式 command 追加；Agent reply 必须关联同一 turn，且失败/取消/未知状态可查询。重复 `sequence`、跨 session/project scope、未知 role 或 plaintext secret MUST 在 UoW 前拒绝且不写 message、turn、audit 或 Outbox。

#### Scenario:追加用户 message 和 Agent reply
- **WHEN** 用户向当前 session 的 conversation 提交输入，且 owner scope/revision 有效
- **THEN** 系统追加一个递增 sequence 的 `user` message 和 pending turn；Agent 完成后追加同一 turn 的 `agent` reply，基础 AssetVersion 不改变

#### Scenario:刷新后恢复 Conversation
- **WHEN** API/Worker 重启或网络中断后读取已有 conversation
- **THEN** 系统按持久 sequence 返回已保存消息/turn 状态，重复读取不发送新 Agent 请求；未知 in-flight 状态保持可诊断，不报告为成功

### Requirement:从已完成 turn 生成 plan
系统 SHALL 提供显式 `generateAssetEditPlanFromConversation` command，只接受同一 session 中已完成、Schema-valid 的 Agent turn、**image/video** primary selection/explicit refs、base AssetVersion/revision 和 owner correlation。生成结果 MUST 保存 turn/message provenance、schema/hash、费用和影响，初始状态为 `pending_review`；story/script 请求 MUST 进入 TextReview successor/stale closure，audio/TimelineVersion 请求 MUST 进入 Timeline editor owner command，均不得在本 change 创建 AssetEditPlan。

#### Scenario:从已完成 turn 创建可审核 plan
- **WHEN** 用户明确选择已完成 Agent turn 并提交生成计划 command
- **THEN** 系统创建 Schema-valid `AssetEditPlan` 和审计/Outbox，返回 planId/revision，基础 AssetVersion 与现有引用保持不变

#### Scenario:无写入拒绝不合格 turn
- **WHEN** turn pending/failed、跨 session、selection/refs 过期、Schema 无效或缺少 base version
- **THEN** 系统返回 validation/stale diagnostic，且不写 Plan、Candidate、Decision、AssetVersion 或 ProviderCall

### Requirement:Candidate 保留原始 asset version
系统 SHALL 将每个编辑结果记录为 `AssetEditCandidate`，引用 session、plan、基础版本和结果版本。Candidate MUST NOT 覆盖、更新或删除既有 `AssetVersion`，结果版本仅能是既有不可变版本或后续追加的新版本。

#### Scenario:登记 candidate
- **WHEN** 已验证计划登记一个与基础版本不同的候选结果引用
- **THEN** 系统追加候选审计事实，且原 AssetVersion 的内容、objectKey、hash 和版本号保持不变

#### Scenario:拒绝原地替换
- **WHEN** 请求试图把候选内容写入既有 AssetVersion 或将结果指向其他项目
- **THEN** 系统拒绝请求且不修改任何既有版本

### Requirement:MVP-A 编辑输入必须是完整 AssetVersion
系统 SHALL 只接受同项目、可解析的完整 image/video `AssetVersion` 作为 `AssetEditPlan` 的 primary selection、base 和 explicit refs，并冻结每个版本的 ID、revision、content hash 与 owner scope。MVP-A MUST NOT 接受或透传 mask、图片选区、局部区域、图层、视频/音频 start/end time、time range 或 segment edit；这些输入即使被所选 Provider capability 支持，也 MUST 在创建 `AssetEditExecution` intent、Outbox、`ProviderCall`、Storage operation 或任何付费提交前返回 `unsupported_feature`。

#### Scenario:以完整 image AssetVersion 创建计划
- **WHEN** 用户选择同项目的完整 image AssetVersion 作为 base，并只添加完整 image/video AssetVersion refs
- **THEN** 系统按版本 ID/revision/hash 校验并允许计划进入 `pending_review`，不从客户端字段推导局部编辑范围

#### Scenario:在外部调用前拒绝 mask 或时间范围
- **WHEN** Plan 或 execute 请求包含 mask、选区、局部区域、start/end time、time range 或 segment edit
- **THEN** 系统返回 `unsupported_feature`，且不写 execution、Outbox、ProviderCall、Storage operation、Candidate、Decision 或 AssetVersion

### Requirement:Execute owner 和 provider result handoff
系统 SHALL 由 AssetEdit application owner 提供显式 `executeAssetEditPlan`、execution status 和 reconciliation commands，且 target type 只能为 image/video。Execute MUST 只接受 `pending_review`、未过期、scope/base revision 匹配且具有费用/能力确认的 Plan，冻结 `planRevision`、base/selection/refs、`runId`、`nodeRunId`、`logicalOperation`、`correlationId`、provider/model/capability snapshot 与 request fingerprint，并在 commit 后写入 Outbox intent。Provider adapter MUST 由对应 image/video change 消费该 intent；AssetEdit MUST NOT 在同一事务内直接调用 Provider、拥有 ProviderCall 或 RunEvent。缺少 nodeRunId 或请求 story/script/audio/TimelineVersion 时 MUST 稳定拒绝且零写入。

#### Scenario:commit 后执行有效 plan
- **WHEN** 用户明确 execute 一个 valid pending_review Plan，并提交精确 run/node/logical operation 与当前费用确认
- **THEN** AssetEdit owner 只追加 queued execution 和 Outbox，commit 后才允许 Provider worker 消费；Plan、base AssetVersion 与 current storyboard reference 在 Provider 完成前保持不变

#### Scenario:在任何副作用前拒绝执行
- **WHEN** Plan 非 pending_review、stale/foreign/read-only、费用确认失配、缺少 run/node/logical operation 或 capability snapshot
- **THEN** 系统返回稳定 validation/conflict，且不写 execution、Outbox、ProviderCall、RunEvent、StoragePort 或 AssetVersion

### Requirement:Execution reconcile 与 candidate 登记
系统 SHALL 以 execution id 与 `runId + logicalOperation` 幂等保存 `queued|running|waiting_reconciliation|succeeded|failed|submission_unknown|cancel_requested|cancelled` 状态。Provider terminal success MUST 通过 verified result handoff 幂等登记一个 `AssetEditCandidate`，其状态初始为 `pending_review`；retry、Worker restart 或 unknown 状态 MUST 先 reconciliation，不得重复收费或重复登记版本。Candidate reject 只追加拒绝审计，不能删除结果 AssetVersion；candidate accept 仍由精确 CAS command 决定。

#### Scenario:provider 成功后登记一个 candidate
- **WHEN** 同一 execution 的 Provider result 已通过 Storage/metadata 校验并携带 candidate provenance
- **THEN** owner 只登记一个 immutable AssetEditCandidate 与 result AssetVersion linkage，重复 result 返回同一 candidate，current storyboard reference 仍不变

#### Scenario:保留 unknown 和 rejected outcome
- **WHEN** Provider transport outcome unknown、取消后晚到 result、candidate 被 reject 或 result hash/revision/provenance 不匹配
- **THEN** execution 保留 unknown/diagnostic 或 candidate rejected 状态，不创建第二次收费提交、不改变 current、不写 Timeline reference

### Requirement:显式人工接受范围
系统 SHALL 仅在人工提交 candidate、`expectedBaseVersionId` 和非空明确 `scope` 后创建 `AcceptDecision`。scope MUST 是含 stable target/reference IDs 与每项 expected revision 的精确集合，显式列出镜头、场、集或勾选目标，并且全部属于同一项目且与计划影响相符。接受 MUST 以 CAS 在一个 UoW 中全有或全无。`AcceptDecision` 与其仍引用的 `AssetVersion` SHALL 长期保持可读取和 append-only；诊断窗口到期、temporary/derivative cleanup、容量维护、恢复或 GC MUST NOT 删除、覆盖或静默压缩 decision，也不得清理仍被其引用的 AssetVersion。

#### Scenario:接受选定 target
- **WHEN** 用户确认一个未过期候选，并提交同项目的明确镜头、场、集或勾选目标集合
- **THEN** 系统仅在全部 target/revision 同时匹配时持久化不可变 AcceptDecision，并仅为该集合建立新引用/追加版本投影

#### Scenario:拒绝隐式或扩大的 scope
- **WHEN** 接受请求没有范围、要求“全部”、包含未选择目标，或包含其他项目/剧集目标
- **THEN** 系统返回 validation 或 forbidden 错误，不创建 AcceptDecision 且不改变引用

#### Scenario:拒绝清理接受决定及被引用版本
- **WHEN** cleanup、容量维护、恢复或 GC 尝试清理超过诊断窗口的 AcceptDecision 或仍被其引用的 AssetVersion
- **THEN** 系统拒绝或跳过该操作，decision、引用、AssetVersion 内容/hash/revision 与 current eligibility 保持不变并留下稳定诊断

### Requirement:Stale 和只读保护
系统 SHALL 在计划基础版本、candidate 基础版本或目标引用 revision 发生变化时报告 stale/impact。接受过期基础版本 MUST 返回 `409 base_version_conflict`；已发布或历史版本上下文 MUST 只读。

#### Scenario:stale candidate 不可接受
- **WHEN** candidate 创建后基础 AssetVersion 或目标 revision 已变化
- **THEN** impact 查询显示 stale 原因，accept 返回 409，且不创建新引用或版本

#### Scenario:published 或历史 source 只读
- **WHEN** 请求向已发布或历史上下文提交计划、候选或接受决定
- **THEN** 系统返回 `forbidden_read_only`，保留所有既有审计事实

### Requirement:Primary selection 和显式 reference 隔离
系统 SHALL 将每个 AssetEditSession 绑定一个 primary selection 与显式 refs。切换 primary selection MUST 清空或重新验证旧 refs，且会话 MUST NOT 从前一 selection 泄漏上下文、隐式“当前项”或跨项目引用。

#### Scenario:切换 selection 不泄漏 context
- **WHEN** 用户将会话从一个 scene/asset selection 切换到另一个 selection
- **THEN** 系统只将新 primary selection 和重新验证的显式 refs 传给 Agent；旧 refs 不再参与计划、候选或接受

### Requirement:分层持久化和可观察失败
系统 SHALL 通过 domain/application/Repository/UoW/Outbox 边界实现编辑审查；外部 Provider 或媒体处理 MUST 在提交后由其他 change 的 Adapter 负责。HTTP 和 contracts MUST 暴露 execute/status/reconcile、candidate compare、accept/reject 及可诊断 validation、not found、read-only、stale 与 409 失败，且响应 MUST NOT 包含媒体 bytes。

#### Scenario:dependency 未配置
- **WHEN** 后续候选生成所需 Provider 或媒体执行依赖未配置
- **THEN** 系统显式记录未配置/失败状态，不自动 fallback、扣费或报告候选已生成
