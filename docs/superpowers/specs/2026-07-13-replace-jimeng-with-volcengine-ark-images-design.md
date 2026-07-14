# 用火山方舟图片协议替换 Jimeng Visual 设计

## 结论

项目完整删除内部旧协议 `jimeng_visual`，新增正式图片协议 `volcengine_ark_images`，对接火山方舟 Seedream 图片生成接口。新协议只允许 `model_type=image`，认证固定为 `bearer`，任务审计供应商值为 `volcengine-ark`。

本设计依据火山方舟官方 Seedream API 文档：

`https://console.volcengine.com/ark/region:cn-beijing/docs/82379/1541523?lang=zh`

官方接口使用：

```http
POST https://ark.cn-beijing.volces.com/api/v3/images/generations
Content-Type: application/json
Authorization: Bearer $ARK_API_KEY
```

`jimeng_visual` 不是该官方协议，而是项目内部对 `volcengine.visual.VisualService`、`cv_sync2async_submit_task`、Access Key + Secret Key 和 `visual.volcengineapi.com` 的旧封装。当前运行库没有该协议模型、`provider='jimeng'` 任务或素材记录，Worker 也没有安装旧 `volcengine` SDK，因此不保留旧适配器或兼容别名。

## 已确认边界

- 每个图片候选独立调用一次 Ark API；请求 `N` 个候选最多产生 `N` 次首次调用。
- 单个候选遇到网络错误、超时、HTTP `429` 或 `5xx` 时最多重试一次，只重试该候选。
- 鉴权、权限、参数等永久错误停止当前任务剩余候选调用。
- 成功候选不得因后续候选失败而重复调用。
- 实施完成后只允许一次“单分镜、单候选”真实验证；最多首调加一次临时错误重试。
- 保存模型、运行自动化测试和更新原型均不得调用 Ark。

## DDD

### 领域概念

- `ApiProtocol::VolcengineArkImages`：显式表示火山方舟图片生成协议，存储值为 `volcengine_ark_images`。
- `AssetGenerationProvider::VolcengineArk`：素材任务的路由与审计值，存储值为 `volcengine-ark`。
- `VolcengineArkImageProvider`：Worker 内唯一负责构造 Ark 请求和解析 Ark 响应的 adapter。
- “候选调用”：一个候选对应一次独立的 Ark 请求，是重试、失败和费用审计的最小单位。

### 规则归属

- 数据库、Rust registry 和 Admin 共同约束类型、协议与认证组合。
- Backend 负责把模型协议映射为任务审计供应商，不按模型名、供应商名或 URL 猜测协议。
- Worker 根据数据库模型快照构造 provider，并负责逐候选调用、重试、结果落盘和任务终态。
- Ark provider 只负责单次 HTTP 契约；跨场景、跨候选编排不进入 provider。

### 状态与审计

任务继续使用现有 `pending -> processing -> completed | failed` 状态。部分候选成功时任务为 `completed`，`result.partial=true`；全部失败时为 `failed`。每个失败候选都写入失败记录，任务累计 `retry_count`，成功图片进入自管素材存储。

## BDD

### 保存 Ark 模型

当管理员选择图片模型和“火山方舟图片生成”时，表单只显示 API Key，不显示 API Secret；保存后模型使用 `api_protocol=volcengine_ark_images`、`auth_scheme=bearer`。提交完整官方端点时，系统保存规范化后的 API 根地址。

### 生成多个候选

当一个分镜请求 3 个候选时，Worker 按候选 1、2、3 分别调用 Ark，且每次请求关闭组图生成。每个成功响应只产生一个候选素材，不接受一次响应替代多个独立计费候选。

### 临时错误

当候选 2 首次调用返回临时错误时，Worker 只重试候选 2 一次。候选 1 不重复调用；候选 2 再次失败后记录失败并继续候选 3。

### 永久错误

当某候选返回 HTTP `400`、`401`、`403` 或其他非临时错误时，Worker 记录当前及剩余未执行候选为失败，并停止后续 Ark 调用。

### 参考图

存在参考素材时，Worker 安全读取 `/assets/...` 本地文件或允许的远程 URL，编码为 data URL 数组放入 Ark `image` 字段。路径越界、读取失败或无法识别图片类型时在调用前失败，不发送损坏请求。

## SDD

### 数据库迁移

新增 append-only migration，同时重建：

- `ai_models_protocol_check`：删除 `jimeng_visual`，新增 `volcengine_ark_images`。
- `ai_models_type_protocol_check`：图片只允许 `openai_images | volcengine_ark_images`。
- `asset_generation_tasks_provider_check`：删除 `jimeng`，新增 `volcengine-ark`。

迁移在修改约束前检查所有模型记录和任务记录，包括逻辑删除与历史终态记录；发现 `jimeng_visual` 或 `provider='jimeng'` 时明确失败，不自动改写。两种协议的认证与请求语义不兼容，自动迁移会制造不可调用配置。

### 地址规范化

仅当已显式选择 `volcengine_ark_images` 时执行协议内规范化：

- 接受根地址 `https://ark.cn-beijing.volces.com/api/v3`。
- 接受完整地址 `https://ark.cn-beijing.volces.com/api/v3/images/generations`。
- 去除末尾 `/` 或固定 `/images/generations` 后保存根地址。
- Worker 固定追加 `/images/generations`。
- 不根据 URL 反推或切换 `api_protocol`。

地址必须使用结构化 URL 解析，保留合法的 scheme、host 和已有根路径；拒绝 query、fragment、非 HTTP(S) 地址以及无法规范化的路径。

