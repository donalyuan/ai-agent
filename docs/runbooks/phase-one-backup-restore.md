# 阶段一手工备份与恢复

版本：`1.0.0`。本 runbook 仅用于明确授权的本地/演练环境，不自动执行恢复，也不保存任何 secret、token 或 credential 值。

## 前置检查

1. 记录稳定 operator UUID、correlation ID、环境名称、当前 Alembic head、Compose 配置 fingerprint 和 maintenance window。
2. 确认 PostgreSQL logical backup 目标、object manifest inventory、Docker Secret keyring reference、Storage credential reference 均可读且权限最小化。
3. 记录每个 artifact 的大小、SHA-256、manifest revision、object checksum/ETag；凭据只记录 owner reference 与 masked status。
4. 任一 artifact、权限、revision 或 reference 缺失时，将恢复状态保持 `blocked`，不得解除 resource admission。

## 备份

1. 使用项目当前 PostgreSQL 工具版本生成 logical backup；记录命令版本、开始/结束时间、退出码和 backup fingerprint。
2. 导出 Storage owner 的 object manifest/reference inventory，不下载或复制非演练所需媒体。
3. 保存已脱敏的 Compose configuration fingerprint；Docker Secret 只记录 key version/reference，密钥本体由授权设施独立备份。
4. 保存 Alembic head、owner row counts 和长期事实基线：`RunEvent`、`AcceptDecision`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要、仍被引用的 `AssetVersion`。

## 恢复顺序

1. 新建隔离恢复环境，保持 Provider/TOS/FFmpeg 为 `unconfigured`，禁止 live operation。
2. 恢复 Compose 配置与 Docker Secret keyring reference，再恢复 PostgreSQL metadata。
3. 校验 Alembic head、owner constraints、row counts 和 object manifest revision。
4. 对演练对象执行 exact checksum/ETag/ownership 对比；仅全部一致时记录 `passed`。缺失、foreign、revision drift 或 mismatch 记录 `failed` 并保持 blocked。
5. 运行 owner read probes 与 Mock/Local smoke；不得以 telemetry、截图或最终状态替代 owner evidence。
6. 只有所有 gate 通过后才由授权 operator 显式解除演练 admission。

## 失败与回滚

- 保留原始退出码、稳定 diagnostic、expected/observed checksum/ETag、manifest revision 和 correlation；不得覆盖原备份。
- 不写 current reference、ExportArtifact 或成功恢复状态，不自动切换 Storage adapter/profile，不自动清理对象。
- 删除隔离恢复环境前保留脱敏 evidence；生产/共享环境保持原状态。
- cleanup/GC 必须跳过长期事实和仍被引用的 `AssetVersion`，只有证明无引用且满足 policy 的 temporary object 可清理。
