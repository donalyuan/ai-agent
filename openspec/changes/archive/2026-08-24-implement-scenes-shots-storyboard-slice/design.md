## Context

## current eligibility

Provider terminal result candidate 必须先经人工 `accept`，scenes 才可对 candidate/provenance/version/hash/target 执行一次 exact current CAS；然后才是 MediaInspect/derivatives 和 Timeline handoff。derivative `pending|failed|stale` 只阻断 Timeline/preview/export，不回滚 accepted/current。review Schema、HTTP DTO、UI action、audit event 统一 `accept|reject|retake`；legacy/unknown `approve` validation 且零 current/retake side effect。

AssetEdit accept 的输入是已存在 version：只追加 AcceptDecision/audit 与该同一 version 的 scenes current eligibility CAS，不复制 bytes/object/ref、不 append 第二 AssetVersion。reject/stale/foreign accept 零 AssetVersion 数、current、Timeline mutation。

当前 `scene.schema.json`/`shot.schema.json` 与 SQLAlchemy 模型只保存 episode/scene 和 display number；没有标题之外的叙事版本、仓储、应用服务、HTTP 端点或排序约束。`Project -> Episode -> Scene -> Shot` 是已定义聚合层次；工作流和故事板是同一版本化事实的投影，而不是相互拥有的数据。

## 与总体计划的实施追溯

本设计直接落实 `plan-phase-one-drama-mvp-a` 的任务 **2.1**，并执行其共享任务 **5.1**（UoW/Outbox）、**5.3**（版本与归属拒绝）、**5.7**（逐 change OpenSpec/status/strict）和 **5.8**（全量质量门）。其实施输入仅为已归档的 Project/Episode/AssetVersion 契约和目标分层架构；总体 change 只协调多个 change 的顺序，绝不被导入或调用为运行时代码。Workflow、catalog 和 AgentScope 的后续实现可消费已发布的 Scene/Shot 事实，但不属于本切片的写入或测试范围。

## Goals / Non-Goals

**Goals:**

- 以 Scene 为聚合根，拥有 Shot 排序、归属、连续性引用和结构化 SpecVersion/AssetBible reference。
- 只对完整 accepted `TextReviewBatch` handoff 落地 Scene/Shot，并对同一明确父 scope 内排序实施显式 expected revision、项目/集归属校验、稳定审计与 Outbox；handoff 的 candidate/source hashes、payload hash、correlation 和 owner ack 保持可审计。
- 提供针对同一版本事实的 storyboard 和 workflow-scope 两种只读 API 投影。
- 规划 additive Schema/ORM/Alembic 迁移，保持现有 `display_number` 的兼容别名和既有数据可回填。

**Non-Goals:**

- 不拥有或实现 WorkflowDraft/WorkflowVersion/WorkflowRun/NodeRun/RunEvent、Temporal、Timeline/TimelineDocument、音频/导出/媒体渲染、Provider 调用、AgentScope、文本或素材生成及前端交付。
- 不实现 storyboard insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑；这些结构编辑属于 MVP-B。
- 不原地修改已发布版本，不以隐式“整集”范围替代 Command 的显式目标，也不把总体协调 change 或未实现的下游能力当作运行时依赖或 fallback。

## Decisions

### 聚合、版本与引用

Scene 是 Episode 内的聚合根，Shot 只经所属 Scene 改写。Scene/Shot 均带稳定 ID 与 revision；Scene 排序只更新同一 Episode 的 Scene 集合，Shot 排序只更新同一 Scene 的 Shot 集合，并将旧顺序写入审计。StorySpec 是项目级、ScriptSpec 是每集级，SceneSpec/ShotSpec 保存不可变版本；每个最小 Spec 均包含 `id`、`schema_version`、`version`、所属稳定 ID、排序/编号、内容字段和上游不可变引用。Story 包含 logline/characters/conflict/beats/continuity，Script 包含 episode goal/conflict/scene order，Scene 包含地点/时间/角色/道具/目标/情绪/对白/shot order，Shot 包含 durationFrames/framing/camera/action/dialogue/first-last-frame/audio/continuity。`SpecVersion` 和 AssetBible 只保存已存在、同项目、不可变版本的 ID/hash/reference，不复制内容或媒体 bytes。选择引用而非内嵌文档，以维持 AssetVersion 的只追加边界及下游可追溯性。

### 编辑语义

编号在 Episode 的 Scene 集合和 Scene 的 Shot 集合中从 1 连续排序。Scene/Shot 的首次落地只消费完整 accepted `TextReviewBatch` handoff；Project/Episode/Scene/Shot owner 各自以 typed batch/orchestration command 校验 handoff id/revision/correlation、candidate/source hashes、payload hash、target ownership 和 expected revision，并以幂等 owner ack 收口，任何缺失/失败 ack 均继续关闭媒体门。MVP-A 的 `ReorderScenes` 必须指定一个 Episode、完整 Scene ID 顺序和该父 scope 的 expected revision；`ReorderShots` 必须指定一个 Scene、完整 Shot ID 顺序和该父 scope 的 expected revision。两个命令均不得新增、删除或改变成员归属。storyboard insert/copy、Scene split/merge、Shot 跨场 move 和批量编辑不暴露 command/API/UI；若旧客户端提交这些请求，返回稳定 `unsupported_feature` 且不写 Scene、Shot、审计或 Outbox。AssetBible 覆盖按 project -> episode -> scene -> shot 解析，resolved reference chain 固定到 ShotSpec；下游只标记 stale，不自动替换。重复/缺失 ID、跨项目/跨集/跨父 scope、旧 revision 和发布/归档对象均返回稳定 validation/conflict，不静默重排。

