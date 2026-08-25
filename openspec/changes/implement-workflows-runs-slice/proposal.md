## Why

## Workflow 运行与审核合同

默认 workflow 只绑定 approved `novel-writing`、`drama-skills`，八项 registry candidate 的另六项均 `pending_provenance`/disabled；它们只能在 node `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 满足时按需读取，不能成为 Worker 启动或默认 Run 前置。Run resolve 仅接受 `adapterInstalled=true`、catalog `approval=approved`、成功 probe 的 capability snapshot、`runnable=true`、`featureGate=MVP-A` 的 operation；首次 connection-test/probe 本身只需 installed/approved/MVP-A、explicit live opt-in/profile/credential/timeout 并在成功后写该 snapshot。MVP-B candidate、TTS/ASR、MiniMax H3、Seedance 2.5 和 Agnes 未选中 mode 均零外部调用。

video workflow 固定 verified Provider terminal result/storage validation -> immutable candidate + existing AssetVersion -> 人工以 exact candidate/source/ShotSpec facts `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff。基础 result 安全验证仅为 download/MIME/checksum/size/duration/dimension/StoredObjectRef，不是 MediaInspect derivative generation；candidate/pending_review 不携带 derivative readiness 作为 accept gate，accept 后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform。derivative pending/failed/stale 仅阻断 Timeline/preview/export，不阻断或撤销 accepted/current。所有 review Signal/HTTP DTO/audit event 用 `accept|reject|retake`；`approve` 与未知 verb validation 且零 current/retake/Run side effect，`accept` 只触发一次 scenes exact CAS。

阶段 0 仅有 WorkflowDraft/WorkflowVersion Schema/ORM 和健康 Activity，无法发布可执行定义、追踪运行、恢复审核等待或向客户端重放事件。本切片把工作流定义和运行事实从 Provider/媒体/AgentScope 业务中独立出来。

## What Changes

- 新增唯一固定 `templateKey=drama-mvp-a-default` 的受控 ensure/bootstrap、明确 project/episode/scene/shot scope、不可变已发布 WorkflowVersion 与 Run source snapshot；MVP-A 不提供通用 Draft、图编辑、连线、保存或发布 command/API。
- 新增 WorkflowRun、NodeRun、幂等键、状态机、取消与 Signal、持久单调事件、审计和 Outbox；冻结 `waiting_review` 的 Run 聚合进入/退出规则、accept/reject 后的 Run+NodeRun 原子转移，以及 legacy `approve`、未知或非法 Signal 不产生任何领域、审计、幂等或 Outbox 写入。
- 将“从失败节点继续”固定为显式创建 successor Run：失败 predecessor 永久保持终态，successor 冻结新的 selection/input snapshot，复用前驱成功节点的精确 owner evidence，并为待执行节点分配新的 `run_id + logical_operation`；不得重启终态 Run、覆盖失败历史或重新收费执行已复用节点。
- 将“从某版本重新运行”固定为从用户明确选择的历史 `RunInputSnapshot` 创建新 rerun Run：展示并冻结原 Brief/SourceMaterial/WorkflowVersion/scope/owner references，所有待执行节点使用新的 `run_id + logical_operation` 和新的费用确认；不得原地重启历史 Run、默认选择 current、隐式升级输入或套用 failed-successor 的 reuse evidence。
- 新增只读 Run/NodeRun detail projection：返回稳定 ID/revision/status、owner 时间/耗时、脱敏输入输出摘要、最近 RunEvent、失败 code/message/retryability 和 allowed actions；不复制 ProviderCall 原始 payload，不返回 secret、媒体 bytes、objectKey 或持久下载 URL，并让取消后的晚到结果保持可诊断而不能覆盖 owner 取消状态。
- Run 启动时冻结 workflow node override > project default > enabled system default 的 Provider/Model/Skill/capability selection snapshot，并冻结实际 Adapter/Profile identity 与 profile revision；未解析选择不得隐式 fallback，付费媒体节点在其审核边界暂停。默认测试只能使用 `Mock Provider +` 显式选择的 Local test/offline profile（adapter identity=`local_workspace`），不得因 TOS 失败切换。
- SkillRouter 唯一确定或用户对 `needs_human_selection` 完成人工裁决后，Run 才可冻结最终 SkillRevision、route decision/selection revision 与 digest；未选择、过期或非候选选择不得创建/启动 Run 或默认选第一项。
- 新增 Run 级 `BudgetGate`：图片/视频批量提交前确认、文本项目阈值超限与 `cost=unknown` 进入 `waiting_review`；确认绑定 `run_id + logical_operation`、fingerprint、稳定本地 user UUID 和 retention policy/version/hold，恢复/重试不得重复收费。
- 新增 SSE `Last-Event-ID` 补发 API，以及 Temporal starter、确定性 Workflow 与无业务 Provider 的 Activity 边界。
- 规划 Run/NodeRun/event/outbox/idempotency 持久化与迁移，并保持现有 Schema/HTTP 兼容；数据库与共享 Schema 的 `schema_version` 是唯一版本事实，HTTP DTO 的 `schemaVersion` 只映射同一值，缺失或冲突时不得写入。

