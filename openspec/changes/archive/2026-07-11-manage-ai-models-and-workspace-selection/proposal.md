## Why

当前文本模型和图片模型分别从 Rust API 与 Python Worker 的环境变量读取，后台“模型与路由”仍是占位页，视频工作台也无法在真实调用前选择模型。这使模型配置、API 调用协议、默认路由、停用控制和历史审计彼此割裂，无法支撑文本、图片、视频三类模型的统一治理。

## What Changes

- 新增统一 AI 模型管理，纳管文本、图片、视频三类模型部署以及供应商、API 调用协议、请求地址、上游模型标识、认证凭据、运行参数、默认路由和状态。
- 在 `admin/` 实现模型添加、编辑、启用、停用、设为默认和删除；未引用模型可物理删除，已引用模型只能逻辑删除。
- 新增工作台只读模型选项接口，确保业务前端只能读取匹配能力且已启用的模型，不接触凭据。
- 将账号策略草稿、选题生成与补充、质量闸门与重写、主题组评审、脚本生成与修改、图片候选批量生成和单分镜重生改为显式传递 `model_id`。
- 一次业务操作选择一个模型，并让该模型贯穿内部步骤和同模型重试；禁止静默回退环境变量、硬编码模型或自动跨模型重试。
- 在运行记录和图片生成任务中保存 `model_id` 与不含密钥的调用快照；Python Worker 从数据库模型配置构造图片 provider。
- 提供一次性环境配置导入命令，导入只新增、不覆盖后台编辑；运行时切换后环境变量不再作为模型配置兜底。
- 视频模型本轮只进入管理后台，不新增视频生成页面、Worker 调用或任何可能计费的视频验证请求。
- **BREAKING**：新发起的 AI 调用必须提供有效、类型匹配且已启用的 `model_id`；现有环境变量不再作为运行时默认模型来源。
- 按用户明确确认，API Key 与可选 API Secret 以原文存入 PostgreSQL；管理 API 仅返回掩码，快照、日志和错误响应不得包含凭据。
- 按用户明确确认，本轮不新增管理员认证或权限校验；该风险被显式记录，但不改变管理接口与工作台只读接口的边界。

## Capabilities

### New Capabilities

- `ai-model-management`: 三类 AI 模型的统一数据模型、API 调用协议记录、默认模型、状态流转、删除规则、管理 API 和后台页面。
- `workspace-model-selection`: 视频工作台所有现有真实模型调用入口的模型选项加载、默认选择、显式传递和不可用状态。
- `model-routed-ai-execution`: Rust API 与 Python Worker 基于 `model_id` 和 API 调用协议解析实际配置、记录调用快照并执行同模型重试。

### Modified Capabilities

- `novex-model-llm`: 文本客户端从 URL 推断协议和环境变量配置改为显式 API 调用协议与数据库模型配置。
- `conversational-agent-runtime`: 每轮可能调用模型的 Agent 消息必须携带 `model_id`，并将模型选择固化到运行记录及内部步骤。

## Impact

- 数据库：新增 AI 模型表，扩展 `agent_runs` 与 `asset_generation_tasks` 的模型引用和调用快照，调整图片任务供应商约束。
- Rust：`backend/` 新增模型仓储、管理与选项 API、默认和状态规则；`crates/novex-model` 新增协议驱动客户端配置与校验。
- Python：`services/video-worker` 改为从数据库任务模型配置构造 `openai_images` 或 `jimeng_visual` adapter。
- 前端：`admin/` 新增 AI 模型管理页；`apps/video-agent/` 在账号策略、选题、历史评审、脚本和素材生成调用点增加模型选择。
- 原型：新增后台模型管理 Pencil 原型，并更新 `docs/prototypes/video-agent/video-agent.pen` 中受影响的工作台页面。
- 部署：先执行 schema migration 与一次性环境导入，再切换 API 和 Worker；不得启动真实图片或视频生成验证。
- 安全：凭据原文入库且管理接口无鉴权是用户已确认风险；任何响应和日志仍禁止泄露凭据。
