## Context

作品库已经通过 `publication_handoffs` 保存明确的 `work_id`、`work_version_id`、成片 artifact 和可选字幕 artifact，并保证只有完成版本可以交接。当前交接状态固定为 `draft`，没有发布域聚合、页面或结果记录；早期 `accounts/publish_tasks` 仍绑定旧 `videos` 模型，不能承载当前不可变作品版本。

抖音和小红书个人账号当前都缺少可用的官方账号 OAuth 与服务端发布能力。系统只能准备发布内容并打开官方创作者网页，最终登录、选文件和发布必须由操作者完成。方案必须诚实表达该边界，同时让人工流程具备计划、提醒、版本一致性、平台隔离和审计能力。

## Goals / Non-Goals

**Goals:**

- 从现有交接单建立唯一发布计划，并始终绑定操作者选定的完成版本。
- 独立管理抖音和小红书的平台文案、发布包、计划时间和人工发布结果。
- 生成可校验、可重复下载、不会泄露内部路径或凭据的平台发布包。
- 区分“准备完成”“已打开官方页面”和“人工确认已发布”，避免虚假状态。
- 提供待发布、逾期、需处理和发布记录视图，为后续手工数据回流提供稳定发布目标。

**Non-Goals:**

- 不建设平台账号、OAuth、多账号选择、Cookie 托管或账号信息同步。
- 不自动控制抖音/小红书网页，不使用浏览器自动化、未公开接口或服务端自动发布。
- 不自动排程执行、不自动同步发布结果或平台指标。
- 不在本 change 接入发布文案 Agent；平台文案由操作者填写，后续 change 可在相同草稿协议上增加 Agent 建议。
- 不删除旧 `accounts/publish_tasks` 历史表，也不迁移无法可靠映射到 `work_versions` 的旧记录。

## DDD

`PublicationHandoff` 是作品生产到发布运营的不可变防腐层，只描述来源作品版本和 artifact。`PublicationPlan` 是发布聚合根，一个 handoff 只能对应一个 plan；`PublicationTarget` 是聚合内的平台实体，同一 plan 对同一平台最多存在一个活动目标。

目标状态为 `draft -> ready -> handed_off -> published`，并允许 `handed_off -> needs_attention -> ready` 与非终态到 `cancelled`。任何文案或 artifact 变化都会增加 `draft_revision` 并使旧发布包失效；`published` 是终态，误录链接通过追加修正事件更新当前投影，不删除原值。

计划整体状态由目标状态确定性派生，不允许独立手工改写。`planned_at` 是运营提醒，不代表平台定时任务，也不触发 Worker。

## BDD

操作者从作品库进入发布后，前端先幂等创建 handoff，再让发布域幂等取得对应 plan 并导航到 `/publishing/workbench?plan=<id>`。操作者可添加抖音、小红书目标，分别维护文案、标签、封面和计划时间。

生成发布包前必须校验成片、封面和 manifest 引用。准备完成后，操作者可直接下载 MP4/封面、复制文案或下载完整包。点击平台入口只记录 `handed_off` 事件并打开受信任的官方 URL；页面继续明确显示“等待人工发布”。

人工发布完成后，操作者填写官方作品链接与实际发布时间并确认。系统只验证 HTTPS 与平台官方域名，不声称验证账号归属或平台发布事实；界面统一标记为“人工确认已发布”。双平台目标互不改变对方状态。

## SDD

新增 `publication_plans`、`publication_targets`、`publication_packages` 和 `publication_events`：

- `publication_plans.handoff_id` 唯一引用 `publication_handoffs`，保存创建、更新时间和归档信息。
- `publication_targets` 保存 `platform`、状态、文案快照、标签、封面引用、`planned_at`、`draft_revision`、交接/发布时间、作品链接和当前人工结果；唯一约束为 `(publication_plan_id, platform)`。
- `publication_packages` 按目标和 revision 保存平台规则版本、manifest、manifest hash、包文件引用和生成时间；同一 revision 只能有一个有效包。
- `publication_events` 追加保存创建、编辑、生成包、下载、复制、打开平台、需处理、确认发布、修正结果和取消事件；payload 必须经过现有敏感字段清理规则。

发布包包含友好命名的 MP4、可选封面、`发布文案.txt`、`发布检查清单.txt` 和不含内部路径的 `manifest.json`。ZIP 对 MP4 使用 store 模式，不再次转码或压缩；以来源 artifact SHA-256、平台、草稿 revision 和平台规则版本计算 manifest hash，相同输入幂等复用。

平台规则采用后端只读版本化 profile，至少包含平台标识、官方创作者入口、允许的作品链接域名、所需文件与已核实时间。实现前必须重新核实小红书官方创作者入口，禁止凭历史地址硬编码；本 change 不增加 Admin 编辑页面。

