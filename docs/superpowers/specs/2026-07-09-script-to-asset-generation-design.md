# Script To Asset Generation Design

## Goal

建设素材 Agent 的第一版脚本到素材候选链路：从已生成脚本分镜出发，优先复用素材库中可用旧素材，缺口处自动生成 AI 图片多候选，AI 视频只创建待人工二次确认的任务草稿。用户最终在脚本详情页选择每个分镜的主素材。第一版不进入作品生产、不生成成片、不建设后台大模型管理页面。

## DDD

`Material` 仍是素材库聚合根，旧素材和 AI 生成素材都进入 `materials`。AI 生成图片必须先下载到自管素材存储，再将稳定 URL 写入 `materials.file_url`，不得只保存供应商临时 URL。

新增 `SceneAssetCandidate` 表达分镜候选素材。候选可以来自旧素材复用，也可以来自 AI 图片生成。候选状态为 `candidate`、`selected`、`rejected`、`failed`；同一分镜最多只能有一个 `selected` 候选。

新增 `AssetGenerationTask` 表达素材生成任务。它不同于现有 `generation_tasks` 视频作品任务，第一版建议独立建表，避免放宽现有 `provider IN ('runway','kling')` 和 `task_type IN ('text_to_video','image_to_video')` 约束。图片任务可自动执行；视频任务只能进入 `draft`，必须人工二次确认后才进入执行态。

供应商配置属于平台控制面。视频工作台只消费已启用供应商、默认模型和限额配置；后续“大模型管理”功能归 `admin/`，不放在视频工作台。

## BDD

运营人员在脚本详情页点击“生成素材候选”。系统读取脚本分镜，先查当前账号素材库中的可用素材，优先推荐人物、固定 IP、常用场景等旧素材。已登记、可用、非归档的旧人物/IP 素材也可作为 AI 图片生成参考图。

图片缺口自动生成候选图。默认每个分镜生成 3 张，可在生成前调整为 1-4 张。每次脚本批量生成最多 12 个分镜 × 4 张 = 48 张，超过上限直接拒绝并提示减少分镜或候选数。

AI 视频缺口只创建待确认任务。页面展示视频任务的二次确认入口，用户确认后才允许启动外部视频生成。

页面布局采用“左侧分镜列表 / 中间候选素材 / 右侧生成设置与任务”。旧素材候选和 AI 图片候选分区展示，右侧承载供应商、候选数量、参考图开关、生成按钮和 AI 视频二次确认入口。用户可以选择某张候选作为分镜主素材，也可以排除候选或对单个分镜重新生成候选。

单镜头重生属于可产生外部图片费用的明确操作。前端必须使用同步请求锁拦截快速连点，并为每次用户操作生成 UUID 格式 `Idempotency-Key`；服务端必须在分镜级事务锁内复用相同 key 或同分镜 `pending/processing` 任务，数据库通过部分唯一索引兜底，并用 `asset_generation_task_requests` 永久记录每个请求 key 实际返回的任务。任务终态后，新 key 才代表新的重生操作；前端未收到成功响应时必须保留原 key 用于人工重试。

未被选中的 AI 图片候选也进入素材库，但必须标记 `ai_generated`、未选候选和来源分镜，便于后续复用并避免重复付费生成。

## SDD

### Data Model

新增 `scene_asset_candidates`：

- `id`
- `project_id`
- `script_id`
- `scene_id`
- `material_id`
- `candidate_type`: `image | video`
- `source`: `existing_material | ai_generated | video_task`
- `status`: `candidate | selected | rejected | failed`
- `rank`
- `generation_task_id`
- `metadata`
- `created_at`
- `updated_at`

约束：

- `scene_id` 必须归属 `script_id`。
- `material_id` 非空时必须归属同一 `project_id`。
- 同一 `scene_id` 只能有一个 `selected` 候选。
- `archived` 素材不得被新选为分镜主素材。

新增 `asset_generation_tasks`：

- `id`
- `project_id`
- `script_id`
- `scene_id`
- `provider`: `gpt-image-2 | jimeng`
- `task_type`: `image_candidates | video_draft | video_generation`
- `status`: `draft | pending | processing | completed | failed`
- `candidate_count`
- `reference_material_ids`
- `params`
- `result`
- `error_message`
- `retry_count`
- `created_at`
- `updated_at`

### Storage

第一版自管存储采用本地持久化卷 + API 静态访问前缀。AI 图片生成后由 worker 下载到类似 `/app/storage/assets/generated/images/...` 的可写目录，并通过 `/assets/...` 暴露稳定访问。`materials.metadata` 至少记录：

