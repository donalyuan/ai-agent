## 1. 现状清单与零费用基线

- [x] 1.1 新增生产 Context 静态 inventory 测试，枚举全部 Rust/Pi LLM node、来源、手工选择/排序/裁剪、完整 Prompt fragment、Pi context blob 与可绕过入口
- [x] 1.2 为项目策略、脚本、选题生成/补充、质量评审/重写、主题组评审、声音和作品节点建立迁移前 Context、最终 Prompt、Schema、参数、调用次数和业务结果 golden fixtures
- [x] 1.3 为 Pi Turn、Tool follow-up、compaction、branch summary、steer/follow-up、原子 Tool request/result 和 Session Tree 建立实际 context hook/fake-provider fixtures
- [x] 1.4 建立共享 tokenizer/context fixtures，覆盖 ASCII、中文、emoji、JSON、Tool Schema、reasoning、多消息、边界窗口、重复/替代/冲突和固定时钟
- [x] 1.5 扩展 canary secret、多模态资产、超长候选、孤立 Tool 消息和历史不完整 binding fixtures，作为脱敏、失败尝试、导出、迁移与删除共同测试数据
- [x] 1.6 实现只读文本模型配置 inventory，列出缺失 context window、Profile、适用性证据和不透明模型名，确认工具不读取或输出凭据
- [x] 1.7 在容器内运行全部迁移前基线并保存零真实模型、零 Tool 外部副作用、零视频生成和零平台发布证据

## 2. Registry v2 与版本化 Context 定义

- [x] 2.1 先编写 Registry v2 schema 失败/成功 fixtures，覆盖 ContextPolicyDefinition、TokenizerProfile、新 node 引用、未知字段、重复版本、owner/source 不兼容和旧 v1 只读兼容
- [x] 2.2 先编写 Rust/TypeScript Policy/Profile canonical serialization、definition digest、registry digest 和 release evidence 跨语言 contract tests
- [x] 2.3 扩展 `agent-definitions` schema/manifest/release index，使 active AgentDefinition 必须精确引用 Prompt 与 Context Policy，旧 node 形态只允许 supported/revoked
- [x] 2.4 为全部现有 Rust/Pi node 建立版本化 Context Policy，并为 exact encoding 与 `utf8-byte-upper-bound@1` 建立明确 applicability/framing/safety 定义
- [x] 2.5 发布引用 Context Policy 的新版 AgentDefinition，保留既有已发布定义内容/digest 并更新不可变 activation evidence
- [x] 2.6 实现 Rust 强类型 loader、引用/生命周期/owner/applicability 校验和启动期 fail-closed self-test
- [x] 2.7 实现 TypeScript 强类型 loader、引用/生命周期/owner/applicability 校验和启动期 fail-closed self-test
- [x] 2.8 扩展 Definition release repository、发布幂等与不可变约束测试，使 Policy/Profile 内容和生命周期 manifest 不可被数据库或原版本覆盖
- [x] 2.9 更新 Rust/Pi 构建镜像打包并验证相同 Registry/asset digest 随发布产物进入两个 Runtime，不依赖生产开发挂载

## 3. 精确 Tokenizer 与声明式保守策略

- [x] 3.1 先编写 TokenizerProfile 解析、适用范围、算法版本、资产 digest、framing 和 safety reserve 的失败/成功测试
- [x] 3.2 固定并引入 Rust `tiktoken-rs` 与 TypeScript `js-tiktoken` 依赖，记录版本和许可，不使用 Pi `chars/4` 作为 Novex 预算入口
- [x] 3.3 建立共享 encoding/framing 资产和 canonical fixture，覆盖 `cl100k_base`、`o200k_base` 及协议消息/Tool/Schema 开销
- [x] 3.4 实现 Rust 精确 tokenizer、特殊 token/Unicode/消息封装计数和 asset digest 验证
- [x] 3.5 实现 TypeScript 精确 tokenizer、特殊 token/Unicode/消息封装计数和 asset digest 验证
- [x] 3.6 实现两端 `utf8-byte-upper-bound@1` 保守计数、显式 applicability 与安全余量，未知模型性质时返回 `tokenizer_profile_unavailable`
- [x] 3.7 运行跨语言逐 fixture token 等价与保守上界测试，并让依赖/资产/实现不一致阻止服务 ready
- [x] 3.8 增加 tokenizer/profile 性能基准和有界缓存测试，确认缓存键包含 Profile version/asset digest 且不会跨版本污染

