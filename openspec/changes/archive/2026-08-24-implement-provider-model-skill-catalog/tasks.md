## 0. 总体计划追溯与边界

本 change 对应 `plan-phase-one-drama-mvp-a` 任务 `2.3`；总体 plan 只协调依赖与集成，不是运行时代码依赖。实施前核验阶段 0 Provider/Skill foundation，保持 catalog 与 `workflows/runs` 并行边界：ProviderCall 是 catalog 唯一调用/费用/幂等账本，RunEvent 仅属 workflows/runs，双方只用 `run_id`、`node_run_id`、`correlation_id` 关联。完整非目标是实现真实 Provider SDK/adapter 或外部调用、让 credential 解密越过 adapter boundary、Provider-specific KMS/SDK 加密、引入 Provider 特定业务逻辑、billing settlement 或无来源 usage 归一化、冻结最终 HTTP path/error envelope、修改阶段 0 的 `Mock Provider +` 显式 Local test/offline profile 默认值，以及拥有或实现 WorkflowRun/NodeRun/RunEvent 状态机与事件历史。通用 AES-256-GCM/Docker Secret/缺主密钥 503/`Mock Provider +` 显式 Local test/offline profile 可用性属于本 change 的 catalog/security owner。

## 1. Domain 与 Contracts

- [x] 1.1 定义 Provider、Profile、Model、CapabilitySnapshot、按来源类型区分 source identity 的 SkillRevision、project-default/workflow-node override、ProviderCall、model-sync candidate diff、projects owner threshold snapshot reference、CostConfirmation 与 usage-audit domain contracts，以及 revision、lifecycle、唯一账本、estimate/actual/currency/source/unknown、`run_id + logical_operation`/fingerprint、30 天诊断、稳定 user_uuid、retention policy/version/hold、`CapabilitySnapshot` 与脱敏 `ProviderCall` 摘要长期 no-GC、AES-256-GCM Docker Secret/真实 Provider 503/Mock 可用和 canonical `schema_version` 到 HTTP `schemaVersion` 的单源映射规则；禁止 catalog 持久化第二份项目阈值。
- [x] 1.2 编写失败的定向 domain/contract tests，覆盖 enable/disable、snapshot immutability、workflow node > project > enabled system selection、parameter validation、AES-256-GCM secret masking、缺少 Docker Secret 时真实 Provider 503 且 `Mock Provider +` 显式 Local test/offline profile 可用、Skill network/subprocess/file/secret 审计与未授权拒绝、model diff 未接受、idempotent logical operations、`submission_unknown` reconciliation、native usage/unknown cost、ProviderCall/RunEvent 所有权边界、超过诊断窗口与不同 hold 下 cleanup/GC 拒绝删除、覆盖或静默压缩 `CapabilitySnapshot`/脱敏 `ProviderCall` 摘要，以及版本缺失、冲突或双独立赋值时无 catalog/ProviderCall/usage/Outbox 写入。
- [x] 1.3 实现 framework-free entities、stable errors、parameter/schema validation 与 selection provenance。

## 2. Persistence 与 Application

- [x] 2.1 定义 Repository/UoW 与通用 `CredentialResolver`/secret-resolver ports，保证 AES-256-GCM 解密只在 catalog/security owner 的 adapter boundary，缺主密钥明确映射 503，Provider-specific KMS 不进入本 change。
- [x] 2.2 增加 additive database migration、SQLAlchemy mappings、constraints 和 transaction-safe append-only ProviderCall/usage-audit persistence，且不创建 RunEvent。
- [x] 2.3 实现 CRUD、synchronization、lifecycle、project-default、projects threshold snapshot reader、CostConfirmation 与 call-audit commands/queries，并编写 in-memory/SQLAlchemy 定向 tests；确认只对精确 threshold snapshot/run/logical operation/fingerprint/revision 生效，catalog 无 threshold 写模型。
- [x] 2.5 增加 Skill catalog/runtime route ownership tests：catalog 只提供 candidate metadata，text/Agent runtime 拥有 decision/selection，workflows/runs 冻结最终 revision；Skill lifecycle mutation 不自动解决 pending decision。
- [x] 2.4 定义并实现 project/run/node/logical-operation scoped 的只读 `ProviderCallSummary` query/DTO，返回冻结 Provider/Profile/Model revision、CapabilitySnapshot、status/timing、native usage、cost value/status/currency/source 与脱敏 failure；覆盖 foreign、partial/unavailable、schema drift、unknown 不归零，以及提示词/SourceMaterial/secret/raw payload/media bytes/objectKey/persistent URL 拒绝，证明读取不写 ProviderCall/RunEvent、不触发外部调用或 reconciliation。

