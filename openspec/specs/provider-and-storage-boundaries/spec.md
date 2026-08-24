# provider-and-storage-boundaries Specification

## Purpose
TBD - created by archiving change establish-phase-zero-foundation. Update Purpose after archive.
## Requirements
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

### Requirement: R4 数据驱动的 Provider/Profile/Model
系统 SHALL 将 Provider、Profile 和 Model 的名称、adapter key、启用状态、认证引用、超时、默认参数和参数 Schema 存在配置或持久化模型中。业务代码 SHALL NOT 硬编码 model、`base_url`、bucket 或 region；新增同协议模型 SHALL 通过配置而非修改业务流程选择。

#### Scenario: 选择配置的模型
- **WHEN** 测试为 Profile 配置一个启用的 Model 与默认参数
- **THEN** Port 调用接收该配置选择结果，而不是业务代码中的固定模型名

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

### Requirement: Multipart operation reuse is fully bound
Reusing a multipart `operation_key` SHALL compare project, profile, canonical object key, expected size, checksum and MIME against the persisted session before returning a session reference or completing it.

#### Scenario: Cross-project reuse is rejected
- **WHEN** a second project reuses an existing operation key with any differing frozen field
- **THEN** storage rejects the request and never returns a session pointing at the first project's object

### Requirement: Object keys use the canonical contract
Storage intent and reference validation SHALL call the shared `canonical_object_key()` contract and reject dot segments, empty segments, trailing slash, query and fragment delimiters, schemes, absolute paths and backslashes.

#### Scenario: Non-canonical key is rejected
- **WHEN** a caller supplies `projects/p/a/./file`, `projects/p/a//file`, `projects/p/file?x` or `projects/p/a/`
- **THEN** storage rejects the intent before creating a session or file

### Requirement: Local proof is fail closed
`LocalWorkspaceAdapter` SHALL NOT issue a successful no-reference `DeleteProof` because it cannot query all owner reference indexes; callers SHALL use a complete composite owner proof.

#### Scenario: Proof index unavailable
- **WHEN** a caller asks Local storage to prove an object has no references
- **THEN** storage raises an object-in-use/unsafe proof error and deletion cannot proceed

### Requirement: Compose Local workspace is shared
When `STORAGE_MODE=local_workspace`, API and Media Worker SHALL mount the same named workspace volume at the configured `WORKSPACE_ROOT`.

#### Scenario: Worker reads API upload
- **WHEN** API writes a Local workspace object and Media Worker materializes the same reference
- **THEN** both containers resolve the same bytes and checksum
