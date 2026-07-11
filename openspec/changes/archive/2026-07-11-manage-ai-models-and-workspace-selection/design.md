## Context

Novex 当前存在两套彼此独立的模型配置路径：Rust API 通过 `OPENAI_*` 环境变量构造文本 LLM 客户端，Python Worker 通过 `OPENAI_IMAGE_*`、`JIMENG_*` 环境变量构造图片 provider。`admin/` 的“模型与路由”仍是占位入口，`apps/video-agent/` 除图片供应商外没有统一模型选择，调用记录也不能稳定回答“本次执行使用了哪一个模型部署和哪一种 API 调用协议”。

本 change 跨越 PostgreSQL、Rust API、`crates/novex-model`、Python Worker、管理后台和视频工作台。用户已确认同时纳管文本、图片、视频三类模型，采用每类默认模型且工作台允许覆盖；一次业务操作只选择一个模型并贯穿内部步骤。用户同时明确接受两项风险：API Key/API Secret 原文入库，本轮管理接口不做认证与权限校验。

## Goals / Non-Goals

**Goals:**

- 建立文本、图片、视频三类模型的统一注册、默认路由、启停和删除规则。
- 将供应商与 API 调用协议分开记录，并由协议显式选择 provider adapter。
- 让现有所有真实模型调用入口显式传递 `model_id`，不再依赖环境变量或硬编码供应商。
- 让 Rust API 与 Python Worker 使用同一数据库模型配置，并保留不含凭据的调用快照。
- 提供可验证、幂等、不会覆盖后台编辑的一次性环境配置导入路径。
- 维持现有图片费用上限和人工跨供应商决策，不新增任何真实视频调用。

**Non-Goals:**

- 不实现作品生产或视频生成调用链。
- 不实现自动模型路由、负载均衡、故障切换或跨模型重试。
- 不实现模型计费账单、Token 统计或供应商合同管理。
- 不实现管理员登录、RBAC、组织级模型隔离或项目级默认模型。
- 不加密数据库中的 API Key/API Secret；该风险由用户明确接受。
- 不通过计费生成请求测试模型连接。

## Decisions

### 1. 使用统一模型部署聚合，不拆分三套表

新增 `ai_models` 表，每条记录代表一个可独立调用的模型部署。相同上游模型若使用不同供应商、地址、协议或凭据，必须建立不同记录。这样可以共享 CRUD、默认、状态、删除和审计规则，同时让类型专属参数保持明确。

替代方案是按文本、图片、视频拆表，类型约束直观但会复制管理和路由逻辑；全部塞入无约束 JSONB 虽然灵活，但无法可靠建立默认唯一性和协议兼容约束。两者均不采用。

### 2. 公共字段结构化，类型专属能力使用受校验 JSONB

`ai_models` 采用以下字段：

- 标识与展示：`id`、`display_name`、`model_type`、`provider_name`、`upstream_model`、`sort_order`、`remark`。
- 协议：`api_protocol`、`protocol_version`、`auth_scheme`、`request_base_url`。
- 凭据：`api_key`、可空 `api_secret`。按用户确认以原文保存。
- 运行参数：`timeout_seconds`、可空 `reasoning_effort`、可空 `max_output_tokens`、`settings`。
- 状态：`status`、`is_default`、`last_call_status`、`last_call_at`、`last_error_summary`。
- 生命周期：`source`、可空 `source_key`、`version`、`deleted_at`、`created_at`、`updated_at`。

`model_type` 取 `text/image/video`，`status` 取 `enabled/disabled/deleted`。首批协议为：

- 文本：`openai_responses`、`openai_chat_completions`。
- 图片：`openai_images`、`jimeng_visual`。
- 视频：`runway_api`、`kling_api`。

`request_base_url` 始终表示 API 根地址，adapter 根据协议追加稳定路径，禁止再通过 URL 后缀猜测协议。一次性导入器负责把旧的 `/responses` 或 `/chat/completions` 完整地址规范化为根地址。

`auth_scheme` 首批支持 `bearer` 与 `access_key_secret`。`settings` 必须先反序列化为类型化配置再使用：图片包括支持尺寸、默认尺寸、单次最大图片数和即梦 `request_key`；视频包括分辨率、宽高比和时长范围。未知字段不得直接进入供应商请求。

