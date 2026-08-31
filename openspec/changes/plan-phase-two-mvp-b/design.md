## Context

阶段一以固定 `drama-mvp-a-default` WorkflowVersion、单用户 localhost、Mock/Local 默认和 owner/CAS 约束完成 MVP-A。阶段二面向已经存在的项目和版本数据，开放工作流编排、统一审核、高级时间线、portable 包和受控协作。该变更是规划合同，不直接修改运行时代码；后续实施必须拆成可独立回滚的 child changes。

## Goals / Non-Goals

**Goals:**

- 保留阶段一数据和 `light` 导出兼容性，增加可演进的 MVP-B 能力。
- 让工作流、审核、时间线、工程包、协作和 Provider 扩展各自拥有明确 owner、版本和幂等命令。
- 让所有跨模块操作都有可验证的输入快照、权限检查、预算/容量 admission、审计事件和失败无副作用。
- 提供可量化的端到端验收，覆盖重启、冲突、恢复、导入校验和真实/未配置 Provider 两种路径。

**Non-Goals:**

- 不在本 change 内实现代码、迁移或 UI；不把规划状态写成已完成。
- 不实现移动端、公开发布平台、无人工门的自动发布、开放插件市场或浏览器 4K 最终渲染。
- 不把 LAN auth 当作生产身份系统；个人项目仍不进入多租户生产部署。

## Decisions

### 1. 采用 capability child changes 和阶段门

每个能力以独立 OpenSpec child change 实施，按 `B0 foundation -> B1 authoring/review -> B2 timeline/providers -> B3 package/collaboration -> B4 automation` 分批验收。相比一次性大 change，这样可以在真实 Provider 或 FFmpeg 缺失时只交付已配置能力，避免半成品互相耦合。

### 2. Workflow 采用 Draft/Version/Run 三层不可变边界

草稿允许自动保存和多人编辑；发布生成 immutable WorkflowVersion，运行只引用已发布版本并冻结 route/capability snapshot。端口类型、DAG/Loop 限制和成本上限由服务端权威校验，React Flow 仅做即时反馈。相比把画布 JSON 直接作为运行输入，该方案能保留历史可复现性并支持冲突检测。

### 3. 审核中心使用聚合投影而非第二事实源

Review Inbox 只保存任务、评论、时间码、QC 结果和 owner command 引用；accept/reject/retake 仍由 text、scene/shot、asset、timeline owner 落地。队列使用事件驱动投影，事件重放必须幂等。这样统一入口不会复制或覆盖各领域版本事实。

### 4. 时间线继续以整数帧和 RenderPlan 为唯一执行语义

高级编辑命令产生 command history 和 immutable TimelineVersion；预览、导出、portable 包均消费同一个 canonical RenderPlan。关键帧、速度、字幕样式、TTS/ASR 和多机位都先编译为版本化中间表示，再分别映射到 PixiJS/FFmpeg。Timeline mask/track matte 不在本 change，单独由 `phase-two-timeline-mask` 设计和验收。相比在前端和 FFmpeg 各自解释 JSON，可避免预览与成片漂移。

### 5. portable 包采用 manifest-first、两阶段导入

导出先生成 manifest、对象清单、hash、权限/许可证和依赖图，再并行打包媒体载荷。导入先执行离线 schema/hash/容量/许可证/冲突预检，只有用户显式确认后才创建新项目或新版本；绝不覆盖现有 owner facts。阶段一 `light` 包继续只读且不可回导。

### 6. 协作采用项目角色 + 乐观并发 + 事件订阅

首版角色为 owner/editor/reviewer/viewer；所有写命令携带 project scope、expectedRevision 和 idempotency key。SSE/事件流只传递脱敏状态和可重读的事实 ID，冲突返回 409 并提供三方差异，不引入无边界的实时 OT/CRDT。LAN 默认显式开启、短期会话 token、可撤销和审计。

### 7. Provider 扩展服从既有 catalog/credential/operation policy

TTS、ASR、音乐和新增视频模式都必须先登记 adapter、capability probe、参数 schema、配额和 feature gate，再允许显式调用。真实请求与默认 Mock/Local 路径分离；callback/webhook 只在适配器证明幂等和签名校验后引入，否则继续 poll/reconcile。供应商 SDK 不进入领域层。

### 8. 模型录入采用 candidate diff + explicit accept

模型目录继续由 catalog owner 持有，但阶段二把“模型录入”作为显式业务闭环：用户可以手工录入模型，或从 OpenAI-compatible `/v1/models` 获取 candidate diff；同步结果不能自动启用、替换默认或改写已冻结的 Run。模型记录必须包含 Provider/Profile、模型标识、operation 能力、参数 Schema、价格单位、限流/并发、feature gate 和 provenance。相比直接覆盖模型表，candidate diff 能避免上游目录变化破坏历史运行可复现性。

### 9. 备份、通知和批量运行使用持久 operation group

