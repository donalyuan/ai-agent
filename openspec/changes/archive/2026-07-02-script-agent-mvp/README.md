# script-agent-mvp

脚本Agent MVP实现：从选题生成结构化视频脚本

## 当前状态

`align-novex-foundation-architecture` 已归档，本 change 可在 Novex 基座结构下恢复开发。已完成的数据库迁移、脚本领域模型、请求/响应模型、Repository trait 和 PostgreSQL Repository 保留在 Novex 新结构中：

- Rust 控制面与已实现脚本 Agent 代码：`backend/`
- video-agent 业务应用入口与后续说明：`apps/video-agent/`
- 后续可复用 AI 能力抽取目标：`crates/*`

恢复本 change 时，继续从 T2.3 开始，并以 Novex 基座目录结构为准。
