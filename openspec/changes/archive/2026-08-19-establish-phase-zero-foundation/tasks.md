## 1. 契约与工程基线（SDD）

- [x] 1.1 建立 R1 单仓库目录、Node/Python 工作区清单、锁定依赖和统一本地检查入口；验证空检出可发现 Web、API、Worker、契约和 Compose 边界。
- [x] 1.2 先为 R2 九个跨层对象添加 Draft 2020-12 元 Schema、有效样例和无效样例测试；验证测试在 Schema 尚未实现时失败。
- [x] 1.3 实现 `Project`、`Episode`、`Scene`、`Shot`、`Asset`、`AssetVersion`、`WorkflowDraft`、`WorkflowVersion` 和 `TimelineDocument` 的 JSON Schema 与 `schema_version`；验证正反样例均符合预期。
- [x] 1.4 为层级引用、Workflow 显式 `scopeType`/`scopeIds`、不可变版本引用和整数帧时间添加契约测试；验证缺失作用域、浮点帧和无效引用被拒绝。
- [x] 1.5 为 Web 类型消费和 API Pydantic 边界添加共享样例测试；验证不维护第二套语义不同的文档契约。

## 2. Web 工作台壳层（R1、BDD）

- [x] 2.1 初始化 React 19、TypeScript 和 Vite 8 的 `apps/web`，先添加壳层渲染、类型、lint 与 build 的失败测试/检查入口。
- [x] 2.2 实现最小桌面工作台壳层和 API 健康状态呈现；验证单元测试、`tsc`、lint 与生产 build 通过，且不实现画布、剪辑或手机端功能。
- [x] 2.3 配置契约类型/校验的消费边界；验证 Web 不在本地复制九份业务文档定义。

## 3. API、DDD 持久化与迁移（R3、TDD）

- [x] 3.1 初始化 `services/api` 的 FastAPI、Pydantic 2、SQLAlchemy/Alembic、pytest、格式和类型工具；验证测试、迁移和应用命令可被统一入口调用。
- [x] 3.2 先编写 Project/Episode/Scene/Shot 稳定 ID、父级关系、状态和 `revision` 的领域测试；验证缺失层级、非法状态或过期 revision 失败。
- [x] 3.3 实现最小领域模型、领域服务边界和初始 Alembic migration；验证空数据库 upgrade 后可创建并读取最小项目层级。
- [x] 3.4 先编写 Asset/AssetVersion、WorkflowDraft/WorkflowVersion 和 TimelineDocument 的版本不可覆盖、显式 scope 与整数帧持久化测试。
- [x] 3.5 实现资产、工作流和时间线的最小持久化模型与冲突结果；验证发布/版本记录不能原地覆盖，且过期草稿返回可诊断冲突。
- [x] 3.6 为 Workflow、Storyboard、Asset 和 Timeline 模块的 DDD 所有权添加架构测试或依赖检查；验证工作流定义不复制 ShotSpec 事实。

## 4. Provider、存储与配置边界（R4、R6、TDD）

- [x] 4.1 先为 `TextModelPort`、`ImageGenerationPort`、`VideoGenerationPort`、`TtsPort`、`AsrPort` 和 `StoragePort` 编写协议与替身测试；验证业务服务不需要供应商 SDK。
- [x] 4.2 定义六个 Port 的输入、结果、错误和关联标识并实现确定性 Mock Provider；验证成功、显式错误和零网络/零费用行为。
- [x] 4.3 先编写 Provider/Profile/Model 配置选择测试，包括禁用、缺失配置和默认参数；验证业务代码中没有 model、`base_url`、bucket 或 region 的固定值。
- [x] 4.4 实现 Provider/Profile/Model 的配置或持久化模型、加载器和显式未配置结果；验证同协议模型仅通过配置即可被选择。
- [x] 4.5 先编写 `LocalWorkspaceAdapter` 根目录约束、对象读写、抽象引用和路径逃逸测试。
- [x] 4.6 实现 `LocalWorkspaceAdapter` 的阶段 0 对象操作；验证不持久化绝对路径且拒绝任何工作区外写入。

## 5. SkillRegistry 与确定性路由（R5、TDD）

- [x] 5.1 先为本地 manifest 解析、固定版本、许可证、启用状态和工具边界添加 `SkillRegistry` 测试。
- [x] 5.2 实现 `SkillRegistry` 的 `list`、`search`、`read` 和 `resolve`；验证不完整或禁用 Skill 不进入可路由候选，且不下载或执行第三方 Skill。
- [x] 5.3 先编写 `deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide` 的排序、审计和人工选择测试。
- [x] 5.4 实现 `SkillRouter` 的确定性过滤、关键词/标签排序、策略裁决和审计记录；验证同输入产生同一基础路由。
- [x] 5.5 实现可选 semantic adapter 接口及不可用/低置信/并列回退；验证它不启动独立服务、不会阻止本地启动，并返回人工选择状态。

## 6. Compose、健康与无密钥运行（R7、BDD）

- [x] 6.1 先为 API 与三类 Worker 的健康状态、结构化日志字段和敏感信息脱敏添加测试。
- [x] 6.2 实现 API/Worker 健康端点或探测入口及结构化日志；验证 Mock 调用日志不输出 API Key、认证头或完整私密响应。
- [x] 6.3 创建 `infra/compose` 的 Web、API、PostgreSQL、Temporal、Agent、Generation、Media 服务及其健康检查；验证 `docker compose config` 通过且默认绑定本机。
- [x] 6.4 添加无真实密钥的 `.env.example`、Mock Provider 与 LocalWorkspace 配置；验证缺少真实 Provider/TOS 配置时返回未配置状态且不产生外部网络请求。
- [x] 6.5 在 Docker 可用环境执行 Compose build、up、健康探测和关闭；若环境不可用，保留原始错误并记录已完成的静态 Compose 证据。

## 7. 阶段 0 质量门与追溯（DDD/BDD/SDD/TDD）

- [x] 7.1 将 Schema 正反样例、Pydantic/ORM、Alembic、Port Mock、LocalWorkspaceAdapter、SkillRegistry、SkillRouter、健康端点和无密钥配置测试纳入统一测试命令；验证任一失败保留原始错误。
- [x] 7.2 配置并执行 Web/Python 的格式与类型检查、Draft 2020-12 Schema 校验、Alembic 空库 upgrade 和 `docker compose config`；验证每项输出可判定结果。
- [x] 7.3 建立 R1-R8 到 OpenSpec requirement、BDD 场景、SDD 契约和 TDD 测试的追溯记录；验证所有条目均可定位到实现与验证命令。
- [x] 7.4 复核范围：确认没有真实 Provider/TOS 调用、付费调用、完整生成、专业剪辑、多人、手机端、发布、TikTok、多 Agent 产品能力或 semantic 模型实现。
- [x] 7.5 更新受影响的项目记忆，记录已验证的工程事实、运行/检查命令与未验证限制；验证不把凭据、主机路径或临时错误写入记忆。
