## MODIFIED Requirements

### Requirement:上下文 Agent 只生成受控素材编辑计划
上下文 Agent UI SHALL 展示经 Schema 校验的 `AssetEditPlan` selection/mask/range、费用、impactAnalysis、staleTargets 和确认要求。UI MUST 不直接执行 Provider 或修改 owner；无 capability、版本冲突或未确认费用时必须禁用 execute。

#### Scenario:展示局部编辑计划
- **WHEN** 用户选择同项目 image/video/audio 素材并生成局部编辑计划
- **THEN** UI 显示 base AssetVersion、selection/range、能力快照、费用和受影响引用，等待显式确认
