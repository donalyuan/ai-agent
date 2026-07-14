## 1. 规格与原型

- [x] 1.1 补齐设计文档、proposal、design 和三项 capability delta spec，并通过 OpenSpec strict validate。
- [x] 1.2 使用 Pencil MCP 更新所有旧协议可见文案，新增“火山方舟图片生成”已选状态，并验证无布局问题。
- [x] 1.3 取得用户对书面规格和 Pencil 原型的明确确认。

## 2. TDD 锁定失败行为

- [x] 2.1 先写 migration 与 Rust registry 测试，要求接受 `volcengine_ark_images + image + bearer`，拒绝 `jimeng_visual` 和 `provider='jimeng'`，运行并确认 RED。
- [x] 2.2 先写 Backend 地址规范化、AI 模型 API 和素材任务映射测试，覆盖根地址、完整端点、非法地址与 `provider=volcengine-ark`，运行并确认 RED。
- [x] 2.3 先写 Worker registry、Ark 请求 JSON、Bearer 认证、参考图 data URL、响应解析、图片类型和脱敏日志测试，运行并确认 RED。
- [x] 2.4 先写 Worker 逐候选编排测试，覆盖 N 次独立调用、当前候选单次重试、永久错误停止、成功不重复和部分成功，运行并确认 RED。
- [x] 2.5 先写 Admin 协议选项、Bearer 凭据联动、空图片尺寸和 Ark 单次最大图片数测试，运行并确认 RED。

## 3. 数据库与 Rust 实施

- [x] 3.1 新增 append-only migration，前置检查旧模型/任务记录并重建 AI 模型协议、类型协议和素材任务 provider 约束。
- [x] 3.2 用 `VolcengineArkImages` 替换 `JimengVisual`，固定 Bearer 认证，并删除 `ImageModelSettings.request_key`。
- [x] 3.3 实现 Ark 根地址结构化规范化，并在创建、更新和运行解析入口保持同一结果。
- [x] 3.4 把素材任务映射和仓储枚举从 `Jimeng`/`jimeng` 替换为 `VolcengineArk`/`volcengine-ark`。
- [x] 3.5 删除 `JIMENG_*` 模型导入字段、导入分支和 `.env.example` 配置，更新相关测试与注释。
- [x] 3.6 运行相关 Rust 测试，确认新旧协议、地址和任务审计约束全部转绿。

## 4. Worker 实施

- [x] 4.1 更新图片模型 registry 与 provider factory，只接受 `openai_images | volcengine_ark_images` 并校验对应 Bearer 认证。
- [x] 4.2 实现 `VolcengineArkImageProvider` 的请求构造、参考图读取/data URL、严格响应解析和 PNG/JPEG/WebP 类型识别。
- [x] 4.3 实现显式 `per_candidate` 执行模式，按候选隔离 attempt、重试、失败记录、永久错误停止和任务汇总。
- [x] 4.4 实现 Ark 请求、响应和 curl 等价结构化日志，保证 Key、Authorization 与图片 base64 不进入日志。
- [x] 4.5 删除 `JimengImageProvider`、VisualService 导入、旧 URL 解析辅助和全部 Jimeng 测试路径。
- [x] 4.6 运行 Worker 相关测试并确认逐候选费用边界、参考图和现有 OpenAI Images 行为全部转绿。

## 5. Admin 实施

- [x] 5.1 更新 API 类型和图片协议选项，显示“火山方舟图片生成”并移除“即梦 Visual”。
- [x] 5.2 实现 Ark 自动 Bearer 联动，只显示 API Key，并保持编辑留空保留原 Key。
- [x] 5.3 修复空图片尺寸序列化，Ark 固定 `max_images_per_request=1`，OpenAI Images 既有配置不回归。
- [x] 5.4 运行 Admin 页面测试、API helper 测试和相关 e2e 测试并确认转绿。

## 6. 综合验证与部署

- [x] 6.1 在容器内运行 Rust workspace、Worker 和 Admin 全量测试，以及 Admin lint/build。
- [x] 6.2 运行 OpenSpec strict validate、`openspec instructions apply` 和 `git diff --check`，同步完成 tasks 状态。
- [x] 6.3 停止可写服务并确认旧协议模型、旧 provider 任务和在途图片任务均为零后，重建 API、Worker 与 Admin；自动化阶段不得调用 Ark。
- [x] 6.4 通过管理 API 把 `see-dream` 保存为 `volcengine_ark_images + bearer`，确认地址被规范化、Key 保留且不要求 API Secret。
- [x] 6.5 经用户再次明确确认后，只创建单分镜、单候选真实任务，最多允许一次临时错误重试，监控到终态并提供脱敏请求/curl 日志。
