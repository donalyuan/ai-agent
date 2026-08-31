## Why

## AssetEdit accept boundary

verified Provider terminal success 是 result `AssetVersion` 的唯一 append 时点；retry/reconcile 返回同一 version/candidate。AssetEdit 仅接受 canonical `accept|reject|retake` review action，legacy/unknown `approve` validation 且零 current/retake mutation。`accept` 只追加 AcceptDecision/audit，并对同一 existing version 执行 scenes current eligibility CAS；不得复制 bytes/object/ref、不得追加第二 AssetVersion。reject、stale、foreign accept 不改 AssetVersion 数、current 或 Timeline。

当前系统只能保存不可变的 `AssetVersion`，不能把 Agent 提议的素材编辑与人工接受范围分离。这会使候选结果、版本影响和过期条件不可审计，且容易误覆盖原始版本。

## What Changes

- 新增 `AssetEditSession`、`AssetEditPlan`、`AssetEditCandidate`、`AcceptDecision` 与影响/stale 记录的领域和持久化契约。
- 将 `AssetEditPlan` 的可执行 target type 收窄为 `image|video`；story/script 由 TextReview successor/stale closure 拥有，audio/Timeline 由 Timeline editor typed commands 拥有。本 change 不为这些类型提供隐含 fallback。
- 将 MVP-A 的编辑输入粒度冻结为完整 image/video `AssetVersion`：base selection 与每个 explicit ref 都必须是同项目、可解析的完整版本。mask、图片选区、局部区域以及视频/音频时间范围编辑延后 MVP-B，并在创建 execution intent 或 `ProviderCall` 前返回 `unsupported_feature`，零业务写入和外部调用。
- 会话、Agent turn、Plan 与 execution 必须只读冻结 AssetBible owner 返回的 accepted `ResolvedContinuitySnapshot` ID/revision/hash；不得复制 AssetBible 内容或直接写 entry/override。snapshot incomplete/stale/foreign/hash mismatch 或目标存在 pending `ContinuityRevisionTask` 时，impact 显示 continuity stale，并在 Plan/execute/accept 的 owner gate 前阻断。
- 在 `AssetEditSession` 边界内新增 Agent conversation、message、turn、用户输入与 Agent 回复的追加事实，并提供从已完成 turn 生成 Schema-valid `AssetEditPlan` 的 owner command；对话生成仍先进入人工候选审核。
- 新增创建会话、提交 Schema-valid 计划、登记候选、查看影响、显式接受或拒绝候选的 API 契约。
- 明确后端 owner：本 change 拥有 `executeAssetEditPlan` 的 execution intent、状态、reconciliation、结果登记和 `AssetEditCandidate` reject；`integrate-gpt-image-provider`/`integrate-agnes-video-provider` 拥有各自 Provider adapter，`implement-provider-model-skill-catalog` 拥有 `ProviderCall`，`implement-scenes-shots-storyboard-slice` 拥有 current storyboard eligibility。`execute` 只在 commit 后通过 Outbox 交接 Provider，不在 Plan command 内直接调用外部服务。
- 图片生成成功先登记未引用 Candidate AssetVersion；本 change 提供 candidate compare/read/reject/accept command 的审查事实，UI 必须展示基础/结果版本、provenance、费用、影响和 stale 状态，只有 Scenes owner 的精确 CAS 才能更新 current storyboard reference。
- 会话绑定一个 primary selection 和显式 refs；切换选择不得泄漏旧上下文。接受操作仅为以 stable target/reference IDs、expected revisions 表达的精确引用集合建立新的引用/版本投影，并以 CAS 在同一事务全有或全无；候选永不替换原 `AssetVersion`。
- 基础版本过期返回 `409`；旧版本、已发布版本和历史运行只读。
- 统一版本字段：数据库与共享 Schema 仅持久化 canonical `schema_version`，HTTP DTO 的 `schemaVersion` 只映射同一个值；缺失、冲突或双独立赋值均拒绝且不得写入编辑审查事实。

## Capabilities

### New Capabilities

- `agent-asset-edit-review`: 可审计的 Agent 素材编辑提议、候选审查与人工范围接受。

### Modified Capabilities

- 无。既有 `assets-slice` 的 append-only `AssetVersion` 契约保持不变。

