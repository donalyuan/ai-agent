# Tasks

## 1. 规格与记忆修正

- [ ] 1.1 更新 `MEMORY.md`，明确正式视频生产工作台归属 `apps/video-agent/`。
- [ ] 1.2 更新 `docs/memory/project-tech-stack.md`，区分 `admin/` 平台管理后台和 `apps/video-agent/` 视频生产工作台。
- [ ] 1.3 更新 `README.md` 和 `apps/video-agent/README.md`，说明两个前端入口的职责、端口和验证命令。
- [ ] 1.4 更新 `DESIGN.md`，将“智能体工作台”的实现边界从 `admin/` 修正为 `apps/video-agent/`。

## 2. 建立 video-agent 前端应用

- [ ] 2.1 在 `apps/video-agent/` 下建立 Next.js + TypeScript 前端应用结构。
- [ ] 2.2 迁移或复用当前 `admin/` 中的工作台页面、样式、API client、测试 setup 和配置。
- [ ] 2.3 保持 `AI-AGENT` 品牌、“智能体工作台”中文标题、六个智能体菜单入口和桌面端布局。
- [ ] 2.4 保持脚本智能体详情为“时间轴对照视图”。
- [ ] 2.5 保持分镜数选择范围为 3 到 12。
- [ ] 2.6 保持项目选择能力，但不得把项目创建或项目管理入口放入脚本生产工作台。

## 3. 收敛 admin 边界

- [ ] 3.1 从 `admin/` 的正式首屏移除脚本生产工作台入口。
- [ ] 3.2 将 `admin/` 调整为平台管理后台入口或保留明确的管理占位页。
- [ ] 3.3 确认 `admin/` 不展示“生成脚本”“时间轴对照视图”等视频生产流程。
- [ ] 3.4 确认平台管理能力不迁入 `apps/video-agent/`。

## 4. Compose 与运行入口

- [ ] 4.1 在项目 Compose 中增加 `apps/video-agent` 前端服务，例如 `ai-agent-video-agent`。
- [ ] 4.2 为新服务分配独立宿主机端口，并在 README 和记忆中同步。
- [ ] 4.3 保持 `/server/docker-compose.yml` 作为统一入口。
- [ ] 4.4 确认 `ai-agent-admin` 和 `ai-agent-video-agent` 服务名、端口、构建上下文清晰区分。

## 5. 测试与验证

- [ ] 5.1 在 `apps/video-agent` 中迁移或新增 Vitest 单测。
- [ ] 5.2 在 `apps/video-agent` 中迁移或新增 Playwright E2E，覆盖桌面端工作台关键约束。
- [ ] 5.3 运行 `openspec validate realign-video-agent-workspace-boundary --json`。
- [ ] 5.4 运行 `apps/video-agent` 前端单测、lint、build 和 E2E。
- [ ] 5.5 运行 `admin` lint/build，确认管理后台仍可构建。
- [ ] 5.6 检查相关容器日志无 `Cannot find module`、`Server Error`、`Error:`、panic 或异常堆栈。
