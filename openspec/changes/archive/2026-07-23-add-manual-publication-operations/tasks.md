## 1. 实施前事实核验与测试基线

- [x] 1.1 重新核对抖音和小红书官方创作者入口与作品链接官方域名，记录带核实时间的平台 profile 依据；本期按用户确认不校验未获稳定官方证据的平台文件与文案限额，禁止猜测或改用非官方自动化
- [x] 1.2 盘点现有 `publication_handoffs`、artifact 下载、幂等写入和敏感字段清理能力，明确可复用边界并证明新写路径不依赖 legacy `accounts/publish_tasks`
- [x] 1.3 先建立发布领域、数据库、API、发布包和前端测试入口，提交可观察到缺失能力的失败用例后再实现对应功能

## 2. 发布领域模型与状态机

- [x] 2.1 实现 `PublicationPlan`、`PublicationTarget`、`PublicationPackage`、`PublicationEvent` 领域类型及平台、状态、事件枚举，并为关键业务字段补充注释
- [x] 2.2 实现目标状态机、非法迁移拒绝、非终态取消、`handed_off -> needs_attention -> ready` 重新准备和 `published` 结果追加修正规则
- [x] 2.3 实现双平台目标隔离、草稿 revision 递增、旧发布包失效和计划整体状态确定性派生
- [x] 2.4 增加领域单元测试，覆盖全部合法/非法迁移、平台隔离、部分完成、revision 失效、终态限制和结果修正

## 3. 数据库结构与仓储

- [x] 3.1 新增 migration，创建 `publication_plans`、`publication_targets`、`publication_packages`、`publication_events` 及外键、唯一约束、状态约束、published 字段一致性约束、索引和数据库注释
- [x] 3.2 在 migration 中加入 JSON 敏感键拒绝约束，并将旧 `accounts`、`publish_tasks` 明确标记为 legacy，只读保留且不迁移不可可靠映射的历史数据
- [x] 3.3 实现发布 repository：handoff 唯一 plan、平台唯一 target、revision 并发保护、版本化 package、追加式 event 和人工结果当前投影
- [x] 3.4 增加数据库集成测试，覆盖 handoff/平台唯一性、作品版本与 artifact 约束、published 字段一致性、并发 revision、事件不可覆盖和敏感数据拒绝

## 4. 应用服务与 API 契约

- [x] 4.1 实现幂等取得发布计划、列表、详情和平台草稿保存应用服务，始终绑定 handoff 中的明确作品版本且不自动选择最新版本
- [x] 4.2 实现生成发布包、下载清单、复制/下载审计、官方网页交接、标记需处理、取消、人工发布确认和结果修正应用服务
- [x] 4.3 实现 proposal 中定义的发布 REST API 与 DTO 校验；所有写请求强制 `idempotency-key`，过期 revision 返回明确冲突且不得产生部分写入
- [x] 4.4 实现 HTTPS 官方作品域名、实际发布时间和平台 profile 版本校验；打开平台仅返回受信任入口并记录 `handed_off`，不得创建 Worker 或自动判定发布成功
- [x] 4.5 在请求、响应、事件 payload 和日志边界拒绝或清理 Cookie、Token、Secret、Authorization、签名查询参数及内部绝对路径
- [x] 4.6 增加 API 集成测试，覆盖幂等重放、明确版本绑定、双平台独立保存、并发冲突、非法状态、逾期展示、官方域名校验、人工确认、结果修正、取消及失败不乐观更新

## 5. 发布包与存储安全

- [x] 5.1 实现来源 artifact 存在性与 SHA-256 校验、平台必填项检查和基于来源 hash、平台、revision、规则版本的确定性 manifest hash
- [x] 5.2 生成友好命名的 MP4、可选封面、`发布文案.txt`、`发布检查清单.txt`、`manifest.json` 和完整 ZIP；MP4 使用 store 模式，不转码、不做有损压缩
- [x] 5.3 按 manifest hash 幂等复用有效发布包，草稿变化后保留旧包审计但禁止其继续作为当前包，并处理失败生成的原子清理
- [x] 5.4 增加发布包测试，覆盖文件完整性、友好命名、ZIP store、重复生成复用、旧 revision 失效、损坏 artifact 拒绝以及包内无凭据、签名 URL 和内部路径

