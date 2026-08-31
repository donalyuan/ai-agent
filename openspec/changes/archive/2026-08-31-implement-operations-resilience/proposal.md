## Why

MVP-A 需要在本地优先运行中可观察地保护 CPU、内存、磁盘/容量与可恢复数据，避免 Local workspace、Worker 临时文件、数据库备份元数据和对象存储 manifest 在资源压力、能力缺失或恢复错误时产生伪成功。PRD 已要求资源/capability 预检、软/硬阈值、手工备份/恢复 runbook 与一次 checksum/ETag 恢复演练，这些职责跨越多个 owner，不能隐藏在 TOS adapter 内。

## What Changes

- 新增跨边界 operations resilience contract：CPU、内存、磁盘/容量的启动/运行期 probe，soft/hard threshold、阻断上传/新生成/预览/导出和稳定诊断。
- 定义 Local workspace、Worker temporary/derivative、数据库 backup metadata 与 object-storage manifest/reference 的责任交接；不拥有 TOS profile、bucket、credential 或 StoragePort。
- 新增手工备份/恢复 runbook，覆盖 PostgreSQL、manifest/版本引用、Compose 配置、Docker Secret 主密钥和对象存储凭据的分项备份、恢复前检查、失败保留和回滚语义。
- 通过一次可重复的 checksum/ETag 恢复演练验证对象引用、数据库记录和诊断的一致性；校验失败必须拒绝恢复成功。
- 为阈值拒绝、诊断、零副作用、重启恢复和演练结果补充正反测试与 E2E-MVPA-001 evidence。

## Capabilities

### New Capabilities

- `operations-resilience`: 跨 Local、Worker、数据库和对象存储的资源/capability 预检、容量保护、手工备份/恢复和校验演练合同。

### Modified Capabilities

- 无。TOS `StoragePort`/`StorageProfile` 行为保持由 `integrate-tos-storage-provider` 拥有。

## Impact

后续实现会影响 `services/api` 的 runtime/diagnostics 与 backup metadata、`workers/*` 的 workspace/temporary cleanup、`infra/compose` 的 health/config 入口、对象存储 manifest/reference 适配和 E2E/BDD/TDD fixtures。不会实现自动化备份、恢复 UI、portable 工程包回导或第二套 TOS adapter。
