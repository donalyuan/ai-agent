## 1. B0 基础、兼容与测试基线

- [ ] 1.1 建立阶段二 B0-B4 依赖图、feature gate 和发布批次清单
- [ ] 1.2 定义阶段二 operation、状态、错误码和 `unconfigured|blocked` 映射表
- [ ] 1.3 固化 Workflow、Review、Timeline、Package、Collaboration、Provider、Operations owner 边界
- [ ] 1.4 盘点阶段一 Alembic/Base metadata 漂移及历史表的读取兼容要求
- [ ] 1.5 为新增 owner 表设计主键、项目归属、revision、hash、retention 和 no-GC 约束
- [ ] 1.6 编写 B0 migration 并完成升级/降级/重复执行 cycle
- [ ] 1.7 建立统一 domain event envelope、sequence、traceparent 和 idempotency contract
- [ ] 1.8 建立 `OperationGroup`、目标集合 hash、逐项状态和重试 contract
- [ ] 1.9 建立 Comment/Timecode/Notification/QCResult 共享 JSON Schema 与错误结构
- [ ] 1.10 建立阶段一历史 Run、AssetVersion、TimelineVersion、ExportArtifact 读取 fixtures
- [ ] 1.11 建立 `light` manifest 兼容 fixtures 和禁止回导测试
- [ ] 1.12 建立 RenderPlan golden samples（视频、字幕、音频、ducking）
- [ ] 1.13 建立 portable manifest、依赖图和导入预检 corpus
- [ ] 1.14 建立 Mock Provider、Local Storage、Fake FFmpeg/TTS/ASR 的阶段二测试 fixtures
- [ ] 1.15 增加 B0 contract、migration、schema、no-GC 和 backward-compatibility 测试
- [ ] 1.16 建立阶段二外部依赖矩阵，明确真实 TOS、FFmpeg、TTS/ASR/音乐 Provider 的 owner、child change、feature gate，以及无真实依赖时仅用 Mock/Local 的验收口径
- [ ] 1.17 在 B3 前选择并冻结 portable 加密容器、密钥引用、轮换和恢复策略，补充加密/损坏/密钥缺失 fixture
- [ ] 1.18 固化 Agent `AssetEditPlan` 的阶段二边界：image/video 局部选区、mask 编辑、视频/音频时间范围移交独立 `agent-asset-local-edit` change，不在本 change 实现
- [ ] 1.19 建立阶段二 UI 原型清单、已确认的 `shadcn/ui + Radix + Tailwind + Lucide` 组件复用检查和用户确认记录；未确认前正式 UI 任务只能使用静态演示数据
- [ ] 1.20 对架构中明确列为 MVP-B 的故事板删除、拆分、合并、跨场移动能力做范围冻结和 owner/依赖登记

## 2. Workflow Authoring

