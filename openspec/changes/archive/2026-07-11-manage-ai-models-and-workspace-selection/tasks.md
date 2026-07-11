## 1. 规格与原型确认

- [x] 1.1 完成 proposal、design 和五个 capability delta spec，并通过 OpenSpec strict 校验。
- [x] 1.2 用户复核并明确确认书面规格，未确认前不得进入实现。
- [x] 1.3 使用 Pencil MCP 新建后台 AI 模型管理原型，覆盖列表、筛选、编辑抽屉、默认替代和删除确认。
- [x] 1.4 使用 Pencil MCP 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖六类工作台模型选择入口及无可用模型状态。
- [x] 1.5 通过 Pencil MCP 读取、截图和布局检查验证两套原型，并取得用户明确开发确认。

## 2. 数据库模型与仓储

- [x] 2.1 先补 migration 失败测试，覆盖三类模型、协议类型、状态、默认唯一性、凭据字段、乐观锁、模型引用和调用快照。
- [x] 2.2 新增追加式 migration，创建 `ai_models` 并扩展 `agent_runs`、`asset_generation_tasks`，不改写已应用 migration。
- [x] 2.3 先补模型仓储契约测试，覆盖创建、筛选、编辑冲突、默认替换、启停和引用查询。
- [x] 2.4 实现模型领域类型、类型化 settings、协议兼容矩阵和 PostgreSQL 仓储。
- [x] 2.5 实现未引用物理删除、已引用逻辑删除和默认模型替代事务。

## 3. 管理 API 与工作台选项 API

- [x] 3.1 先补管理路由测试，覆盖三类 CRUD、条件校验、密钥掩码、留空不覆盖、稳定错误码和版本冲突。
- [x] 3.2 实现 `/api/admin/models` 列表、详情、创建、编辑、设为默认、启停和删除路由。
- [x] 3.3 先补工作台选项接口测试，覆盖类型过滤、默认排序、无可用模型和敏感字段缺失。
- [x] 3.4 实现 `/api/model-options` 只读 DTO，确保请求地址、凭据和完整运行配置不出现在响应中。
- [x] 3.5 补充日志和错误脱敏测试，确保 API Key/API Secret 不进入响应、快照或日志。

## 4. novex-model 协议驱动客户端

- [x] 4.1 先改写 `crates/novex-model` 客户端测试，用显式 `api_protocol` 覆盖 Responses、Chat Completions 和不支持协议。
- [x] 4.2 实现协议 enum、根地址规范化、协议路径构造和认证配置，删除运行时 URL 后缀推断。
- [x] 4.3 保持 prompt 级输出 Token、Responses SSE、推理等级、超时、错误映射和兼容 User-Agent 行为。
- [x] 4.4 增加模型解析器或 factory 边界，使 backend 能从数据库模型配置创建 request-scoped `LLMClient`，测试可注入 fake client。

## 5. Rust 业务调用链模型路由

- [x] 5.1 先补请求 DTO 和路由失败测试，让账号策略草稿、主题组评审和直接脚本生成强制接收文本 `model_id`。
- [x] 5.2 先补 Agent Runtime 测试，让 script/topic 每轮消息携带 `model_id`，并覆盖缺失、停用和类型不匹配。
- [x] 5.3 实现 request-scoped 模型解析，并让脚本生成/修改、选题生成/补充、质量闸门、重写、主题组评审和策略草稿贯穿同一模型。
- [x] 5.4 在真正调用前写入 `agent_runs.model_id` 与非敏感 `model_snapshot`，模型编辑不得改变历史快照。
- [x] 5.5 保持所有临时错误重试使用同一模型，删除环境变量和硬编码模型运行时兜底。

## 6. 图片任务与 Python Worker

- [x] 6.1 先补后端图片任务测试，将批量生成和单分镜重生从供应商字符串改为图片 `model_id`，保持幂等和费用上限。
- [x] 6.2 实现任务模型引用、类型与状态校验，放宽旧供应商 check 但保留历史 provider 审计字段。
- [x] 6.3 先补 Worker 测试，覆盖数据库加载 `openai_images`、`jimeng_visual`、凭据解析、实际执行快照和不读环境兜底。
- [x] 6.4 将 Worker provider factory 改为模型配置驱动，彻底移除 `image_provider_from_env` 运行时路径。
- [x] 6.5 覆盖模型停用后待执行任务无外部调用、在途任务自然结束、同模型最多一次临时重试和永久错误不跨模型。

## 7. 一次性环境配置导入

- [x] 7.1 先补导入器测试，覆盖文本 OpenAI、OpenAI 图片、即梦、地址规范化、缺失凭据、稳定 `source_key` 和重复跳过。
- [x] 7.2 实现要求显式原文凭据确认参数的独立导入命令，输出不得包含任何真实凭据。
- [x] 7.3 在容器内对测试配置验证导入幂等和不覆盖后台编辑，禁止启动图片 Worker或真实供应商调用。
- [x] 7.4 更新 `.env.example`、Compose 和运行文档，明确环境变量仅用于一次性导入，数据库是运行时唯一来源。

## 8. 管理后台前端

- [x] 8.1 在 Pencil 原型明确确认后，先补 `admin` API wrapper 测试，覆盖筛选、CRUD、默认、启停、删除和掩码 DTO。
- [x] 8.2 先补页面测试，覆盖类型切换、表格字段、表单条件项、编辑留空凭据、版本冲突和各确认弹窗。
- [x] 8.3 实现 AI 模型管理页面、紧凑筛选表格和右侧编辑抽屉，复用现有 `DESIGN.md` 视觉语言。
- [x] 8.4 实现无模型、加载、错误、最近调用状态、默认替换、逻辑删除与物理删除反馈。
- [x] 8.5 补 Admin E2E，使用 mocked API 验证完整生命周期，不触发任何供应商调用。

## 9. 视频工作台前端

- [x] 9.1 在 Pencil 原型明确确认后，先补模型选项 API wrapper 和请求 payload 测试。
- [x] 9.2 先补页面测试，覆盖账号策略草稿、选题生成/补充、主题组评审、脚本确认、脚本对话和图片生成模型选择。
- [x] 9.3 实现按操作类型加载选项、默认选择、允许切换、无模型禁用和停用竞态刷新且保留输入。
- [x] 9.4 将素材生成供应商控件替换为图片模型选择，并让批量任务与单镜头重生提交同一选中 `model_id`。
- [x] 9.5 补工作台 E2E，逐个验证所有真实模型调用入口并确认没有新增视频生成入口。

## 10. 迁移与全量验证

- [x] 10.1 在不启动图片 Worker的前提下应用 migration，运行一次性导入并核验默认模型、协议和凭据配置标记。
- [x] 10.2 运行 Rust workspace 测试、Python Worker 测试、两套前端单测/Lint/构建和 Playwright E2E。
- [x] 10.3 使用 fake provider 验证文本和图片端到端路由、停用拦截、快照和同模型重试，确认零真实模型费用。
- [x] 10.4 使用 Pencil MCP 完成最终截图与布局问题检查，并运行 OpenSpec strict validate。
- [x] 10.5 执行 `openspec instructions apply --change "manage-ai-models-and-workspace-selection" --json`，确认任务进度与实际一致。
- [x] 10.6 运行 diff、敏感信息和影响范围检查，确认未记录真实凭据、未加入鉴权伪实现、未修改无关模块。