### 3. 数据库与应用共同维护默认和状态规则

数据库使用部分唯一索引保证每个 `model_type` 最多一个未删除默认模型。应用服务在事务中维护以下规则：

- 第一条启用模型自动成为该类型默认模型。
- 只有启用且未删除模型可以设为默认。
- 停用或删除默认模型时，若仍有其他启用模型，请求必须提供 `replacement_model_id` 并原子替换。
- 若不存在其他启用模型，允许明确停用或删除默认模型，该类型进入无默认、不可调用状态。
- 编辑使用 `version` 乐观锁，冲突返回 `model_version_conflict`。

### 4. 引用决定物理删除或逻辑删除

未被运行记录或生成任务引用的模型可以物理删除。已被引用的模型只能设置 `status=deleted` 和 `deleted_at`，从管理默认列表和工作台选项中移除，但保留历史关联。逻辑删除模型不得恢复、不得成为默认、不得发起新调用。

### 5. 管理 DTO、工作台 DTO 与数据库凭据严格分离

管理 API 使用 `/api/admin/models`，支持列表、详情、创建、更新、设为默认、启停和删除。管理响应只返回 `api_key_masked`、`api_secret_masked` 与是否已配置标记，不返回原文；编辑请求凭据留空表示保持原值。

工作台使用 `GET /api/model-options?type=<text|image|video>`，只返回 `model_id`、显示名称、类型、供应商、协议、上游模型标识和默认状态。该接口不返回请求地址、运行参数或任何凭据字段。

本轮不为管理 API 增加鉴权。由于现有后端使用通配 CORS，该选择会允许所有可访问 API 的用户修改模型配置；风险在 proposal、design 和实现注释中明确记录，不以掩码响应冒充访问控制。

### 6. 每次业务执行显式选择模型

以下入口必须传递 `model_id`：

- AI 账号策略草稿。
- topic Agent 首次生成与历史补充生成。
- 主题组评审。
- 从选题确认生成脚本。
- script Agent 生成和修改脚本的每轮消息。
- 图片候选批量生成和单分镜重生。

手工新增选题、读取列表、更新状态和准备脚本参数不调用模型，不展示选择器。视频模型本轮只在管理后台出现。

同一业务操作内的意图解析、主生成、质量闸门、最多一次重写和允许的临时错误重试都复用同一已解析模型配置。对话不永久绑定模型，下一轮消息可以选择另一个模型。

### 7. 在真正调用前解析状态并写入快照

`agent_runs` 和 `asset_generation_tasks` 新增可空 `model_id` 与 `model_snapshot`。历史记录允许为空；新调用由应用层强制非空。快照包含显示名称、类型、供应商、协议、协议版本、请求根地址、上游模型标识、推理等级、超时和非敏感类型参数，不包含 API Key/API Secret。

文本调用在请求开始时解析模型、校验状态和类型，然后创建 request-scoped `LLMClient` 并保存快照。图片任务创建时保存 `model_id`，Worker 真正取任务时重新检查模型状态并保存实际执行快照，再构造 provider。这样后台编辑发生在排队期间时，快照仍准确描述真正执行的配置。

### 8. 停用阻止待执行调用，但不伪造在途取消

停用后立即拒绝新请求。Worker 领取尚未执行的任务时若发现模型已停用或删除，将任务标记失败且不调用供应商。已经发出的 HTTP 请求无法保证撤销，允许自然完成并记录真实结果。默认变化只影响后续选择，不修改历史执行。

稳定错误码为 `model_not_found`、`model_disabled`、`model_type_mismatch`、`protocol_not_supported`、`invalid_model_config`、`no_default_model` 和 `model_version_conflict`。供应商错误摘要不得包含凭据。

### 9. 由 API 调用协议显式选择 adapter

`crates/novex-model` 的 OpenAI-compatible 文本客户端接收显式 `api_protocol`，不再用 URL 是否以 `/responses` 结尾决定请求格式。`openai_responses`、`openai_chat_completions` 分别构造对应路径和请求体。

