# Script To Asset Generation Design

## Goal

为素材 Agent 建设第一版脚本到素材候选链路。系统从脚本分镜出发，先复用素材库旧素材，再对图片缺口自动生成多候选；AI 视频仅创建待确认任务，不自动执行。用户最终在脚本详情页选择每个分镜的主素材。

## DDD

`Material` 仍是素材库聚合根，归属 `projects.id`。旧素材和 AI 生成素材统一进入 `materials`。

新增 `SceneAssetCandidate` 表示某个分镜的候选素材。候选可以来自旧素材复用、AI 图片生成或 AI 视频待确认任务。候选状态为 `candidate`、`selected`、`rejected`、`failed`。同一分镜最多只能有一个 `selected`。

新增 `AssetGenerationTask` 表示素材生成任务。它和现有 `generation_tasks` 分开，避免把现有视频作品任务约束扩宽。图片任务可自动执行；视频任务默认为 `draft`，必须人工确认后才可启动。

供应商配置属于平台控制面，后续“大模型管理”归 `admin/`。本 change 只消费可用供应商配置，不建设后台配置页面。

## BDD

用户在脚本详情页触发“生成素材候选”。系统展示生成计划，包括分镜数、候选图数量、供应商、参考图数量和是否超过 48 张上限。

用户确认后，系统创建图片生成任务和视频待确认任务。页面按“左侧分镜列表 / 中间候选素材 / 右侧生成设置与任务”展示：旧素材候选和 AI 图片候选分区呈现，右侧可选择 `gpt-image-2` 或即梦、候选数量、是否使用旧人物/IP 参考图。

图片生成成功后，候选图自动进入素材库。用户可以选择一张作为分镜主素材，也可以排除候选或对单个分镜重新生成。未选中的 AI 图片候选继续保留在素材库，便于后续复用。

图片任务创建成功后，页面必须立即展示批量或单镜头图片任务及其状态。只要当前脚本存在 `pending` 或 `processing` 图片任务，页面就持续刷新任务和候选；任务全部进入终态后停止刷新。失败任务必须显示后端错误，不能只留下空候选区。

失败任务允许操作者在二次确认后从当前素材生成页面清理。清理采用软删除：任务、错误、候选数量、生成参数、结果摘要和费用审计继续保留在数据库，只通过 `dismissed_at` 标记页面隐藏时间。清理不改变任务终态、不删除已生成素材、不调用 Worker 或供应商，也不产生费用。只有 `failed` 任务可清理，当前页面不提供恢复入口。

操作者选择分镜后，中间候选素材区必须在候选列表上方展示当前镜头完整内容，按“旁白”和“画面”左右分栏。左侧分镜列表继续只展示镜头序号、时长和状态，避免长脚本文本挤压分镜导航。

视频任务在人工二次确认前不执行，不产生外部视频生成费用。

## SDD

### 数据库

新增 `scene_asset_candidates`：

- `id`
- `project_id`
- `script_id`
- `scene_id`
- `material_id`
- `candidate_type`
- `source`
- `status`
- `rank`
- `generation_task_id`
- `metadata`
- `created_at`
- `updated_at`

新增 `asset_generation_tasks`：

- `id`
- `project_id`
- `script_id`
- `scene_id`
- `provider`
- `task_type`
- `status`
- `candidate_count`
- `reference_material_ids`
- `params`
- `result`
- `error_message`
- `retry_count`
- `dismissed_at`
- `created_at`
- `updated_at`

约束：

- `candidate_type IN ('image', 'video')`
- `source IN ('existing_material', 'ai_generated', 'video_task')`
- `scene_asset_candidates.status IN ('candidate', 'selected', 'rejected', 'failed')`
- `asset_generation_tasks.provider IN ('gpt-image-2', 'jimeng')`
- `asset_generation_tasks.status IN ('draft', 'pending', 'processing', 'completed', 'failed')`
- 同一 `scene_id` 只能有一个 `selected` 候选。

### 存储

第一版采用本地持久化卷 + API 静态访问前缀。Worker 将图片写入 `/app/storage/assets/generated/images/...`，API 暴露 `/assets/...`。`materials.metadata` 记录 `storage_provider=local`、`source=ai_generated`、`generation_task_id`、`candidate_status`、`source_scene_id` 和 `reference_material_ids`。

