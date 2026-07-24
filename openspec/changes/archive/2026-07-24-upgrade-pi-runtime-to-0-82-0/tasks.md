## 1. 发布与升级前证据

- [x] 1.1 确认三个 npm 包共同发布 `0.82.0`、GitHub `v0.82.0` 存在，并记录 exports、Node engine、Harness breaking change、tool factory 与 SQLite 差异结论

## 2. 依赖与失败基线

- [x] 2.1 在隔离目录生成并审核三包 `0.82.0` 的精确 manifest/lockfile，校验原文件 SHA-256 后应用到 Runtime
- [x] 2.2 在 Node.js 24 Docker build 中记录旧 `env + AgentTool` 装配面对 `0.82.0` 的预期类型失败，确认破坏点与上游 changelog 一致

## 3. Harness 兼容适配

- [x] 3.1 扩展 fake-provider Tool Loop 测试，以 `write -> edit(old_text/new_text)` 锁定升级前工具协议与 SSE/持久化行为
- [x] 3.2 将 Runtime 迁移到 `ExecutionToolContext + AgentHarnessTool`，保留 Novex 自有四工具 schema 和行为

## 4. Runtime 容器门禁

- [x] 4.1 在 Node.js 24 Docker 中通过 clean install、build、lint、全量 fake-provider 测试和 `npm audit --audit-level=high`

## 5. SQLite 与 Compose 重建

- [x] 5.1 记录升级前 Session 集合，停止 Runtime，创建并验证 SQLite source volume 的带时间戳 backup volume
- [x] 5.2 Compose 强制重建 Runtime，验证 health/ready、Session 集合，并再次重启确认持久化恢复

## 6. 跨服务回归

- [x] 6.1 检查测试无真实外部调用后，通过 Rust workspace 与 Video Worker 全量本地回归

## 7. 项目事实同步

- [x] 7.1 更新 memory、ARCHITECTURE 和 README 中直接相关的 Pi 版本与 Harness 适配决策

## 8. OpenSpec 收口

- [x] 8.1 复核依赖、代码、备份和测试证据，运行 strict validation 与 apply instructions 并确认 `all_done`