## 4. Context 合同与确定性 Compiler

- [x] 4.1 先编写 ContextCandidate/tagged payload/asset/atomic group schema tests，覆盖稳定身份、来源版本、fact key、render_order、时间、hash 和非法组合
- [x] 4.2 先编写来源 allowlist、owner、required、过期、supersedes、稳定 identity/hash 去重和 confirmed fact 冲突失败测试
- [x] 4.3 先编写 P0-P4、trust 与 priority 独立、预算选择与 render_order 分离、同层稳定 tie-break、输入乱序和跨语言 digest contract tests
- [x] 4.4 先编写原子 Tool 组、JSON/已确认事实不可截断、required 超限、固定开销超限和输出预算不得借用测试
- [x] 4.5 先编写 ContextSnapshot/ContextCompileAttempt 最小化、decision code、BudgetLedger、固定时钟和脱敏 canonical fixtures
- [x] 4.6 在 `novex-ai-core` 实现共享 Context 类型、稳定错误、ContextCompiler 流水线和 canonical digest
- [x] 4.7 在 Pi Runtime 实现相同 schema/错误/ContextCompiler 语义，并通过全部跨语言 fixtures
- [x] 4.8 增加确定性属性测试，验证候选输入顺序、Map/Object 顺序和重复执行不会改变结果
- [x] 4.9 增加静态测试，禁止 ContextCompiler 调用模型、业务 Repository、Tool 或当前来源覆盖历史证据

## 5. Prompt prepare/finalize 与最终预算复核

- [x] 5.1 先编写 PromptCompiler `prepare/finalize` contract，覆盖固定 System/User 模板、变量、Tool/输出 Schema、动态占位和逻辑消息
- [x] 5.2 先编写完整预算公式测试，逐项断言 System、User 固定部分、Tool、输出 Schema、framing、输出预留、安全余量和动态预算
- [x] 5.3 先编写最终 LogicalModelInput 复核、`context_budget_exceeded`、禁止重裁剪/借用/透明重试和 provider overflow 兼容缺陷测试
- [x] 5.4 实现 Rust PreparedPromptEnvelope、CompiledContext finalize、PromptSnapshot v2 和历史 v1 读取
- [x] 5.5 实现 TypeScript PreparedPromptEnvelope、CompiledContext finalize、PromptSnapshot v2 和历史 v1 读取
- [x] 5.6 对全部 under-budget v1 node 运行 byte/normalized semantic golden，修正任何未批准的空白、顺序、Schema 或参数差异
- [x] 5.7 验证 PromptSnapshot v2 的实际逻辑消息与 Rust provider/Pi hook 最终输入一致，不再保存可漂移的重复候选列表

## 6. 模型配置、Fingerprint 与切换前门禁

- [x] 6.1 先编写 ai_models 文本配置测试，覆盖显式 context_window、Profile key/version、图片/视频空值、历史缺失和 enabled 新写入拒绝
- [x] 6.2 新增 PostgreSQL 可空迁移列 `context_window`、`tokenizer_profile_key`、`tokenizer_profile_version` 及格式/范围注释和索引/约束
- [x] 6.3 扩展模型 Domain/Repository/API DTO 与类型专属校验，新建/编辑 enabled 文本模型必须解析兼容 Profile，且管理响应不暴露凭据
- [x] 6.4 更新一次性环境配置导入，只有显式收到 context window/Profile 时才创建可执行文本模型，不按模型名猜测
- [x] 6.5 扩展 Rust/TypeScript ModelBehavior 和 behavior_fingerprint，把 context window、Profile key/version 与预算相关 settings 纳入 canonical hash
- [x] 6.6 更新 Rust/Pi resolver，在 binding 与每次调用前校验 Profile 存在、状态、applicability 和 fingerprint，缺失时 fail-closed
- [x] 6.7 实现模型 Context readiness/inventory 报告，明确区分凭据轮换、行为漂移、配置缺失和 Profile 不兼容
- [x] 6.8 对当前 enabled 文本模型执行只读 preflight；不得自动修改 Zeek-ai/CYT 或推断 `gpt-5.6-luna`，生产切换前要求操作者显式提供窗口与 Profile

