## 1. 现状清单与零费用基线

- [x] 1.1 新增生产文本模型调用静态 inventory 测试，枚举项目策略、脚本、选题生成、质量评审/重写、主题组评审、声音、作品以及 Pi Turn/Tool Loop/compaction/branch summary 节点，并让未登记入口失败
- [x] 1.2 为全部 Rust 生产节点补齐迁移前 System/User Prompt、输出 Schema、token 参数、调用次数、重试与 Run/Step 顺序 golden fixtures
- [x] 1.3 为 Pi 普通 Turn、Tool 后续 Turn、compaction、branch summary、steer/follow-up 和 fork 补齐 fake-provider 基线测试，确认当前 SSE、Session Tree 与唯一终态
- [x] 1.4 建立 canary secret、多模态资产引用和历史不完整数据 fixtures，作为脱敏、导出、迁移和删除合同的共同测试数据
- [x] 1.5 运行基线测试并记录零真实模型、零视频生成、零平台发布的验证证据

## 2. Definition Registry 与跨语言 Loader

- [x] 2.1 先编写 AgentDefinition、PromptDefinition、版本索引、变量/信任等级和 executor owner 的 JSON Schema 失败/成功 fixtures
- [x] 2.2 先编写 Rust/TypeScript canonical serialization 与 digest 跨语言 contract tests，覆盖字段顺序、默认值、空值、重复 key 和 hash 不一致
- [x] 2.3 创建仓库级 `agent-definitions/` 目录、统一 schema、manifest、独立模板和发布索引结构
- [x] 2.4 为 inventory 中全部 Rust/Pi 节点建立行为等价的 v1 AgentDefinition 与 PromptDefinition，并声明唯一 executor owner、模型能力和 Tool/profile
- [x] 2.5 实现 Rust 强类型只读 loader、canonical digest、引用校验和启动期 fail-closed 校验
- [x] 2.6 实现 TypeScript 强类型只读 loader、canonical digest、引用校验和启动期 fail-closed 校验
- [x] 2.7 更新 Rust 与 Pi 构建/镜像打包，使同一 registry digest 随发布产物进入两个 Runtime，且生产运行不依赖开发挂载
- [x] 2.8 新增不可变 definition release repository 与发布校验测试，禁止数据库模板正文和相同 key/version 不同 digest

## 3. PromptCompiler 与结构化输入

- [x] 3.1 先编写跨语言 PromptCompiler contract tests，覆盖固定 System/User 分层、变量类型、未知/缺失变量、信任等级、来源、输出 Schema、Tool Schema 和大小限制
- [x] 3.2 先编写动态字符串进入 System、无效版本引用、非法 Tool/profile 和编译异常不得降级的失败测试
- [x] 3.3 在共享合同下实现 Rust PromptCompiler 与不可变 PromptSnapshot 类型
- [x] 3.4 在共享合同下实现 TypeScript PromptCompiler 与不可变 PromptSnapshot 类型
- [x] 3.5 把现有 Rust/Pi Context 装配结果转换为带 ID、来源和信任等级的结构化输入，保持当前选择、顺序与裁剪行为不变
- [x] 3.6 对全部 v1 节点运行 golden regression，修正任何未批准的 Prompt、Schema、参数或 fake-provider 业务结果差异

## 4. 模型行为 Fingerprint 与固定 Binding

- [x] 4.1 先编写 Rust/TypeScript behavior_fingerprint contract tests，覆盖协议、规范化地址、上游模型、reasoning、输出上限、context window、行为 settings 与凭据排除
- [x] 4.2 实现跨语言一致的 canonical fingerprint 和 AgentDefinition 模型能力校验，未知能力或配置一律 fail-closed
- [x] 4.3 先编写 Rust Conversation 首次调用原子绑定、并发首轮、同模型继续、不同模型拒绝、仅凭据轮换和行为变化阻断测试
- [x] 4.4 新增 PostgreSQL Conversation/非会话 Run definition 与模型 binding migration、repository 和约束，并实现 `model_rebind_required` 语义
- [x] 4.5 保持 Rust Conversation 既有 URL、请求/响应字段，在首个模型请求前建立 binding，后续 `model_id` 仅用于一致性校验
- [x] 4.6 先编写 Pi Session 创建必须使用 `agent_key`、拒绝 `system_prompt`、固定 binding、普通 fork 继承和显式升级 fork 测试
- [x] 4.7 新增 Pi SQLite namespaced Session binding schema/repository，并把创建、打开、fork 与 ready check 接入不可变 binding
- [x] 4.8 在 Rust/Pi 每次模型调用前重新解析 `ai_models`，允许 fingerprint 不变的凭据轮换，阻断停用、删除、能力不兼容和行为漂移

## 5. ModelCall 持久化与统一审计核心

