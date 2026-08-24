## 1. OpenSpec 与状态合同

- [x] 1.0 建立对 `plan-phase-one-drama-mvp-a` 总体任务 `2.2` 及共享任务 `5.1`、`5.2`、`5.3`、`5.5` 的追溯测试；核验直接实施依赖仅为阶段 0 模块化单体、Ports/Adapters、Outbox/Temporal/Worker 边界及既有 Schema/ORM 占位。完整非目标是拥有 Provider/Profile/Model 配置或 ProviderCall 账本、真实 Provider SDK/adapter/模型调用、文本 AgentScope 业务、图片/视频/音频生成、FFmpeg/媒体渲染、Timeline 与前端，以及以 Temporal 内部表、进程内事件、SSE 连接或总体协调 change 作为业务事实源、复制 RunEvent 历史；证明 RunEvent 仅由 workflows/runs 拥有。
- [x] 1.1 以 DDD/BDD/SDD/TDD 固定唯一 `drama-mvp-a-default` published source、scope、通用 Draft/graph/publish mutation 的 `unsupported`/零写入语义、WorkflowRun/NodeRun 的完整状态集合、终态、所有合法转移、`waiting_review` 聚合进入/恢复/重算规则、取消、审核 Signal、RunEvent、SSE、Temporal 边界及 canonical `schema_version` 到 HTTP `schemaVersion` 的同值映射和冲突无写入语义。
- [x] 1.2 增加 contracts/architecture 失败测试，确认 interfaces -> application -> domain 依赖和 Workflow 不拥有 Provider/媒体/AgentScope。

## 2. 领域、应用与持久化

- [x] 2.1 先写唯一固定 published Version/binding 的 ensure/bootstrap、scope、不可变 hash、legacy Draft 只读兼容与通用 Draft/graph/publish mutation `unsupported`/零写入测试，再实现 entities、errors、Commands/Queries、Repository/UoW/Outbox ports。
- [x] 2.2 先写 WorkflowRun/NodeRun 合法与非法转移、终态重启、`cancel_requested` 竞态、结构化文本节点连续生成且只在完整 TextReviewBatch 进入一次审核、batch accept 后 handoff candidate/source hashes/payload hash/expected revisions 与全 owner idempotent ack 才原子恢复 Run+NodeRun、成功后从 running 聚合为 succeeded/再次 waiting、reject 原子失败、legacy `approve`/未知/重复 Signal 零领域/审计/幂等/Outbox/current/retake 写入、workflow node > project > enabled system 的冻结 selection snapshot、batch 接受前付费媒体暂停、`submission_unknown` reconciliation、幂等和重启恢复的 domain/application/BDD 失败测试，再实现运行 application 服务。
- [x] 2.2b 先写 failed predecessor 终态不可变、successor lineage、精确 succeeded-node reuse evidence、新 selection snapshot、新 logical operation、重复收费拒绝和 `submission_unknown` 先 reconciliation 测试，再实现 `CreateSuccessorRunFromFailure`；任何 stale/foreign/mismatch 必须零 successor/Outbox/Temporal/ProviderCall。
- [x] 2.2d 先写历史 `RunInputSnapshot` 精确选择、`rerunOfRunId` lineage、source 不可变、全部节点新 logical operation/新费用确认、无 failed-successor reuse、无 default-current/implicit upgrade/fallback，以及 missing/foreign/stale/unrunnable/`submission_unknown` 零副作用测试，再实现 `CreateRunFromHistoricalSnapshot`。
- [x] 2.2c 增加 SkillRouteDecision/Selection start gate：唯一选择或人工 selection 才冻结 decision/selection revision、SkillRevision ID/digest；pending/stale/disabled/unapproved/non-candidate/settings-only 状态零 Run/NodeRun/Temporal/TextModel/Provider，不默认首项。
- [x] 2.2a 先写 BudgetGate/费用确认失败测试，再实现图片/视频批量提交前确认、文本项目阈值超限进入 `waiting_review`、`cost=unknown` 明确确认、确认绑定同一 `run_id + logical_operation`、稳定本地 `user_uuid` 和 `retention_policy/version/hold`；覆盖重复提交、重试、刷新/Worker 恢复与 `submission_unknown` reconciliation 不重复收费。
- [x] 2.3 先写 SQLAlchemy 并发、每 Run 单调 event sequence、ProviderCall 仅关联不复制事件、Agnes event normalization、idempotency、事务和长期 RunEvent no-GC 测试，再实现 Repository/UoW、审计/Outbox 映射；超过诊断窗口、不同 hold、cleanup/容量维护/恢复/GC 后事件仍可读取、append-only 且 SSE replay 不变。
- [x] 2.4 新增并验证可逆 Alembic migration，补齐 workflow project/scope/hash 约束及 run/node/event/idempotency/outbox 表。

## 3. 接口与 Temporal

