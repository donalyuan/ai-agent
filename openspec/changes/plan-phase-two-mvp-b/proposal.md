## Why

阶段一（MVP-A）已经打通从创意/改编素材到结构化剧本、镜头、媒体审核、时间线和轻量导出的单用户本地闭环，但工作流仍是固定模板，审核分散在多个页面，时间线只覆盖基础剪辑，light 包不能迁移，且无法安全地让第二位协作者接入。阶段二（MVP-B）需要把这条闭环提升为可编排、可复核、可迁移、可协作的生产工具，同时保持阶段一的版本、CAS、幂等、owner 和 fail-closed 约束。

## What Changes

- 增加可编辑 WorkflowDraft：节点/端口类型、连线校验、子流程、受控 Loop、条件/并行/合并/重试/人工审核控制节点、模板目录、草稿自动保存、发布不可变 WorkflowVersion、差异和回滚；运行只能绑定已发布版本。Run pause/resume 保留为阶段二后续批次，不进入首批 B1。
- 补齐画布生产能力：持久化节点位置/尺寸/分组/视口，提供缩放、平移、框选、复制、对齐、小地图、ELK 自动布局、可见节点渲染和缩略图/代理预览。
- 增加架构已约定的故事板结构命令：删除、拆分、合并和跨场移动；所有结构变化通过 owner typed command、CAS、影响分析和 stale 传播落地。
- 增加统一审核中心：跨文本/镜头/媒体/时间线的队列、批量决策、版本锚定评论、时间码、提醒和可选语义/视觉 QC；审核结论仍由各 owner 事实源落地。
- 扩展 Timeline/NLE：复制、吸附、Undo/Redo、版本恢复、字幕样式、Narration/TTS、ASR 对齐、速度/循环、轨道锁定、多机位、关键帧和基础调色；预览与 FFmpeg 继续共享 RenderPlan。Timeline mask/track matte 另立 `phase-two-timeline-mask` change。
- 增加 portable ProjectPackage：打包完整媒体载荷、清单、校验和依赖；支持导入预检、冲突报告、显式恢复和回导，不改变历史 owner 事实。
- 增加协作与访问基础：localhost 之外的受控 LAN、账户/会话、项目角色、共享评论、锁与冲突解决；默认仍关闭网络暴露。
- 接入阶段一延后的 Provider/能力：Fish Audio TTS、Groq ASR、音乐/环境声生成以及更多已 probe 的视频模式；每项能力继续受 catalog、capability snapshot、预算和 explicit opt-in 约束。
- 完善模型录入与目录管理：支持手工录入、OpenAI-compatible `/v1/models` 同步、候选差异审核、模型能力/参数/价格/限流配置、启停、全局默认和项目覆盖；历史引用模型只能停用。
- 增加批量与跨项目运行/用量中心、自动化备份恢复和可操作通知；批量操作必须产生可追踪的 operation group，禁止隐式跨集修改。
- 明确 Agent `AssetEditPlan` 的局部选区、mask、视频/音频时间范围仍不属于本 change；相关请求保持 `unsupported_feature`，与 Timeline 的 mask/track matte 能力分离。
- 保持非目标：移动端、开放插件市场、无审核的自动发布、平台发布接口和浏览器内 4K 最终渲染留到阶段三或单独变更。

## Capabilities

### New Capabilities

- `phase-two-workflow-authoring`: 可编辑工作流草稿、图校验、版本发布、子流程、受控循环、控制节点、模板、画布布局和 Run pause/resume 交接。
- `phase-two-review-center`: 统一审核队列、任务分派、评论/时间码、批量决策、提醒和可配置自动质检结果聚合。
- `phase-two-advanced-timeline`: 专业化时间线、整体复制、独立自动保存、音频/字幕增强、关键帧和 RenderPlan parity。
- `phase-two-portable-package`: portable 工程包的导出、导入预检、恢复和冲突处理。
- `phase-two-collaboration-access`: LAN 访问、身份、项目角色、协作会话和冲突解决。
- `phase-two-provider-expansion`: TTS、ASR、音乐和新增视频模式的统一 Provider 接入。
- `phase-two-model-catalog`: 模型录入、候选同步、能力参数维护、默认绑定和历史引用保护。
- `phase-two-automation-operations`: 批量运行、跨项目用量、自动备份恢复和通知。

### Modified Capabilities

- `episode-timeline-editing`: 从 MVP-A 基础剪辑扩展到高级编辑命令、可恢复历史、整体复制、独立自动保存和关键帧。
- `episode-audio-export`: 增加 portable payload、TTS/ASR/音乐轨道及可回导清单，保留 `light` 兼容性。
- `provider-model-skill-catalog`: 增加阶段二 operation 的 feature gate、配额、回调/轮询和权限约束。
- `operations-resilience`: 增加自动化备份、恢复演练编排和跨项目容量/任务治理。

## Impact

- 影响 `apps/web` 的 Workflow 编辑器、审核中心、高级时间线、项目包和协作页面；继续使用已确认的 `shadcn/ui + Radix + Tailwind + Lucide` 组件体系。
- 影响 `services/api` 的 workflows、reviews、timelines、audio、exports、providers、usage、auth/collaboration 模块及其 OpenAPI/事件契约；新增 migration 必须保持 owner 边界和历史数据可读。
- 影响 Agent/Generation/Media Worker、Temporal task queue、Outbox、FFmpeg/ASR/TTS 适配器和存储容量；真实 Provider 仍需显式配置、probe 和可撤销凭据。
- 引入可选的 LAN auth、审计/通知和备份目标，但不改变阶段一默认 localhost、Mock Provider + Local profile 和无外部副作用测试路径。
