## 1. 规格与原型

- [x] 1.1 建立 change，补齐 proposal、design 与三项 capability delta spec，并通过 strict validate。
- [x] 1.2 更新 `docs/prototypes/admin/model-management.pen`，新增“图片模型 + OpenAI Responses”添加状态并完成布局检查。
- [x] 1.3 获取用户对更新后 Pencil 原型的明确开发确认。

## 2. 数据库与 Rust 兼容矩阵

- [x] 2.1 先补 `novex-model`、AI 模型 API 与 migration 失败测试，证明 `image + openai_responses` 当前被拒绝且其他非法组合仍被拒绝。
- [x] 2.2 新增 append-only migration 调整 `ai_models_type_protocol_check`，只给图片集合增加 `openai_responses`。
- [x] 2.3 更新 `ApiProtocol::supports`、AI 模型仓储校验和素材生成 provider 映射，使图片 Responses 模型可创建并可建立图片任务。
- [x] 2.4 运行相关 Rust 测试并确认新组合通过、既有协议矩阵无回归。

## 3. Python 模型解析与 Responses Provider

- [x] 3.1 先补模型注册表失败测试，证明图片模型当前拒绝 `openai_responses`。
- [x] 3.2 更新图片模型注册表允许 `openai_responses + bearer`，保持安全快照不含凭据。
- [x] 3.3 先补 Responses provider 失败测试，覆盖 `/responses` 路径、标准请求体、默认尺寸、参考图、严格结果解析和非法 base64。
- [x] 3.4 实现非流式 Responses 图片 provider，并输出不含凭据和图片正文的结构化请求/响应摘要日志。

## 4. 逐候选执行与费用边界

- [x] 4.1 先补逐候选失败测试，覆盖三候选三次调用、只重试失败候选、临时错误后继续、永久错误停止和部分成功。
- [x] 4.2 为图片 provider 增加显式 `batch/per_candidate` 请求模式，保留 OpenAI Images 与即梦批量路径。
- [x] 4.3 重构图片任务处理，使 Responses 每候选一次调用、成功候选不重复、重试和失败数量准确回写。
- [x] 4.4 运行 Worker 全量测试，确认既有下载、存储、批量 provider、任务领取和错误审计无回归。

## 5. Admin 协议选项

- [x] 5.1 在原型确认后先补页面失败测试，断言图片模型包含 `OpenAI Responses`，文本/视频选项不变，切换协议使用 Bearer 认证。
- [x] 5.2 更新管理后台协议映射与表单联动，不改变抽屉布局和其他字段语义。
- [x] 5.3 运行 Admin 页面测试、全量测试、lint 和 build。

## 6. 综合验证

- [x] 6.1 运行 Rust workspace 格式化、相关测试与全量测试。
- [x] 6.2 运行 Python Worker 全量测试及 Admin 全量验证。
- [x] 6.3 运行 OpenSpec strict validate、`openspec instructions apply` 和 `git diff --check`。
- [x] 6.4 重建 API、Worker、Admin，确认健康检查与运行态 migration 生效。
- [x] 6.5 将目标图片模型配置为 `image + openai_responses` 和含 `/v1` 的请求根地址，创建单分镜单候选受控任务，核对脱敏日志与真实终态。
