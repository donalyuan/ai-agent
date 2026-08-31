## MODIFIED Requirements

### Requirement:R4 六个业务 Port
业务领域 SHALL 仅通过 `TextModelPort`、`ImageGenerationPort`、`VideoGenerationPort`、`TtsPort`、`AsrPort` 与 `StoragePort` 发起模型或存储副作用。Port SHALL 定义可测试的输入、结果、错误和关联标识边界，业务服务不得直接依赖供应商 SDK。`StoragePort` MUST 提供有项目/profile/key/operation scope 的 multipart `create/resume/uploadPart/complete/abort`、`presignRead/presignWrite`、`stat`、`uploadFromWorkspace/downloadToWorkspace` 与受引用证明保护的 `delete`；它返回 immutable storage reference 和 metadata，不返回或持久化媒体 bytes。项目资产中心通用上传 MUST 由 Assets owner 的 `AssetVersionReservation` 发起，并在全部 StoragePort 操作中复用 `operationKey=asset-upload:{projectId}:{assetId}:{reservationId}`；StoragePort MUST NOT 登记 AssetVersion。

#### Scenario:以 Mock 替换 Provider
- **WHEN** 测试向业务服务注入任一模型 Port 的 Mock 实现
- **THEN** 服务通过统一结果完成调用路径，且测试不加载供应商 SDK 或网络凭据

#### Scenario:media consumer uses the complete StoragePort contract
- **WHEN** GPT Image、Agnes 或 FFmpeg consumer 需要上传、恢复、读取、下载或删除对象
- **THEN** 它只通过包含 project/profile/operation scope 的 StoragePort 操作，且不会直接调用 TOS SDK 或拥有 AssetVersion、ProviderCall、RunEvent、ExportJob

#### Scenario:asset center upload preserves owner handoff
- **WHEN** project asset center 使用 reservation 创建、恢复、取消或 reconcile 通用上传
- **THEN** StoragePort 始终复用 reservation 派生的固定 operation key，只返回 session/object facts；AssetVersion registration 仍由 Assets owner 显式完成

### Requirement:R6 Mock Provider 和失败可见性
阶段 0/阶段 1 默认测试 SHALL 提供可预测、无网络和无费用的 `Mock Provider` 与显式选择的 `Local test/offline profile`（adapter identity=`local_workspace`），覆盖六个 Port 的基础成功与可识别错误路径。真实 Provider/TOS 配置缺失、禁用、不支持、凭据/主密钥缺失、scope/validation failure 或不可恢复 multipart conflict 时 SHALL 返回显式可诊断结果；TOS timeout/connection/安全可重试远程失败 SHALL 返回 retryable diagnostic 和稳定 operation/session correlation。所有上述情况 MUST NOT 回退到其他 adapter、隐式真实服务或伪造成功；运行开始后 Adapter/Profile 选择冻结。

#### Scenario:缺少真实 Provider 或 TOS 配置
- **WHEN** 运行模式请求未配置的真实适配器
- **THEN** 调用返回可诊断的未配置结果，且日志和网络记录不显示真实外部请求

#### Scenario:Local 与 TOS 之间不隐式 fallback
- **WHEN** Local operation 或 explicit TOS operation 失败
- **THEN** 系统保留该 adapter 的原始可诊断错误；不得选择另一个 adapter、创建替代对象或报告成功

### Requirement:R6 LocalWorkspaceAdapter
`LocalWorkspaceAdapter` SHALL 仅在显式选择的开发、离线或测试 profile 中实现 `StoragePort` 的对象操作，并把所有文件限制在配置的工作区根目录内；profile 的 adapter identity 固定为 `local_workspace`。它 MUST 支持与 TOS contract 同等的 project-scoped multipart、presign、`stat`、workspace transfer 与 delete proof 输入的可测试语义；持久化契约 SHALL 保存抽象对象引用和元数据，不得保存宿主绝对路径；路径逃逸、非法 canonical key、跨项目 scope、过期 presign、part/complete conflict 和未经证明 delete SHALL 被拒绝。它不得因真实 TOS 未配置而伪装为 TOS 成功，也不得被作为 TOS 失败 fallback。

#### Scenario:写入并读取工作区测试对象
- **WHEN** 测试将对象写入配置的 LocalWorkspace 根目录再读取它
- **THEN** 返回的对象标识可被解析，且持久化引用不暴露绝对路径

#### Scenario:拒绝工作区外路径
- **WHEN** 调用方提供试图离开工作区根目录的对象路径
- **THEN** adapter 拒绝操作且不创建范围外文件

#### Scenario:Local contract exposes no real TOS success
- **WHEN** 测试环境未选择 enabled TOS profile
- **THEN** Local 仅以 `local_workspace` adapter identity 返回结果；明确 TOS request 仍返回 `unconfigured`，而非把 Local result 标记为 TOS

## DDD / BDD / SDD / TDD

- **DDD**：StoragePort 是外部副作用边界，storage 的 object/session 不拥有 consumers 或 AssetVersion。
- **BDD**：上述场景保持 `Mock Provider +` 显式 Local test/offline profile 的成功和路径逃逸负例，并新增 multipart/presign/delete/fallback 可观察行为。
- **SDD**：此 delta 扩展 Port API、错误/关联字段和 Local compatibility，不改动 AssetVersion HTTP/schema_version 或 objectKey 规则。
- **TDD**：先更新 Port contract tests 和现有 runtime composition tests；真实 TOS adapter 由独立 opt-in probe 覆盖。

## Current / Defined / Todo

- **Current**：阶段 0 Local 接口较窄，TOS 为 explicit-unconfigured，已有 runtime test 拒绝未知 storage mode。
- **Defined**：完整 StoragePort 和无 fallback 语义。
- **Todo**：实现 v2 DTO/适配器兼容层、更新 consumers 和运行分层测试。

## Dependencies and Acceptance Commands

依赖 `tos-storage-provider` new capability、已归档 AssetVersion/objectKey contract 和现有 runtime composition tests。验收运行相关 port/runtime/adapter tests、`openspec instructions apply --change integrate-tos-storage-provider --json`、`openspec validate integrate-tos-storage-provider --strict --json`、`pnpm run check` 与 `git diff --check`；真实 TOS 不属于默认测试。
### Requirement:Storage uses the shared credential boundary
TOS MUST 消费 catalog CredentialResolver，且 MUST NOT 持久化第二套 cipher/keyring。real profile 缺少 master key 时返回 503 `credential_master_key_unavailable`；`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）保持可用。

#### Scenario:Credential failure does not leak storage secrets
- **WHEN** resolution 或 re-encryption 失败
- **THEN** 不暴露或用于 fallback 的 key material、objectKey 或 workspace URI。
