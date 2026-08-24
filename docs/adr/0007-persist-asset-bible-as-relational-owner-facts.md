# ADR-0007：以关系表持久化 AssetBible owner 事实

## 状态

已接受。

## 背景

阶段一早期用 `phase_one_documents` 保存尚未分配 owner 的结构化占位事实。AssetBible 已成为独立聚合，拥有稳定 entry、不可变 version、四层 assignment、resolved snapshot、impact、AcceptDecision、revision task 与 handoff ack；继续整体写入 JSON document 会失去行级 CAS、项目归属、唯一性与不可变约束，并形成双事实源。

## 决策

1. `asset_bibles`、entry/version、relationship、assignment、snapshot、impact、AcceptDecision、revision task 与 handoff ack 使用独立关系表作为唯一 canonical persistence。
2. entry version 只追加且数据库 revision 固定为 1；AssetBible、entry 与 task 使用行级 revision CAS。项目归属、类型、hash、边和 task 去重由 FK、唯一与 check constraints 共同保护。
3. 旧 `phase_one_documents` AssetBible collections 仅在关系表为空时作为迁移输入；首次成功 UoW 提交写入关系表并删除这些旧 collection，之后不再回写。
4. impact preview 只追加 analysis；accept 在一个 UoW 内重新校验 analysis、AssetBible/entry/target revisions、完整 target refs/set hash，并将全部 CAS 输入纳入幂等 fingerprint。成功只追加 successor、AcceptDecision、audit/Outbox 与 revision tasks，不自动改写下游 owner 或调用 Provider。
5. HTTP mutation 使用同值 `If-Match`/`expectedRevision`、canonical `schemaVersion` alias 和 project scope guard；consumer projection 只暴露冻结的 ID/revision/hash 与 owner references。

## 结果

AssetBible 可独立并发、迁移和审计；Scene/Shot、Agent、Provider、Run 与 UI 只能通过 typed command 或只读 projection 集成。legacy document fallback 保留升级路径，但不会长期维持两套可写事实。
