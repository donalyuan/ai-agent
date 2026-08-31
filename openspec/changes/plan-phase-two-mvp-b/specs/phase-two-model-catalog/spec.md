## ADDED Requirements

### Requirement:手工模型录入与目录生命周期
系统 SHALL 允许用户为已登记 Provider/Profile 手工录入 ModelCatalogEntry，并维护模型标识、operation 能力、参数 Schema、价格单位、并发/限流、feature gate、runnable、provenance 和 revision。创建、编辑、启用、停用和默认绑定 MUST 使用 project/system scope、expectedRevision/If-Match 和审计；不得把模型名写死在业务代码中。

#### Scenario:录入并启用已 probe 模型
- **WHEN** 用户为已安装且 approved 的 Provider/Profile 录入模型并填写合法 capability/parameter schema，且该 operation 已通过显式 probe
- **THEN** 系统创建可审计目录记录，允许显式 enable/default/run，历史引用保持可读

#### Scenario:拒绝不完整模型录入
- **WHEN** 模型缺 Provider/Profile、operation、参数 Schema、feature gate、费用状态或 capability snapshot
- **THEN** 返回 validation/unconfigured，不创建可运行模型、不修改默认绑定、不产生 ProviderCall

### Requirement:模型同步使用 candidate diff 和显式接受
对于 OpenAI-compatible Provider，系统 SHALL 支持带认证、超时和脱敏原始响应的 `/v1/models` 同步，并把新增、变更、移除保存为 candidate diff。同步 MUST NOT 自动启用模型、替换全局/项目默认、修改 capability snapshot 或改变历史 Run。

#### Scenario:查看并接受同步差异
- **WHEN** `/v1/models` 返回新增或参数变化，用户逐项确认 candidate diff
- **THEN** 系统以 CAS 追加/更新目录记录并保存接受审计，未接受项保持 candidate

#### Scenario:同步失败不污染目录
- **WHEN** Provider 返回超时、429、未授权或响应 Schema 无法解析
- **THEN** 保存脱敏失败诊断，既有模型、默认绑定和 capability snapshot 不变，且不重试未知请求

### Requirement:默认绑定与历史引用保护
系统 SHALL 支持 system default、project override 和 workflow-node selection 的优先级解析，并支持显式解绑 system/project default。若优先级链上没有 enabled 且 successfully probed 的模型，新的 Run MUST 以 `unconfigured` 或 `blocked` fail-closed；不得隐式切换到未确认模型。Run start 冻结最终 Model/Profile/CapabilitySnapshot，解绑或停用不得回写历史冻结快照。被 WorkflowVersion、Run、ProviderCall 或 ExportArtifact 引用的模型 MUST 禁止物理删除，只允许停用。

#### Scenario:项目覆盖不改变历史运行
- **WHEN** 用户为项目选择新的 enabled model override
- **THEN** 新 Run 使用新 selection，历史 Run/Export 继续显示原 Model revision 和 capability snapshot

#### Scenario:拒绝删除历史引用模型
- **WHEN** 用户删除存在任一历史引用或无法证明无引用的模型
- **THEN** 返回 `model_in_use` 或 `reference_proof_unavailable`，只提供 disable，不覆盖历史事实

#### Scenario:解绑默认后阻断新运行
- **WHEN** 用户显式解绑项目和 system default，或停用唯一可运行的默认模型，并启动新的 Run
- **THEN** 解析结果为 `unconfigured`/`blocked` 并给出可定位原因；不修改历史 Run/Export 的冻结 selection，不创建 ProviderCall 或 Outbox