- [ ] 2.1 定义 WorkflowDraft/WorkflowVersion/Node/Edge/Port 的领域对象和状态机
- [ ] 2.2 实现 WorkflowDraft、节点、边和发布版本的数据库模型与 repository
- [ ] 2.3 定义节点类型、输入输出类型、cardinality、scope 和 `allowedSkills` registry
- [ ] 2.4 实现端口类型兼容性和 required input 的 domain validator
- [ ] 2.5 实现项目 scope、跨集引用和 owner handoff 的图校验
- [ ] 2.6 实现 DAG 检测、不可达节点、重复边和缺失入口/出口校验
- [ ] 2.7 实现子流程引用、版本固定和递归深度限制
- [ ] 2.8 实现 Loop 节点最大次数、预算上限和终止条件校验
- [ ] 2.9 实现草稿创建、节点/边增删改和 expectedRevision CAS command
- [ ] 2.10 实现草稿自动保存、恢复、checkpoint 和 dirty 状态查询
- [ ] 2.11 实现草稿 diff、变更摘要和按节点定位的 diagnostic API
- [ ] 2.12 实现发布 command、不可变 WorkflowVersion、发布审计和回滚指针
- [ ] 2.13 将 Run start 绑定 published WorkflowVersion 并冻结 route/capability/input snapshot
- [ ] 2.14 实现 React Flow 节点库、端口提示、连线即时校验和画布状态恢复
- [ ] 2.15 实现 WorkflowVersion 比较、发布确认、错误定位和只读历史视图
- [ ] 2.16 增加非法连线、无界 Loop、CAS 冲突、重复发布和重启恢复测试
- [ ] 2.17 完成自定义 Workflow 发布到 Run 的 Playwright 闭环
- [ ] 2.18 定义条件、并行、合并、重试、人工审核控制节点的 schema、端口、状态和 `allowedSkills`
- [ ] 2.19 实现控制节点执行语义、分支汇合、重试次数、人工等待和预算/付费 operation 边界
- [ ] 2.20 建立 Workflow 模板目录、模板版本、发布权限和从模板创建 Draft 的 scope/owner 重绑定规则
- [ ] 2.21 实现模板复制、版本固定、导入冲突、禁用模板和模板来源审计
- [ ] 2.22 持久化节点位置、尺寸、分组、视口、缩放级别和手工布局 revision，并支持重启恢复
- [ ] 2.23 实现画布缩放/平移/框选/复制/对齐/小地图和端口可访问交互，节点内容按缩放层级加载
- [ ] 2.24 接入 ELK.js 自动布局、缩略图/代理预览和 `onlyRenderVisibleElements`，确保手工位置优先并完成大画布基准
- [ ] 2.25（B4 后续批次，非 B1 首批）定义 Run `pause_requested|paused|resume_requested|running` 状态机、权限、取消冲突和审计事件
- [ ] 2.26（B4 后续批次，非 B1 首批）实现 pause/resume command、Temporal signal、API/SSE 状态投影和幂等 key
- [ ] 2.27（B4 后续批次，非 B1 首批）暂停期间禁止新付费 operation，Worker 重启后恢复暂停状态，resume 继续使用原 frozen snapshot
- [ ] 2.28（B4 后续批次，非 B1 首批）增加重复 pause/resume、无权限、暂停与取消竞态、signal 丢失和恢复失败测试
- [ ] 2.29 定义故事板删除、拆分、合并、跨场移动的 typed command、scope、revision 和影响范围
- [ ] 2.30 实现结构变更对 Scene/Shot/ShotSpec、AssetBible 引用和 Timeline 引用的 stale/impact 投影
- [ ] 2.31 实现结构变更确认、权限、幂等、批量目标固定和历史版本只读保护
- [ ] 2.32 增加结构变更 CAS、跨项目/跨集越权、引用冲突和零副作用测试
- [ ] 2.33 实现控制节点、模板实例化和故事板结构编辑的正式 UI 入口，并在用户确认原型后接入真实 command
- [ ] 2.34（B4 后续批次）实现 Run pause/resume UI、状态提示和恢复失败诊断

## 3. Unified Review Center

