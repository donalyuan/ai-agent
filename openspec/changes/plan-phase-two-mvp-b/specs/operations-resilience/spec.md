## MODIFIED Requirements

### Requirement:手工备份与恢复 runbook
系统 SHALL 提供版本化手工和自动备份 runbook，分别记录 PostgreSQL backup/restore、object manifest/reference inventory、Compose configuration、Docker Secret keyring 和 object-storage credential reference 的前置检查、fingerprint、恢复顺序、权限、失败保留、回滚和 operator UUID。自动任务 MUST 使用 operation group 和保留策略；runbook MUST NOT 保存 secret、token 或私有凭据值，且恢复不得绕过隔离校验、checksum/ETag gate 或用户确认。

#### Scenario:自动备份按策略保留
- **WHEN** scheduler 创建备份 operation group
- **THEN** 生成带 manifest/hash/retention 的 artifact，容量 admission 失败时保持 blocked，不删除长期审计事实

#### Scenario:恢复缺少校验时阻断
- **WHEN** 任一必需 backup artifact、权限、manifest revision、checksum 或 ETag 缺失
- **THEN** 恢复保持 blocked，记录 operator/correlation，零 current reference、ExportArtifact 或成功状态写入
