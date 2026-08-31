# Video Agent Workbench 阶段二产品需求（MVP-B）

**文档状态**：需求基线草案，待阶段二实施前评审确认
**基线日期**：2026-08-31
**前置版本**：阶段一 MVP-A 已完成的项目/剧集、结构化文本、Scene/Shot、AssetBible、Asset Center、Provider catalog、Timeline、Media、Export、Local resilience 和 observability 能力
**规范入口**：[OpenSpec 阶段二规划](../openspec/changes/plan-phase-two-mvp-b/)

## 1. 阶段定位

阶段一解决“单用户在本机完成一次可追踪生产闭环”；阶段二解决“把这条闭环变成可编排、可复核、可迁移、可协作的生产工具”。阶段二只扩展已经存在的 owner 和版本边界，不重做阶段一，也不把缺少账号、许可或二进制的能力写成已可用。

阶段二的交付对象是个人创作者及小型制作协作组，核心结果是：可自定义工作流、统一审核和评论、高级时间线、可回导 portable 工程包、受控 LAN 协作，以及可配置的 TTS/ASR/音乐和更多视频 Provider。

## 2. DDD：领域与所有权

### 2.1 领域边界

| 领域 | 阶段二新增/扩展事实 | 不得拥有的事实 |
| --- | --- | --- |
| Workflows | `WorkflowDraft`、图校验、`WorkflowVersion`、发布、子流程、控制节点、模板和画布布局 | Run 状态、业务素材版本 |
| Runs | 后续批次的 pause/resume 状态、Temporal signal、冻结快照和运行控制审计 | Workflow 图定义、业务素材版本 |
| Reviews | ReviewTask、Comment、Timecode、QCResult、Notification projection | 文本/镜头/资产/时间线 current |
| Timelines/Audio | command history、关键帧、速度/循环、字幕样式、TTS/ASR/Cue | 原始媒体 bytes、Provider credential |
| Exports/Packages | portable manifest、payload、导入 session、冲突报告 | AssetVersion/TimelineVersion 历史事实 |
| Collaboration | user/session、项目角色、presence、锁和冲突 diff | 内容 owner 的业务决策 |
| Providers/Model Catalog | Provider、Profile、模型录入、operation capability、quota、usage | 业务工作流拓扑和项目阈值 |
| Operations | operation group、backup/restore run、跨项目聚合 | provider transport、owner current |

### 2.2 状态与不变量

- `Draft -> Published Version -> Run` 三层分离；发布版本和运行快照不可变。
- 审核入口统一，但审核结论必须回写原 owner；投影滞后或冲突时只重读，不重复执行。
- 所有写命令携带 `project scope + expectedRevision + idempotencyKey`；冲突返回 409，禁止静默覆盖。
- 批量任务先冻结目标集合、版本、权限、预算和 capability snapshot；禁止隐式扩大范围。
- 旧 AssetVersion、TimelineVersion、RunEvent、AcceptDecision、CapabilitySnapshot 和脱敏 ProviderCall 为长期审计事实，不得被 GC、导入或恢复覆盖。
- Agent `AssetEditPlan` 的局部选区、mask 编辑、视频/音频时间范围不在本阶段；这些请求必须稳定返回 `unsupported_feature`，不得由 Timeline 的 mask/track matte 能力隐式代替。

## 3. 产品目标与成功标准

### 3.1 目标

1. 创作者可以在不改代码的情况下编排一个合法的短剧生产工作流并发布新版本。
2. 审核者可以在一个收件箱中处理跨文本、镜头、媒体和时间线的任务，并定位到精确版本/时间码。
3. 剪辑者可以完成可恢复的高级编辑，预览和最终 FFmpeg 成片使用同一 RenderPlan。
4. 项目可以导出完整 portable 包，在另一工作区预检、解决冲突后回导。
5. 第二位协作者可以使用受控 LAN 和角色权限参与，不破坏现有 CAS 与审计。
6. 阶段一延后的 Provider 能力可以逐项 probe、启用、计费和回收，未配置时保持明确的 `unconfigured`。
7. 模型可以手工录入或从 OpenAI-compatible `/v1/models` 同步，经候选差异审核后再启用或设为默认。

