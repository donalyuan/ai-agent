# Video Agent

`apps/video-agent` 是 Novex AI Agent 基座中的首个业务应用，负责视频内容生产链路：

```text
选题 -> 脚本 -> 素材匹配 -> 视频生成 -> 发布分发 -> 数据回流 -> 策略优化
```

当前已验证的 `script-agent-mvp` 能力保留为该应用的初始业务能力。本目录已经承载 `VEDIO-AGENT / 视频工作台` 的正式 Next.js 前端入口，当前实现“脚本创作”菜单下的项目选择、脚本生成、时间轴对照详情和状态更新闭环。

## 菜单控制

视频工作台一级导航由后端接口 `GET /api/video-workspace/menus` 返回，数据来源为 PostgreSQL 表 `video_workspace_menus`。前端不再硬编码 6 个 Agent 作为一级导航；内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务由数据库控制排序、可见性、启用状态和模块归属。

## 当前实现归属

- Rust 控制面 API 暂位于 `backend/`，后续可复用能力逐步抽入 `crates/*`。
- 视频业务上下文、产品说明和正式视频生产工作台前端归属本目录。
- Python 视频运行时归属 `services/video-worker/`。
- `admin/` 只承载 Novex 平台管理后台，不作为视频生产工作台入口。

## 运行入口

统一从 `/server/docker-compose.yml` 启动：

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-api ai-agent-video-worker ai-agent-video-agent
```

- 服务名：`ai-agent-video-agent`
- 宿主机端口：`18183`
- 容器端口：`3000`
- 本地访问：`http://127.0.0.1:18183`

## 本地验证

```bash
npm run test
npm run lint
npm run build
npm run test:e2e
```
