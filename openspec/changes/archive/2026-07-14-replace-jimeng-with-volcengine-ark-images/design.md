## Context

`jimeng_visual` 当前映射旧火山视觉服务 `VisualService.cv_sync2async_submit_task`，使用 Access Key + Secret Key、`req_key` 和 `visual.volcengineapi.com`。用户实际使用的是火山方舟 Seedream 图片生成接口：Bearer API Key 调用 `/api/v3/images/generations`。两者不是同一认证或请求协议，复用旧协议会让 Admin 强制要求不存在的 API Secret，并让 Worker 选择错误 adapter。

当前运行库没有 `jimeng_visual` 模型、`provider='jimeng'` 任务或对应素材，Worker 环境也没有安装旧 `volcengine` SDK。图片任务已有异步领取、模型快照、临时错误重试、部分成功、自管素材存储和 `/assets/...` 本地参考图读取基础。

官方契约来源：`https://console.volcengine.com/ark/region:cn-beijing/docs/82379/1541523?lang=zh`。

## Goals / Non-Goals

**Goals:**

- 用显式 `volcengine_ark_images` 协议完整替换 `jimeng_visual`。
- 使用 Bearer API Key 调用 Ark，并让 Admin、数据库、Rust 与 Worker 使用同一兼容矩阵。
- 每个候选独立调用一次；单候选临时错误最多重试一次，永久错误停止剩余调用。
- 支持参考图 data URL、严格 base64 响应解析、自管存储和脱敏请求日志。
- 通过 append-only migration 删除旧协议和旧任务 provider 的合法入口。

**Non-Goals:**

- 不支持图片 Responses、Seedream 组图、流式图片响应或一次请求多候选。
- 不保留 Jimeng 别名、VisualService SDK 或 `JIMENG_*` 环境变量兜底。
- 不按模型名、供应商名或 URL 猜测协议。
- 不自动跨模型或跨供应商重试，不新增视频调用。

## Decisions

### 1. 新增正式协议，而不是复用现有协议

新增 Rust `ApiProtocol::VolcengineArkImages`，存储值 `volcengine_ark_images`，只支持 `ModelType::Image`，固定认证 `AuthScheme::Bearer`。任务审计值使用 `volcengine-ark`。

不复用 `openai_images`：虽然 endpoint 后缀相似，但 Ark 的参考图字段、顺序图参数、响应错误和调用计费边界不同。不改造 `jimeng_visual`：旧名称和 Access Key + Secret Key 契约会继续制造错误配置。

### 2. 旧协议使用前置检查后删除

追加 migration 在重建约束前检查所有模型和任务历史；存在 `jimeng_visual` 或 `jimeng` 时中止。当前运行库为零记录，因此无需数据迁移。协议间认证与 settings 不可无损转换，自动改写不采用。

### 3. 保存规范化根地址，Worker 固定追加路径

Backend 仅在显式 Ark 协议下，把根地址或完整 `/images/generations` 地址规范化为根地址。使用结构化 URL 解析，拒绝 query、fragment 和非 HTTP(S) 地址。Worker 固定追加 `/images/generations`。

这保持了“配置保存根地址、adapter 拥有稳定路径”的现有边界，也不会通过 URL 反推协议。

### 4. Ark 使用逐候选执行模式

Ark provider 每次只处理 `candidate_count=1`。Worker 的图片执行器显式区分现有 OpenAI 批量模式与 Ark `per_candidate` 模式；逐候选模式独立生成 task suffix、attempt、结果和失败记录。

不把循环放进 Ark provider：重试、停止剩余候选、素材落盘和任务汇总属于 Worker 编排，放入 provider 会混淆 HTTP adapter 与任务状态职责。

### 5. 复用标准 HTTP 基础，不引入旧 SDK

Ark provider 复用现有 JSON POST、HTTP 错误分类和可注入 fake transport。临时错误限定为网络、超时、HTTP `429` 和 `5xx`；其他 HTTP 或响应契约错误为永久错误。

这避免重新引入当前 Worker 不具备的 `volcengine` SDK，并能完整测试实际请求 JSON。

### 6. 参考图与结果都在边界处严格转换

参考素材通过现有安全 binary loader 读取，按 magic bytes 识别 PNG、JPEG、WebP 后编码成 data URL。结果只接受 `data[].b64_json`，解码后再次识别图片类型并使用匹配扩展名保存。

不把本地 `/assets/...` URL直接发送给上游，也不无条件把结果命名为 PNG。

### 7. 日志输出可审计摘要，不输出秘密或图片正文

每个候选 attempt 记录请求和结果事件，可包含脱敏 curl 等价信息。`Authorization` 固定显示 `***`，`image` 和 `b64_json` 只记录数量/字节数，禁止正文进入日志。prompt 可以保留用于定位生成偏差。

## Risks / Trade-offs

- [部署前出现旧协议记录] -> migration 明确失败；停服窗口内由操作者显式处理，禁止自动转换。
- [逐候选调用增加请求次数] -> 这是用户确认的计费边界；每分镜仍限制 1-4，总任务仍限制 48。
- [临时错误导致额外一次计费尝试] -> 只重试当前候选一次，累计 attempt 与 retry_count，成功候选不重复。
- [永久错误发生在部分成功之后] -> 保留已落盘素材，剩余候选写失败，任务以 `completed + partial=true` 结束。
- [日志泄露凭据或 base64] -> 结构化脱敏器与专门测试覆盖 Key、Authorization、参考图和结果图正文。
- [Ark 返回非 PNG 图片] -> 按 magic bytes 识别 PNG/JPEG/WebP；未知类型拒绝落盘。
- [地址规范化破坏代理路径] -> 只删除精确固定后缀，保留 scheme、host 和此前根路径；无关路径明确拒绝。

## Migration Plan

1. 通过 Pencil MCP 更新 Admin 原型并取得确认。
2. 先写数据库、Rust、Worker 和 Admin 失败测试。
3. 停止可写 API 与图片 Worker，确认旧协议模型、旧 provider 任务和 `pending/processing` 图片任务均为零。
4. 应用 append-only migration，再部署 Rust API、Worker 和 Admin。
5. 运行全量自动化和服务健康验证；此阶段不调用 Ark。
6. 用户再次确认后，只执行一条单分镜、单候选真实验证；最多首调加一次临时错误重试。

应用版本回退不得删除或回滚已应用 migration。若必须恢复旧版本，需要先评估已保存 Ark 模型与任务，并使用新的追加 migration 明确处理数据边界。

## Open Questions

无。协议命名、认证、地址规则、逐候选计费、重试上限、日志脱敏和真实验证上限均已确认。
