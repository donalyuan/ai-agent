## Context

运行态证据显示 Admin 请求 `PUT /api/admin/models/21ca4433-b8a3-430c-a72d-7092a00bc44e/default` 返回 `405 Method Not Allowed` 和 `allow: POST`。前端 `setDefaultAiModel()` 写死 `PUT`，后端路由使用 `post(handlers::set_default_ai_model)`；数据库中的原默认模型和目标模型均未变化。

## Goals / Non-Goals

**Goals:**

- 恢复管理后台“设为默认”操作。
- 让前端方法与后端公开路由保持单一契约。
- 用自动化测试锁定请求方法。

**Non-Goals:**

- 不修改默认模型业务规则、版本号语义或事务实现。
- 不新增 `PUT` 兼容路由。
- 不修改 UI 视觉或交互结构。

## Decision

前端改为发送 `POST`。后端契约已经存在并由仓储测试覆盖原子替换行为；修改后端为 `PUT` 会扩大影响面，同时偏离已注册和已部署的 API。双方法兼容会掩盖契约漂移，因此不采用。

## Testing

- Admin API 客户端测试先断言 `POST` 并观察 RED，再修改实现。
- 页面测试覆盖点击“设为默认”后刷新列表且不显示错误。
- Admin 全量测试、lint、build 通过后，通过真实 API 请求验证数据库默认标记切换。
