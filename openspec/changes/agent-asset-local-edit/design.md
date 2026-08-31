## Context

阶段一 Agent `AssetEditPlan` 只支持 image/video 整体候选编辑。局部选区、mask、视频时间范围和音频时间范围需要新的输入事实、Provider capability 和影响分析，不能由 Agent 直接改写 AssetVersion 或 Timeline。

## Goals / Non-Goals

**Goals:**

- 为 image/video/audio 建立可校验的 selection/range 编辑计划和候选闭环。
- 复用现有 AssetVersion、ProviderCall、BudgetGate、CAS、人工确认和 no-GC 约束。
- 在能力不足、版本过期或成本未知时 fail-closed。

**Non-Goals:**

- Agent 不直接执行数据库、Storage 或自由格式 FFmpeg 命令。
- 不改变文本 successor/stale、Timeline 基础命令或 owner 历史事实。

## Decisions

- `AssetEditPlan` 冻结 base AssetVersion、selection geometry、mask/range、provider capability、费用和预期输出；未知字段拒绝。
- execute 只追加 execution intent 和 Outbox，结果先登记候选；accept 必须明确替换镜头、场次、集或选择引用集合。
- 图片局部编辑、视频时间范围和音频时间范围分别按 Provider capability 校验，不支持时返回 `unsupported_feature`，不隐式降级为整版操作。
- 所有输入携带 project scope、base revision/hash、idempotency key；基础版本变化返回 409，禁止重复收费。

## Risks / Trade-offs

- [Risk] 不同 Provider 对局部区域和时间范围语义不同 → 统一 DTO 加 capability mapping，未经 probe 不允许调用。
- [Risk] 局部结果影响多个下游引用 → 先计算 impact/stale 集合，用户确认后再交接 owner。
- [Risk] 大 mask 或长音视频范围消耗过高 → BudgetGate、容量上限和显式确认前置。

## Migration Plan

只新增 Plan/Execution/Candidate/Impact 表和 API，不回填或改写阶段一 AssetVersion；feature gate 默认关闭，旧请求保持原有整体编辑语义。

## Open Questions

- 首批支持的图片区域格式、视频/音频时间范围精度和 Provider 列表需在实现前通过 capability probe 冻结。