### Admin 表单

图片协议选项改为：

- `OpenAI Images`
- `火山方舟图片生成`

选择 Ark 时自动设置 `auth_scheme=bearer`，只渲染 API Key。默认图片尺寸为空时保存 `supported_sizes=[]`、`default_size=null`，不得生成 `supported_sizes=[""]`。Ark 的 `max_images_per_request` 固定为 `1`；工作台的每分镜 1-4 个候选仍由任务编排为多次独立调用。

### Ark 请求

每次候选请求为：

```json
{
  "model": "<upstream_model>",
  "prompt": "<scene prompt>",
  "sequential_image_generation": "disabled",
  "response_format": "b64_json",
  "watermark": false,
  "stream": false
}
```

非空默认尺寸写入 `size`。存在参考图时写入 `image`，值为单个或多个 data URL；请求日志中必须整体脱敏该字段。

### 响应解析与文件类型

响应必须包含 `data` 数组和一个有效 `b64_json`。`data[].error`、缺失结果、非法 base64 或不支持的图片字节都视为该候选失败。Worker 根据图片 magic bytes 识别 PNG、JPEG 或 WebP，并使用匹配扩展名保存；不得无条件写成 `.png`。

### 错误分类

- 临时错误：网络错误、超时、HTTP `429`、HTTP `5xx`。
- 永久错误：其他 HTTP 错误、响应契约错误、解码错误、配置错误、参考图读取错误。
- 临时错误重试耗尽：只失败当前候选，继续下一个候选。
- 永久错误：停止剩余上游调用，同时保留此前成功结果。

### 请求日志

每次候选 attempt 输出一条结构化请求日志和一条结果日志，至少包含：任务 ID、模型 ID、协议、候选序号、attempt、method、URL、脱敏请求摘要、HTTP 状态、耗时和错误类别。可输出脱敏的 curl 等价信息，但必须满足：

- API Key 和 `Authorization` 值使用 `***`。
- 参考图和结果图 base64 不进入日志。
- 响应只记录字段结构、结果数量、错误摘要和图片字节数。
- 错误摘要继续执行现有长度限制和换行压缩。

### 删除旧实现

删除 Rust `JimengVisual`、Worker `JimengImageProvider`、VisualService SDK 导入、`extract_jimeng_image_urls`、`JIMENG_*` 环境变量导入与 `.env.example` 配置。删除 `ImageModelSettings.request_key`；不保留旧协议别名、URL 猜测或环境变量兜底。

## TDD

### 数据库与 Rust

- 先写 migration 测试，证明新协议组合可保存，旧协议和旧任务 provider 被拒绝。
- 先写 registry 测试，证明 `volcengine_ark_images + image + bearer` 合法，`jimeng_visual` 无法解析。
- 先写地址规范化测试，覆盖根地址、完整端点、末尾斜线、非法 query/fragment 和无关路径。
- 回归文本、OpenAI Images、Runway 与 Kling 现有组合。

### Worker

- 先写 provider factory、请求 JSON、Bearer 认证、参考图 data URL、响应解析和图片类型识别测试。
- 先写逐候选测试：`N` 个候选产生 `N` 次调用；单候选临时错误最多额外一次；成功候选不重复；永久错误停止剩余调用；部分成功正确入库。
- 校验日志不包含 Key、Authorization 原值或任意 base64 正文。
- 删除旧 Jimeng 测试，并证明未安装 `volcengine` SDK 不影响 Worker。

### Admin

- 先写协议选项测试，证明不再出现“即梦 Visual”，出现“火山方舟图片生成”。
- 先写认证联动测试，证明 Ark 只显示 API Key。
- 先写空尺寸测试，证明不会提交 `supported_sizes=[""]`。

### 综合验证

在容器内运行 Rust workspace、Worker 全量测试、Admin 测试/lint/build、OpenSpec strict validate 和 `git diff --check`。自动化验证全程使用 fake HTTP，不调用 Ark。

真实验证前再次确认数据库没有 `pending/processing` 图片任务，后台 Worker 默认关闭。只创建一条单分镜、单候选任务并监控到终态；实际调用上限为首次一次，只有临时错误时允许一次重试。

## 原型设计说明

- 设计系统继续使用项目 `DESIGN.md`，不新增颜色或布局体系。
- 参考 `IBM Carbon` 的表单分组、校验与错误反馈，以及 `GitHub Primer` 的低干扰边框和操作层级。
- 保留现有紧凑抽屉结构，新增 Ark 已选状态；凭据区只显示一个全宽 API Key 字段。
- 不采用营销式大标题、产品摄影、全屏强调色数据区或新卡片层级。

## 非目标

- 不支持 `image + openai_responses`。
- 不保留 `jimeng_visual` 或 `jimeng` 兼容值。
- 不实现 Seedream 组图生成、流式响应或一次调用多候选。
- 不自动跨模型、跨协议或跨供应商重试。
- 不新增视频生成调用。

## 部署与回退

1. 部署前停止可写 API 与图片 Worker，确认旧协议模型、旧 provider 任务和在途图片任务均为零。
2. 应用 append-only migration，再部署 Rust API、Worker 和 Admin。
3. 验证数据库约束、Admin 协议选项、API 保存和 Worker 健康状态。
4. 真实调用只在用户再次确认后执行受控单候选验证。

若 migration 前置检查失败，保持新版本不启动，显式处理冲突记录后重试。回退应用版本时不得回滚已应用 migration；需要追加反向 migration，并重新评估已经保存的 Ark 模型和任务记录。
