# Account Strategy Profile Design

## Goal

为内容策略沉淀第一版内容账号策略资料。后端继续使用 `projects` 作为内容生产边界和 `project_id` 外键来源，前端展示为“账号”。账号策略资料作为内容策略下的独立二级页面“账号策略”，用户可以手动填写，也可以让 AI 生成策略草稿；草稿必须经人工确认保存后才生效。选题生成、质量闸门和主题组评审读取同一份已保存的结构化策略上下文。

## DDD

`projects` 继续表示内容账号、账号方向或内容生产边界。前端不再把该对象称为“项目”，但后端第一版不迁移既有 `project_id` 链路。

新增 `AccountStrategyProfile`：

- `target_audience`：目标受众。
- `content_pillars`：内容支柱。
- `tone_style`：表达风格和语气。
- `forbidden_topics`：禁区。
- `reference_accounts`：参考账号或参考对象。
- `topic_preferences`：选题偏好。

`accounts` 表仍表示未来发布平台账号，不属于本次范围。

## BDD

运营人员在“内容策略 > 账号策略”查看当前账号策略资料。资料缺失时页面提示补齐，但不阻塞当前选题池中的人工新增选题。

运营人员可以编辑账号名称、定位摘要、描述和结构化策略资料，也可以点击“AI 生成草稿”。AI 草稿基于当前账号资料和用户补充方向生成，只预填表单并展示草稿摘要，不直接保存。运营人员确认或修改后点击保存，页面才展示最新资料；后续选题生成、质量闸门和主题组评审使用最新已保存资料。

当前选题池不展示账号策略区块、策略资料状态/摘要或进入“账号策略”的入口。切换当前账号时，账号策略页、选题池和历史生成展示该账号对应数据，不能混用其他账号资料。

## SDD

数据库新增：

```sql
ALTER TABLE projects
    ADD COLUMN strategy_profile JSONB NOT NULL DEFAULT '{}'::jsonb;
```

API：

- `GET /api/projects` 返回 `strategy_profile`。
- `POST /api/projects` 接受可选 `strategy_profile`。
- `POST /api/projects/:project_id/strategy-profile/draft` 返回 AI 生成策略草稿，不写数据库。
- `PUT /api/projects/:project_id/strategy-profile` 更新账号名称、定位摘要、描述和策略资料。

前端页面：

- 内容策略二级菜单第一版为“账号策略”“历史生成”“当前选题池”。
- “账号策略”是本版策略资料查看、AI 草稿生成、编辑和保存的独立页面。
- “当前选题池”不展示账号策略区块、策略资料状态/摘要或进入账号策略页的入口。
- 后续完整账号管理独立出来时，再复用或迁移本版策略资料能力。

更新接口请求体：

```json
{
  "name": "AI工具变现账号",
  "positioning": "面向普通创作者的 AI 工具变现教程账号",
  "description": "重点讲清楚可执行流程、风险和真实案例。",
  "strategy_profile": {
    "target_audience": "想用 AI 提升副业效率的新手创作者",
    "content_pillars": ["工具教程", "变现案例", "避坑指南"],
    "tone_style": "直接、清晰、少术语，强调步骤和风险",
    "forbidden_topics": ["夸大收益", "灰产引流", "虚假承诺"],
    "reference_accounts": ["某AI工具教程号", "某副业案例号"],
    "topic_preferences": "优先选择能在 60 秒内讲清楚步骤并引导收藏的选题"
  }
}
```

草稿生成请求体：

```json
{
  "direction_notes": "面向想用 AI 做副业的新手，优先做教程和避坑，不要夸大收益。"
}
```

草稿生成响应：

```json
{
  "draft": {
    "target_audience": "想用 AI 提升副业效率的新手创作者",
    "content_pillars": ["工具教程", "变现案例", "避坑指南"],
    "tone_style": "直接、清晰、少术语，强调步骤和风险",
    "forbidden_topics": ["夸大收益", "灰产引流", "虚假承诺"],
    "reference_accounts": ["某AI工具教程号"],
    "topic_preferences": "优先选择能在 60 秒内讲清楚步骤并引导收藏的选题"
  },
  "draft_summary": "草稿偏向教程、案例和风险提醒，适合先验证 AI 工具变现方向。"
}
```

成本和风险控制：

- 草稿生成只允许手动触发，不在页面加载、账号切换或保存后自动触发。
- 单次请求最多调用一次 LLM；临时供应商错误最多重试一次。
- `max_output_tokens` 限制在 1200 到 1500 区间。
- AI 输出必须通过 JSON Schema 和后端字段校验；失败时不预填、不保存。

后端新增统一上下文格式化函数，将账号名称、定位摘要、描述和 `strategy_profile` 格式化为 Agent prompt 片段。以下 prompt 必须使用同一上下文：

- `build_topic_generation_prompt`
- `build_topic_quality_gate_prompt`
- `build_topic_group_review_prompt`

## TDD

后端测试：

- migration 默认值。
- `GET /api/projects` 返回策略资料。
- `POST /api/projects` 创建带策略资料的账号。
- `PUT /api/projects/:project_id/strategy-profile` 保存并返回最新资料。
- `POST /api/projects/:project_id/strategy-profile/draft` 返回结构化草稿且不落库。
- 参数校验和项目隔离。
- 草稿生成失败、输出非法和超长补充方向。
- 三类 topic prompt 均包含账号策略资料。

前端测试：

- 顶部选择器展示“当前账号”。
- 内容策略左侧二级菜单展示“账号策略”“历史生成”“当前选题池”，并在账号策略页激活“账号策略”。
- 账号策略独立页面展示账号策略资料和缺失提示。
- 当前选题池不展示账号策略区块、策略资料状态/摘要或编辑入口。
- 编辑表单回显、提交、保存成功刷新。
- AI 草稿生成成功后预填表单并展示草稿摘要，保存前正式资料不变。
- AI 草稿生成失败时保留当前表单内容。
- 保存失败不覆盖旧资料。
- 切换账号后展示对应资料。

## Prototype Gate

进入前端实现前，必须通过 Pencil MCP 更新 `docs/prototypes/video-agent/video-agent.pen`，并获得用户明确确认。原型必须覆盖“内容策略 > 账号策略”独立二级页、账号策略资料卡片、AI 草稿生成区、编辑表单、缺失状态、顶部“当前账号”和保存后回显；当前选题池不展示账号策略区块。

## Scope Boundary

本次不做平台账号凭据、发布授权、外部平台抓取、`accounts` 表改造、移动端适配，也不迁移既有 `project_id` 外键。