## Impact

预期影响 `services/api` 的 asset-edit domain/application/repository/interface 分层、`packages/contracts` JSON Schema、Alembic、Outbox 与 API/BDD 测试。完整非目标是实现或拥有 Provider Adapter、提示词生成、图片/视频编辑、AssetBible entry/override/resolver、自动接受、跨项目复制、历史/已发布版本重写，改变 AssetVersion 的 objectKey/hash/append-only/现有 HTTP 契约，以及实现 Timeline、音频、FFmpeg、MP4/SRT、`ProjectPackage`、`exportProfile` 或 `portable` payload；Provider 只消费本 change 的 execution handoff 并回传 verified result handoff。

## 总体计划追溯与边界

- 本 change 反向追溯到 `plan-phase-one-drama-mvp-a`：总体任务 `1.2` 要求保留该追溯，直接实施任务为 `3.4`；共享版本、UoW/Outbox、`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）与验收规则还须满足总体任务 `5.1`--`5.5`。
- 实施前置为总体任务 `2.1` 的 scenes/shots、`2.1a` 的 AssetBible continuity、`2.2` 的 workflows/runs、`2.3` 的 provider/model/skill catalog，以及已归档的 AssetVersion 契约；它们提供 target resolver、accepted continuity snapshot、运行审计和不可变素材事实。`plan-phase-one-drama-mvp-a` 只协调 change 顺序和验收，既不是运行时代码依赖，也不授权本 change 导入其他 change 的实体或副作用。
- 完整非目标是实现或拥有 Provider Adapter、提示词生成、图片/视频编辑、自动接受、跨项目复制、历史/已发布版本重写，改变 AssetVersion 的 objectKey/hash/append-only/现有 HTTP 契约，以及实现 Timeline、音频、FFmpeg、MP4/SRT、`ProjectPackage`、`exportProfile` 或 `portable` payload；这些职责只属于既有 AssetVersion 所有者、后续 Provider 或总体任务 `4.1`--`4.3` 的 timeline/export change 和 MVP-B。

## DDD / BDD / SDD / TDD

- **DDD**：Session 是编辑意图边界；Plan、Candidate 与 AcceptDecision 是不可篡改审计事实；AssetVersion 仍由既有资产边界拥有。
- **DDD 补充**：`AssetEditPlan` 只能执行 image/video；`runId + nodeRunId + logicalOperation` 在 execute 阶段必须完整，缺少 typed owner contract 的其他类型请求稳定拒绝。
- **BDD**：用户能够审查候选和影响后，仅接受明确选中的范围；过期、隐式范围、局部 mask/选区/时间范围和只读来源均可观察地失败。
- **SDD**：API、Schema、数据库和错误语义均为新增且 additive；不改变既有资产路径和对象键契约。
- **TDD**：先覆盖领域不变量和应用命令，再覆盖 SQLAlchemy/Alembic、HTTP/contract 与端到端 BDD 负例。
- **TDD 补充**：先覆盖 execute/reconcile/reject 的状态机和 `runId + nodeRunId + logicalOperation` 幂等，再覆盖 Provider result -> Candidate 登记、candidate compare、精确 CAS accept/reject 与 UI 零副作用。

## Current / Defined / Todo

- **Current**：没有 `AssetEditPlan`、候选编辑、影响分析、接受决定或 stale 代码、Schema、表和 API。
- **Defined**：本 change 的计划、候选、范围接受、409 与只读契约。
- **Todo**：实现、迁移、受控副作用、测试及兼容性验证。

## Timeline handoff 边界

AssetEdit accept 仅追加 AcceptDecision/audit，并以同一既有 AssetVersion 做 scenes accepted/current storyboard-reference eligibility exact CAS；不创建、删除或覆盖 Timeline Clip/SoundCue，不追加 AssetVersion，也不越过 project/Episode scope。

## Candidate provenance 一致性

**DDD**：AssetEdit 和 image accept 均交付同一 accepted/current eligibility shape。**BDD**：candidate/provenance/hash/revision/target mismatch 拒绝。**SDD**：handoff 包含 accepted provenance、candidateId、AssetVersion id/revision/hash、project/episode/target，精确 CAS。**TDD**：验证 current 不被 status-only 或 stale/foreign 输入覆盖。