跨集批量生成、导出、QC 和恢复均建立 operation group，固定目标集合和版本快照，允许逐项成功/失败但禁止隐式扩大范围。备份包含 PostgreSQL、object manifest、配置和密钥引用，恢复先在隔离 workspace 校验 checksum/ETag，再由用户确认切换。

### 10. Run 控制、模板和画布交互属于独立可验收边界

Run 的 pause/resume 由 runs owner 持有状态机，但排在首批 B1 之后的后续批次；首批 B1 只交付 Workflow authoring、模板、控制节点、画布和故事板结构命令。pause/resume 通过 Temporal signal 驱动，暂停期间不启动新的付费 operation，Worker 重启后保持暂停状态；取消、暂停和恢复的竞态必须由状态机和幂等键裁决。Workflow 模板以版本化目录事实存在，从模板实例化时重绑定项目 scope、owner 和权限。画布布局、视口、分层渲染和 ELK 自动布局是独立的 presentation facts，手工布局优先，不能写回运行状态。

### 11. 故事板结构变化继续由 Scene/Shot owner 负责

删除、拆分、合并、跨场移动不由 Workflow 或 Review 投影直接修改。结构命令必须由 Scene/Shot owner 以 expectedRevision 接收，计算对 ShotSpec、AssetBible 和 Timeline 引用的 impact/stale 集合，经过权限和显式确认后提交；历史版本和已冻结 Run/Export 只读。该边界避免故事板结构编辑形成第二套事实源。

### 12. 外部依赖和前端原型是阶段门

真实 TOS、FFmpeg、TTS/ASR/音乐 Provider 的实现归属、凭据和可运行条件在 B0 依赖矩阵中冻结。缺少真实依赖时，只能使用 Mock/Local/Fake adapter 完成离线验收，运行时状态保持 `unconfigured` 或 `blocked`。阶段二所有新增 UI 先以静态演示数据制作可运行原型，完成组件库/token 复用和用户确认后才进入正式 API、数据库和业务联调。

## Risks / Trade-offs

- [Risk] 高级时间线功能显著扩大 RenderPlan 语义 → 先冻结小的 feature subset 和 golden fixtures，新增能力必须通过 preview/FFmpeg parity。
- [Risk] 统一审核投影可能滞后或重复消费事件 → 使用事件版本、唯一键、checkpoint 和可重放修复命令；owner 决策仍是最终事实。
- [Risk] portable 包包含敏感媒体和凭据引用 → manifest 不含密钥，载荷加密/短期 grant，导入执行 license/容量/病毒和路径穿越检查。
- [Risk] LAN auth 增加攻击面 → 默认关闭、绑定明确接口、短 TTL token、CSRF/Origin 校验、审计和一键撤销；不承诺生产级 SSO。
- [Risk] 新 Provider 的 callback/费用语义不一致 → 初期统一 poll/reconcile，unknown 状态禁止重提；capability snapshot 必须记录费用和终局映射。
- [Risk] 自动备份在本地磁盘造成容量压力 → 采用软/硬阈值、保留策略和 dry-run 估算，硬阈值时 fail-closed。

## Migration Plan

1. B0：完成 schema reconciliation、事件/权限基线、feature gate 和 golden fixtures；仅新增表/索引，不改变阶段一读路径。
2. B1：启用 WorkflowDraft/Version、模板/控制节点、画布、故事板结构命令、Review Inbox 和评论投影；旧固定 WorkflowVersion 继续可运行。Run pause/resume 不作为 B1 首批退出条件。
3. B2：逐项启用高级 timeline 与 TTS/ASR/音乐 adapter；Timeline mask/track matte 和 Agent 局部编辑不在本 change，分别由独立 change 规划；先以 Mock/Local 和离线 fixtures 验收，再做显式 live probe。
4. B3：上线 portable 导出/导入和受控 LAN；迁移只复制 manifest/ref，不重写历史 AssetVersion/TimelineVersion。
5. B4：启用批量 operation group、跨项目用量、自动备份和通知；在 B1 之后的 B4 后续批次单独开启 Run pause/resume；完成一次隔离恢复演练后再开放默认开关。
6. 任一批次失败时，关闭对应 feature gate，保留已生成的 immutable facts，回滚入口/投影 migration；禁止删除阶段一数据或把失败 operation 标记为成功。

## Open Questions

- 首个生产级身份方案是本地账户、OIDC 还是反向代理委托？B3 先实现可撤销本地会话，生产 SSO 另立 change。
- portable 包采用哪种加密容器（ZIP+AES-GCM 或目录/对象清单+外部加密）仍待 B3 设计阶段依据容量、恢复速度和密钥托管验证选定；在冻结前或验证未通过时，portable 只能保持 `blocked`，不得以未认证压缩包替代。
- TTS/ASR 的首个可验证 Provider 和音乐生成 Provider 的商业/数据保留条款尚未确认，未确认前保持 catalog candidate。
- 关键帧的浏览器性能上限、代理规格和最大并发需要用真实素材基准测量；Timeline mask/track matte 的性能和跟踪算法在独立 change 中测量。
