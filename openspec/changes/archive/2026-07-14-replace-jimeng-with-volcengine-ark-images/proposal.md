## Why

当前 `jimeng_visual` 是项目内部旧 VisualService 适配器，不是用户要使用的火山方舟 Seedream API，导致 Ark Bearer API Key 配置切换到该协议后因缺少 API Secret 而保存失败。系统需要用官方 Ark 图片协议完整替换旧协议，并让配置、执行、重试、日志和费用边界保持一致。

## What Changes

- **BREAKING**：完整删除 `jimeng_visual` 协议、`jimeng` 任务供应商值、VisualService Worker 和 `JIMENG_*` 环境变量导入。
- 新增图片专用协议 `volcengine_ark_images`，固定使用 `bearer`，对接 Ark `/images/generations`。
- Admin 新增“火山方舟图片生成”选项，只显示 API Key，并规范化 Ark 根地址。
- Worker 为每个候选独立调用一次 Ark；当前候选临时错误最多重试一次，永久错误停止剩余调用。
- 支持把参考素材转换为 Ark `image` data URL，并严格解析 `data[].b64_json` 后写入自管存储。
- 输出不含凭据与图片 base64 的脱敏请求、响应和 curl 等价日志。
- 新增 append-only migration；发现旧协议模型或旧 provider 任务时停止迁移，不隐式转换。
- 实施后的真实验证限制为单分镜、单候选，最多首次调用加一次临时错误重试。

## Capabilities

### New Capabilities

- `volcengine-ark-image-generation`: 定义 Ark 图片请求、逐候选调用、参考图、响应解析、错误分类、重试和脱敏日志契约。

### Modified Capabilities

- `ai-model-management`: 图片协议矩阵从 `jimeng_visual` 替换为 `volcengine_ark_images`，认证改为 Bearer，并增加 Ark 地址规范化与 Admin 表单要求。
- `model-routed-ai-execution`: 图片任务根据新协议构造 Ark provider，任务审计值从 `jimeng` 替换为 `volcengine-ark`，删除旧 Worker 与环境变量路径。

## Impact

- 数据库：重建 AI 模型协议/类型约束和素材任务 provider 约束。
- Rust：更新 `novex-model` registry、模型保存规范化、素材任务映射、旧配置导入和测试。
- Python Worker：新增 Ark provider 与逐候选执行模式，删除 Jimeng provider，扩展安全日志、参考图和文件类型解析。
- Admin：更新协议类型、认证联动、图片配置序列化、页面测试和 Pencil 原型。
- 运维：部署前必须确认不存在旧协议记录和在途图片任务；自动化阶段不发起真实计费调用。
