## 1. OpenSpec 与测试先行

- [x] 1.1 补齐 `bootstrap-project` 的 proposal、design、spec 与 tasks artifacts
- [x] 1.2 为 Rust API 新增 `/health` 失败测试
- [x] 1.3 为 Python Worker 新增 `/health` 失败测试

## 2. 最小服务骨架

- [x] 2.1 实现 Rust/Axum API 最小服务与 `/health`、`/ready`
- [x] 2.2 实现 Python/FastAPI Worker 最小服务与 `/health`
- [x] 2.3 实现 Next.js 前端最小页面

## 3. Docker Compose 环境

- [x] 3.1 新增 `/server/video-agent/docker-compose.yml` 并复用 `biga-postgres` 与 `bs-redis`
- [x] 3.2 将 `/server/video-agent/docker-compose.yml` 加入 `/server/docker-compose.yml` 顶层 include
- [x] 3.3 增加幂等数据库初始化服务创建 `video_agent`

## 4. 文档与验证

- [x] 4.1 更新 README 与 CLAUDE 中经验证的环境命令
- [x] 4.2 验证顶层 Compose 能识别 video-agent 服务
- [x] 4.3 启动 API、Worker、Web 并验证健康检查
- [x] 4.4 运行 Rust API 与 Python Worker 相关测试
- [x] 4.5 运行 OpenSpec 校验并确认 tasks 状态
