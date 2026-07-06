# script-agent-mvp Specification Delta

## MODIFIED Requirements

### Requirement: 脚本 Agent API 生成结构化脚本

系统 SHALL 提供脚本 Agent API，将用户输入或已确认选题转换为包含标题、hook 和有序分镜的结构化脚本。脚本生成请求 MAY 携带 `topic_id`；当携带 `topic_id` 时，系统 SHALL 校验选题归属、选题状态，并在生成成功后保存选题关联和选题快照。

#### Scenario: 用户从已确认选题生成脚本

- **GIVEN** 数据库中存在一个项目
- **AND** 该项目下存在一条 `approved` 选题
- **WHEN** 用户提交 `project_id`、`topic_id`、`style` 和 `scene_count`
- **THEN** 系统 SHALL 返回新建脚本 ID、标题、hook、状态和有序分镜列表
- **AND** 系统 SHALL 将脚本保存到 `scripts`
- **AND** `scripts.topic_id` SHALL 指向该选题
- **AND** `scripts.content.topic_snapshot` SHALL 保存生成时的选题快照
- **AND** 系统 SHALL 将该选题状态更新为 `scripted`

#### Scenario: 非 approved 选题不能生成脚本

- **GIVEN** 数据库中存在一条状态为 `idea` 或 `archived` 的选题
- **WHEN** 用户使用该 `topic_id` 请求生成脚本
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建脚本
- **AND** 系统 SHALL NOT 更新选题状态

#### Scenario: 选题与项目不匹配时拒绝生成

- **GIVEN** 数据库中存在项目 A 和项目 B
- **AND** 项目 A 下存在一条 `approved` 选题
- **WHEN** 用户使用项目 B 的 `project_id` 和项目 A 的 `topic_id` 请求生成脚本
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建脚本