Python Worker 按 `openai_images` 或 `jimeng_visual` 构造已有 provider。供应商名称只用于展示，不能参与 adapter 分支判断。视频协议只做字段和能力校验，不实例化 provider。

### 10. 一次性导入后彻底停止运行时环境兜底

新增独立导入命令，要求显式传入确认参数后才会把原文凭据写入数据库。导入器识别现有文本 OpenAI、OpenAI 图片和即梦环境变量，为每种部署写入稳定 `source_key`；相同 `source_key` 已存在时跳过，绝不覆盖后台编辑。

部署顺序为：应用 schema migration；运行导入器；校验已启用模型与默认模型；部署数据库路由版本 API 和 Worker；从运行环境移除模型配置变量。运行时代码不得在数据库无模型或模型不可用时回退环境变量。

### 11. 前端采用紧凑管理表格和就近选择器

`admin/` 的模型页采用类型切换、筛选工具栏、紧凑表格和右侧编辑抽屉。设计参考 Ant Design 的后台信息密度、IBM Carbon 的表单分组与校验、GitHub Primer 的状态与行操作；复用根级 `DESIGN.md`，不引入营销卡片、嵌套卡片或新视觉体系。

`apps/video-agent/` 在每个真实调用命令附近放置模型选择器。选择器按操作类型加载选项并自动选中默认模型；若选中模型在提交前被停用，后端拒绝，前端刷新选项并保留用户输入。

正式编码前新增后台模型管理 Pencil 原型，并更新 `docs/prototypes/video-agent/video-agent.pen` 的账号策略、当前选题池、历史生成、脚本生成确认、脚本 Agent 和素材生成页面。所有原型必须经 Pencil MCP 读取、截图和布局检查后由用户明确确认。

## Risks / Trade-offs

- [凭据原文入库] 数据库、备份或只读查询权限泄露会直接暴露全部供应商凭据。→ 按用户最终确认执行；API、快照、日志和错误响应只使用掩码或完全省略凭据，并在后续 change 引入加密。
- [管理接口无鉴权] 可访问 API 的用户可以篡改模型路由、消耗额度或中断 AI 功能。→ 按用户最终确认暂不处理；保持管理与工作台 DTO 隔离，并把管理员认证列为后续阻塞级安全工作。
- [任意请求地址导致 SSRF] 无鉴权管理者可配置内部地址。→ 仅接受 `http/https`、禁止 URL 内嵌凭据并记录风险；彻底消除仍依赖后续管理员权限与网络出口策略。
- [停用无法撤回在途请求] 已发出的第三方请求可能继续计费。→ 在请求前和 Worker 领取时校验状态，UI 明确停用只保证阻止新调用和待执行任务，不伪造取消结果。
- [环境切换造成能力中断] 未导入默认模型就部署新运行时会让生成接口不可用。→ 部署顺序强制先导入与校验，运行时不提供隐式兜底。
- [协议增长] 新供应商可能需要不同认证或请求结构。→ 新增显式协议 enum 和 adapter，每个协议独立校验与测试，不允许用任意 JSON 请求模板绕过代码审查。

## Migration Plan

1. 新增 `ai_models` 表和约束；为运行、任务添加可空模型引用与快照，保留历史数据。
2. 实现并测试模型仓储、管理 API、只读选项 API和协议适配，不切换运行时。
3. 运行一次性导入命令，将现有文本、OpenAI 图片和即梦配置写入数据库；不启动图片 Worker。
4. 校验每个已配置类型的默认模型、协议、地址和凭据配置标记，禁止输出凭据。
5. 切换 Rust API 与 Python Worker 为数据库配置；新调用强制 `model_id`。
6. 更新两套前端与 Pencil 原型，使用 fake provider 完成测试。
7. 移除运行时模型环境变量依赖，保留导入器读取能力仅用于明确的一次性命令。

回滚时可以回退 API/Worker 代码并临时恢复旧环境变量；数据库 migration 保持追加式，不删除已导入模型或历史模型引用。回滚不得自动启动图片 Worker或触发供应商调用。

## Open Questions

无。产品边界、API 调用协议、默认路由、停用语义、删除语义、凭据原文入库和无鉴权风险均已由用户确认。