- [x] 5.1 先编写统一 ModelCall schema/state machine contract tests，覆盖 prepared、唯一终态、root/attempt、owner/node/binding、PromptSnapshot、usage 和错误
- [x] 5.2 先编写调用前持久化失败不得发请求、重试不得覆盖、重复终态拒绝和部分流输出不得静默重试测试
- [x] 5.3 新增 PostgreSQL `model_calls` migration、索引、不可变输入/唯一终态约束、repository 与 Run/Step 关联
- [x] 5.4 新增 Pi SQLite `novex_model_calls` namespaced schema、索引、repository 与 Session/entry 关联，不修改 Pi 上游私有表
- [x] 5.5 实现共享 schema_version、结构化 redactor 和持久化拒绝规则，覆盖凭据、认证头、Cookie、敏感 URL、secret 字段与错误/日志
- [x] 5.6 实现文本脱敏全文和多模态资产 ID/version/hash/MIME 快照，禁止 base64、临时签名 URL 和原始请求头落盘
- [x] 5.7 实现 Rust AuditedModelExecutor，在 compile、prepared 持久化、provider 调用、输出/usage/错误和唯一终态之间保持 fail-closed 顺序
- [x] 5.8 实现 Pi audited `Models`/Provider 组合 wrapper，关闭不可见透明重试并按既有上限/退避为每个显式 attempt 建立 ModelCall

## 6. Rust 全生产节点一次性迁移

- [x] 6.1 先将项目策略草稿与其允许重试的测试改为断言 definition node、结构化输入和独立 ModelCall，再迁移到 AuditedModelExecutor
- [x] 6.2 先将直接脚本完整生成、metadata、单镜生成测试改为断言 v1 golden 与调用级审计，再迁移全部脚本生成入口
- [x] 6.3 先将会话脚本生成意图与分镜修改测试改为断言固定 binding、PromptSnapshot、ModelCall 和既有业务结果，再迁移对应 Adapter
- [x] 6.4 先将选题普通/补充生成测试改为断言账号策略、历史 Context、v1 Prompt 和 ModelCall，再迁移生成节点
- [x] 6.5 先将质量评审、重写、再评审和各 attempt 测试改为断言相同 binding、独立 node/ModelCall 与既有质量结果，再迁移完整质量链
- [x] 6.6 先将主题组评审测试改为断言 definition、结构化来源、输出 Schema、Run/Step 与 ModelCall，再迁移评审入口
- [x] 6.7 先将声音推荐和作品意图/规划测试改为断言 definition、领域 Gate、业务输出与 ModelCall，再迁移对应 Adapter
- [x] 6.8 迁移 inventory 新发现的其他生产文本模型入口，并更新静态 inventory 与 golden fixtures
- [x] 6.9 收紧 Rust Bootstrap 和模块可见性，使生产 Adapter 不能获得裸 LLMClient，并让静态扫描仅允许 provider、AuditedModelExecutor 与测试 fixture
- [x] 6.10 运行 Rust Agent/Conversation/API 回归，确认 HTTP、消息 metadata、业务结果、Run/Step 类型与顺序、事务和失败收尾未发生未批准变化

## 7. Pi 组合式 Wrapper 全路径接入

- [x] 7.1 先编写 wrapper 架构测试，禁止 Pi 源码修改、私有继承、monkey patch、未导出路径、私有字段反射和复制 agent/tool loop
- [x] 7.2 实现持有 AgentHarness 的 Novex wrapper，代理公开 prompt、steer、followUp、compact、navigateTree、abort 与事件订阅，并由同层 SessionCoordinator 代理公开 SessionRepo fork
- [x] 7.3 通过公开 `before_agent_start`/`context` hook 接入固定 Definition、PromptCompiler 和实际 Context 捕获
- [x] 7.4 通过公开 provider request/payload/response hook 与 audited Models 接入 fingerprint 校验、调用前持久化、输出/错误/usage 收尾
- [x] 7.5 通过公开 `tool_call` hook 保持 Tool Gate，并验证 `chat`/`workspace` profile、Novex read/write/edit/bash schema 和 toolContext 不变
- [x] 7.6 迁移普通 Turn 与 Tool Loop 后续 Turn，断言每次实际模型步骤/重试有独立 ModelCall 且 SSE 只有一个终态
- [x] 7.7 迁移 compaction 与 branch summarization，断言固定 binding、独立 ModelCall、Session Tree 行为和“不升级为正式 Memory”不变
- [x] 7.8 迁移 steer/follow-up，断言其作为带来源的 User 层动态输入进入后续调用，且不能改写 Definition、已确认事实、Memory 或 Gate
- [x] 7.9 更新 Pi Session 创建/详情/错误合同与 README，删除任意 system_prompt 执行路径和默认 helpful system prompt
- [x] 7.10 运行 Pi build、lint、fake-provider、SSE、工具、abort、fork、compaction、持久化与 Pi public API compatibility 回归

## 8. 审计 API、导出、回放与删除