- [ ] 3.1 定义 ReviewTask、ReviewDecision、Comment、Timecode、QCResult 实体
- [ ] 3.2 建立 ReviewTask 与 text/scene/shot/asset/timeline owner 的引用关系
- [ ] 3.3 建立 Review Inbox 事件投影表、checkpoint 和 projection version
- [ ] 3.4 实现按 project、episode、ownerType、status、assignee 的 cursor 查询
- [ ] 3.5 实现 accept/reject/retake 到各 owner typed command 的映射器
- [ ] 3.6 实现评论版本锚定、frame/timecode 合法性和 stale 标记
- [ ] 3.7 实现批量审核 operation group、固定目标集合和逐项结果
- [ ] 3.8 实现语义 QC adapter 的 candidate、unavailable、failed 和 retryable 状态
- [ ] 3.9 实现视觉 QC adapter 的图片/视频输入 fingerprint 和结果引用
- [ ] 3.10 实现脱敏 SSE 事件、通知偏好和本地提醒状态
- [ ] 3.11 实现事件重复/乱序/缺失的重放、checkpoint 恢复和 projection_stale 处理
- [ ] 3.12 实现 Review Center 页面、筛选、批量操作、评论和时间码定位
- [ ] 3.13 增加跨 owner CAS、权限、过期候选、重复事件和敏感信息泄漏测试
- [ ] 3.14 完成统一审核中心的文本到媒体跨页面 Playwright 闭环
- [ ] 3.15 实现 ReviewTask 的 assign/reassign/claim/unassign command、审计和幂等
- [ ] 3.16 实现 dueAt、超期状态、提醒升级和按 assignee/due 状态筛选
- [ ] 3.17 校验 reviewer/editor/owner 对任务分派、决策和 QC 操作的权限边界
- [ ] 3.18 定义可配置 QC policy、阈值、证据引用和人工 override 状态机
- [ ] 3.19 实现 QC 失败后的重跑、转人工审核和禁止重复 Provider 调用语义
- [ ] 3.20 增加分派竞态、超期提醒、QC override/重跑和权限回归测试
- [ ] 3.21 实现 ReviewTask 分派、dueAt、QC policy/override 的正式 UI 和可读错误状态
- [ ] 3.22 定义并实现 ReviewTask `open|assigned|in_review|completed|reopened|cancelled|overdue` 生命周期、完成后 reopen/取消命令和锁定释放规则
- [ ] 3.23 增加 ReviewTask reopen/取消/锁定超时/完成后 owner 变更的 CAS、幂等、权限和 Playwright 回归测试

## 4. Advanced Timeline and Audio

- [ ] 4.1 定义 TimelineCommand、CommandHistory、Undo/Redo 栈和恢复语义
- [ ] 4.2 实现复制、吸附、批量选择和 track lock 的 domain command
- [ ] 4.3 实现历史 TimelineVersion 恢复为新 current revision
- [ ] 4.4 扩展 Clip schema 支持 keyframe、speed、loop 和多机位 source
- [ ] 4.5 实现关键帧插值、边界校验和非法曲线拒绝
- [ ] 4.6 实现基础调色的 schema、容量上限和 capability gate；Timeline mask/track matte 由独立 `phase-two-timeline-mask` change 负责
- [ ] 4.7 扩展 Caption schema 支持字体、位置、样式、safe-area 和时间范围
- [ ] 4.8 实现 TTS/Narration 音频轨道、角色 voice binding 和人工确认
- [ ] 4.9 实现 ASR 时间戳导入、对齐修订、source hash 校验和字幕接受
- [ ] 4.10 实现音乐/环境声生成结果到 SoundCue/AssetVersion 的 handoff
- [ ] 4.11 扩展 canonical RenderPlan 和 FFmpeg filter graph compiler
- [ ] 4.12 同步 PixiJS preview compiler、代理降级和渲染能力诊断
- [ ] 4.13 实现高级时间线 UI、键盘快捷操作、锁定状态和可访问焦点
- [ ] 4.14 增加关键帧、速度、字幕、TTS/ASR、Undo/Redo 和 track lock 测试
- [ ] 4.15 完成时长、字幕边界、音频 onset、ducking 和 plan hash golden parity
- [ ] 4.16 完成高级 Timeline 到预览/导出的 Playwright 闭环
- [ ] 4.17 实现 Timeline/Cut 整体复制，重绑定 project/episode/scope，保留来源 TimelineVersion 和引用 hash
- [ ] 4.18 定义 TimelineDraft 独立自动保存、checkpoint、dirty/clean、恢复和失败重试策略
- [ ] 4.19 实现自动保存与 current Cut 的 CAS、命名/发布关系和恢复为新 revision 语义
- [ ] 4.20 增加 Timeline 整体复制、自动保存失败、重启恢复和与导出并发冲突测试

## 5. Provider Expansion

