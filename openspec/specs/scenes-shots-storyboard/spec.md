# scenes-shots-storyboard Specification

## Purpose
TBD - created by archiving change implement-scenes-shots-storyboard-slice. Update Purpose after archive.
## Requirements
### Requirement:video current CAS、canonical review verb 与 AssetEdit version reuse
scenes SHALL 仅在 Provider terminal result candidate 经人工 `accept` 后执行一次 exact current CAS，之后才可 MediaInspect/derivatives 和 Timeline handoff。derivative `pending|failed|stale` SHALL 仅阻断 Timeline/preview/export，MUST NOT 撤销 accepted/current。review Schema/HTTP DTO/UI action/audit event 仅允许 `accept|reject|retake`；legacy/unknown `approve` MUST validation 且零 current/retake side effect。

AssetEdit accept SHALL 仅追加 AcceptDecision/audit 并对既有同一 AssetVersion 执行 scenes eligibility/current CAS；MUST NOT 复制 bytes/object/ref 或 append 第二 AssetVersion。reject、stale、foreign accept MUST 零 AssetVersion 数/current/Timeline mutation。

#### Scenario:相同 version 的 accept 不产生第二个素材版本
- **WHEN** scenes 接收 current AssetEdit candidate 的有效 accept，或接收 stale/foreign/legacy action
- **THEN** 前者仅写一次 exact CAS；后者 validation 或拒绝，二者均不追加第二 AssetVersion 或更改 Timeline

### Requirement:Phase-one traceability and slice boundary
系统 SHALL 将本 capability 追溯到 `plan-phase-one-drama-mvp-a` 的任务 **2.1**，并遵守共享任务 **5.1**、**5.3**、**5.5**。其实现 MUST 直接复用已归档的 Project/Episode/AssetVersion 契约；总体协调 change MUST NOT 成为运行时代码依赖。本 capability 的完整非目标是拥有或实现 WorkflowDraft/WorkflowVersion/WorkflowRun/NodeRun/RunEvent、Temporal、Timeline/TimelineDocument、音频/导出/媒体渲染、Provider 调用、AgentScope、文本或素材生成及前端交付；它 MUST NOT 原地修改已发布版本、以隐式“整集”替代显式 Command 范围，或把总体协调 change 与未实现的下游能力当作运行时依赖或 fallback。

#### Scenario:implement only the owned narrative slice
- **WHEN** 实施方为 Scene/Shot 添加领域、持久化或 HTTP 行为
- **THEN** 系统仅在本切片拥有的聚合和投影中写入，并以既有 Project/Episode/AssetVersion 事实作边界，不启动 WorkflowRun、不调用 Provider 或 AgentScope

#### Scenario:reject orchestration ownership leakage
- **WHEN** 实施尝试通过 workflow-scope 创建或启动 Run，或把总体协调文档当作运行时配置
- **THEN** 架构依赖/契约测试失败，且不产生场景、镜头、运行或外部副作用

#### Scenario:reject non-goal ownership leakage
- **WHEN** 本切片尝试拥有任一列明的非目标、原地修改已发布版本或以隐式“整集”扩大 Command 范围
- **THEN** 架构依赖/契约测试失败，且不写入 Scene、Shot、审计或 Outbox

### Requirement:Scene and Shot aggregate ownership
系统 SHALL 将 `Scene` 作为 Episode 内聚合根，将 `Shot` 作为唯一归属 Scene 的排序子实体。每个对象 MUST 保存稳定 UUID、`schema_version`、revision、状态和显式 project/episode 归属；编号在其父集合内 MUST 从 1 连续。

