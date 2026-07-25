## Why

Novex 现有 Rust 与 Pi LLM 节点仍由分散代码、任意 `system_prompt` 和运行级模型快照驱动，无法证明一次模型调用实际使用了哪一版角色、Prompt、结构化输入与模型行为配置。现在需要在继续建设 Context、Memory 和领域 Agent 之前建立统一、不可变、可回放的执行基线，否则后续演进会放大不可审计、静默漂移和双轨治理风险。

## What Changes

- 建立随代码发布的 Definition Registry，统一治理版本化 `AgentDefinition`、`PromptDefinition`、结构化 manifest 与模板，并由 Rust/TypeScript 强类型 loader 只读加载各自负责的定义。
- 建立固定 System/User 分层的 `PromptCompiler`，按信任等级隔离动态输入并严格校验变量、版本引用、输出契约与 Tool/能力边界；编译失败时禁止降级执行。
- Session 创建时固定 Agent/Prompt 版本与 `model_id + behavior_fingerprint`；版本或模型行为变化必须显式 fork 或重新绑定，凭据轮换可透明继续，撤销版本不得破坏历史快照。
- **BREAKING**：Pi Session 创建 API 删除任意 `system_prompt` 输入，改为必填 `agent_key`；保存过自定义 system prompt 的历史会话只能读取，并须显式 fork 后才能继续执行。
- 用户插话只作为可审计的 `steer`、`follow-up` 或领域修改请求影响后续生成，不得覆盖版本化定义、已确认事实、正式 Memory 或领域 Gate。
- 为每次实际模型调用及每次重试建立独立不可覆盖的 `ModelCall`，在外部请求前持久化脱敏后的完整逻辑输入，并记录输出、错误、Tool Schema、Context/Memory 来源和模型行为快照。
- 提供统一的模型调用摘要、脱敏详情、导出和无副作用 dry-run replay API；真实模型对比必须新建有明确预算的 `EvalRun`。
- 建立 `candidate -> active -> supported -> revoked` 生命周期与静态验证、dry-run、安全、质量和限额评测门禁；真实模型评测必须先取得预算确认。
- 一次性迁移全部现有 Rust/Pi 生产 LLM 节点，不保留旧 Prompt 或未审计模型调用双轨；历史数据按可证明程度回填，不伪造缺失快照。
- 保持现有 Rust Prompt 语义、输出 Schema、模型参数、视频 Conversation API、业务结果和领域 Gate；不迁移视频业务执行到 Pi，不新增 Admin UI、Context Compiler、正式 Memory、Planner 或虚拟制作团队。

## Capabilities

### New Capabilities

- `agent-definition-registry`: 定义代码级 Agent/Prompt 注册表、版本生命周期、强类型加载、Prompt 编译、Session 固定与历史迁移规则。
- `model-call-audit`: 定义跨 Rust/Pi 的调用前持久化、脱敏快照、查询导出、保留删除与无副作用回放契约。
- `agent-prompt-evaluation`: 定义 candidate 激活门禁、评测预算、不可变报告、回归基线与版本撤销/回滚规则。

### Modified Capabilities

- `personal-agent-runtime`: Pi Session 创建改用 `agent_key`，并由组合式 wrapper 通过 Pi 官方公开 hook/API 注入编译 Prompt、调用审计和用户插话边界。
- `local-agent-session-persistence`: Pi SQLite 增加固定 Definition/模型行为绑定、调用快照关联和历史自定义 system prompt 的只读迁移语义。
- `agent-runtime-kernel`: Rust Adapter 通过定义注册表和受审计模型调用入口执行全部现有 LLM 节点，同时保持领域运行与 Gate 归属。
- `conversational-agent-runtime`: Rust Conversation 从逐轮任意选模改为 Session 固定模型行为，并补充定义版本固定与显式迁移规则。
- `model-routed-ai-execution`: 模型配置解析增加 `behavior_fingerprint` 判定，模型调用快照从运行级扩展为每次调用级且禁止未审计外部调用。
- `novex-foundation-architecture`: 明确跨 Rust/Pi 的 Definition、Prompt 编译、模型调用审计与评测边界，以及不得修改 Pi 私有实现或复制 Tool Loop 的约束。

## Impact

- 影响 `backend/`、`crates/novex-agent`、`crates/novex-ai-core`、`crates/novex-model`、`crates/novex-eval`、`services/agent-runtime/` 及仓库级 Definition Registry 新目录。
- PostgreSQL 需新增定义发布、Session/Conversation 绑定、`ModelCall`、`EvalRun`、`EvalReport` 等不可变记录；Pi SQLite 需新增 Definition/模型绑定和 Pi 调用审计关联结构。
- Pi Session 创建与历史会话继续执行协议发生破坏性变化；Rust 视频 Conversation API 的外部 URL、请求/响应和领域行为保持不变，但内部模型选择生命周期将迁移到 Session 绑定。
- 需要覆盖所有现有 Rust/Pi LLM 节点的 golden regression、迁移、静态检查、fake-provider、审计完整性和跨服务回归测试；默认验证不得调用真实模型或产生外部费用。