## 6. 数据库菜单与作品库衔接

- [x] 6.1 新增菜单 migration，将未启用的 `publish-scheduler` 规划节点替换为 active 的“发布工作台”，使用模块键 `publishing.workbench` 和路由 `/publishing/workbench`
- [x] 6.2 扩展作品库“进入发布”流程，在 handoff 创建成功后幂等取得 plan 并导航到 `/publishing/workbench?plan=<id>`，失败时保留当前页面与明确错误
- [x] 6.3 增加菜单与作品库路由测试，验证数据库菜单唯一来源、完整七个一级菜单、明确计划加载和重复进入不创建重复计划

## 7. 发布工作台原型确认

- [x] 7.1 使用 `awesome-design-md` 检查或补齐本项目 `DESIGN.md`，再使用 `awesome-design-systems` 选择适合高密度运营工作台的真实设计系统参考并记录采用的约束
- [x] 7.2 仅通过 Pencil MCP 更新 `docs/prototypes/video-agent/video-agent.pen`，完成待发布、发布记录、平台目标编辑、准备检查、人工交接、结果登记及加载/空/错误/冲突状态的桌面原型
- [x] 7.3 在原型中验证共享工作台骨架、可读字号、统一 Select、固定尺寸、长文案、双平台部分完成和窄桌面无裁切/重叠/横向溢出
- [x] 7.4 向用户展示 Pencil 原型并取得“确认开发”或等价明确口令；未确认前不得开始第 8 组前端实现任务

## 8. 发布工作台前端实现

- [x] 8.1 实现 `/publishing/workbench` 路由及共享工作台导航集成，按 `plan` 参数加载明确计划且不得静默选择其他作品版本
- [x] 8.2 实现待发布与发布记录视图，支持平台、真实状态、时间和关键词筛选，并展示计划时间、逾期、最近动作、人工结果和审计入口
- [x] 8.3 实现抖音/小红书独立草稿编辑、封面选择、检查清单、发布包生成与下载、文案复制和“去平台发布”动作，不展示账号绑定、OAuth、多账号或自动排程控件
- [x] 8.4 实现 `ready`、`handed_off`、`needs_attention`、`published`、`cancelled` 的真实交互与人工结果登记/修正，统一标识“等待人工发布”和“人工确认已发布”
- [x] 8.5 实现加载、空、读取错误、写入错误、revision 冲突、artifact 损坏和安全重试状态，禁止失败写入通过乐观 UI 显示为成功
- [x] 8.6 增加前端组件测试，覆盖明确计划加载、双平台编辑隔离、计划提醒、包失效、交接后仍待确认、人工结果校验、筛选和错误恢复

## 9. 集成验收与回归

- [x] 9.1 运行发布领域、数据库、API 与现有作品库/菜单相关 Rust 测试，修复回归并确认 legacy 发布表未收到新写入
- [x] 9.2 在项目容器内运行 Video Agent lint、单元测试和生产构建，确认类型、路由和共享骨架无回归
- [x] 9.3 运行桌面 E2E，覆盖作品完成版本进入发布、双平台分别准备、下载/复制、打开官方入口、等待人工发布、需处理重做、人工确认、部分完成和发布记录
- [x] 9.4 使用 Playwright 在宽桌面和窄桌面视口截图校验 Pencil 对齐、无裁切/重叠/横向溢出，并确认外部链接始终为受信任官方入口
- [x] 9.5 检查数据库、API 响应、日志、事件和发布包，证明无 Cookie、Token、Secret、Authorization、签名查询参数或内部绝对路径泄露，且没有浏览器自动化、未公开 API 或发布 Worker
- [x] 9.6 运行 `openspec instructions apply --change add-manual-publication-operations --json`，确认任务进度与实际一致；全部实现和验证完成后仅报告可归档，等待用户明确归档命令