### 3.2 阶段二成功标准

- 至少一个自定义 WorkflowDraft 通过图校验并发布，重启后可从该版本恢复运行。
- 一个跨文本/镜头/媒体/Timeline 的 Review Inbox 完成批量决策、评论和时间码定位，重复事件不产生重复决策。
- 高级时间线 golden fixtures 达到：时长误差不超过 1 帧、字幕边界误差不超过 1 帧、音频 onset/sync 不超过 1 帧。
- portable 包完成一次导出 -> 隔离预检 -> 显式确认 -> 新项目/新版本回导闭环；损坏包和 hash 冲突均零写入。
- 未授权角色、过期 token、revision 冲突、quota unknown、容量硬阈值和 Provider unknown 均有稳定拒绝与零副作用证据。

## 4. BDD：用户场景与验收

本节的 B1-B7 是业务能力编号；第 6 节的 B0-B4 是交付批次编号，两套编号相互独立。模型目录属于 B7 能力，按 Provider/模型 schema 稳定性在 B2-B3 期间实施；Run pause/resume 属于 B1 能力的后续批次，明确排在首批 B1 退出门之后并归入 B4 后续批次。

### B1 工作流编排

- **Given** 一个已存在项目和当前 owner revision，**When** 用户编辑节点、连接端口并发布，**Then** 服务端校验类型、scope、DAG/Loop 上限并生成不可变 WorkflowVersion。
- **Given** 一个连接类型不兼容或 Loop 无最大次数的草稿，**When** 用户保存/发布，**Then** 返回可定位 diagnostic，不创建版本、Run 或 Outbox。
- **Given** 已发布 WorkflowVersion，**When** Worker 重启，**Then** 使用原 frozen snapshot 恢复，不重新路由或切换 Provider。
- **Given** 用户从模板目录创建 WorkflowDraft，**When** 模板版本或源项目随后变化，**Then** Draft 保持已固定版本并绑定新项目 scope，不受源变更影响。
- **Given** 画布包含大量节点，**When** 用户缩放或平移，**Then** 只渲染可见节点，低缩放级别隐藏预览和参数，手工布局优先于 ELK 自动布局。
- **Given** 用户对 Scene/Shot 执行删除、拆分、合并或跨场移动，**When** 结构命令通过 owner CAS 校验，**Then** 生成影响/stale 集合，不覆盖历史 ShotSpec、TimelineVersion 或已冻结 Run。

### B2 统一审核

- **Given** 多个 owner 的待审核任务，**When** reviewer 按项目/集/状态筛选并 accept/reject/retake，**Then** 每项调用对应 owner command，历史版本保持可读。
- **Given** 一条评论锚定 `assetVersionId + revision + frame`，**When** 源版本变化，**Then** 评论保留并标记 stale，不移动到新版本。
- **Given** 批量目标中一项 CAS 过期，**When** 提交批量决策，**Then** 逐项报告成功/失败，不扩大目标集合、不重复调用 Provider。
- **Given** ReviewTask 已分派或超过 dueAt，**When** reviewer 认领、改派或处理超期任务，**Then** 权限、CAS、提醒升级和审计结果可追踪，重复操作幂等。
- **Given** QC 结果低于冻结阈值，**When** reviewer 选择重跑、转人工或带理由 override，**Then** 保留原始证据并禁止重复 Provider 调用或自动接受 owner 事实。

### B3 高级时间线与音频

