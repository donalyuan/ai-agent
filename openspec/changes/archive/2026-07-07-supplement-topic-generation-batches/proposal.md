## Why

当前内容策略已经支持按历史生成批次查看和管理选题，但已经生成过的历史批次无法再次补充选题。运营人员如果发现某批次选题数量不足或方向需要扩展，只能重新发起一次普通生成，导致新批次与原批次没有明确关系，后续查看和追溯不直观。

用户已确认补充生成应作为“原批次的补充批次”，而不是把新选题直接追加回原批次。

## What Changes

- 为 `topic_generation_batches` 增加补充关系，使一个批次可以声明自己是某个原始批次的补充批次。
- 扩展选题 Agent 生成入口，允许从历史批次详情发起补充生成。
- 补充生成创建新的生成批次，新选题关联补充批次本身，原始批次保持不变。
- 历史生成页展示原始批次与补充批次之间的关系，并支持切换查看补充批次选题。
- 批次查询、统计和软删除规则继续以未软删除选题为准。

## Capabilities

### Modified Capabilities

- `content-topic-management`: 增加选题补充生成批次关系和补充生成规则。
- `topic-history-management`: 增加历史批次详情中的补充入口、补充批次展示和切换查看。
- `conversational-agent-runtime`: 扩展 `topic` Agent 消息输入与输出 metadata，支持补充生成上下文。

## Impact

- 数据库：新增递增 migration，为 `topic_generation_batches` 增加 `supplement_of_batch_id`、注释和索引。
- 后端：扩展 repository、Agent runtime 和 API model，校验补充批次存在、同项目、成功且有可见选题。
- 前端：历史生成页增加补充入口、补充表单、补充批次列表和刷新逻辑。
- 测试：覆盖补充批次关系、错误路径、同项目约束、历史页交互和现有批次统计不回退。
- 原型：更新 `docs/prototypes/video-agent/video-agent.pen`，补充历史批次详情中的补充生成交互。