本期不校验抖音或小红书的文件大小、时长、封面尺寸和文案字数限额。平台公开页面未提供可稳定引用的限额契约，系统不得猜测或硬编码；发布包仍必须校验本地来源 artifact 完整性与平台所需文件，未来取得可核验官方规则后另行更新 profile 版本。

主要 API：

- `POST /api/publication-handoffs/:handoff_id/publication`：幂等取得发布计划。
- `GET /api/publications`、`GET /api/publications/:id`：列表和详情。
- `PUT /api/publications/:id/targets/:platform`：保存平台草稿并增加 revision。
- `POST /api/publication-targets/:id/package`：校验并生成或复用发布包。
- `GET /api/publication-targets/:id/downloads`：返回直接文件和完整包下载清单。
- `POST /api/publication-targets/:id/handoff`：记录交接并返回受信任官方入口。
- `POST /api/publication-targets/:id/published`：登记人工发布结果。
- `POST /api/publication-targets/:id/result-corrections`：追加结果修正。
- `POST /api/publication-targets/:id/needs-attention`、`POST /api/publication-targets/:id/cancel`：处理异常和取消。

除只读查询外，写请求必须使用 `idempotency-key`。API、数据库 JSON、事件和包文件均不得包含 Cookie、Token、Secret、Authorization、签名 URL 查询参数或内部存储绝对路径。

## TDD

- 领域测试覆盖全部合法/非法状态迁移、目标隔离、整体状态派生、revision 失效和发布终态修正。
- 数据库测试覆盖 handoff 唯一性、平台唯一性、作品版本外键、published 字段约束、追加事件和敏感字段拒绝。
- API 测试覆盖幂等创建、过期 revision、缺失/损坏 artifact、官方域名校验、结果修正和取消。
- 包测试覆盖友好文件名、ZIP store、manifest hash、重复生成复用、无内部路径和无敏感信息。
- 前端/E2E 覆盖作品库跳转、双平台草稿、计划提醒、下载/复制/打开平台、等待人工发布、结果登记和历史筛选。

## Decisions

### 交接与发布聚合分离

不扩展 `publication_handoffs.status` 承载完整生命周期。交接是作品域输出，发布计划是发布域事实；分离后未来增加官方 API 平台时不需要改变作品库契约。

### 平台目标独立快照

不使用“公共文案实时继承 + 平台覆盖”的可变模型。创建目标时复制初始内容，之后各平台独立修改，避免一个平台的变更使另一个已交接包失效。

### 人工确认而非伪验证

系统只能校验作品链接格式和官方域名，不能验证账号归属与真实发布状态。状态和 UI 明确使用“人工确认”，避免把不可观察外部事实包装成自动同步。

### 保留计划时间但不执行

运营人员仍需要日历和逾期管理，因此保留 `planned_at`；它只驱动排序与提醒，不进入任务队列，也不产生失败重试语义。

### 旧发布表退出新写路径

旧表与当前作品版本无法无损映射，强行复用会形成两套作品身份。新代码只写新发布域表，旧表保留历史并通过数据库注释标明 legacy，后续数据治理 change 再决定归档方式。

## Risks / Trade-offs

- [用户在官方网页修改了文案] -> 确认发布时允许录入最终文案摘要，发布事件保留系统准备快照与人工确认快照。
- [点击官方入口后未发布] -> `handed_off` 永远不自动转为 `published`，待发布列表持续显示待确认。
- [平台规则或入口变化] -> profile 带版本和核实时间，变更后旧包仍保留原规则快照，新包按新 revision 生成。
- [大 MP4 生成 ZIP 消耗资源] -> 使用 store 模式并按 manifest hash 缓存，不做重新编码或重复打包。
- [人工链接误录] -> 限制 HTTPS 官方域名，允许追加修正但不覆盖历史事件。
- [旧表继续造成认知混淆] -> migration 增加 legacy 注释，代码搜索和测试保证新发布模块不引用旧 repository/model。

## Migration Plan

1. 新增发布域表、约束和平台 profile，并标记旧发布表为 legacy。
2. 实现领域状态机、repository 和 API，先完成数据库/API 失败测试。
3. 更新数据库菜单，将 `发布排程` 替换为启用的 `发布工作台`。
4. 按设计技能与 Pencil 流程完成原型并取得明确确认后实现页面。
5. 将作品库“进入发布”接到幂等 plan 创建与工作台路由。
6. 完成发布包、人工结果和桌面 E2E；旧表全程只读保留。
7. 回滚时禁用发布工作台与新写 API，保留新发布计划、包和事件，不回写作品版本。

## Open Questions

无阻塞问题。小红书官方创作者入口和平台字段限制属于实施前必须重新核实的外部事实，不授权扩大为非官方自动化。
