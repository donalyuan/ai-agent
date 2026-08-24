# ADR-0004：分离 Skill 来源与保护长期审计事实

- 状态：已接受
- 日期：2026-08-22

## 背景

阶段一需要同时管理 AgentScope 2.x 运行时、Git Skill 与公开 Markdown Skill。三者不能共用 `third_party/skills/<name>/<commit>`：AgentScope 是 Worker 运行时依赖，公开 Markdown 也没有可信 Git commit。另一方面，`retention_policy/version/hold`、append-only 和临时文件清理边界尚不足以证明跨 owner 的关键审计事实不会被自动清理。

## 决策

- AgentScope 2.x 作为 Agent Worker 的独立 runtime dependency，由依赖清单与 lock 管理，不作为 SkillRevision 或 Skill vendor 内容。
- Git Skill 使用 commit/digest 固定 source identity；公开 Markdown Skill 使用 archive URL、获取时间、digest 与 license status 固定 source identity。
- Worker 启动只读取 Registry index 与 approved metadata；SkillRouter 确定选中 revision 后才按需读取固定快照中的 `SKILL.md` 与必要 `references`。
- `RunEvent`、`AcceptDecision`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和仍被引用的 `AssetVersion` 属于长期 no-GC 事实。诊断窗口到期、Worker temporary/derivative cleanup、容量维护、恢复和 GC 不得删除、覆盖或静默压缩这些事实。
- 只有明确无引用且符合 retention policy 的临时对象可以清理；清理长期事实或被引用 AssetVersion 的尝试必须拒绝或跳过并留下稳定诊断。

## 后果

- `SkillRevision` 需要按来源类型表达 source identity，不能继续强制所有来源提供 `source_commit`。
- AgentScope runtime lock、Skill provenance、Registry startup 和 lazy-loading 需要分别提供合同与失败测试。
- 总体 OpenSpec、各事实 owner child change、Agent Worker 清理边界与 operations resilience 必须包含同一跨 owner retention/no-GC 验收，并将证据纳入 `E2E-MVPA-001`。
- AgentScope runtime dependency、八项 Registry candidate、approved revision、按来源类型的 provenance、启动只读 metadata 与路由后惰性读取已由 `integrate-agentscope-text-skills` 实现并通过测试；catalog 持久化、完整 capability snapshot 与 retention/no-GC cleanup 仍由后续 child 实施。