- **Given** current Cut 和合法整数帧命令，**When** 用户复制、吸附、Undo/Redo、关键帧、速度或字幕样式，**Then** 立即持久化新 revision 和 RenderPlan hash。
- **Given** ASR/TTS/音乐结果与输入 hash 匹配，**When** 用户显式接受，**Then** 追加字幕/音频 AssetVersion 或 revision，并保存 provenance、usage 和授权。
- **Given** PixiJS 与 FFmpeg plan hash 不一致，**When** 请求预览或导出，**Then** 阻断成功并返回 renderer diagnostic。
- **Given** 用户复制整条 Timeline 或关闭编辑器，**When** TimelineDraft 自动保存/恢复，**Then** 以相同 checkpoint 恢复为新 revision，不覆盖 current Cut、历史版本或冻结导出。

### B4 portable 工程包

- **Given** 固定的项目/集/TimelineVersion，**When** 用户导出 portable，**Then** 生成 manifest、完整媒体载荷、校验、依赖和授权信息；历史 light 包不变。
- **Given** 导入包存在同 ID 不同 hash 或路径穿越，**When** 执行预检，**Then** 只生成冲突报告，不创建项目、AssetVersion、TimelineVersion 或 ProviderCall。
- **Given** 预检通过且用户确认，**When** 导入中断后重试，**Then** 使用同一 package hash/operation key 恢复，不重复上传或登记。
- **Given** 导入提交阶段部分写入失败，**When** 系统执行回滚，**Then** 清理未登记临时对象，不改变 current reference、历史 owner facts 或已存在项目。

### B5 协作与访问

- **Given** 未显式开启 LAN，**When** 远程客户端请求服务，**Then** 服务只监听 localhost/127.0.0.1，远程请求不可达。
- **Given** viewer 或过期会话，**When** 执行编辑、审核、导出或导入，**Then** 返回 forbidden/unauthorized，零业务副作用。
- **Given** 两个 editor 使用同一 revision，**When** 同时提交，**Then** 一个成功，另一个收到 409 和差异，必须重读后再提交。
- **Given** owner 邀请、移除或变更项目成员角色，**When** 操作人提交成员管理命令，**Then** 权限、最后一个 owner 保护、会话撤销和审计结果可追踪。

### B6 Provider 与运维

- **Given** TTS/ASR/音乐 operation 尚未 probe 或 quota unknown，**When** 用户请求 live invocation，**Then** 保持 candidate/unconfigured 或 blocked，不发外部请求、不 fallback。
- **Given** 跨集导出或备份任务，**When** 某项失败，**Then** operation group 保留逐项状态，可单项重试，不重跑 succeeded 项。
- **Given** 备份恢复 hash/ETag 不一致或容量硬阈值，**When** 执行恢复，**Then** 阻断并保留原始诊断，不修改 current reference。
- **Given** B4 后续批次开启 Run pause/resume，**When** owner 暂停或恢复运行，**Then** 按固定状态机和 Temporal signal 处理，不启动新的付费 operation，且不改变 frozen snapshot；该能力不属于 B1 首批退出门。
- **Given** 用户打开 Operations UI，**When** 查看批量任务、备份或恢复，**Then** 可看到逐项状态、失败项重试、容量阻断、恢复确认和脱敏通知，且只操作其有权限的项目。

### B7 模型录入与目录管理

- **Given** 已登记 Provider/Profile 和已通过 probe 的 operation，**When** 用户手工录入模型，**Then** 系统保存模型标识、能力、参数 Schema、价格、限流、feature gate 和 provenance，并支持显式启用。
- **Given** OpenAI-compatible Provider 返回 `/v1/models` 结果，**When** 用户打开同步差异，**Then** 系统展示新增/变更/移除 candidate diff；未接受前不改默认、不启用、不影响历史 Run。
- **Given** 模型已被 WorkflowVersion、Run 或 ProviderCall 引用，**When** 用户尝试删除，**Then** 只允许停用并保留历史引用。

## 5. SDD：接口、数据和约束

### 5.1 关键数据对象