## Capabilities

### New Capabilities
- `workflows-runs`: 固定默认 WorkflowVersion 的 ensure/bootstrap、已发布来源校验、版本化运行、Temporal 编排和持久事件/SSE；MVP-A 不提供通用工作流图编辑、连线、保存或发布 command/API/UI。

### Modified Capabilities

- 无。

## Impact

后续实现将影响 contracts、API 的 domain/application/adapters/interfaces、Agent Worker、Temporal 注册、Alembic 与分层测试。该切片不实现 Provider SDK、图片/视频/FFmpeg、AgentScope 文本业务或 Timeline。

## 与总体计划的追溯与边界

- 本 change 落实 `plan-phase-one-drama-mvp-a` 的总体任务 **2.2**，并受共享工程任务 **5.1**、**5.2**、**5.3**、**5.5** 约束。
- 直接实施依赖是阶段 0 已定义的模块化单体、Ports/Adapters、Outbox/Temporal/Worker 边界及既有 Schema/ORM 占位；Scene/Shot、catalog、文本和媒体 Provider 业务均不是建立运行领域的实施前置条件。
- `plan-phase-one-drama-mvp-a` 仅是 OpenSpec 协调和验收依据，不是任何 Workflow、Activity、adapter、HTTP 或数据库组件的运行时代码依赖。
- 本 change 拥有 WorkflowRun、NodeRun 与 RunEvent 的业务事实；ProviderCall 仅可关联 run/node/correlation，不拥有或复制同一事件历史。完整非目标是拥有 Provider/Profile/Model 配置或 ProviderCall 账本、真实 Provider SDK/adapter/模型调用、文本 AgentScope 业务、图片/视频/音频生成、FFmpeg/媒体渲染、Timeline 与前端；也不把 Temporal 内部表、进程内事件或 SSE 连接当作业务事实源，不复制 RunEvent 历史，不把总体协调 change 当作运行时依赖。

## 默认工作流闭合

本 change 拥有 `templateKey=drama-mvp-a-default` 的固定、版本化、已发布默认创作 Workflow、project-scoped `ProjectDefaultWorkflowBinding`、幂等 ensure/bootstrap 和 Run source snapshot。首次 ensure 仅创建或校验唯一固定模板、已发布 Version 与 binding；重复 ensure 不新增版本。旧 `WorkflowDraft` Schema/记录仅作技术遗留的只读兼容，或由 bootstrap 内部冻结为 source，不构成可编辑产品能力。MVP-A 不提供 Draft/graph mutation、发布或版本升级 command/API/UI；节点/port contract 显式连接 AgentScope text、媒体审核和 Timeline handoff，禁止 UI 猜测。

## Run gate 合同

**DDD**：Run 只消费 immutable owner handoff。**BDD**：Text stale closure 未精确 batch CAS、image 未精确 eligibility accept 均阻断媒体端口。**SDD**：node payload 冻结 candidate/source hashes/revisions 与 expected revisions，不创建第二事实源。**TDD**：断言 blocked gate 前 RunEvent/ProviderCall/outbox/Temporal 零副作用。

## MVP-A Workflow UI boundary

后端仅提供固定模板的 ensure/bootstrap、published source snapshot 和 Run 绑定，以便空项目可恢复运行；工作台只读展示 WorkflowVersion 来源、节点状态和诊断。通用工作流图 editor、节点/边编辑、连线校验、草稿保存和发布 command/API/UI 明确属于 MVP-B，不能成为 MVP-A 的实现或浏览器验收项。