## 7. PostgreSQL Context 持久化与所有权

- [x] 7.1 先编写 PostgreSQL migration/repository tests，覆盖不可变 Snapshot/CompileAttempt、owner/node、digest、状态和采用/排除 payload 约束
- [x] 7.2 新增 `context_snapshots`、`context_compile_attempts`、Conversation/Run Context binding 和 `model_calls.context_snapshot_id` migration/索引/注释
- [x] 7.3 实现 Context audit repository port 与 PostgreSQL repository，保证成功记录不可覆盖、失败记录无虚假 ModelCall
- [x] 7.4 实现 Snapshot + prepared ModelCall 本地事务和 Context/Prompt/digest 一致性校验，任一持久化失败时不得调用 provider
- [x] 7.5 实现 CompileAttempt 失败前持久化、排除项最小化和 canary secret/base64/临时 URL 拒绝
- [x] 7.6 扩展 Run/Step/Conversation 错误关联，使 Context 失败引用 CompileAttempt ID 且保持既有失败收尾
- [x] 7.7 扩展 PostgreSQL owner 删除事务，验证 Session/Run 删除级联、fork/rebind/revoke 不删除和相邻 owner 隔离

## 8. Pi SQLite Context 持久化与 Session Binding

- [x] 8.1 先编写 namespaced SQLite schema/repository tests，覆盖 Context binding、Snapshot、CompileAttempt、ModelCall FK 摘要和不可变约束
- [x] 8.2 新增 `novex_context_snapshots`、`novex_context_compile_attempts` 和 Session Context binding schema，不修改或依赖 Pi 私有表
- [x] 8.3 实现 Pi Context repository 与 Snapshot + prepared ModelCall namespaced 事务，写入失败阻止 provider 请求
- [x] 8.4 扩展 Session 创建、打开、普通 fork、升级 fork 和重启恢复，使 Agent/Prompt/Policy/Profile/model binding 完整固定
- [x] 8.5 扩展 durable deletion intent/reconciliation，清理 Context binding/Snapshot/CompileAttempt/ModelCall 并保留相邻 fork
- [x] 8.6 增加 SQLite 中断、重启、幂等、脱敏、孤立 Snapshot 和删除恢复测试

## 9. Rust AuditedModelExecutor 统一编译链

- [x] 9.1 先改写 AuditedModelExecutor tests，要求 ContextCompileRequest、两阶段 Prompt、最终复核、Snapshot 和 ModelCall 的严格顺序
- [x] 9.2 扩展执行 binding 与 port，使 Executor 持有固定 Policy/Profile、Tokenizer、ContextCompiler、PromptCompiler 和 Context repository
- [x] 9.3 将 AuditedModelRequest 从预装配 fragments 改为 node 变量与原子 ContextCandidate，并保持 output parser/重试/owner 合同
- [x] 9.4 实现 schema/冲突/预算/tokenizer 失败只保存 CompileAttempt、零 ModelCall、零 provider 的失败路径
- [x] 9.5 实现成功 Snapshot -> prepared ModelCall -> provider -> 唯一终态路径，并让每个显式重试独立重新编译/建档且复用固定 binding
- [x] 9.6 实现 provider context overflow 的 `tokenizer_profile_incompatible` 终态和 binding 阻断，禁止同一调用缩短重试
- [x] 9.7 收紧模块可见性与 Bootstrap，使生产 Adapter 无法获得旧 PromptCompileInput、裸 Context helper 或绕过 Executor 的 repository

## 10. Rust 全生产节点原子 Context 迁移