- `WorkflowDraft(id, projectId, revision, nodes[], edges[], status)`；`WorkflowVersion` 增加 `versionHash`、发布者和发布时间。
- `WorkflowTemplate(id, version, source, status, allowedScopes)`；画布布局保存 `nodeGeometry`、`viewport`、`layoutRevision`，不与 Run 状态混存。
- `RunControl(runId, state, requestedBy, signalId, frozenSnapshotHash)`；pause/resume 属于 B1 之后的后续批次，command 必须可幂等重放。
- `ReviewTask(id, ownerType, ownerId, ownerRevision, status, dueAt)`；`Comment` 增加 `anchorType`、`frame/timecode` 和 `targetRevision`。
- `QCPolicy(id, revision, thresholds, evidenceRequirements)` 与 `QCResult` 的状态、override 理由和 operator 审计。
- `TimelineCommand(id, cutId, expectedRevision, commandType, payloadHash)`；高级 payload 只允许 schema 注册字段，时间统一 30fps 整数帧。
- `PortableManifest(schema_version, exportProfile=portable, packageHash, objects[], dependencies[], authorization[], models[], skillRevisions[])`；禁止 secret、明文 token、不可解析持久 URL。
- `CollaborationSession(userUuid, projectId, role, expiresAt, revokedAt)`；SSE 事件只含脱敏引用和 revision。
- `OperationGroup(id, targetSetHash, snapshotHash, itemStates[])`；批量操作使用相同 group/key 恢复。

### 5.2 API 与事件

- 新增资源使用 project-scoped 路由，写入支持 `If-Match`/`expectedRevision`，冲突统一 409。
- 后续批次的 Run 控制暴露 pause/resume/status API 和脱敏 SSE 事件；模板目录暴露固定版本查询、实例化和权限诊断；故事板结构命令使用 Scene/Shot owner typed API。
- 事件采用 W3C Trace Context；所有通知、SSE、日志和审计禁止密钥、提示词全文、原始 Provider payload、媒体 bytes 和私有 object URL。
- Provider 新 operation 必须暴露 `featureGate`、`runnable`、`capabilitySnapshotId`、并发/限流/配额和 poll/reconcile 策略。
- Model Catalog Entry 必须暴露 Provider/Profile、模型标识、operation 能力、参数 Schema、价格单位、provenance、revision、`featureGate`、`runnable` 和 capability snapshot；同步只能产生 candidate diff，接受后才可成为目录事实。
- `light` 与 `portable` 共用 schema version 和 owner 引用字段；light 仅引用、不可回导，portable 才携带媒体载荷。

### 5.3 兼容与迁移

- 阶段一历史 Run、AssetVersion、TimelineVersion、ExportArtifact、light manifest 必须可读。
- 新 migration 只追加表/列/索引或可逆投影；不得删除、覆盖或重解释历史 owner facts。
- 任何新 Provider 未通过 probe 时可部署但不可运行；默认测试始终 `Mock Provider + Local test/offline profile`。

## 6. 优先级与交付批次

| 批次 | 交付 | 进入条件 | 退出证据 |
| --- | --- | --- | --- |
| B0 | schema reconciliation、权限/事件基线、feature gate、golden fixtures | 阶段一退出证据可读 | migration cycle、兼容和 fixture 报告 |
| B1 | Workflow authoring、模板/控制节点、画布、故事板结构命令、Review Center、评论/通知 | B0 完成 | 发布/结构变更/冲突/重放/批量审核 E2E |
| B2 | Advanced Timeline、TTS/ASR/音乐 adapter | B1 owner handoff 完成 | RenderPlan parity、Provider probe、字幕/音频回归 |
| B3 | portable 包、导入恢复、LAN/角色/协作 | B2 package schema 稳定 | round-trip、越权、hash 冲突、协作冲突 E2E |
| B4 | operation group、跨项目用量、自动备份恢复 | B3 审计与权限稳定 | 隔离恢复演练、容量/GC/no-GC、通知证据 |

