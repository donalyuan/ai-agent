# provider-and-storage-boundaries Specification

## Purpose
TBD - created by archiving change establish-phase-zero-foundation. Update Purpose after archive.
## Requirements
### Requirement: R4 六个业务 Port
业务领域 SHALL 仅通过 `TextModelPort`、`ImageGenerationPort`、`VideoGenerationPort`、`TtsPort`、`AsrPort` 与 `StoragePort` 发起模型或存储副作用。Port SHALL 定义可测试的输入、结果、错误和关联标识边界，业务服务不得直接依赖供应商 SDK。

#### Scenario: 以 Mock 替换 Provider
- **WHEN** 测试向业务服务注入任一模型 Port 的 Mock 实现
- **THEN** 服务通过统一结果完成调用路径，且测试不加载供应商 SDK 或网络凭据

### Requirement: R4 数据驱动的 Provider/Profile/Model
系统 SHALL 将 Provider、Profile 和 Model 的名称、adapter key、启用状态、认证引用、超时、默认参数和参数 Schema 存在配置或持久化模型中。业务代码 SHALL NOT 硬编码 model、`base_url`、bucket 或 region；新增同协议模型 SHALL 通过配置而非修改业务流程选择。

#### Scenario: 选择配置的模型
- **WHEN** 测试为 Profile 配置一个启用的 Model 与默认参数
- **THEN** Port 调用接收该配置选择结果，而不是业务代码中的固定模型名

### Requirement: R6 Mock Provider 和失败可见性
阶段 0 SHALL 提供可预测、无网络和无费用的 Mock Provider，覆盖六个 Port 的基础成功与可识别错误路径。真实 Provider 配置缺失、禁用或不支持时 SHALL 返回显式未配置/不可用结果，且不得回退到隐式真实服务。

#### Scenario: 缺少真实 Provider 配置
- **WHEN** 运行模式请求未配置的真实适配器
- **THEN** 调用返回可诊断的未配置结果，且日志和网络记录不显示真实外部请求

### Requirement: R6 LocalWorkspaceAdapter
`LocalWorkspaceAdapter` SHALL 实现 `StoragePort` 的阶段 0 开发和测试所需对象操作，并把所有文件限制在配置的工作区根目录内。持久化契约 SHALL 保存抽象对象引用和元数据，不得保存宿主绝对路径；路径逃逸 SHALL 被拒绝。

#### Scenario: 写入并读取工作区测试对象
- **WHEN** 测试将对象写入配置的 LocalWorkspace 根目录再读取它
- **THEN** 返回的对象标识可被解析，且持久化引用不暴露绝对路径

#### Scenario: 拒绝工作区外路径
- **WHEN** 调用方提供试图离开工作区根目录的对象路径
- **THEN** adapter 拒绝操作且不创建范围外文件
