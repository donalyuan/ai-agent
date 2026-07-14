## 1. 规格与根因

- [x] 1.1 复现错误并确认 `PUT` 返回 `405 Method Not Allowed`、后端只允许 `POST`。
- [x] 1.2 明确只修正前端请求方法，不增加后端兼容路由或 UI 改动。
- [x] 1.3 通过 OpenSpec strict validate 并取得书面规格确认。

## 2. TDD 修复

- [x] 2.1 先补 Admin API 客户端和页面失败测试，断言默认切换使用 `POST` 且成功后刷新列表。
- [x] 2.2 运行聚焦测试并确认因当前 `PUT` 实现而 RED。
- [x] 2.3 将 `setDefaultAiModel()` 改为 `POST`，不修改其他模型操作。
- [x] 2.4 运行聚焦测试并确认 GREEN。

## 3. 验证

- [x] 3.1 运行 Admin 全量测试、lint 和生产 build。
- [x] 3.2 执行 OpenSpec strict validate、`openspec instructions apply` 和 `git diff --check`。
- [x] 3.3 重建 Admin，并通过真实默认切换请求确认列表和数据库状态正确。