优先级规则：B0 是所有批次前置；B1 是最小可用阶段二；B2/B3 可并行但 portable 必须消费稳定的 Timeline/Provider schema；B4 最后开放默认开关。

## 7. 非功能要求

- **安全**：LAN 默认关闭；会话短 TTL、可撤销、Origin/CSRF 校验；凭据使用既有 envelope/keyring，不进入包、日志或事件。
- **可靠性**：API/Worker/导入/备份重启可恢复；unknown 先 reconcile；同一 operation key 不重复收费或登记。
- **性能**：普通 API P95 `<500ms`（不含媒体传输、长连接和外部 Provider）；审核列表和时间线使用分页/虚拟化；大画布仅渲染可见节点。
- **可追踪性**：每次命令、ProviderCall、导入、恢复和批量项具备 trace/correlation、owner ID、revision、hash 和 operator UUID。
- **可访问性**：桌面 Chrome/Edge、键盘可达、焦点可见、状态不只依赖颜色；协作和审核操作提供可读错误。
- **前端交付门**：所有新增 UI 先完成静态演示原型、组件库/token 复用检查和用户确认，再接入真实 API/数据库；未确认原型不得改变正式业务行为。
- **数据保留**：长期审计事实 no-GC；portable 临时对象和失败工作文件按 retention policy 清理。

## 8. 明确不包含

阶段二不包含移动端页面、内容发布平台接口、自动无审核发布、开放插件市场、生产级 SSO/多租户、浏览器内 4K 最终渲染、完整专业调色台和未完成许可/能力验证的 Provider。Agent `AssetEditPlan` 的图片局部选区、mask 编辑、视频/音频时间范围，以及 Timeline mask/track matte 均不在本 change，分别由 [`agent-asset-local-edit`](../openspec/changes/agent-asset-local-edit/) 和 [`phase-two-timeline-mask`](../openspec/changes/phase-two-timeline-mask/) 独立规划。上述能力不能通过阶段二 feature flag 偷渡。

## 9. 依赖与开放问题

- 真实 TTS/ASR/音乐 Provider 的账号、数据保留和商业许可尚未确认；确认前只做 adapter contract、Mock 和显式 probe。
- portable 加密容器（ZIP+AES-GCM 或目录/对象清单+外部加密）需在 B3 设计阶段基于容量和恢复速度定案。
- 真实 TOS 的实现归属、凭据和验收环境需在 B0 依赖矩阵中冻结；未具备 TOS 时 portable 只允许 Mock/Local 离线验收并标记 `unconfigured`/`blocked`。
- LAN 身份先采用本地账户/短期会话；是否接入 OIDC 或反向代理委托另立安全变更。
- 关键帧和多机位的性能上限需用真实素材基准测量后再冻结默认上限；Timeline mask/track matte 与 Agent 局部编辑分别在独立 change 中定义性能和能力边界。

## 10. 阶段二退出门

阶段二只有在以下条件全部满足后才能标记完成：

1. B0-B4 所有 OpenSpec tasks 勾选完成，strict validation 无未解释失败。
2. 阶段一历史数据、light 包和默认 Mock/Local 闭环回归通过。
3. 自定义 Workflow、统一审核、高级 Timeline、portable round-trip、协作冲突、Operations UI 和备份恢复均有正向与反向证据。
4. 未配置或能力不足的 Provider、容量、权限、hash/revision 冲突均保持 fail-closed，且无重复收费/覆盖/泄漏。
5. 文档、OpenAPI、Schema、Alembic head、UI 状态和运行时 feature gate 一致；任何未实现能力明确显示 `unconfigured`、`blocked` 或 `MVP-Candidate`。

**专项退出约束**：ReviewTask 在完成后必须支持基于新 owner revision 的 `reopen`，取消必须释放锁并抑制后续提醒；模型目录必须验证默认解绑/停用后的新 Run fail-closed，且协作 UI 必须覆盖成员、会话、LAN 诊断、锁和冲突重读。