#### Scenario:apply accepted text batch to ordered scene and shot
- **WHEN** scenes owner 接收完整且已 accepted 的 `TextReviewBatch` handoff，其中逐项包含 handoff id/revision/correlation、Scene/Shot candidate id/hash、目标 project/episode/scene、各 aggregate 的 expected revision、payload hash 与 command schema version
- **THEN** Project/Episode/Scene/Shot 各 owner 仅通过其 typed batch/orchestration command 幂等落地自己的事实并返回匹配 owner ack；Scene/Shot owner 才在同一归属集合中分配连续编号、写入初始 revision、审计和 Outbox，且全部 owner ack 完整前不得解锁媒体

#### Scenario:reject incomplete or invalid batch ownership or revision
- **WHEN** handoff 不是 accepted 完整 batch，或任一 candidate/source hash、payload hash、target ownership、编号、归档父对象或 expected revision 无效、过期或不匹配
- **THEN** 系统返回稳定 validation/conflict 错误且不写入任何 Scene、Shot、审计、Outbox 或 owner ack，也不解锁媒体

### Requirement:Canonical schema version mapping
系统 SHALL 以数据库与共享 Schema 的 `schema_version` 作为 Scene/Shot 唯一版本事实。HTTP DTO 的 `schemaVersion` MUST 只映射同一个 canonical 值，且实现 MUST NOT 独立持久化或推导第二个版本事实。

#### Scenario:map the canonical version to HTTP
- **WHEN** API 序列化或反序列化有效 Scene/Shot DTO
- **THEN** `schemaVersion` 与 canonical `schema_version` 值相同，且持久化层只保存一个版本事实

#### Scenario:reject missing or conflicting version values without writes
- **WHEN** 请求缺少必需版本、同时提供冲突的 `schema_version` 与 `schemaVersion`，或实现尝试分别赋值
- **THEN** API 在 UoW 前返回稳定 validation error，且不写入 Scene、Shot、审计或 Outbox

### Requirement:Versioned narrative and asset references
系统 SHALL 让 Scene/Shot 引用不可变 `SpecVersion`、AssetBible owner 的 resolved snapshot 和 AssetVersion reference，并 MUST 保存被引用对象的稳定 ID 与版本/hash。Scene/Shot MUST NOT 拥有/复制 AssetBible entry、override、resolver 或 impact task，也 MUST NOT 保存媒体 bytes 或将可变草稿伪装为已固定引用。

#### Scenario:attach same-project immutable references
- **WHEN** 调用方提交同项目且存在的不可变 SpecVersion 与 AssetBible reference
- **THEN** 系统保存可审计引用并在 storyboard/workflow-scope 视图返回相同 ID、版本和 hash

#### Scenario:reject cross-project or mutable reference
- **WHEN** 引用不存在、跨项目、跨集或没有不可变版本标识
- **THEN** 系统在写入前拒绝请求并保留现有聚合版本

#### Scenario:resolve the complete AssetBible override chain
- **WHEN** Scene/Shot 提交 AssetBible owner 已按 project -> episode -> scene -> shot 解析的同项目 accepted resolved snapshot
- **THEN** 系统校验 snapshot ID/revision/hash/target 后固定到 ShotSpec；任何无效、stale、foreign 或 incomplete snapshot 拒绝且不改写下游引用

#### Scenario:连续性任务不自动改写镜头
- **WHEN** AssetBible owner 为某 Shot 创建 pending ContinuityRevisionTask
- **THEN** storyboard projection 显示 needs-revision 和 task reference，旧 ShotSpec/current media/Timeline 保持不变，只有显式 successor command/ack 才能更新

### Requirement:Explicit same-parent scene and shot reorder commands
系统 SHALL 在 MVP-A 仅支持 Scene 在同一 Episode 内重排、Shot 在同一 Scene 内重排。`ReorderScenes` 与 `ReorderShots` MUST 指定唯一父 scope、该父集合的完整成员 ID 顺序和 expected revision，且 MUST NOT 新增、删除或改变成员归属；成功时在一个 UoW 内提交顺序、revision、审计和 Outbox。

