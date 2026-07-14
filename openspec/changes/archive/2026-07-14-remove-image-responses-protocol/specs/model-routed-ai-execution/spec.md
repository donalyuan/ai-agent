## MODIFIED Requirements

### Requirement: Python Worker 必须从数据库模型配置构造图片 provider

Python Worker SHALL 根据图片任务的 `model_id` 读取统一模型配置，并 SHALL 按图片模型的 `api_protocol` 构造 `openai_images` 或 `jimeng_visual` provider。

#### Scenario: 执行 OpenAI Images 任务

- **GIVEN** 待执行任务引用启用的 `openai_images` 图片模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 读取数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置
- **AND** Worker SHALL 在调用前保存任务模型快照

#### Scenario: 执行即梦任务

- **GIVEN** 待执行任务引用启用的 `jimeng_visual` 图片模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 使用数据库中的 Access Key、Secret Key、协议版本和类型化图片设置
- **AND** Worker SHALL NOT 读取 `JIMENG_*` 作为运行时兜底

#### Scenario: 图片 Responses 配置不得调用上游

- **GIVEN** Worker 读取到 `api_protocol=openai_responses` 的图片模型配置
- **WHEN** Worker 解析该模型
- **THEN** Worker SHALL 返回 `invalid_model_config`
- **AND** Worker SHALL 将任务标记为失败
- **AND** Worker SHALL NOT 发起供应商请求

#### Scenario: 其他不兼容图片协议

- **GIVEN** 图片任务对应模型使用其他未支持协议
- **WHEN** Worker 解析该模型
- **THEN** Worker SHALL 返回 `invalid_model_config`
- **AND** Worker SHALL NOT 调用供应商
