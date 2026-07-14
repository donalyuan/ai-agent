## Why

管理后台“设为默认”按钮当前发送 `PUT /api/admin/models/:model_id/default`，而后端公开契约只接受 `POST`。实际请求返回 `405 Method Not Allowed`，导致操作者无法通过页面切换默认模型。

## What Changes

- 修正 Admin API 客户端，使默认模型切换使用后端已注册的 `POST` 方法。
- 增加请求方法回归测试，防止前后端契约再次漂移。
- 保持后端路由、默认模型原子事务、版本冲突和页面布局不变。

## Capabilities

### Modified Capabilities

- `ai-model-management`: 明确管理后台设为默认操作必须调用 `POST /api/admin/models/:model_id/default` 并携带当前版本。

## Impact

- Admin：修改 `setDefaultAiModel()` 请求方法和相关测试。
- Rust API、数据库和 Pencil 原型：无改动。