Worker 和 API 必须挂载同一个持久化卷。前端收到 `/assets/...` 相对 URL 时，必须按 API `baseUrl` 解析，不能请求工作台自身端口。

### Provider

第一版支持 OpenAI `gpt-image-2` 和即梦。默认 `gpt-image-2`，用户可切换即梦。OpenAI 参数按官方文档实现；即梦字段在实现前必须再以火山引擎官方文档/SDK 或实测确认。

同供应商临时错误最多自动重试 1 次。不得自动跨供应商重试；跨供应商重试必须人工确认。

Worker 后台消费默认关闭，只能通过 `ASSET_GENERATION_WORKER_ENABLED=true` 显式开启。Worker 必须加载独立图片供应商配置；OpenAI 图片地址优先读取 `OPENAI_IMAGE_BASE_URL`，不得把文本 `/responses` 端点直接拼成图片端点。任务已领取后若供应商配置或永久调用错误，Worker 必须把任务和对应失败候选写入终态，不能遗留永久 `processing` 任务。

当 `OPENAI_IMAGE_BASE_URL` 未配置且文本地址以 `/responses` 结尾时，Worker 必须按 OpenAI-compatible 约定推导到 `/v1` API 根路径。图片请求使用与文本客户端一致的兼容 `User-Agent`。鉴权、权限、非法请求等永久错误在首个分镜失败后必须停止剩余分镜调用，并为未执行分镜写入失败候选；不得继续发送同一批次的重复无效请求。

### API

- `POST /api/scripts/:script_id/asset-generation-plan`
- `POST /api/scripts/:script_id/asset-generation-tasks`
- `GET /api/scripts/:script_id/asset-candidates`
- `PUT /api/scenes/:scene_id/asset-candidates/:candidate_id/select`
- `PUT /api/scenes/:scene_id/asset-candidates/:candidate_id/reject`
- `POST /api/scenes/:scene_id/asset-generation-tasks`
- `POST /api/asset-generation-tasks/:task_id/confirm`
- `POST /api/asset-generation-tasks/:task_id/dismiss`

### 失败任务清理与审计

`dismissed_at TIMESTAMPTZ NULL` 是任务从素材生成页面隐藏的唯一事实来源，不改变 `status=failed`。任务列表默认排除 `dismissed_at IS NOT NULL` 的任务；候选列表只排除这些任务关联的 `failed` 候选，已成功生成并入库的素材及非失败候选不得受影响。

`POST /api/asset-generation-tasks/:task_id/dismiss` 必须在单条原子更新中校验任务存在且状态为 `failed`。首次清理写入 `dismissed_at` 并返回 `200 OK`；同一任务重复清理返回既有结果，保持幂等。`draft`、`pending`、`processing`、`completed` 任务必须返回 `409 Conflict`，不得被隐藏。该路由只更新本地数据库，不进入任务队列，不调用 Worker 或供应商。

当前系统没有可信用户身份或鉴权上下文，因此本 change 不新增无法可靠填充的 `dismissed_by`。后续接入统一身份后，应通过独立 change 增加操作者审计，而不是写入伪造或固定用户。

### 运行态数据库迁移

API 的 `connect_runtime_pg_pool` 在返回连接池前执行内嵌 SQLx migrator，再执行依赖业务表的运行态状态同步。这样容器重建后不会出现“新路由已加载但数据库仍缺字段”的半升级状态；任一 migration 失败都会阻止 API 启动，避免把存储错误暴露给用户。

### 单镜头重生费用幂等

单镜头重生采用三层防重，但费用安全必须由服务端保证：

1. 前端使用同步请求锁拦截同一页面同一时刻的快速连点，并为每次明确的用户操作生成 UUID 格式 `Idempotency-Key`。
2. `POST /api/scenes/:scene_id/asset-generation-tasks` 必须校验 `Idempotency-Key`。同一个 key 重试时返回原任务；同一分镜已有 `pending` 或 `processing` 图片任务时，即使来自不同页面、设备或不同 key，也返回该在途任务。
3. 数据库使用部分唯一索引保证同一分镜最多存在一个 `pending/processing + image_candidates` 任务；`asset_generation_task_requests` 永久保存每个到达服务端的 key 与实际返回任务的映射。仓储在分镜级事务锁内执行“按 key 复用、按在途任务复用并补记 key 映射、否则创建”，避免并发检查与插入竞态。

