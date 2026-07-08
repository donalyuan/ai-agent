# account-strategy-profile Design

## DDD

第一版继续以 `projects` 作为内容生产边界和数据库外键边界，但在视频工作台前端统一展示为“账号”。这里的“账号”是内容账号或内容方向，不等同于 `accounts` 表中的平台发布账号。

新增 `AccountStrategyProfile` 概念，作为内容账号的结构化策略资料。它不拥有脚本、选题或素材生命周期，只为内容策略、选题生成和后续数据回流提供稳定上下文。

第一版策略字段固定为：

- `target_audience`：目标受众。
- `content_pillars`：内容支柱，字符串数组。
- `tone_style`：表达风格和语气。
- `forbidden_topics`：禁区或不做方向，字符串数组。
- `reference_accounts`：参考账号或参考对象，字符串数组。
- `topic_preferences`：选题偏好。

`projects.positioning` 和 `projects.description` 保留。`positioning` 继续作为账号定位摘要，`description` 作为补充说明；`strategy_profile` 承载结构化详情。`accounts` 表不在本 change 中改造。

## BDD

运营人员打开内容策略下的独立二级页面“账号策略”时，能看到当前账号的定位摘要和结构化策略资料。资料缺失时，页面展示“待补齐策略资料”的提示，并提供编辑入口。

运营人员可以手动填写策略资料，也可以点击“AI 生成草稿”。AI 草稿基于账号名称、定位摘要、描述和用户补充方向生成，只预填编辑表单，不直接保存。运营人员可继续修改，点击保存后才写入 `projects.strategy_profile`。

运营人员保存策略资料后，后续选题生成、质量闸门和主题组评审都应读取最新资料。人工新增选题不因策略资料为空而被阻塞；选题 Agent 生成可以继续执行，但界面应提示策略资料越完整，生成越稳定。

当前选题池不展示账号策略区块、策略资料状态/摘要或进入“账号策略”的编辑入口。这样选题池保持选题筛选和生成主任务，账号策略资料只在独立二级页面维护。

当用户切换当前账号时，“账号策略”“历史生成”“当前选题池”都展示对应账号的数据，不泄露其他账号资料。

## SDD

新增 migration：

```sql
ALTER TABLE projects
    ADD COLUMN strategy_profile JSONB NOT NULL DEFAULT '{}'::jsonb;

COMMENT ON COLUMN projects.strategy_profile IS '内容账号结构化策略资料，供内容策略页、选题 Agent、质量闸门和主题组评审使用。';
```

后端模型新增 `AccountStrategyProfile`，并在 `Project` / `ProjectResponse` 中返回 `strategy_profile`。

建议 API：

- `GET /api/projects`：返回每个账号的 `strategy_profile`。
- `POST /api/projects`：允许创建账号时传入可选 `strategy_profile`。
- `POST /api/projects/:project_id/strategy-profile/draft`：基于当前账号资料和用户补充方向生成策略资料草稿；只返回草稿，不写数据库。
- `PUT /api/projects/:project_id/strategy-profile`：更新账号名称、定位摘要、描述和结构化策略资料。

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

校验规则第一版：

- `name` 必填，去空格后长度 1 到 120。
- `positioning` 长度不超过 500。
- `description` 长度不超过 2000。
- `target_audience`、`tone_style`、`topic_preferences` 单字段长度不超过 1000。
- `content_pillars`、`forbidden_topics`、`reference_accounts` 每组最多 20 项，单项长度不超过 120，保存前去空和去重。
- `direction_notes` 长度不超过 1000。

草稿生成成本控制：

- 只允许用户手动点击触发，不在页面加载、账号切换或保存后自动触发。
- 每次请求最多调用一次 LLM；仅对供应商临时错误复用项目既有短重试策略，最多重试一次。
- `max_output_tokens` 第一版限制为 1200 到 1500 区间。
- 草稿生成失败只展示错误，不创建或修改任何 `projects` 记录。

Agent 上下文统一由后端函数组装，例如 `format_account_strategy_context(project)`。该上下文至少包含账号名称、定位摘要、描述、目标受众、内容支柱、风格语气、禁区、参考账号和选题偏好。以下 prompt 必须使用同一上下文：