- [ ] 5.1 定义 `TtsPort`、`AsrPort`、`MusicGenerationPort` 和统一 operation DTO
- [ ] 5.2 为三类新 Port 实现 Deterministic Mock 和 Local offline adapter
- [ ] 5.3 实现 Fish Audio adapter、参数映射、submit/poll/result/reconcile
- [ ] 5.4 实现 Groq ASR adapter、时间戳解析、重试和 unknown 终局
- [ ] 5.5 接入首个音乐/环境声 Provider adapter 和结果下载校验
- [ ] 5.6 为新增 Provider 建立 credential、capability probe、feature gate 和 catalog UI
- [ ] 5.7 为新增视频模式建立 operation schema、quota、rate-limit 和 poll/reconcile policy
- [ ] 5.8 将媒体/字幕结果登记为 immutable AssetVersion 或 caption revision
- [ ] 5.9 接入 usage、cost、license、retention、ProviderCall 和审计关联
- [ ] 5.10 增加未 probe、unconfigured、429、quota unknown、credential error、callback 签名失败和禁止 fallback 测试
- [ ] 5.11 对每个真实 Provider 执行显式 probe 并保存 capability evidence
- [ ] 5.12 定义 ModelCatalogEntry、ModelCapability、ParameterSchema、PriceUnit 和 ModelProvenance 数据结构
- [ ] 5.13 实现手工录入模型的 create/edit/disable API、表单校验和 expectedRevision CAS
- [ ] 5.14 实现 OpenAI-compatible `/v1/models` 拉取、认证、超时和原始响应脱敏
- [ ] 5.15 实现同步结果的新增/变更/移除 candidate diff 持久化
- [ ] 5.16 实现 candidate diff 的逐项查看、接受、拒绝和审计
- [ ] 5.17 实现模型 operation 能力、参数 Schema、价格、并发/限流和 quota 配置
- [ ] 5.18 实现全局默认、项目覆盖、feature gate 和 capability snapshot 绑定
- [ ] 5.19 实现历史引用探测；被 Run/WorkflowVersion/ProviderCall 引用的模型只允许停用
- [ ] 5.20 增加模型录入、同步冲突、默认覆盖、历史引用删除保护和 provider isolation 测试
- [ ] 5.21 完成模型目录 UI：录入、同步、差异审核、启停、默认和项目覆盖
- [ ] 5.22 完成模型目录到 Workflow/Run/ProviderCall 的 Playwright 闭环
- [ ] 5.23 实现 system/project default 的显式解绑、禁用模型后的解析结果和新 Run `unconfigured|blocked` 行为；不得回写历史冻结快照
- [ ] 5.24 增加默认解绑、最后可用模型停用、Workflow-node > project > system 优先级和历史 Run 不变的 contract/domain/API 测试
- [ ] 5.25 在模型目录 UI 展示默认解绑、停用影响、不可运行原因和重新绑定确认，并完成相应 Playwright 场景

## 6. Portable ProjectPackage

- [ ] 6.1 定义 portable manifest schema、schema_version 和 package hash
- [ ] 6.2 构建 AssetVersion、TimelineVersion、WorkflowVersion、Provider/Skill、授权依赖图
- [ ] 6.3 实现对象清单、checksum/ETag、MIME、大小和媒体载荷索引
- [ ] 6.4 实现 package 加密容器和密钥引用隔离
- [ ] 6.5 实现 portable 导出 preflight、容量 admission、license/hold 检查
- [ ] 6.6 实现分片打包、断点恢复、artifact 上传、verify 和 register
- [ ] 6.7 实现隔离 workspace 导入 session 和中断恢复
- [ ] 6.8 实现 schema/hash/路径穿越/格式/容量/病毒扫描/许可证预检
- [ ] 6.9 实现同 ID 不同 hash、缺对象、版本漂移的 conflict report 和映射
- [ ] 6.10 实现用户确认后的新项目/新版本创建与幂等回导
- [ ] 6.11 实现 light/portable 兼容、损坏包、权限越界和 round-trip 测试
- [ ] 6.12 完成 portable 导出、导入预检、确认回导的 Playwright 闭环
- [ ] 6.13 按已冻结加密方案完成密钥缺失、密文篡改、分片恢复和安全擦除测试
- [ ] 6.14 增加导入提交阶段的部分失败回滚、临时对象清理和 current reference 不变测试

## 7. Collaboration and Access

