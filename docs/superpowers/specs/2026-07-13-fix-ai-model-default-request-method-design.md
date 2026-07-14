# 修复 AI 模型设为默认请求方法

## 背景

管理后台点击“设为默认”时，前端向 `/api/admin/models/:model_id/default` 发送 `PUT`，但 Rust API 只注册了 `POST`。运行态复现返回 `405 Method Not Allowed`，响应头为 `allow: POST`，数据库默认状态未改变。

## 方案

保持后端 `POST` 路由、版本校验和默认模型原子替换事务不变，只把 Admin API 客户端 `setDefaultAiModel()` 的请求方法从 `PUT` 改为 `POST`。不增加 `PUT` 兼容路由，避免维持两套重复契约。

请求固定为：

```http
POST /api/admin/models/:model_id/default
Content-Type: application/json

{"version":<current_version>}
```

成功后页面沿用现有流程重新加载模型列表；版本冲突和后端业务错误继续通过现有 `ApiError` 展示。

## 边界

- 不修改页面布局、按钮文案或 Pencil 原型。
- 不修改后端路由、数据库 schema、默认模型事务或并发控制。
- 不为错误方法增加兜底兼容。

## 验证

1. 先让 Admin API 客户端测试断言默认切换必须使用 `POST`，确认当前实现 RED。
2. 修改请求方法后确认聚焦测试 GREEN。
3. 运行 Admin 全量测试、TypeScript lint 和生产 build。
4. 使用目标图片模型当前版本执行一次真实 `POST`，确认默认标记原子切换且列表刷新后无“请求失败”。