- [x] 3.1 先写 HTTP/BDD/contract 失败测试，再实现固定 default ensure/start/cancel/signal/read/SSE API 与稳定错误映射；通用 draft/edit/publish/node/edge/connection API 必须返回 `unsupported` 且零写入；缺失、冲突或双独立赋值 `schema_version`/`schemaVersion` 时在 UoW 前失败且不写领域、审计或 Outbox。
- [x] 3.1a 定义并实现 secret-free Run/NodeRun detail query/DTO：stable ID/revision/status、WorkflowVersion refs、来自冻结 node scope 的 project/episode/scene/shot stable IDs/revisions `scopeRefs`、owner timestamps/elapsed source、最近 RunEvent、failure code/message/retryability、allowed actions，以及只含安全标量/数量/状态/owner ID/revision/hash/reference 的 input/output summary；覆盖前端不得按标题/顺序推断 scope、foreign scope、plaintext/full text/media bytes/objectKey/persistent URL/raw Provider payload 拒绝、partial diagnostic、cancel allowed-state 与取消晚到结果不覆盖 owner 状态，证明 query 零 mutation。
- [x] 3.1b 定义并实现 project-scoped historical RunInputSnapshot list/detail 与 rerun command HTTP contracts，返回精确 source/input/selection refs、runnable/费用 diagnostics 和 lineage；模糊别名、缺失快照、过期 revision 或不可运行 selection 在 UoW 前失败。
- [x] 3.2 先写 starter/Workflow/Activity integration 测试，再实现稳定 Workflow ID、AlreadyStarted 复用和 `run_id + logical_operation` 幂等。
- [x] 3.3 验证 Worker 仅注册确定性 Workflow 和无 Provider 业务 Activity，默认运行使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；运行开始后 Adapter/Profile 冻结且失败不得切换。

## 4. 验证

- [x] 4.1 运行定向 domain/application（状态表、waiting_review 聚合/accept/reject、预算闸门/unknown cost/费用确认、非法转移、取消和 Signal 零持久化副作用）、adapter/SQLAlchemy（每 Run sequence、ProviderCall 关联、Agnes submit/poll/cancel/result normalization、`submission_unknown` reconciliation 与长期 RunEvent cleanup/GC refusal）、Temporal（AlreadyStarted/取消竞态/恢复不重复收费）、HTTP/SSE/contract/BDD（cursor、foreign Run、legacy `approve`、重复 Signal、`run_id + logical_operation` 绑定、`schema_version`/`schemaVersion` 同值映射及冲突无写入）及 migration 测试和 SQLite/PostgreSQL cycle。
- [x] 4.2 运行 `openspec instructions apply --change "implement-workflows-runs-slice" --json`、`openspec status --change "implement-workflows-runs-slice" --json`、`openspec validate "implement-workflows-runs-slice" --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 与 `git diff --check -- openspec/changes/implement-workflows-runs-slice`。

## 5. 默认 Workflow Bootstrap 与冻结来源

- [x] 5.1 定义唯一 `templateKey=drama-mvp-a-default`（或语义等价唯一 key）的 revision/schema/contentHash、固定 text/media/timeline port contracts 及正反 Schema fixtures，禁止 UI 推断节点端口；明确 MVP-A UI 只读 source/status，不提供 graph edit/connect/save/publish。
- [x] 5.2 实现 project-scoped `ProjectDefaultWorkflowBinding`、由用户显式文本生成的 owner create/start command 触发的空系统 ensure/bootstrap；首次创建/校验唯一固定 template、published Version/binding，重复 ensure 幂等且不创建第二 Version，页面加载/视图切换不得触发 ensure 或其他 workflow mutation。MVP-A 不实现显式版本升级或通用 Draft/graph/publish command/API。
- [x] 5.3 实现 Run source snapshot，冻结 workflowVersionId/versionNumber/contentHash/definition/scope/bindingRevision；binding 后续更新不得改变历史 Run。
- [x] 5.4 在 UoW 前验证缺失、non-published、cross-project、scope/hash mismatch、stale/noncurrent binding，分别返回 `workflow_unconfigured` 422、`workflow_version_unavailable` 409、`workflow_source_conflict` 409，断言 Run/NodeRun/RunEvent/audit/Outbox/Temporal 均零写入。
- [x] 5.5 为 bootstrap、source snapshot、端口 handoff、Provider 无 fallback、通用 Draft/graph/publish mutation `unsupported`/零写入与上述拒绝路径添加 domain/application/HTTP/BDD tests，并执行现有 strict/定向验收命令。
- [x] 5.6 添加 text successor stale closure 与 image accepted eligibility pre-media gate tests；partial/stale/foreign/hash/revision mismatch 断言 Run/ProviderCall/Outbox/Temporal/remote submit 零副作用。
- [x] 5.6a 冻结并测试 `drama-mvp-a-default` 的 `media.generate.image|video -> media.review.image|video -> media.inspect -> timeline.handoff` stages、父 `media.generate` 兼容 logical operation、每 stage nodeRun/operation owner，以及 video Provider terminal result candidate -> `accept|reject|retake` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff；覆盖 `accept` 单次 CAS、legacy `approve`/unknown 零副作用、derivative pending/failed/stale 仅阻断 Timeline/preview/export、retake 新 operation 不重复收费。