- `storage_provider=local`
- `source=ai_generated`
- `generation_task_id`
- `candidate_status`
- `source_scene_id`
- `reference_material_ids`

后续可将存储 adapter 替换为 MinIO/S3，不改变 `materials.file_url` 作为稳定访问 URL 的业务语义。

### Providers

第一版真实图片生成供应商为 OpenAI `gpt-image-2` 与即梦。默认使用 `gpt-image-2`，生成前可切换即梦。OpenAI 接口口径按官方文档核对；即梦接口字段在实现前必须再次以火山引擎官方文档/SDK 或实测为准。

失败不得自动跨供应商重试；跨供应商重试必须人工确认。同供应商临时错误最多自动重试 1 次。

### API

- `POST /api/scripts/:script_id/asset-generation-plan`
- `POST /api/scripts/:script_id/asset-generation-tasks`
- `GET /api/scripts/:script_id/asset-candidates`
- `PUT /api/scenes/:scene_id/asset-candidates/:candidate_id/select`
- `PUT /api/scenes/:scene_id/asset-candidates/:candidate_id/reject`
- `POST /api/scenes/:scene_id/asset-generation-tasks`
- `POST /api/asset-generation-tasks/:task_id/confirm`

`asset-generation-plan` 返回分镜数、图片候选数、视频待确认数、供应商、参考素材数量、是否超过上限和风险提示。创建任务 API 不同步等待外部生成。

单镜头重生接口首次创建返回 `201 Created`，相同 key 重试或复用同分镜在途任务返回 `200 OK`。相同 key 即使在任务终态后迟到重试，也返回原任务；任务终态后使用新 key 才创建新任务。

### Worker

`services/video-worker` 领取图片生成任务，调用供应商，下载图片，写本地存储，创建 `materials`，并回写 `scene_asset_candidates`。部分成功允许保留：成功图片入素材库，失败分镜显示失败状态并允许单独重生。下载失败时该候选失败，不写入 `materials`。

### Frontend

正式前端开发前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并获得用户明确确认。正式页面在脚本详情内新增素材候选区，不新增独立大模型管理入口。

## TDD

后端：

- 创建生成计划时校验脚本、分镜、供应商和 48 张上限。
- 未启用供应商不可创建任务。
- 创建任务后 API 不等待 worker。
- 同一分镜只能选中一个候选，重新选择会取消旧选中。
- 归档素材不能新选为主素材。
- AI 视频任务未确认前不执行。

Worker：

- fake provider 覆盖成功、临时错误重试、永久失败。
- 同供应商临时错误最多重试 1 次。
- 跨供应商不自动重试。
- 下载成功后写入本地存储并创建 `materials`。
- 下载失败不写入 `materials`。
- 部分成功时成功候选保留，失败候选标记 failed。

前端：

- 展示左分镜、中候选、右设置布局。
- 可选择供应商、候选数和参考图开关。
- 展示旧素材候选与 AI 图片候选分区。
- 可选择、排除、单分镜重生候选。
- AI 视频显示二次确认入口。

E2E：

- 从脚本详情生成素材候选，看到旧素材优先、AI 图片候选、选择主素材并绑定分镜。

常规验证：

- `docker exec ai-agent-api cargo fmt -- --check`
- `docker exec ai-agent-api cargo test`
- `docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings`
- `docker exec ai-agent-video-agent npm run lint`
- `docker exec ai-agent-video-agent npm run test`
- `docker exec ai-agent-video-agent npm run build`
- `docker exec ai-agent-video-agent npm run test:e2e`
- `openspec validate --all`

## OpenSpec Plan

新建 OpenSpec change：`script-to-asset-generation`。

Artifacts：

- `openspec/changes/script-to-asset-generation/proposal.md`
- `openspec/changes/script-to-asset-generation/design.md`
- `openspec/changes/script-to-asset-generation/specs/script-to-asset-generation/spec.md`
- `openspec/changes/script-to-asset-generation/specs/material-library-management/spec.md`
- `openspec/changes/script-to-asset-generation/tasks.md`

实现过程中，代码改动必须与 `tasks.md` 同步。完成后执行 `openspec instructions apply --change "script-to-asset-generation" --json` 并确认状态。

## Scope Boundary

本次不做：

- 作品生产和成片生成。
- 发布运营和平台分发。
- 后台大模型管理页面。
- 自动跨供应商重试。
- 无确认的视频生成扣费执行。
- 完整对象存储/MinIO/S3 管理后台。
- 移动端适配。
