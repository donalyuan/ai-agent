# Video Agent

`apps/video-agent` 是 Novex AI Agent 基座中的首个业务应用，负责视频内容生产链路：

```text
选题 -> 脚本 -> 素材匹配 -> 视频生成 -> 发布分发 -> 数据回流 -> 策略优化
```

当前已验证的 `script-agent-mvp` 能力保留为该应用的初始业务能力。恢复业务开发前，应先完成 `align-novex-foundation-architecture`，再继续 `script-agent-mvp` 后续任务。

## 当前实现归属

- Rust 控制面 API 暂位于 `backend/`，后续可复用能力逐步抽入 `crates/*`。
- 视频业务上下文、产品说明和后续应用前台入口归属本目录。
- Python 视频运行时归属 `services/video-worker/`。
