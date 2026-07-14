## MODIFIED Requirements

### Requirement: Python Worker 必须从数据库模型配置构造图片 provider

Python Worker SHALL 根据图片任务的 `model_id` 读取统一模型配置，并 SHALL 按图片模型的 `api_protocol` 构造 `openai_images`、`openai_responses` 或 `jimeng_visual` provider。

#### Scenario: 执行 OpenAI Images 任务

- **GIVEN** 待执行任务引用启用的 `openai_images` 图片模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 读取数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置
- **AND** Worker SHALL 在调用前保存任务模型快照

#### Scenario: 执行 OpenAI Responses 图片任务

- **GIVEN** 待执行任务引用启用的 `openai_responses` 图片模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 使用数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置构造 Responses 图片 provider
- **AND** Worker SHALL 在调用前保存包含 `model_type=image` 与 `api_protocol=openai_responses` 的任务模型快照

#### Scenario: 执行即梦任务

- **GIVEN** 待执行任务引用启用的 `jimeng_visual` 图片模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 使用数据库中的 Access Key、Secret Key、协议版本和类型化图片设置
- **AND** Worker SHALL NOT 读取 `JIMENG_*` 作为运行时兜底

#### Scenario: 不兼容图片协议

- **GIVEN** 图片任务对应模型使用图片 Worker 未支持的协议
- **WHEN** Worker 解析该模型
- **THEN** Worker SHALL 将任务标记为失败并记录 `invalid_model_config`
- **AND** Worker SHALL NOT 调用供应商
