## MODIFIED Requirements

### Requirement: Python Worker 必须从数据库模型配置构造图片 provider

Python Worker SHALL 根据图片任务的 `model_id` 读取统一模型配置，并 SHALL 按 `api_protocol` 构造 `openai_images` 或 `volcengine_ark_images` provider。

#### Scenario: 执行 OpenAI Images 任务

- **GIVEN** 待执行任务引用启用的 `openai_images` 模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 读取数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置
- **AND** Worker SHALL 在调用前保存任务模型快照

#### Scenario: 执行火山方舟图片任务

- **GIVEN** 待执行任务引用启用的 `volcengine_ark_images` 模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 使用数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置构造 Ark provider
- **AND** Worker SHALL 在调用前保存不含凭据的任务模型快照
- **AND** Worker SHALL NOT 读取 `JIMENG_*` 或其他环境变量作为运行时兜底

#### Scenario: 旧 Jimeng 配置不得调用上游

- **GIVEN** Worker 读取到 `api_protocol=jimeng_visual` 的图片模型配置
- **WHEN** Worker 解析该模型
- **THEN** Worker SHALL 返回 `invalid_model_config`
- **AND** Worker SHALL NOT 发起供应商请求

### Requirement: 系统必须提供幂等的一次性环境配置导入

系统 SHALL 提供显式命令，将现有文本 OpenAI 和 OpenAI 图片环境配置导入统一模型表；导入完成后运行时 SHALL 不再读取这些变量作为模型配置，且系统 SHALL NOT 导入旧 Jimeng 环境配置。

#### Scenario: 首次导入现有配置

- **WHEN** 操作者使用明确的原文凭据确认参数运行导入命令
- **THEN** 系统 SHALL 为存在完整 OpenAI 环境配置的部署创建模型记录
- **AND** 系统 SHALL 规范化旧完整端点为请求根地址
- **AND** 系统 SHALL 为每条导入记录保存稳定 `source_key`

#### Scenario: 旧 Jimeng 环境变量不再导入

- **GIVEN** 环境中仍存在任意 `JIMENG_*` 配置
- **WHEN** 操作者运行模型导入命令
- **THEN** 系统 SHALL 忽略这些旧配置
- **AND** 系统 SHALL NOT 创建 `jimeng_visual` 或 `volcengine_ark_images` 模型
- **AND** Ark 模型 SHALL 由 Admin 或模型 API 使用 Bearer API Key 显式创建

#### Scenario: 重复执行导入

- **GIVEN** 相同 `source_key` 已存在
- **WHEN** 操作者再次运行导入命令
- **THEN** 系统 SHALL 跳过已有记录
- **AND** 系统 SHALL NOT 覆盖后台编辑后的地址、协议、凭据或默认状态

#### Scenario: 导入后缺少可用模型

- **WHEN** 导入完成但某类没有完整配置
- **THEN** 系统 SHALL 报告该类型未配置
- **AND** 系统 SHALL NOT 创建带空凭据的可用模型
- **AND** 运行时 SHALL NOT 回退旧环境变量

## ADDED Requirements

### Requirement: 图片协议必须映射到稳定任务审计值

Backend SHALL 仅根据图片模型协议写入素材任务 provider 审计值，并 SHALL 保持任务模型引用和快照可追溯。

#### Scenario: Ark 模型创建素材任务

- **GIVEN** 操作者选择启用的 `volcengine_ark_images` 图片模型
- **WHEN** Backend 创建图片候选任务
- **THEN** 任务 SHALL 保存该 `model_id`
- **AND** 任务 `provider` SHALL 为 `volcengine-ark`
- **AND** 任务 SHALL NOT 保存 `jimeng`

#### Scenario: OpenAI Images 审计值保持不变

- **GIVEN** 操作者选择启用的 `openai_images` 图片模型
- **WHEN** Backend 创建图片候选任务
- **THEN** 任务 `provider` SHALL 继续为 `gpt-image-2`
