# ADR-0008：规范化 Catalog 与图片候选交接边界

## 状态

已接受

## 决策

Provider/Profile/Model 及 CapabilitySnapshot、SkillRevision、ProviderCall、Quota/Policy、CostConfirmation 等 catalog owner facts 使用关系表持久化；旧 `phase_one_documents` 仅作为一次性迁移输入，新 catalog 写入不得回到通用文档账本。Provider/Profile/Model 使用 revision CAS，调用、能力、Skill 和 quota 事实保留 append-only 语义，调用终态仅按 CAS 更新同一幂等账本行。

GPT Image operation 在任何 ProviderCall、StoragePort 或 AssetVersion 写入前校验 project-owned accepted AssetBible snapshot 的 id/revision/hash、entry version refs 与 pending continuity task。成功结果先登记 immutable、unreferenced image candidate 与 AssetVersion，只有后续 scenes owner 的 exact CAS 才能成为 current；Mock/Local 是默认路径，真实 transport 缺失保持 `unconfigured`。

## 依据

- [阶段一 Catalog change](../../openspec/changes/archive/2026-08-24-implement-provider-model-skill-catalog/design.md)
- [GPT Image change](../../openspec/changes/archive/2026-08-24-integrate-gpt-image-provider/design.md)
