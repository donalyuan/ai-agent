# Change: 实施 Provider、Model 与 Skill Catalog

## Registry 与 runnable gate

Catalog 逐项登记 `drama-skills`、`novel-writing`、`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira` 的 provenance、approval、enabled。前两项为 `verified_snapshot`/`approved`/`true`；其余六项为 `pending_provenance`/`not_approved`/`false`。默认 `drama-mvp-a-default` 只绑定前两项；后六项不得成为 Worker 启动或默认 Run 前置，只能在 node `allowedSkills`、`requiredCapabilities` 与 `selectionMode=fixed|inherit` 通过后按需读取。

每个 Provider/Profile/Model operation 保存 `adapterInstalled`、catalog `approval`、`operationCapabilitySnapshot`、`runnable`、`featureGate`。首次显式 connection-test/probe 只要求 installed、approved、`featureGate=MVP-A`、用户 explicit live opt-in、已选 profile、可解析 credential 与 timeout，不得预先要求 snapshot 或 `runnable=true`，也不得因 disabled-for-run 阻断；probe 成功后冻结 successfully probed snapshot。snapshot 缺失、`runnable=false` 与 disabled-for-run 只阻断 enable/default/Run resolve/live operation invocation，后者同时要求 installed、approved、successfully probed snapshot、`runnable=true`、`featureGate=MVP-A`。MVP-B candidate、uninstalled、not-approved 或缺少 explicit opt-in/profile/credential/timeout 的 operation 仍零 probe/外部调用；TTS/ASR、MiniMax H3、Seedance 2.5 与 Agnes 未选中 mode 保持不可运行。

## 原因

阶段 0 从进程内配置选择 Provider 和 Skill。阶段 1 需要可审计、由数据库管理的 catalog，使项目能选择已启用的模型与冻结能力，而不在业务代码中嵌入 Provider 选择或密钥。

## 变更内容

- 增加由数据库管理的 Provider、Profile、Model、CapabilitySnapshot、SkillRevision、项目默认值、参数 schema、ProviderCall 与最小 usage-audit 概念。
- catalog 不持久化 Project 的文本费用阈值；它读取 projects owner 的 immutable threshold snapshot，并只拥有与精确 paid operation 绑定的 `CostConfirmation` 和 usage/cost audit。
- 提供 project/run/node/logical-operation scoped 的脱敏 ProviderCall summary query，供 Run detail 与 ShotCard 显示实际模型 revision、调用状态、native usage 和 cost value/status/source；不返回原始请求/响应、提示词、密钥或媒体位置，也不复制 RunEvent。
- 定义 CRUD、启用/禁用、model sync 候选 diff、workflow node > project default > enabled system default 选择、AES-256-GCM Docker Secret 密钥边界、native usage/保留审计及可诊断失败行为。
- 为每个 Provider/Profile operation 增加并发上限、速率窗口/额度、429/`Retry-After` policy 与 quota snapshot/status；admission 在外部提交前执行，unknown quota 保持 unknown。存在 CapabilitySnapshot、ProviderCall、Run、project default 或 workflow 历史引用的 Model 只允许停用，不允许物理删除。
- 固定数据库与共享 Schema 的 `schema_version` 为 catalog 唯一版本事实；HTTP DTO 的 `schemaVersion` 只映射同一值，缺失、冲突或双独立赋值在 UoW 前失败且无写入。
- 保持现有六个 Port、`DeterministicMockProvider`、显式 Local test/offline profile（adapter identity=`local_workspace`）、`SkillRegistry` 与 `SkillRouter` 为默认测试执行边界；Local 不是真实 Provider/TOS 失败 fallback。

## 能力

### 新增能力

- `provider-model-skill-catalog`：持久化 Provider/model/Skill catalog、项目选择、审计与生命周期要求。

### 修改能力

无。

## 总体计划追溯与边界

本 change 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `2.3`，其唯一职责是 DB catalog、冻结 capability snapshot、SkillRevision、项目绑定、ProviderCall/usage 审计、密钥掩码及 Mock 默认测试。总体计划只协调 change 的依赖 DAG、集成顺序和共享工程规则，不是运行时代码依赖。

直接依赖是阶段 0 已存在的 Provider/Skill foundation：六个 Port、进程内 catalog、`SkillRegistry`、`SkillRouter`、`DeterministicMockProvider` 和显式 Local test/offline profile（adapter identity=`local_workspace`）；它不依赖 `workflows/runs` 的实现。需要关联运行时只保存 `run_id`、`node_run_id` 与 `correlation_id`，不拥有或写入 RunEvent。

完整非目标包括实现真实 Provider SDK/adapter 或外部调用、让 credential 解密越过 adapter boundary、引入 Provider 特定业务逻辑、billing settlement 或无来源 usage 归一化、冻结最终 HTTP path/error envelope、修改阶段 0 的 `Mock Provider +` 显式 Local test/offline profile 配置，以及拥有或实现 WorkflowRun/NodeRun/RunEvent 状态机与事件历史。通用 Provider Credential 的 AES-256-GCM、Docker Secret 主密钥、主密钥缺失时真实 Provider 503 和 `Mock Provider +` 显式 Local test/offline profile 可用性属于本 change 的 catalog/security owner；Provider-specific KMS、SDK 加密和各 UI 自行解密仍为非目标。

Project `textCostConfirmationThreshold` 的 identity/version/value 归 `extend-projects-episodes-creative-slice`；catalog 只消费其 snapshot ID/revision/hash/value/currency，不复制或编辑阈值。SkillRouter 的运行级候选、过滤/排序原因、歧义和人工 selection 归 `integrate-agentscope-text-skills`；catalog 只提供 SkillRevision/lifecycle metadata，设置页启停不构成某次 Run 的路由选择。

## 影响

后续文本、图片和视频 adapter 消费 catalog 选择与冻结 snapshot。预期实现将涉及 API 的 domain/application/adapters/interfaces、迁移、contracts 与测试；本设计既不安装 SDK，也不创建真实 Provider adapter。

## Catalog 与 Credential 具体合同

**DDD**：Provider/Profile/Model/Skill 是 stable entity，Skill 内容只 append `SkillRevision`。**BDD**：create/edit/enable/disable revision conflict 409 零写入；主密钥缺失真实 provider 503，`Mock Provider +` 显式 Local test/offline profile 可用。**SDD**：command/API 使用 expectedRevision/If-Match；AES-256-GCM envelope 固定 algorithm/ciphertext/12-byte nonce/16-byte authTag/keyVersion/aadVersion/canonical profile/credential AAD，`(keyVersion,nonce)` 唯一。**TDD**：rotation/re-encrypt/legacy replacement failure 与禁用不改 snapshot。

## Operation admission 与历史引用保护

**DDD**：catalog 拥有 operation policy/quota observation 和 Model lifecycle，不拥有 Provider worker semaphore。**BDD**：超并发、超速率、429、quota exhausted/unknown 与删除被引用 Model 均有稳定拒绝。**SDD**：policy/snapshot 带 provider/profile/operation/revision/capturedAt/source；`Retry-After` 仅作为冻结 policy 范围内的 retry input。**TDD**：覆盖 restart 下计数恢复、unknown fail-visible、no fallback/no extra ProviderCall 和 disable-only history。