### AssetBible owner handoff

AssetBible owner 按 project -> episode -> scene -> shot 解析覆盖；Scene/Shot 只校验并把 immutable resolved snapshot ID/revision/hash 固定到 ShotSpec，不在自身 repository 复制 entry/assignment/chain。ContinuityRevisionTask 只使相关 projection 可见 stale/needs-revision，不能自动替换 ShotSpec、current media 或 Timeline。原“Scene/Shot 解析 AssetBible”的表述统一解释为调用 owner resolver 并消费结果，不形成第二事实源。

### 持久化与 API

新增 scene/shot title、排序、SpecVersion/AssetBible references、audit/outbox 表/列及唯一/外键/检查约束；迁移先回填可从现有 `display_number` 推导的数据，再启用约束。数据库与共享 Schema 的 `schema_version` 是唯一版本事实；HTTP 采用 `/v1/projects/{projectId}/episodes/{episodeId}/scenes` 及嵌套 shots，DTO 的 `schemaVersion` 只映射同一个 canonical 值，其余字段使用 camelCase。缺失版本、`schema_version`/`schemaVersion` 冲突或双独立赋值必须在 UoW 前返回稳定 validation error，不写 Scene、Shot、审计或 Outbox。`GET storyboard` 返回 Scene/Shot 阅读顺序；`GET workflow-scope` 返回相同 IDs/revisions/reference 的可调度范围，不创建 run。

### DDD/BDD/SDD/TDD

DDD 规则位于 framework-free domain；application 仅依赖 Repository/UoW/Outbox ports；adapter 映射在边界层；interfaces 不访问 Session。BDD 覆盖用户可见的同父 scope 排序、MVP-B 结构编辑拒绝和双视图结果。SDD 锁定 Schema、DB、API、兼容性和错误。TDD 先写 domain/application/adapter/HTTP/contract/migration 的失败测试，再实现最小路径。

## Risks / Trade-offs

- [排序请求混入其他父 scope 成员] -> 在 UoW 前校验完整成员集合、父 scope 和 expected revision，失败时零写入。
- [旧 `display_number` 数据无连续性] -> 迁移显式排序回填；无法证明归属时失败并保留原始数据。
- [引用版本被后续变更误用] -> 只允许不可变版本 ID/hash，发布后不允许原地替换。
- [双视图漂移] -> 两个投影来自同一 query/read model，并以契约测试比较稳定 ID/revision/order。

## Migration Plan

1. 先增加合同与失败测试，定义 Scene/Shot、SpecVersion/AssetBible reference、排序与错误码。
2. 新建 additive Alembic revision，回填既有 `display_number`，再增加 project/episode 归属、唯一编号、引用和 revision 约束。
3. 部署 Repository/UoW、Commands/Queries 和 HTTP；先用 `Mock Provider +` 显式 Local test/offline profile 与测试数据库验证。
4. 回滚只撤回新端点和新列/表；已发布或审计事实不重写。迁移 downgrade 必须经过 SQLite/PostgreSQL cycle 验证。

## 待实现取证

- storyboard 与 workflow-scope 的最终 HTTP path/统一错误 envelope 仍需按 additive contract 验证；它不改变已冻结的字段、覆盖或编辑语义。
- MVP-B 结构编辑请求必须由合同测试证明不暴露 command/API/UI；兼容入口只能返回 `unsupported_feature`，不得改变 Scene/Shot 或下游引用。

## DDD / BDD / SDD / TDD

- **DDD**：Scene 拥有稳定 Shot、排序、版本和 AssetBible 覆盖解析。
- **BDD**：覆盖 accepted handoff 创建、同父 scope 排序、MVP-B 结构编辑拒绝、双视图与 stale 不自动替换。
- **SDD**：固定 Spec、Schema、API、迁移、归属和不可变引用边界。
- **TDD**：先写领域/应用失败场景，再验证 adapter、HTTP、migration 与 BDD。

## Current / Defined / Todo

- **Current**：只有 Scene/Shot Schema/ORM 占位和已归档的 Project/Episode/AssetVersion 事实。
- **Defined**：稳定 ID、Story/Script/Scene/Shot 最小结构、AssetBible 覆盖和编辑语义。
- **Todo**：完成此 change 的未勾选任务和 migration/API 验证。

## Accepted media eligibility

**DDD**：Scene/Shot projection 只表述 target/reference/current 与 acceptance provenance。**BDD**：foreign、other Episode、unaccepted 或非-current 版本不可成为候选。**SDD**：Timeline consumer 读 projection 事实而非猜测 status；既有 AssetVersion append-only 保持兼容。**TDD**：先写 owner projection 的正反 contract，非目标是 Clip/SoundCue command、UI 或媒体渲染；执行本 change 已列 strict/迁移/API 验收。

Image candidate acceptance 的 CAS 输入必须同时含 candidateId、AssetVersion id/revision/hash、project/episode/target 与 accepted provenance；任何字段不匹配均拒绝且不改变 current projection。

Video candidate acceptance 复用同一 exact fields，并额外绑定 ShotSpec revision/hash、duration/aspect snapshot 与 `VideoTakeCandidate` revision。Scenes owner 提供 current-video eligibility read/accept/reject projection；Provider、workflows 或 Timeline 不得直接写该 projection。reject/retake/late-result 只保留 candidate audit，不能覆盖既有 current。
