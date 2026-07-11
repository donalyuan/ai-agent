## ADDED Requirements

### Requirement: 新 AI 调用必须显式解析有效模型

所有新文本和图片 AI 调用 SHALL 携带 `model_id`，系统 SHALL 在调用供应商前校验模型存在、未删除、已启用且类型匹配。

#### Scenario: 成功解析文本模型

- **WHEN** 文本业务请求提交一个启用的 `text` 模型 ID
- **THEN** 系统 SHALL 从数据库读取协议、地址、凭据和运行参数
- **AND** 系统 SHALL 构造本次请求专用的文本客户端

#### Scenario: 模型类型不匹配

- **WHEN** 图片生成请求提交文本模型 ID
- **THEN** 系统 SHALL 返回 `model_type_mismatch`
- **AND** 系统 SHALL NOT 调用任何供应商

#### Scenario: 不得回退旧配置

- **WHEN** 请求未提交模型、模型不存在或模型已停用
- **THEN** 系统 SHALL 返回稳定模型错误
- **AND** 系统 SHALL NOT 回退环境变量、默认硬编码模型或其他供应商

### Requirement: 每次真实调用必须记录非敏感模型快照

系统 SHALL 在真正发起供应商请求前，将 `model_id` 和不含凭据的实际配置快照写入对应运行或生成任务。

#### Scenario: 文本运行保存快照

- **WHEN** Agent 或直接生成接口准备调用文本模型
- **THEN** `agent_runs` 或对应运行记录 SHALL 保存模型引用与快照
- **AND** 快照 SHALL 包含供应商、协议、协议版本、地址、上游模型标识、推理等级和超时
- **AND** 快照 SHALL NOT 包含 API Key 或 API Secret

#### Scenario: 模型编辑不改变历史快照

- **GIVEN** 一次运行已经保存模型快照
- **WHEN** 管理员随后编辑模型名称、地址或协议
- **THEN** 历史运行 SHALL 保留原快照
- **AND** 历史审计 SHALL NOT 使用当前模型字段覆盖原值

### Requirement: 内部步骤和重试必须复用同一模型

一次业务操作内的所有模型步骤和允许的临时错误重试 SHALL 使用请求解析得到的同一模型配置。

#### Scenario: 选题生成包含多个步骤

- **WHEN** topic Agent 执行候选生成、质量闸门和最多一次重写
- **THEN** 各步骤 SHALL 使用同一 `model_id`
- **AND** 每个运行步骤 SHALL 可追溯到该模型

#### Scenario: 临时错误重试

- **WHEN** 文本或图片供应商发生允许重试的临时错误
- **THEN** 重试 SHALL 使用同一模型部署
- **AND** 系统 SHALL NOT 自动选择另一个模型或供应商

### Requirement: Python Worker 必须从数据库模型配置构造图片 provider

Python Worker SHALL 根据图片任务的 `model_id` 读取统一模型配置，并 SHALL 按 `api_protocol` 构造 `openai_images` 或 `jimeng_visual` provider。

#### Scenario: 执行 OpenAI Images 任务

- **GIVEN** 待执行任务引用启用的 `openai_images` 模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 读取数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置
- **AND** Worker SHALL 在调用前保存任务模型快照

#### Scenario: 执行即梦任务

- **GIVEN** 待执行任务引用启用的 `jimeng_visual` 模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 使用数据库中的 Access Key、Secret Key、协议版本和类型化图片设置
- **AND** Worker SHALL NOT 读取 `JIMENG_*` 作为运行时兜底

### Requirement: 停用模型必须阻止待执行任务产生外部调用

系统 SHALL 在接受新请求和 Worker 真正执行任务两个时点校验模型状态，并 SHALL 如实处理已经发出的在途请求。

#### Scenario: 待执行图片任务对应模型被停用

- **GIVEN** 图片任务已创建但尚未调用供应商
- **AND** 管理员停用了任务模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 将任务标记为失败并记录模型不可用错误
- **AND** Worker SHALL NOT 调用供应商

#### Scenario: HTTP 请求已在途

- **GIVEN** 供应商 HTTP 请求已经发出
- **WHEN** 管理员停用模型
- **THEN** 系统 SHALL 允许该在途请求自然完成并记录真实结果
- **AND** 系统 SHALL NOT 伪造取消成功

### Requirement: 系统必须提供幂等的一次性环境配置导入

系统 SHALL 提供显式命令，将现有文本 OpenAI、OpenAI 图片和即梦环境配置导入统一模型表；导入完成后运行时 SHALL 不再读取这些变量作为模型配置。

#### Scenario: 首次导入现有配置

- **WHEN** 操作者使用明确的原文凭据确认参数运行导入命令
- **THEN** 系统 SHALL 为存在完整环境配置的部署创建模型记录
- **AND** 系统 SHALL 规范化旧完整端点为请求根地址
- **AND** 系统 SHALL 为每条导入记录保存稳定 `source_key`

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

### Requirement: 模型路由改造必须保持现有费用边界

模型选择和协议路由 SHALL NOT 放宽图片生成数量、重试和人工确认规则，也 SHALL NOT 发起视频生成验证。

#### Scenario: 图片候选费用上限保持不变

- **WHEN** 操作者使用任一启用图片模型创建候选任务
- **THEN** 每分镜候选数 SHALL 保持 `1-4`
- **AND** 每脚本批量图片数 SHALL 不超过 `48`
- **AND** 临时错误最多 SHALL 按既有规则重试一次

#### Scenario: 禁止自动跨模型重试

- **WHEN** 当前图片模型返回永久错误或重试后仍失败
- **THEN** 系统 SHALL 将任务标记失败
- **AND** 系统 SHALL NOT 自动调用其他模型或供应商

#### Scenario: 视频模型不产生费用

- **WHEN** 管理员创建、编辑或启停视频模型
- **THEN** 系统 SHALL NOT 调用视频供应商
- **AND** 自动化验证 SHALL 使用本地校验或 fake provider