- [x] 10.1 先将项目策略草稿测试改为断言字段级来源、优先级、最终 Prompt 等价和独立 ContextSnapshot，再迁移实现
- [x] 10.2 先将脚本完整生成、metadata、单镜生成测试改为断言原子候选、scene_count 变量和 golden，再迁移全部直接入口
- [x] 10.3 先将会话脚本生成意图与分镜修改测试改为断言用户指令、当前脚本/分镜事实、历史消息和固定 binding，再迁移 Adapter
- [x] 10.4 先将选题普通/补充生成测试改为断言账号策略字段、原始要求、已有选题、历史消息、去重/预算和最终业务结果，再迁移入口
- [x] 10.5 先将质量评审、重写、再评审测试改为断言 candidate/confirmed fact 分离、冲突与各 attempt ContextSnapshot，再迁移完整质量链
- [x] 10.6 先将主题组评审测试改为断言来源版本、原子候选、排序、Run/Step 与 ModelCall 关联，再迁移入口
- [x] 10.7 先将声音推荐和作品意图/规划/修改测试改为断言脚本/素材/作品来源、领域 Gate 和最终 Prompt/业务输出，再迁移入口
- [x] 10.8 迁移 inventory 新发现的其他 Rust 生产文本节点，并更新 node/Policy/source allowlist 与 fixtures
- [x] 10.9 删除 `truncate_for_prompt`、完整 generation_prompt fragment 和旧生产 Context 拼接 helper；静态 inventory 对任一残留入口失败
- [x] 10.10 运行 Rust Agent/Conversation/API 回归，确认业务 URL、DTO、消息 metadata、Run/Step、事务、输出与 Gate 无未批准变化

## 11. Pi Public Context Hook 全路径接入

- [x] 11.1 先编写 Pi wrapper 架构测试，确认公开 context hook 可替换消息，并禁止源码修改、私有表/字段、monkey patch 和复制 Tool/Context Loop
- [x] 11.2 实现 AgentMessage -> ContextCandidate -> AgentMessage 映射，保持 role/content/thinking/asset 与稳定 entry/source 元数据
- [x] 11.3 实现 toolCall/toolResult 原子组校验，孤立、错配或超预算时 fail-closed 且不执行后续 Tool/模型步骤
- [x] 11.4 在公开 context hook 接入固定 Policy/Profile、ContextCompiler 和最终消息替换，并让 before_provider_request 只执行 binding/事务复核
- [x] 11.5 迁移普通 Turn 与 Tool follow-up，断言每个模型步骤/重试有独立 ContextSnapshot/ModelCall 且 SSE 只有一个终态
- [x] 11.6 迁移 compaction 与 branch summary，断言各自 node Policy、预算、Session Tree 和“不升级为正式 Memory”保持不变
- [x] 11.7 迁移 steer/follow-up，断言 P0、来源/entry、原子插入顺序和 Definition/Memory/Gate 不可改写
- [x] 11.8 删除 Pi 单一 context JSON blob、旧 queued fragment 拼接和未治理 messages 路径，更新 Session/API 错误合同与 README
- [x] 11.9 运行 Pi build、lint、fake-provider、工具、SSE、abort、fork、compaction、持久化和 public API compatibility 回归

## 12. Context 审计 API、导出、回放与删除

- [x] 12.1 先编写 Rust/Pi 一致的 Context 列表、详情、导出 DTO/schema contract，覆盖分页、摘要、正文权限、source runtime 与 record hash
- [x] 12.2 实现 Rust ContextSnapshot/CompileAttempt 摘要、脱敏详情和版本化导出 API，并在 ModelCall 详情提供 Context 关联
- [x] 12.3 实现 Pi ContextSnapshot/CompileAttempt 摘要、脱敏详情和版本化导出 API，并在 ModelCall 详情提供 Context 关联
- [x] 12.4 先编写 replay 顺序与零副作用测试，覆盖 Context -> Prompt -> ModelCall、固定时钟、来源变化和缺失历史依赖
- [x] 12.5 实现 Rust/Pi dry-run replay 与结构化 diff，禁止读取当前领域事实覆盖历史、调用模型/Tool 或修改 Session/Run
- [x] 12.6 扩展统一删除、Eval source-deleted 和导出安全测试，确认 Context 原文随 owner 删除而聚合指标可按既有规则保留
- [x] 12.7 执行 canary secret、排除项全文、base64、临时 URL、日志/SSE/错误响应审查，无法脱敏时 fail-closed

