## ADDED Requirements

### Requirement: R1 单仓库工程边界
系统 SHALL 建立同一仓库中的 `apps/web`、`services/api`、`workers/agent`、`workers/generation`、`workers/media`、`packages/contracts` 和 `infra/compose` 边界。Web SHALL 使用 React 19、TypeScript 与 Vite 8，API SHALL 使用 FastAPI 与 Pydantic 2，关系数据 SHALL 使用 PostgreSQL 与 Alembic。

#### Scenario: 新开发者检查工程边界
- **WHEN** 开发者检出阶段 0 工程
- **THEN** 可以在约定目录找到 Web、API、三类 Worker、共享契约与 Compose 配置，且没有要求先创建额外仓库

### Requirement: R7 本地 Compose 运行形态
Docker Compose SHALL 定义 Web、API、PostgreSQL、Temporal、Agent Worker、Generation Worker 与 Media Worker 服务。服务 SHALL 使用显式配置边界与健康检查，默认本地访问 SHALL 不暴露到非本机接口。

#### Scenario: 解析 Compose 配置
- **WHEN** 开发者以阶段 0 示例环境解析 Compose
- **THEN** 配置包含七类所需服务及其依赖关系，并通过 `docker compose config` 校验

### Requirement: 无真实凭据启动模式
系统 SHALL 提供不含真实 API Key、model、`base_url`、bucket 或 region 的示例配置。该模式 SHALL 选择 Mock Provider 和 `LocalWorkspaceAdapter`，并将未配置的真实外部服务显示为未配置而非调用成功。

#### Scenario: 空凭据本地启动
- **WHEN** 开发者只使用示例配置启动本地服务
- **THEN** 健康检查可返回基础服务状态，且不会发起真实 Provider 或对象存储请求

### Requirement: R7 健康和结构化运行信号
API 与每类 Worker SHALL 提供可探测的健康状态；API、Worker 和 Provider/Storage 边界 SHALL 输出结构化日志，并不得记录 API Key、认证头、真实密钥或完整私密响应。

#### Scenario: 健康探测和日志脱敏
- **WHEN** 测试探测 API 与 Worker 并触发一个 Mock 调用
- **THEN** 探测结果标识服务状态，日志包含关联字段且不包含秘密值