#### Scenario:reorder only within one explicit parent scope
- **WHEN** 调用方以 current expected revision 提交同一 Episode 的完整 Scene 顺序，或同一 Scene 的完整 Shot 顺序
- **THEN** 系统只更新该父集合的连续排序和 revision，成员稳定 ID 与归属不变，并在同一事务追加审计和 Outbox

#### Scenario:reject cross-scope or incomplete reorder
- **WHEN** Command 省略父 scope、包含 foreign/duplicate/missing member、试图改变归属或使用旧 revision
- **THEN** 系统返回稳定 validation/conflict，且不改变 Scene、Shot、审计或 Outbox

#### Scenario:reject deferred storyboard structural edits
- **WHEN** MVP-A 客户端请求 storyboard insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑
- **THEN** 对应 command/API/UI 不存在，或兼容入口返回 `unsupported_feature`，且不改变 Scene、Shot、审计、Outbox 或下游引用

### Requirement:Storyboard and workflow-scope projections
系统 SHALL 提供从同一版本化 Scene/Shot 事实读取的 storyboard 和 workflow-scope API。storyboard MUST 按 Episode/Scene/Shot 顺序提供阅读投影；workflow-scope MUST 返回显式可调度 IDs、revisions 和固定引用，且 MUST NOT 创建或启动 Workflow/Run。

#### Scenario:read two consistent views
- **WHEN** 客户端读取同一 Episode 的 storyboard 与 workflow-scope
- **THEN** 两个响应对共同 Scene/Shot 返回相同稳定 ID、revision、顺序和引用版本

#### Scenario:absent or foreign scope
- **WHEN** 客户端请求不存在或其他项目的 Episode/Scene scope
- **THEN** API 返回稳定 not-found/forbidden 错误，不泄露其他项目投影

### Requirement:Accepted storyboard asset eligibility facts
系统 SHALL 为 episode-scoped storyboard/reference 投影明确 accepted immutable AssetVersion 的 target、TextReview/媒体审核或 AssetEdit accept 来源及当前性事实；Timeline consumer MUST 不得仅从 `AssetVersion.status` 推断 eligibility。

#### Scenario:exclude unaccepted or foreign media
- **WHEN** AssetVersion 未经 accepted review、不是当前 storyboard/reference、属于其他项目或其他 Episode
- **THEN** projection 不将其作为 timeline media candidate，且不改写 AssetVersion

### Requirement:已接受 storyboard provenance
eligibility projection MUST 包含 `candidateId`、accepted provenance、immutable AssetVersion id/revision/hash 与 project/episode/target；只有精确 CAS acceptance 才可设置 current storyboard reference。

#### Scenario:Unaccepted or stale image is rejected
- **WHEN** candidate 未接受、foreign、stale，或 revision/hash 不匹配
- **THEN** projection 保持不变，且不创建 downstream reference。

### Requirement:Accepted video storyboard eligibility
系统 SHALL 由 scenes/storyboard owner 保存 `VideoTakeCandidate` 的 accepted/current eligibility，并以 candidate revision、accepted provenance、AssetVersion id/revision/hash、project/episode/shot target、ShotSpec revision/hash、duration/aspect snapshot 执行 exact CAS。reject、retake、cancelled-late、stale 或 foreign candidate MUST NOT 改变 current；derivative readiness MUST NOT 阻断 accept/current。Timeline 只能读取 accepted/current projection 且所有必需 derivative ready 的组合。

#### Scenario:accept one video take as current
- **WHEN** 用户提交未过期 video candidate 与全部 expected revisions 的 accept command
- **THEN** owner 在一个 UoW 中建立 current video reference 和 audit，其他 candidate/current references 不被隐式扩大或覆盖

#### Scenario:reject or stale video take
- **WHEN** candidate 被 reject/retake、source/ShotSpec/hash/revision 变化，或 derivative 未 ready
- **THEN** current projection 保持不变并返回可诊断 eligibility gate；不写 Timeline Clip、ExportJob 或 ProviderCall