- [x] 8.1 先编写 Rust/Pi 一致的列表、详情、导出 DTO contract tests，覆盖分页筛选、摘要默认值、schema_version、source runtime 与 record hash
- [x] 8.2 实现 Rust backend ModelCall 摘要列表、脱敏详情和版本化导出 API，只读取 PostgreSQL 所有数据
- [x] 8.3 实现 Pi Runtime ModelCall 摘要列表、脱敏详情和版本化导出 API，只读取 SQLite 所有数据
- [x] 8.4 先编写 dry-run replay 零模型、零 Tool、零领域写入、零 Session/Run 变更和结构化 diff 测试
- [x] 8.5 实现 Rust/Pi dry-run replay，使用历史 definition/version 与编译输入，拒绝在 replay 入口原地执行真实模型
- [x] 8.6 先编写 Session/Run 明确删除级联、fork/rebind/revoke 不删除以及聚合 EvalReport source-deleted 标记测试
- [x] 8.7 实现 PostgreSQL 所有权删除事务与 EvalReport 来源删除标记，并以 Pi 公开 SessionRepo + namespaced durable deletion intent/reconciliation 实现 SQLite 可恢复删除，确认相邻 fork/Run 数据不受影响

## 9. EvalRun、EvalReport 与版本门禁

- [x] 9.1 先编写 candidate 静态、dry-run、安全、结构化输出、核心质量、token/成本阈值和任一失败禁止激活测试
- [x] 9.2 新增 PostgreSQL EvalRun/EvalReport migration、case set 版本、批准预算、不可变完成报告和 source deletion 字段
- [x] 9.3 实现零费用评测 runner 与 v1 golden baseline report，明确记录真实模型调用数为零
- [x] 9.4 先编写真实模型 EvalRun 缺少确认拒绝、预算固定、达到 case/token/cost/retry 上限停止和 fingerprint 漂移阻断测试
- [x] 9.5 实现需显式预算确认的真实模型 EvalRun 入口，使每个评测 attempt 复用 ModelCall 审计且不覆盖源快照
- [x] 9.6 实现 candidate 发布校验，要求不可变通过报告或 v1 golden baseline 引用，且 API 不得直接在线修改 active Registry
- [x] 9.7 实现 supported 回滚与 revoked 阻断测试，确认新 Session 选择规则、旧 Session 不静默升级和历史评测/审计保留

## 10. 历史数据幂等迁移

- [x] 10.1 先编写迁移 dry-run 和幂等测试，覆盖 Pi 无自定义 Prompt、自定义 Prompt、Rust 已知 agent_type、已有/缺失模型证据和 partial audit
- [x] 10.2 实现迁移计划报告，列出可自动绑定、只读、待首次模型绑定、legacy_partial_audit 和无法映射记录，不修改数据
- [x] 10.3 实现 Pi 无自定义 Prompt Session 到 `personal.general@1` 的幂等 binding 与迁移事件
- [x] 10.4 实现 Pi 自定义 system prompt Session 只读标记、显式 fork、丢弃或降级为可见 user instruction，确保旧文本永不再进 System
- [x] 10.5 实现 Rust Conversation v1 Definition 确定性回填和可信模型 snapshot fingerprint 回填，证据不足时保留待首次绑定
- [x] 10.6 将缺失准确 Prompt/Context 的历史 Run 标记为 `legacy_partial_audit`，禁止创建伪造 ModelCall
- [x] 10.7 在备份与迁移前后统计校验下执行实际 PostgreSQL/SQLite migration，验证 Session/Conversation/Run ID 集合、树、消息和领域数据不丢失

## 11. 唯一入口切换与完整性门禁

- [x] 11.1 增加运行期和 CI 完整性检查，要求 inventory 中每个生产节点存在有效 Definition、Prompt、binding 与调用级审计
- [x] 11.2 在 Rust/Pi 全路径、迁移与回归通过后单次切换唯一生产入口，不保留旧 Prompt、任意 system_prompt、裸 LLM 调用或 feature flag 双轨
- [x] 11.3 验证数据库 release 记录不能反向覆盖代码定义，candidate/active/supported/revoked 与 Session 选择规则符合规格
- [x] 11.4 验证发布回滚只切换代码 manifest/二进制且保留向前兼容新表、历史 binding、ModelCall 和 EvalReport，不执行破坏性逆迁移

## 12. 全量验证与事实同步

- [x] 12.1 在容器内运行 Rust format、build、clippy、workspace 全量测试及新增 migration/API/架构静态检查
- [x] 12.2 在 Node.js 24 容器内运行 Pi clean install、build、lint、全量测试、SQLite 重启恢复和 high-level audit
- [x] 12.3 在容器内运行 Video Worker 全量 pytest 与 Compose health/readiness/跨服务回归，确认视频业务 Gate 和非文本任务不受影响
- [x] 12.4 执行 canary secret、导出、日志、SSE 和错误响应安全审查，确认无凭据、原始认证信息、base64 大对象或自定义旧 system prompt 泄露
- [x] 12.5 执行全 inventory golden regression 与调用计数检查，确认每次模型步骤只有一个入口和一个对应 attempt 记录，且常规验证未调用真实模型
- [x] 12.6 更新 `ARCHITECTURE.md`、相关 README 与 `docs/memory/agent-foundation-direction.md` 的实际实现事实，不新增 Admin UI 或未实现能力声明
- [x] 12.7 运行 `openspec validate establish-versioned-agent-prompt-execution --strict` 与 apply instructions，确认 tasks 进度和仓库实际状态一致
