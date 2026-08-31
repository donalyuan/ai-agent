## Context

## 单一 append 与 accept CAS

Provider terminal success 是结果 AssetVersion 的唯一 append point，retry/reconcile 必须返回同一 version/candidate。review Schema/API/UI/audit action 仅 `accept|reject|retake`；legacy/unknown `approve` validation 且零 current/retake side effect。AssetEdit `accept` 仅 append AcceptDecision/audit，再以 candidate/provenance/version/hash/target exact CAS 写同一 version 的 scenes current eligibility；不得复制 bytes/object/ref 或 append 第二 AssetVersion。reject/stale/foreign accept 零 AssetVersion/current/Timeline mutation。

阶段 0 已提供项目归属的 `Asset`、只追加 `AssetVersion`、revision、UoW、StoragePort 和 HTTP 错误边界。当前没有编辑计划或候选；本 change 只能在这些不可变版本之上增加审查工作流。所有写 Command 在一个 UoW 中写领域事实与 Outbox；任何 Provider 或媒体副作用在 commit 后且不属于本 change 的 Adapter。

## Goals / Non-Goals

**Goals:**

- 以稳定 UUID、canonical `schema_version`、revision 和状态审计持久化 `AssetEditSession`、Schema-valid `AssetEditPlan`、候选及接受决定；HTTP DTO 的 `schemaVersion` 只映射同一个版本值。
- `AssetEditSession` 同时拥有 conversation、message 与 turn 的追加事实；用户输入和 Agent 回复按 session/scope、turn、sequence、correlation 绑定。从已完成 Agent turn 生成 Plan 是显式 application command，生成的 Plan 状态为 `pending_review`，不得直接 execute/accept。
- 对计划的输入/输出 AssetVersion、目标镜头/场/集、变更类型和引用影响做结构化校验；每一个候选保留基础版本与内容引用。
- 冻结 MVP-A 编辑粒度为完整 image/video `AssetVersion`；primary selection、base 和 explicit refs 都是完整版本引用，不接受 mask、选区、局部区域或任意媒体时间范围。
- 为 Session、conversation turn、Plan 与 execution 冻结 AssetBible owner 返回的 accepted `ResolvedContinuitySnapshot` ID/revision/hash，只保存 owner references，不复制或修改 entry/override/resolver facts。
- 让接受命令带 `expectedBaseVersionId`、显式 `scope` 和人工确认；接受只追加 AcceptDecision/audit 与同一既有 version 的 scenes eligibility exact CAS，绝不修改、复制或追加 AssetVersion。
- 将计划或来源版本变更后产生的 stale/impact 暴露给读取接口；基础版本不匹配统一映射为 `409`。

**Non-Goals:**

- 不实现或拥有 Provider Adapter、提示词生成、图片/视频编辑、自动接受、跨项目复制或历史/已发布版本重写。
- 不改变 `AssetVersion` 的 objectKey、hash、append-only 或现有 HTTP 契约。
- 不实现 Timeline、音频、FFmpeg、MP4/SRT、`ProjectPackage`、`exportProfile` 或 `portable` payload；它们不是编辑审查聚合的模型或运行时职责。
- 不实现图片 mask/选区/局部区域编辑，也不实现视频或音频时间范围编辑；这些能力延后 MVP-B，不能被 Provider capability 或客户端字段提前开启。

### 0. 总体计划追溯与实施前置