- `build_topic_generation_prompt`
- `build_topic_quality_gate_prompt`
- `build_topic_group_review_prompt`

错误处理：

- 项目不存在：返回 404 或现有项目不存在错误语义。
- 参数非法：返回 400，并说明具体字段问题。
- 存储失败：返回现有 storage error 语义。
- 草稿生成失败：返回 AI 草稿生成错误，前端保留当前表单内容。
- AI 草稿输出非法：返回结构化输出错误，不使用部分字段预填。
- Agent 读取策略资料失败：本轮 Agent run 失败，不应退回无策略上下文继续生成。

前端展示：

- 顶部选择器文案由“当前项目”改为“当前账号”。
- 内容策略二级菜单新增“账号策略”，并作为账号策略资料查看和编辑的独立页面。
- 内容策略二级菜单第一版顺序为“账号策略”“历史生成”“当前选题池”；后续完整账号管理独立出来时，再调整入口归属。
- 账号策略页展示账号策略资料卡片、缺失提示、AI 草稿生成区和编辑表单。
- 当前选题池不展示账号策略区块、策略资料状态/摘要或“去账号策略补齐/编辑”的入口。
- 编辑表单覆盖账号名称、定位摘要、描述和六个结构化策略字段。
- 编辑表单提供“AI 生成草稿”入口，草稿结果预填表单并展示草稿摘要。
- 保存成功后本地项目列表和当前页面资料同步刷新。
- 资料为空时展示缺失提示，但不禁用人工新增选题。

## TDD

后端先补失败测试：

- migration 为 `projects` 增加 `strategy_profile`，默认值为 `{}`。
- `GET /api/projects` 返回 `strategy_profile`。
- `POST /api/projects` 可创建带策略资料的账号。
- `PUT /api/projects/:project_id/strategy-profile` 保存并读取最新资料。
- `POST /api/projects/:project_id/strategy-profile/draft` 返回结构化草稿但不落库。
- 更新接口拒绝空账号名称、超长字段、过多数组项和跨项目写入。
- 草稿生成拒绝超长补充方向，LLM 输出非法时不预填。
- 选题生成 prompt 包含账号策略资料。
- 质量闸门 prompt 包含账号策略资料和禁区。
- 主题组评审 prompt 包含账号策略资料。

前端先补失败测试：

- 顶部选择器展示“当前账号”。
- 内容策略左侧二级菜单展示“账号策略”“历史生成”“当前选题池”，并在账号策略页激活“账号策略”。
- 账号策略独立页面展示账号策略资料卡片和缺失提示。
- 当前选题池不展示账号策略区块、策略资料状态/摘要或编辑入口。
- 编辑表单可回显、修改并提交策略资料。
- 点击 AI 生成草稿后表单被草稿预填，但保存前当前账号资料不变。
- 保存失败时展示错误，不覆盖本地旧资料。
- 切换账号后展示对应账号策略资料。

E2E：

- 用户能在“内容策略 > 账号策略”编辑账号策略资料并看到保存后的回显。
- 生成选题前后端仍读取已保存账号策略资料，但当前选题池页面不展示账号策略摘要。

## 风险与取舍

- 继续使用 `projects` 命名会造成后端技术名与前端业务名不一致。取舍：第一版优先兼容现有 `project_id` 外键，前端展示为“账号”，后续如需要彻底改名再另起 change。
- JSONB 策略资料灵活但约束弱。取舍：第一版字段稳定且通过 API 校验控制结构，避免过早拆多表。
- 策略资料越多，prompt 越长。取舍：后端统一格式化并限制字段长度，避免每个 Agent 自行拼接。
- 账号资料可能与未来发布平台账号混淆。取舍：文案明确为“内容账号策略”，不改 `accounts` 表，不接触平台凭据。

## 原型要求

进入前端实现前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并获得明确确认。原型至少覆盖：

- “内容策略 > 账号策略”独立二级页面。
- 账号策略资料编辑表单、AI 草稿生成区和保存后回显。
- 当前选题池页面不展示账号策略区块、策略资料状态/摘要或编辑入口。
- 顶部选择器从“当前项目”改为“当前账号”。
- 策略资料缺失状态。
- 保存成功后的回显状态。
