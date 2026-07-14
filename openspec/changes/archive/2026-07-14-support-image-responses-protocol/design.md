## Context

统一模型管理当前把 `ApiProtocol::OpenAiResponses` 仅视为文本协议：PostgreSQL 约束、Rust `ApiProtocol::supports`、管理后台协议选项和 Python 图片模型注册表都会拒绝 `image + openai_responses`。图片 Worker 因此只能把 `gpt-image-2` 发送到 `/images/generations`，无法接入仅暴露 Responses 兼容入口的中转。

用户已确认该中转按以下假设接入：请求 `<request_base_url>/responses`，请求体使用 `model`、`input`、`tools=[{"type":"image_generation"}]` 与强制 `tool_choice`，响应从 `output[].type=image_generation_call` 的 `result` 字段读取 base64 图片。该组合是中转兼容扩展；不推断或放开其他跨类型组合。

## Goals / Non-Goals

**Goals:**

- 允许图片模型保存和使用 `openai_responses`，其余协议兼容矩阵不变。
- 让 Responses 图片任务按候选逐次调用，保留现有 `1-4` 张/分镜、`48` 张/脚本和同模型重试边界。
- 让单个候选失败不重复调用已经成功的候选，并准确记录部分成功、失败候选和重试次数。
- 支持 prompt-only 与已有参考图输入，并从标准 `image_generation_call.result` 提取图片。
- 输出足以诊断路径、模型、候选、请求字段和响应类型的脱敏结构化日志。

**Non-Goals:**

- 不允许任意模型类型与任意协议组合。
- 不修改文本 Responses 客户端、Chat Completions、OpenAI Images、即梦、Runway 或可灵的既有请求格式。
- 不支持 Responses SSE/streaming；请求不设置 `stream=true`，只接受完整 JSON 响应。
- 不自动跨模型或跨供应商重试，不新增视频生成调用。
- 不根据供应商名称、模型名称或 URL 猜测协议。

## Decisions

### 1. 扩展兼容矩阵，而不是新增重复协议枚举

保留 `api_protocol=openai_responses`，让其同时支持 `text` 与 `image`。运行时仍先按 `model_type` 选择业务执行器，再按 `api_protocol` 选择该类型内的 adapter：Rust 文本调用继续使用现有 Responses 客户端，Python 图片 Worker 新增 Responses 图片 provider。

备选方案是新增 `openai_responses_image`。该方案能隔离语义，但会把同一个线级协议复制成两个管理枚举，并使中转配置难以与实际协议名称对应，因此不采用。

### 2. Responses 图片按候选独立调用

Responses 图片 provider 每次只请求一张图片。每个分镜的候选数为 `N` 时执行 `N` 个独立调用；每个候选的临时错误最多只重试该候选一次。成功图片立即形成内存结果并在场景处理完成后落库，后续候选失败不得重新请求已经成功的候选。

批量型 `openai_images` 与 `jimeng_visual` 保持原有一次请求生成多张的行为。Worker 在 provider 上使用显式请求模式区分 `batch` 与 `per_candidate`，避免根据类名或协议字符串在通用处理函数中猜测。

### 3. 使用标准非流式 Responses 图片工具请求

每个候选请求：

```json
{
  "model": "<upstream_model>",
  "input": [
    {
      "role": "user",
      "content": [
        { "type": "input_text", "text": "<scene prompt>" }
      ]
    }
  ],
  "tools": [{ "type": "image_generation" }],
  "tool_choice": { "type": "image_generation" }
}
```

存在参考图时，Worker 下载参考图并在同一 `content` 中追加 base64 data URL 形式的 `input_image`。`settings.default_size` 非空时写入 `image_generation` tool 的 `size`；不发送 `n`，也不设置 streaming。

响应必须包含至少一个 `type=image_generation_call` 且 `result` 为非空 base64 字符串。缺失、非法 base64 或非对象 JSON 都按永久响应格式错误处理，不做其他字段兜底解析。

### 4. 保留部分成功并精确停止永久错误

临时错误重试后仍失败时，记录当前候选失败并继续下一个候选。认证失败、配置失败、非法请求或响应结构错误属于永久错误：记录当前候选失败，停止当前分镜剩余候选与后续分镜调用，并由现有任务编排补齐失败候选记录。

任务最终只要至少生成一张图片就为 `completed` 且 `result.partial=true`；全部失败则为 `failed`。`retry_count` 累加实际发生的候选重试次数。

### 5. 日志只记录脱敏请求与响应摘要

每次尝试打印单行 JSON 日志，包含事件名、任务/分镜/候选标识、attempt、URL、model、timeout、prompt、参考图数量、tool 配置和响应 HTTP 状态或错误摘要。日志不得包含 `Authorization`、API Key、API Secret、参考图 base64、返回图片 base64或完整二进制正文。

为避免调用库吞掉 HTTP 状态，底层 JSON POST 返回解析结果及状态摘要，或在 provider 边界统一记录成功/异常。已有错误消息继续最多保留 1000 字符的供应商响应摘要。

### 6. 数据库和 API 同步收紧到唯一新增组合

新增 append-only migration 重建 `ai_models_type_protocol_check`：图片协议集合增加 `openai_responses`，文本和视频集合不变。Rust `ApiProtocol::supports`、仓储验证、素材生成 provider 映射、Python 注册表和 Admin 选项使用同一兼容矩阵语义，并分别用测试防止漂移。

## Risks / Trade-offs

- [中转并不完全遵循已确认的非流式 Responses 格式] → 严格失败并打印脱敏请求/响应摘要，不增加猜测式字段兼容；根据真实日志另立变更。
- [每候选一次调用提高费用与延迟] → 保持候选数上限，界面现有数量选择不变；只重试当前失败候选一次，不重复成功候选。
- [参考图 base64 会增大请求体] → 只发送用户已选择的参考图，日志不记录正文，继续使用模型超时配置。
- [数据库、Rust、Python、Admin 兼容矩阵可能漂移] → 为四层分别增加同一组合的回归测试，并保留数据库最终约束。
- [现有批量 provider 被逐候选重构影响] → 通过显式请求模式保留批量路径，先运行现有 Worker 全量测试再验证新增协议。

## Migration Plan

1. 更新 Pencil 原型，确认图片模型协议下拉新增 `OpenAI Responses`，布局不变。
2. 应用数据库 migration，放开唯一组合；旧记录无需回填。
3. 部署 Rust API、Python Worker 和 Admin。
4. 在管理后台把目标图片模型协议改为 `openai_responses`，请求地址配置为包含版本前缀的 API 根地址，例如 `https://host/v1`。
5. 使用单分镜、单候选任务进行首次受控验证；确认日志和响应结构后再恢复 2-4 个候选。

回滚时先停用对应图片模型或关闭图片 Worker，再回滚应用代码。由于新组合数据不满足旧数据库约束，数据库约束不得直接回滚；应先把相关模型改回兼容协议或逻辑删除，再恢复旧约束。

## Open Questions

无。中转真实返回若偏离已确认格式，以脱敏日志为新证据另行设计，不在本变更中加入猜测式兼容。