本设计反向追溯至 `plan-phase-one-drama-mvp-a` 的总体任务 `1.2` 和直接实施任务 `3.4`，并执行共享任务 `5.1`--`5.5` 的 UoW/Outbox、版本、`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）与验证规则。实施必须以总体任务 `2.1`（scenes/shots）、`2.1a`（AssetBible continuity）、`2.2`（workflows/runs）、`2.3`（provider/model/skill catalog）完成且已归档的 AssetVersion 契约可供取证为前提；它们只提供本 change 所需的稳定 target resolver、accepted continuity snapshot/task projection、运行审计和素材版本事实。

`plan-phase-one-drama-mvp-a` 是 OpenSpec 协调和验收关系，不是运行时代码依赖。asset-edit 的 application/domain 不得为满足追溯而直接依赖总体 plan、Timeline/export 或其他 child change 的代码；缺失前置事实时实施应显式阻塞，而非伪造对象或 fallback。

## Decisions

### 1. Session owns review facts; Asset owns versions

新增表以 `project_id`、`session_id`、`plan_id`、`candidate_id`、`decision_id` 和审计时间建立外键归属。Plan payload 使用 Draft 2020-12 JSON Schema 校验并保存 `schema_version`/version/hash；Candidate 仅引用基础/结果 `AssetVersion` 与 Session。数据库列和共享 Schema 的 `schema_version` 是唯一事实源，HTTP DTO `schemaVersion` 只做同值命名映射；任一侧缺失、值冲突或试图分别赋值时必须 validation 失败，且不得写 session、plan、candidate、decision、impact 或 Outbox。替代的“在 AssetVersion 上附加可变 edit 字段”会破坏既有不可变事实，故拒绝。

### 2. Explicit scope is a closed set

`AcceptDecision` 必须持久化调用者选择的 `shotIds`、`sceneIds`、`episodeIds` 或显式勾选目标，以及每个稳定 target/reference ID 的 expected revision、candidate/revision。该集合是精确 CAS：所有 target 在一个 UoW 中同时满足预期才建立全部引用，否则一个也不写。空集合、混合其他项目/剧集、自动推导的扩大范围和与计划影响不相交的范围均在 application 校验前失败。替代的“接受整个计划”不能满足人工范围控制，故拒绝。AcceptDecision 与仍被其引用的 AssetVersion 属于长期 no-GC 事实；诊断窗口到期、temporary/derivative cleanup、容量维护、恢复或 GC 均不得删除、覆盖或静默压缩 decision，亦不得清理被引用版本。

### 3. Staleness is derived and retained

接受和读取都比较 Candidate 的基础版本、Plan revision 与当前引用 revision；不一致时写可诊断 `stale` 影响记录，并拒绝接受为 `409 base_version_conflict`。旧版本、已发布版本或历史运行上下文由状态守卫拒绝写入。替代的静默重基会隐藏人工审查对象变化，故拒绝。

### 4. API and dependency boundary

接口采用 `/v1/projects/{projectId}/asset-edit-sessions` 下的 additive REST 资源；请求使用共享 contracts Schema，响应不含媒体 bytes。application 只依赖 Repository/UoW/Outbox port；SQLAlchemy、FastAPI、Storage 和后续 Provider 放在 adapter/interface。失败保持既有稳定错误 envelope，并新增 validation、not_found、forbidden_read_only、base_version_conflict 与 stale 的可诊断代码。

### 5. 会话选择隔离

每个 `AssetEditSession` 保存一个 primary selection（project/episode/scene/shot/node/asset/version 之一）和显式 refs；Agent 输入只能由这两个集合组成。切换 primary selection 时必须清空或重新验证旧 refs，不能从 UI “当前项”隐式保留任何上一会话上下文。引用不属于当前 project 或与 primary selection 不兼容时，在调用任何 Provider 前失败。

### 6. Execute owner and Provider handoff

`AssetEdit` 是 image/video `AssetEditPlan` execute 的后端 owner。`executeAssetEditPlan` 只接受 `pending_review`、未过期且已通过费用/能力确认的 Plan，并冻结 `planRevision`、base version、selection/refs、`runId`、`nodeRunId`、`logicalOperation`、`correlationId`、model/provider/capability snapshot 和 request fingerprint。Command 在同一 UoW 中追加不可变 `AssetEditExecution` intent 与 Outbox，commit 后才由对应 Provider child 的 worker 消费；它不得在事务内直接调用 Provider，也不得拥有 `ProviderCall` 或 `RunEvent`。story/script、audio、TimelineVersion 请求必须转交各自 owner，不能由本 change 猜测或执行。

Execution 状态固定为 `queued|running|waiting_reconciliation|succeeded|failed|submission_unknown|cancel_requested|cancelled`。同一 execution 重试、Worker 重启或 `submission_unknown` 必须先按 execution id、`runId + logicalOperation` 和 Provider request id reconciliation；不能确认时保持 unknown，不创建第二个收费请求。Provider 成功只回传 verified immutable `AssetVersion` result handoff（含 provider/call provenance、content hash、revision、project/episode/target），由 AssetEdit application 幂等登记一个 `AssetEditCandidate`；AssetEdit 不把 Provider result 直接写成 current。

`AssetEditCandidate` 状态固定为 `generated|pending_review|rejected|accepted|stale|superseded`。reject 只追加 `AcceptDecision`/reject audit，不删除 AssetVersion；accept 仍是精确 candidate/base/target CAS。图片 candidate 与 AssetEdit candidate 使用同一个 `candidateId` namespace 和 provenance shape；GPT Image 可直接登记为 AssetEdit candidate，Scenes owner 另以相同 exact fields 建立 current eligibility，禁止把 status-only AssetVersion 当作接受证据。

### 7. MVP-A 只接受完整 AssetVersion

`AssetEditPlan` 的 primary selection、base 和每个 explicit ref 必须解析为同项目的完整 image/video `AssetVersion`，并冻结 version ID、revision、content hash 与 owner scope。MVP-A Schema/API 不表达 mask、bounding region、selection path、局部图层、start/end time、time range 或 segment edit；即使某 Provider capability 支持这些参数，也不得透传或暗中降级为整版编辑。任何此类输入必须在 Plan validation 或 execute application guard 中返回 `unsupported_feature`，且发生在 `AssetEditExecution` intent、Outbox、`ProviderCall`、Storage operation 或付费提交之前。

### 8. Agent context 只读冻结 AssetBible snapshot

创建 Session 或 conversation turn 时，application 必须从 AssetBible owner 读取当前 target 的 accepted `ResolvedContinuitySnapshot`，并在 Session、turn、Plan 与 execution 中保存同一 snapshot ID/revision/hash 及必要 GenerationSpec/AssetVersion owner refs。Agent input 可读取 owner 提供的投影，但不得把 entry/override 正文复制为本聚合事实，也不得提交任何 AssetBible mutation。

生成 Plan、execute 和 candidate accept 前必须重新核对 snapshot 与 `ContinuityRevisionTask` projection。snapshot incomplete、foreign、hash/revision mismatch，或 target 存在 pending task 时，Plan/impact 标记 `continuity_stale` 并返回稳定 conflict；不创建新的 Agent/Provider request、execution、Outbox、ProviderCall、Candidate 或 scenes current mutation。用户必须先经 AssetBible/Scene/Shot owner 的显式 successor/ack 解决任务，再重新生成 Plan；客户端不得用缓存 snapshot 自动重基。

## Data and API Contract

数据库迁移新增 session、conversation、message、turn、plan、execution、candidate、decision、impact/stale 表及项目/资产版本/目标引用外键、每 conversation 单调 sequence、唯一候选序号、canonical `schema_version` 和索引；迁移不回填或改写既有 AssetVersion。公开 Schema 至少定义 Conversation、Message、Turn、`AssetEditPlan`、`AssetEditExecution`、`AssetEditCandidate`、`AcceptDecision`、`EditImpact`、AssetBible snapshot/task reference 与 Provider result handoff，使用 `schema_version` 并禁止未定义字段。API 包含创建/读取 session/conversation、追加用户消息、读取 Agent turn/reply、从 conversation turn 生成 image/video Plan、提交 plan、execute/status/reconcile、登记/列出/比较 candidate、读取 impact、accept/reject；HTTP DTO 的 `schemaVersion` 必须映射同一 `schema_version`，execute 必须包含 `runId + nodeRunId + logicalOperation`，接受请求必须包含 `expectedBaseVersionId`、accepted AssetBible snapshot identity 与明确范围；其他类型以及 mask/选区/局部区域/时间范围请求返回 `unsupported_feature` 且零写入、零 Outbox、零 ProviderCall。

## Risks / Trade-offs

- [Scene/Shot 目标切片尚未实现] -> Repository port 先表达稳定 ID 和项目/剧集归属校验；落地时由依赖 change 提供实际 resolver，不伪造目标记录。
- [Schema 与 ORM 映射可能漂移] -> contracts、domain、adapter 共用 fixture 与正反 contract tests。
- [Outbox 目标未实现] -> 保留 port/事务边界和显式未配置失败，不引入 fallback Provider。
- [并发接受] -> 数据库唯一约束和 expected revision/base-version compare-and-set，冲突返回 409。

## Migration Plan

1. 添加 JSON Schema、domain/application 和测试。
2. 添加可逆 Alembic 表、索引、外键与 SQLAlchemy Repository；升级前仅创建新表，不迁移既有资产。
3. 添加 HTTP 接口、Outbox 记录和 BDD；先使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。
4. 回滚只删除未被部署数据依赖的新表；生产回滚策略和历史数据保留窗口待实施时确认。

## 待实现取证

- 最终 HTTP path 与统一错误 envelope 的 additive 兼容策略。
- Scene/Shot/发布状态的权威 resolver、影响图深度和 stale 传播展示。
- Provider result 只通过 execution result handoff 进入 AssetEdit candidate；是否由 image/Agnes 创建新 AssetVersion 由各 Provider child 的 verified storage flow 决定。无论来源，接受必须遵守 frozen exact-set CAS，且任何晚到取消结果保持未引用 candidate。

## DDD / BDD / SDD / TDD

- **DDD**：Session/Plan/Candidate/Decision 的状态流转和不变量位于 domain；范围解析与 UoW 位于 application。
- **BDD**：计划无效、候选不覆盖原版、显式接受、基础版本过期、已发布/历史只读均为验收场景。
- **SDD**：定义 JSON Schema、REST、表/索引/外键、端口、Outbox、409 失败及不兼容项。
- **TDD**：先红灯定义 validation/transition/compare-and-set，再完成 adapter、HTTP、migration 和 BDD。
- **TDD 补充**：execution retry/unknown、Provider result 幂等 Candidate、image candidate compare/reject、candidate 类型混用和 accept 前零 current/Timeline 写入必须有失败测试。

## Current / Defined / Todo

- **Current**：仅 Asset/AssetVersion 事实可用；编辑审查能力不存在。
- **Defined**：本设计冻结 conversation/message/turn 到 Plan、候选审查、精确接受的边界和失败语义，不冻结尚未证实的 Scene/Shot resolver 或 Provider 行为。
- **Todo**：在依赖切片就绪后实现和验证全部任务。

## Accepted AssetEdit 交接

**DDD**：Edit decision/audit 与 Assets append 分离，Timeline 只消费 handoff。**BDD**：accept 不创建 Timeline reference，foreign/stale 仍显式失败。**SDD**：payload 包含 immutable version、project/Episode target、accept/current provenance；保持 AssetVersion append-only 兼容。**TDD**：先覆盖 handoff/零 Timeline 写入，随后 adapter/HTTP/BDD；非目标是 preview/export 或 Storage upload。验收使用既有 strict、定向与 diff 命令。

handoff 与 scenes eligibility 使用相同 exact CAS 字段：accepted provenance、candidateId、AssetVersion id/revision/hash、project/episode/target；hash/revision/status-only/stale/foreign 不能更新 current。
