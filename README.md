# 视频 Agent 工作台

**已实现：阶段 0**：本地优先工程基线（React 工作台壳层、FastAPI、PostgreSQL、Temporal、三类 Worker、共享 JSON Schema、Mock Provider 与 LocalWorkspaceAdapter），并已完成首个 `projects/episodes` 业务切片。默认不调用真实 Provider 或 TOS。

**已定义：接口/目标架构**：模块化单体、Ports/Adapters、分层依赖与后续切片边界已由 ADR-0003 和技术架构文档定义。

**待实现：产品能力**：真实 Provider/TOS、AgentScope、FFmpeg、完整生成/音频/媒体链路、专业剪辑、协作、移动端和平台发布。

## 前置条件

- Node.js 22.12+ 与 pnpm 10.19
- Python 3.12 与 uv
- Docker Compose（运行完整本地环境时需要）

## 常用命令

```sh
pnpm install --frozen-lockfile
uv sync --project services/api --frozen --all-groups
pnpm run check
pnpm run compose:up
pnpm run compose:down
```

服务仅绑定 `127.0.0.1`。运行后 Web 位于 `http://127.0.0.1:5174`，API health 位于 `http://127.0.0.1:8000/v1/health/ready`；`projects/episodes` API 位于同一 API 服务的 `/v1/projects`、`/v1/projects/{project_id}/episodes` 等路径。
