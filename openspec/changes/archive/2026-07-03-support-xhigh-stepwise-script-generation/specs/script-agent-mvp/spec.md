# script-agent-mvp Specification Delta

## ADDED Requirements

### Requirement: xhigh 推理等级使用分步脚本生成

当脚本 Agent 使用 Responses API 且 `OPENAI_REASONING_EFFORT` 为 `xhigh` 时，系统 SHALL 使用分步串行生成模式生成结构化脚本，避免单次完整脚本请求触发供应商上游超时或 `502 upstream_error`。

#### Scenario: xhigh 下生成完整脚本

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为 `xhigh`
- **AND** 用户已创建内容项目
- **WHEN** 用户提交 `project_id`、`topic`、`style` 和 `scene_count` 到 `POST /api/scripts/generate`
- **THEN** 系统 SHALL 先请求 LLM 生成 `title` 和 `hook`
- **AND** 系统 SHALL 按分镜序号串行请求 LLM 生成单个 `scene`
- **AND** 系统 SHALL 聚合为一个完整脚本响应
- **AND** 响应结构 SHALL 与非分步生成模式保持一致
- **AND** 系统 SHALL 将脚本保存到 `scripts`
- **AND** 系统 SHALL 将所有分镜保存到 `scenes`

#### Scenario: xhigh 下保持分镜顺序和数量

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为 `xhigh`
- **WHEN** 用户请求生成 `N` 个分镜，其中 `N` 在 3 到 12 范围内
- **THEN** 系统 SHALL 返回严格 `N` 个分镜
- **AND** 分镜 `sequence` SHALL 从 1 到 `N` 连续递增
- **AND** 任一单分镜输出序号不匹配时，系统 SHALL 视为无效 LLM 输出并重试该步骤

#### Scenario: 非 xhigh 配置保留完整生成路径

- **GIVEN** `OPENAI_REASONING_EFFORT` 未设置为 `xhigh`
- **WHEN** 用户请求生成脚本
- **THEN** 系统 SHALL 保留现有完整脚本一次性生成路径
- **AND** 系统 SHALL 不强制拆分为单分镜请求