首次创建返回 `201 Created`，复用已有任务返回 `200 OK`。任务进入 `completed` 或 `failed` 后，新的用户操作使用新 key，可以创建下一轮重生任务；所有曾复用该任务的旧 key 迟到重试仍返回旧任务，不得创建新任务。前端只有在收到成功响应后才清除本次 key，响应丢失后的人工重试必须复用原 key。

### 前端

前端实现前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并获得明确确认。页面入口放在脚本详情，不在视频工作台新增大模型管理入口。

失败任务卡片显示“清理失败任务”文字操作。确认弹窗必须说明任务及失败候选将从页面隐藏、数据库继续保留审计、不会调用供应商且不会产生费用。确认成功后同时刷新任务和候选；请求期间锁定该任务的清理操作，失败时保留任务卡片并在原位显示错误。

候选区的镜头上下文取自已加载的 `Scene.narration` 和 `Scene.visual_description`，标题显示镜头序号与时长，正文使用稳定的双栏布局。字段为空时显示明确的“未填写旁白”或“未填写画面”，不能折叠整个上下文区域。

## TDD

后端测试：

- 生成计划校验脚本、分镜、供应商和 48 张上限。
- 创建任务后 API 不等待 worker。
- 未启用供应商不可创建任务。
- 同一分镜只能选中一个候选。
- 归档素材不能新选为主素材。
- 视频任务未确认前不执行。

Worker 测试：

- fake provider 成功生成图片并入库。
- 临时错误最多重试 1 次。
- 永久失败标记任务和候选 failed。
- 下载失败不写入 `materials`。
- 部分成功保留成功候选。

前端测试：

- 展示左分镜、中候选、右设置布局。
- 可选择供应商、候选数和参考图开关。
- 展示旧素材候选与 AI 图片候选。
- 可选择、排除、单分镜重生候选。
- AI 视频显示二次确认入口。
- 图片任务展示批量/单镜头范围、候选数量、状态和错误信息。
- 在途图片任务自动刷新，完成后候选素材自动出现并停止轮询。
- `/assets/...` 候选和素材 URL 使用 API 地址加载。
- 当前镜头内容在候选区上方按旁白、画面双栏展示，左侧分镜列表保持紧凑。
- 失败任务清理需要二次确认，提交期间防重复，成功后同步隐藏任务与关联失败候选。
- 清理失败时保留任务、错误和重试入口，不出现误隐藏。

失败任务清理测试：

- migration 新增可空 `dismissed_at`，不删除或改写历史任务。
- 仅 `failed` 任务可清理，非失败状态返回 `409 Conflict`。
- 重复清理同一失败任务幂等返回，不重复写入、不触发 Worker 或供应商。
- 默认任务列表排除已清理任务，候选列表只排除其关联失败候选。
- 任务错误、候选数量、生成参数、结果摘要和已生成素材在清理后仍可由数据库审计。

运行配置测试：

- 运行态数据库连接会先应用全部待执行 migration，并记录最新版本。
- Worker 健康检查暴露后台消费是否启用。
- 缺少供应商配置或永久错误时，已领取任务进入 `failed`，不会停留在 `processing`。
- Compose 默认不启动计费消费；显式开启后 Worker 和 API 共享素材卷。

E2E：

- 从脚本详情生成素材候选，选择主素材并绑定分镜。

## Prototype Gate

正式前端实现前必须通过 Pencil MCP 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖脚本详情中的素材候选生成与选择流程，并等待用户明确确认。

## Scope Boundary

本 change 不做：

- 作品生产和成片生成。
- 发布运营和平台分发。
- 后台大模型管理页面。
- 自动跨供应商重试。
- 未确认的视频生成扣费执行。
- MinIO/S3 管理后台。
- 移动端适配。
- 失败任务恢复入口。
- 在统一身份系统落地前记录 `dismissed_by`。
