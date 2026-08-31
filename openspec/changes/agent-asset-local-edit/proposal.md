## Why

阶段一 Agent `AssetEditPlan` 只支持 image/video 整体候选编辑，局部选区、mask、视频时间范围和音频时间范围需要独立的输入校验、成本估算、候选影响分析和安全确认。拆成独立 change 可以避免 Agent 直接修改 owner 事实或与 Timeline 编辑语义混用。

## What Changes

- 新增图片局部选区、mask、局部图层和视频/音频时间范围编辑计划。
- 扩展 `AssetEditPlan`、候选、执行 intent、impact/stale 和接受决定 Schema。
- 将局部编辑绑定到基础 AssetVersion、selection、range、Provider capability 和预算快照。
- 结果先登记为候选，用户确认后才通过 owner command 更新明确引用范围。
- 增加局部输入越界、版本冲突、权限、容量、unknown 和无重复收费测试。

## Capabilities

### New Capabilities

- `agent-asset-local-edit`: Agent 局部图片/视频/音频编辑计划、执行、候选和接受闭环。

### Modified Capabilities

- `context-agent-candidate-review-ui`: 增加局部 selection/range 展示、确认和冲突处理约束。

## Impact

- 影响 `services/api` asset_edits、assets、reviews 和 provider operation owner。
- 影响 Agent Worker、Image/Video/TTS/ASR adapter、Storage 和 Generation Worker。
- 必须复用现有 AssetVersion、CAS、ProviderCall、BudgetGate 和 no-GC 约束；不允许 Agent 直接写数据库、覆盖文件或绕过人工确认。
