# Change: 集成 Agnes Video Provider

## Provider handoff 与视频审核

Agnes 的首次 explicit connection-test/probe 只需 `adapterInstalled=true`、catalog `approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout，成功后冻结 capability snapshot，不要求既有 snapshot 或 `runnable=true`；explicit live invocation 还需该成功 snapshot 与 `runnable=true`。未选中 Agnes mode 与 MVP-B candidate 零外部调用，默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。verified Provider terminal success 是 result `AssetVersion` 唯一 append 时点，retry/reconcile 返回同一 version/candidate。

视频顺序固定为 Provider terminal result candidate -> 人工以 exact candidate/source/ShotSpec facts `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff。`accept|reject|retake` 是唯一 review action，legacy/unknown `approve` validation 且零 current/retake/AssetVersion/Timeline side effect。derivative pending/failed/stale 仅阻断 Timeline/preview/export，不阻断、撤销或降级已 accepted/current。

## 原因

当前 `VideoGenerationPort` 没有持久化的异步提交生命周期。阶段 1 需要一个受控 Agnes 流程，捕获账号已启用的 capability snapshot，使 submit/poll/cancel/result 具备幂等性，并将已验证输出登记为不可变 AssetVersions。

## 变更内容

- 通过 `VideoGenerationPort` 定义 Agnes `submit`、`poll`、`cancel` 与 `result` 行为；实施 probe 优先考察 v2.0 稳定候选，但 probe 前不硬编码 model/mode ID，MVP-A 最终仅支持已配置账号实测通过并冻结的一个 image-to-video mode，明确不接入 2.5 preview。
- 为每次请求冻结已启用账号的 capability snapshot，并将 `poll` observations 归一为单一幂等 activity model；`submission_unknown` 必须先 reconciliation。
- 每次 `submit` 必须绑定当前 storyboard 的 image AssetVersion、对应 ShotSpec、显式 duration 和 aspect ratio；任一引用过期或不一致时不得调用 Agnes。
- 将已验证 video results 作为 StoragePort references 登记，并追加关联 runs/audit 的 AssetVersions。
- 视频结果先登记 `VideoTakeCandidate`/`pending_review`，由 workflows/runs 的审核 command 显式 `accept|reject|retake`；accept 后仅交付 scenes/storyboard 的 exact current-video eligibility，reject 不删除结果，retake 必须创建 successor candidate 和新的 `logicalOperation`，未经 accept 不得进入 Timeline。
- Agnes Provider 只拥有 capability probe、submit/poll/cancel/result transport、ProviderCall attempt/native usage 证据；缩略图、关键帧索引、波形、proxy 与 canonical 媒体元数据由 media worker 的 inspection/derivative port 生成，不由 Provider 或 Timeline 生成。
- 保持 `Mock Provider +` 显式 Local test/offline profile 默认测试行为；真实 Agnes invocation 必须是显式 opt-in probe，而不是对所有公开 capability 的声明，且不得因 live 失败切换 profile。

## 能力

### 新增能力

- `agnes-video-generation`：受控的异步 Agnes video operation lifecycle 与 result registration。

### 修改能力

无。

## 总体计划追溯与边界

本 change 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `3.3`，并受共享任务 `5.1`、`5.2`、`5.4`、`5.5` 约束。总体计划只协调交付顺序，不构成运行时代码依赖。直接依赖已归档 AssetVersion、`implement-workflows-runs-slice` 的 Run/RunEvent 边界及 `implement-provider-model-skill-catalog` 的 catalog、冻结 snapshot 和 ProviderCall 账本。

ProviderCall 是 catalog 领域唯一持久化一次调用、费用和幂等账本；RunEvent 仅属 workflows/runs，并只经 `run_id`、`node_run_id`、`correlation_id` 与本 change 关联。Agnes `poll` 原始观测可保存到 ProviderCall attempt metadata；归一业务状态只能由 workflows/runs 追加 RunEvent，不能创建重复 event history。

完整非目标包括声明所有公开 Agnes capability、隐式 live execution、同步 video generation、callback/webhook、文本/图片 orchestration、raw video persistence、必要 Activity contract 之外的新 Temporal workflow、冻结最终 HTTP error envelope、billing settlement，以及重做 catalog 或 WorkflowRun/NodeRun/RunEvent 状态机与事件历史。本 change 只复用 AssetVersion 与 catalog 已拥有的 canonical `schema_version`；HTTP `schemaVersion` 仅由所有者映射同一值，Agnes adapter/application 不创建独立的 Provider 专用版本事实。

## 影响

预期实现涉及 VideoGenerationPort adapter/application、catalog snapshots、Temporal Activity boundary、所需 persistence/migration、StoragePort/AssetVersion 与 contract/integration tests。它不实现 general public-capability matrix 或 text/image generation，也不拥有人工审核、Timeline current reference 或 media derivative records。

## Submit 前 gate

**DDD**：Agnes consumer 只读 scenes eligibility projection。**BDD**：unaccepted/stale/foreign/hash/revision mismatch 被拒绝且无 ProviderCall/external submit。**SDD**：submit payload 精确绑定 accepted candidate/provenance、ShotSpec、duration/aspect-ratio snapshot。**TDD**：所有 preflight mismatch 断言零副作用。
