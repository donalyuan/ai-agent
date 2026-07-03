## Overview

本 change 修正前端代码归属：`VEDIO-AGENT / 视频工作台` 是视频内容生产业务工作台，正式承载位置应为 `apps/video-agent/`。`admin/` 保留为平台控制面管理后台，不继续承载脚本生产、素材生产、视频生成、发布排期等日常生产流程。

这不是废弃当前页面，而是把当前 `admin/` 中已确认的桌面工作台、蓝色视觉风格、六个智能体菜单、脚本生成表单、分镜时间轴对照视图和 Playwright 验证，迁移到正确的业务应用边界。

## Boundary

### `apps/video-agent/`

`apps/video-agent/` 承载视频内容生产业务应用。正式工作台应包含并持续扩展以下流程：

1. 选题智能体：选题池、热点分析、选题评分、选题确认。
2. 脚本智能体：脚本生成、分镜确认、版本对比、状态流转。
3. 素材智能体：素材候选、语义匹配、素材替换、素材清单确认。
4. 视频智能体：模板选择、配音字幕、视频生成、成片预览。
5. 发布智能体：平台选择、标题封面标签、发布时间、发布状态。
6. 优化智能体：数据回流、评论反馈、策略优化建议。

当前实现阶段只要求迁移并保持脚本智能体闭环，其他五个智能体继续保留入口和布局位置。

### `admin/`

`admin/` 只承载平台管理后台。适合保留或新增的能力包括：

1. 用户、角色、权限。
2. 模型供应商、模型路由、API Key 配置。
3. Agent 配置、Prompt 模板、工具和 MCP 管理。
4. Worker、任务队列、日志、审计、成本、限额、健康检查。
5. 平台账号、系统级项目配置、环境诊断。

`admin/` 不应继续作为视频生产工作台的正式入口。

## Migration Approach

推荐采用“先复制验证，再收敛旧入口”的迁移方式：

1. 在 `apps/video-agent/` 下建立 Next.js + TypeScript 前端应用，复用当前 `admin/` 的页面组件、API client、样式、Vitest 单测和 Playwright E2E。
2. 为新应用增加 Compose 服务，例如 `ai-agent-video-agent`，分配独立宿主机端口。
3. 将 `VEDIO-AGENT / 视频工作台` 首屏迁入 `apps/video-agent`，并保持桌面端原型确认的视觉和交互。
4. 将视频工作台 Pencil 原型源文件固定为 `docs/prototypes/video-agent/video-agent.pen`，后续原型修改都更新该文件，不再使用 `docs/prototypes/script-agent-workspace/` 截图目录。
5. 修改测试，使正式 E2E 针对 `apps/video-agent` 运行，并继续覆盖六个智能体菜单、分镜数 3-12、无项目管理入口、时间轴对照视图。
6. 将 `admin/` 回退或改造成平台管理入口，不再展示脚本生产页面。
7. 更新 README、`MEMORY.md`、`docs/memory/project-tech-stack.md` 和 OpenSpec 主规格。

## Risks

### Compose 和端口混乱

新增 `apps/video-agent` 前端服务后，`admin` 和 `video-agent` 都可能是 Next.js 应用。需要在 Compose、README 和测试命令里明确服务名和端口，避免开发者进入错误页面。

### 复制后双份业务页面分叉

迁移时如果保留 `admin` 里的同一套生产页面，会导致后续修改分叉。验收必须要求 `admin` 不再作为脚本生产工作台入口。

### 误把管理功能搬到业务应用

迁移只针对视频生产工作台。模型配置、MCP 管理、任务队列、日志、权限等平台管理能力仍归属 `admin`。

## Test Strategy

1. OpenSpec：`openspec validate realign-video-agent-workspace-boundary --json`。
2. 前端单测：在 `apps/video-agent` 中运行 Vitest，覆盖 API client 和工作台页面行为。
3. E2E：在 `apps/video-agent` 中运行 Playwright，覆盖桌面端生产工作台关键路径。
4. 构建：`apps/video-agent` 和 `admin` 都必须通过 lint/build。
5. Compose：从 `/server/docker-compose.yml` 启动时，新业务工作台服务和 `admin` 服务必须都可识别，且端口说明准确。