- [ ] 7.1 定义本地 User、Session、ProjectMembership 和 role domain model
- [ ] 7.2 实现账户创建、登录、短期 token、刷新、撤销和 operator UUID
- [ ] 7.3 实现 Origin/CSRF、重放保护、请求限流和安全审计
- [ ] 7.4 实现 owner/editor/reviewer/viewer 权限矩阵
- [ ] 7.5 将 project scope 和角色授权接入所有阶段二写 API
- [ ] 7.6 实现 LAN 显式 opt-in、监听地址校验和启动诊断
- [ ] 7.7 实现 presence、在线成员和 session heartbeat 的脱敏事件
- [ ] 7.8 实现协作评论、mention、通知和权限过滤
- [ ] 7.9 实现 revision 冲突 diff、重读和重新提交交互
- [ ] 7.10 实现轨道/资源锁的租约、超时和释放
- [ ] 7.11 增加未授权、token 过期、重放、LAN 默认关闭、并发 CAS 和锁超时测试
- [ ] 7.12 完成双用户协作冲突与权限 Playwright 闭环
- [ ] 7.13 实现项目成员邀请、移除、角色变更和会话撤销
- [ ] 7.14 增加成员变更、最后一个 owner 保护和权限缓存失效测试
- [ ] 7.15 实现协作访问 UI：LAN 开关/诊断、成员与角色管理、在线会话撤销、presence、锁状态和冲突 diff 重读入口；正式联调前完成静态原型确认

## 8. Automation and Operations

- [ ] 8.1 实现跨集生成、审核、QC、导出和恢复的 OperationGroup aggregate
- [ ] 8.2 实现目标集合快照、snapshot hash 和逐项幂等 key
- [ ] 8.3 实现失败项单独重试、成功项跳过和 group summary
- [ ] 8.4 实现跨项目运行/用量查询、权限过滤和 cost unknown 展示
- [ ] 8.5 实现备份 scheduler、手工触发、保留策略和备份 manifest
- [ ] 8.6 实现 PostgreSQL、object manifest、Compose config、keyring reference 的备份 adapter
- [ ] 8.7 实现备份容量预估、soft/hard threshold admission 和 blocked diagnostic
- [ ] 8.8 实现隔离恢复 workflow、operator confirmation、checksum/ETag gate 和 rollback
- [ ] 8.9 实现通知模板、脱敏渲染和操作人/trace/correlation 关联
- [ ] 8.10 增加 backup/restore/restart/GC/no-GC/hard-limit zero-side-effect 测试
- [ ] 8.11 完成一次可重复的隔离恢复演练并保存证据
- [ ] 8.12 实现 Operations UI：OperationGroup 逐项状态/重试、用量与 cost unknown、备份策略、手工备份、恢复确认、容量阻断和通知诊断；正式联调前完成静态原型确认
- [ ] 8.13 完成 Operations UI 到批量重试、备份恢复和阻断诊断的 Playwright 闭环，并验证权限/脱敏/零副作用

## 9. 阶段二集成与退出验收

- [ ] 9.1 为 B0-B4 建立阶段二证据矩阵和 owner/prerequisite/failure/no-side-effect 字段
- [ ] 9.2 完成 Mock/Local 默认路径的创意到 portable 导出完整闭环
- [ ] 9.3 完成自定义 Workflow、统一审核、高级 Timeline 的跨页面导航验收
- [ ] 9.4 完成 portable round-trip、协作冲突和权限拒绝验收
- [ ] 9.5 完成 Provider explicit probe；未配置项保持 `unconfigured`/`blocked`
- [ ] 9.6 验证阶段一历史运行、资产、TimelineVersion、ExportArtifact 和 light 包可读
- [ ] 9.7 验证普通 API P95 `<500ms`、大画布虚拟化和时间线代理降级
- [ ] 9.8 执行 contract/domain/application/API/worker/Playwright、安全、性能、OpenSpec strict 和文档一致性检查
- [ ] 9.9 完成阶段二 UI 原型确认记录、组件库/token 复用、键盘可达、焦点可见和无障碍证据
- [ ] 9.10 完成 B1 首批控制节点、模板和故事板结构变更的正反向 E2E 与 no-side-effect 证据；Run pause/resume 仅在后续批次开启后单独验收