## 13. Context Eval 与版本生命周期

- [x] 13.1 先编写 Policy/Profile candidate schema、跨语言 token、确定性、安全、预算、核心 Prompt/业务和任一失败禁止激活测试
- [x] 13.2 扩展 EvalRun/EvalReport definition kind、Context case set、Policy/Profile、选择 diff、BudgetLedger 和 tokenizer 指标持久化
- [x] 13.3 实现全生产 node 零费用 Context baseline runner，逐 node 标记 equivalent/non-equivalent 并记录真实模型调用数为零
- [x] 13.4 实现行为变化 Context candidate 的 zero-cost runner；确需真实质量比较时复用既有显式 case/token/retry/cost 确认门禁
- [x] 13.5 扩展发布校验，要求 Policy/Profile candidate 有不可变通过报告，API 不得在线修改 active Registry
- [x] 13.6 实现 supported 回滚、revoked 阻断、Profile overflow 安全撤销与历史 Snapshot/EvalReport 保留测试

## 14. 历史迁移、显式配置与唯一入口切换

- [x] 14.1 先编写 PostgreSQL/SQLite Context 迁移 dry-run 与幂等测试，覆盖等价、证据不足、不完整历史、已迁移和 parent/fork 关联
- [x] 14.2 实现统一迁移计划报告，按 Runtime/Agent/node 列出 equivalent、context_migration_required、模型配置缺失和不可映射原因且不修改数据
- [x] 14.3 实现 Rust Conversation baseline binding 或只读 `context_migration_required`，保留消息、Run/Step、Definition/model binding 和显式 fork/rebind 来源
- [x] 14.4 实现 Pi Session baseline binding 或只读 `context_migration_required`，保留 Session Tree/entries 并通过公开 SessionRepo 显式 fork/rebind
- [x] 14.5 在备份和前后统计校验下执行实际 migration；不得伪造历史 ContextSnapshot，既有不完整 Run 保持 `legacy_partial_audit`
- [x] 14.6 运行生产切换 preflight，若任一将执行的文本模型缺少操作者确认的 context window/Profile 则明确阻断，不自动回填或降低门禁
- [x] 14.7 在全路径、配置、迁移和回归通过后单次切换新版 active Agent/Policy/Profile 与唯一 Compiler 入口，不保留旧 feature flag/兼容 helper
- [x] 14.8 验证回滚只重新发布 supported Registry/二进制并保留向前兼容表、Context binding/Snapshot/Attempt/ModelCall/Eval 证据，不执行破坏性逆迁移

## 15. 全量验证与事实同步

- [x] 15.1 在容器内运行 Rust format、build、clippy、workspace 全量测试及新增 schema/migration/API/架构静态检查
- [x] 15.2 在 Node.js 24 容器内运行 Pi clean install、build、lint、全量测试、SQLite 重启恢复和高层 audit 回归
- [x] 15.3 在容器内运行 Video Worker 全量 pytest 与 Compose health/readiness/跨服务回归，确认领域 Gate 和非文本任务不受影响
- [x] 15.4 运行全 inventory golden、跨语言 tokenizer/context fixtures、调用计数和确定性重复测试，确认每个实际模型步骤只有一个 ContextSnapshot 与一个 ModelCall attempt
- [x] 15.5 验证常规测试、迁移、replay 和评测真实模型调用数为零，且未产生视频生成、平台发布或其他外部费用
- [x] 15.6 更新 `ARCHITECTURE.md`、Runtime README、模型配置说明和 `docs/memory/agent-foundation-direction.md` 的实际实现事实，不声明未落地 Memory/Planner/UI
- [x] 15.7 运行 `openspec validate establish-governed-context-compilation --strict` 与 apply instructions，确认 tasks 进度、发布/迁移门禁和仓库实际状态一致