## 3. Interfaces 与 Compatibility

- [x] 3.1 增加 catalog management、masked credentials、validation errors 与 project overrides 的 contract/HTTP BDD coverage；HTTP `schemaVersion` 只映射 canonical `schema_version`，缺失或冲突在 UoW 前拒绝。
- [x] 3.2 将 catalog resolution 接至既有六个 Ports、`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）测试组合、SkillRegistry 与 SkillRouter，且不新增真实 adapter；覆盖八项 candidate 的 provenance/approval/enabled、Git commit/digest 与公开 Markdown archive URL/获取时间/digest/license status、默认仅 approved `novel-writing`/`drama-skills` binding、Worker 启动只读取 Registry index/approved metadata、路由后按需读取 `SKILL.md`/references、`allowedSkills`/`requiredCapabilities`/`selectionMode=fixed|inherit` 按需读取，以及首次 connection-test/probe 仅需 installed/approved/MVP-A、explicit live opt-in/profile/credential/timeout、成功后冻结 snapshot 且不需既有 snapshot/`runnable=true` 或 disabled-for-run；snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default/resolve/live invocation，后者需 installed/approved/successfully-probed snapshot/`runnable=true`/`featureGate=MVP-A`；MVP-B/uninstalled/not-approved/缺 opt-in/profile/credential/timeout、TTS/ASR/MiniMax H3/Seedance 2.5/Agnes 未选中 mode 零 probe/外部调用。
- [x] 3.3 增加 architecture tests，证明 `interfaces -> application -> domain` 方向、无 plaintext credential serialization、catalog 不拥有 RunEvent，Skill 未授权访问/脚本被拒绝，诊断至少 30 天且保留 `retention_policy/version/hold/user_uuid`，并证明长期 `CapabilitySnapshot` 与脱敏 `ProviderCall` 摘要不进入自动 cleanup/GC 候选。

## 4. 验证

- [x] 4.1 运行定向 domain/application/adapter/contract/BDD tests，覆盖 retry/duplicate-charge、unconfigured paths、ProviderCall 唯一账本、`schema_version`/`schemaVersion` 同值映射及冲突无写入。
- [x] 4.2 运行 `openspec instructions apply --change implement-provider-model-skill-catalog --json`、`openspec status --change implement-provider-model-skill-catalog --json`、`openspec validate implement-provider-model-skill-catalog --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 与 `git diff --check`。
- [x] 4.3 添加 Provider/Profile/Model/Skill create/edit/enable/disable expectedRevision/If-Match 409 zero-write 测试，以及 SkillRevision append-only/历史 snapshot 保留测试。
- [x] 4.4 添加 envelope 字段/长度/AAD/`(keyVersion,nonce)` unique、Docker Secret 32-byte versioned keyring、cursor rotation/re-encrypt idempotent recovery、legacy replacement failure 与 old-key zero-reference retirement 测试。
- [x] 4.5 定义并实现 `ProviderOperationPolicy` 与 append-only `ProviderQuotaSnapshot` contracts/persistence/queries，覆盖 per-operation maxConcurrency、rate window/limit、bounded admission、429/`Retry-After`、known/unknown/exhausted、restart/lease recovery 和 policy revision conflict；所有拒绝发生在新 ProviderCall/external submit 前且不 fallback。
- [x] 4.6 为 Model delete/disable 添加历史引用 proof：CapabilitySnapshot、ProviderCall、Run、project default、WorkflowVersion 任一引用或 proof unavailable 时 delete 返回 `model_in_use|reference_proof_unavailable`，只允许 disable；覆盖无引用显式删除和 disable 不改历史 identity/snapshot/audit。
