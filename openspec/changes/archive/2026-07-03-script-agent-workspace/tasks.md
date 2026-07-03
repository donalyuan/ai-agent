# 任务清单

## Phase 0: 设计上下文与原型门禁

- [x] **T0.1 补齐 DESIGN.md**
  - [x] 基于 `awesome-design-md` 创建项目根 `DESIGN.md`
  - [x] 明确 `AI-AGENT` 工作台颜色、字体、间距、按钮、表单、列表、状态标签和响应式规则
  - [x] 明确“智能体工作台”展示名和六个智能体菜单预留规则
  - [x] 明确禁止 hero、装饰渐变、嵌套卡片和硬编码项目入口

- [x] **T0.2 记录真实设计系统参考**
  - [x] 基于 `awesome-design-systems` 确认参考 `Ant Design`、`IBM Carbon`、`GitHub Primer`
  - [x] 在设计记录中写清借鉴模式与拒绝模式

- [x] **T0.3 完成 Pencil MCP 原型确认**
  - [x] 使用 `Pencil MCP` 输出桌面工作台原型
  - [x] 原型展示 `AI-AGENT` / “智能体工作台”标题和六个智能体菜单入口
  - [x] 覆盖无项目、无脚本、生成中、生成失败、状态更新失败状态
  - [x] 等待用户确认原型后再进入编码
  - [x] 当前环境已有 `Pencil MCP`，无需启用替代方案

---

## Phase 1: 后端项目 API 支撑

- [x] **T1.1 定义项目 API 模型**
  - [x] 新增 `CreateProjectRequest`
  - [x] 新增 `ProjectResponse`
  - [x] 增加字段校验：`name` 必填且长度受限，`positioning` 和 `description` 可为空但长度受限
  - [x] 先写模型序列化和校验测试

- [x] **T1.2 扩展 ProjectRepository**
  - [x] 先写 repository contract 测试覆盖创建和列表
  - [x] 增加 `create_project` 方法
  - [x] 增加 `list_projects` 方法
  - [x] 保留现有 `project_exists` 行为

- [x] **T1.3 增加项目 HTTP 路由**
  - [x] 先写路由测试覆盖 `GET /api/projects`
  - [x] 先写路由测试覆盖 `POST /api/projects`
  - [x] 注册项目路由到 `build_app_with_state`
  - [x] 确认错误响应结构与脚本 API 风格一致

- [x] **T1.4 后端验证**
  - [x] 运行 `docker exec ai-agent-api cargo test -p novex-api`
  - [x] 确认新增项目 API 测试和既有脚本 Agent 测试全部通过

---

## Phase 2: 前端测试与 API Client 基础

- [x] **T2.1 建立前端测试基础**
  - [x] 为 `admin/` 增加 React/TypeScript 测试命令
  - [x] 添加必要的测试依赖和配置
  - [x] 确认 `npm run lint`、`npm run build`、前端测试命令可独立执行

- [x] **T2.2 实现 API 配置读取**
  - [x] 先写测试覆盖默认 API base URL
  - [x] 先写测试覆盖 `NEXT_PUBLIC_API_BASE_URL` 覆盖行为
  - [x] 实现统一 API base URL 读取函数

- [x] **T2.3 实现 typed API client**
  - [x] 先写测试覆盖项目列表、项目创建、脚本列表、脚本读取、脚本生成、状态更新请求
  - [x] 实现 `Project`、`ScriptSummary`、`ScriptDetail`、`Scene` 等 TypeScript 类型
  - [x] 实现 fetch 包装与错误响应解析
  - [x] 确保接口错误不会被吞掉为通用异常

---

## Phase 3: 工作台 UI 组件

- [x] **T3.1 项目选择组件**
  - [x] 先写组件测试覆盖有项目、无项目
  - [x] 实现项目下拉选择
  - [x] 不在脚本工作台展示项目创建或项目管理入口

- [x] **T3.2 脚本列表组件**
  - [x] 先写组件测试覆盖空列表和选中项
  - [x] 实现状态 segmented control 或等价控件
  - [x] 展示标题、状态、分镜数、创建时间
  - [x] 列表项尺寸稳定，长标题不撑破布局

- [x] **T3.3 脚本详情与分镜组件**
  - [x] 先写组件测试覆盖标题和分镜顺序
  - [x] 展示分镜旁白、视觉描述、情绪和时长
  - [x] 使用时间轴对照视图呈现分镜顺序、旁白和画面指令
  - [x] 支持无选中脚本状态
  - [x] 确保桌面面板内文本不重叠、不溢出

- [x] **T3.4 脚本生成组件**
  - [x] 先写组件测试覆盖工作台渲染
  - [x] 实现选题 textarea
  - [x] 实现风格选择：`knowledge`、`story`、`tutorial`
  - [x] 实现分镜数选择：3 到 12
  - [x] 生成中禁用重复提交但不清空输入

- [x] **T3.5 状态更新组件**
  - [x] 先写组件测试覆盖详情状态区域渲染
  - [x] 实现状态更新操作
  - [x] 更新成功后同步列表和详情
  - [x] 更新失败时保留原状态并显示错误

---

## Phase 4: 页面集成

- [x] **T4.1 替换 admin 首屏为工作台**
  - [x] 保留必要的服务健康提示，但首屏主体必须是脚本 Agent 工作台
  - [x] 集成项目选择、脚本列表、详情、生成和状态操作
  - [x] 使用 `DESIGN.md` 约束的布局和样式

- [x] **T4.2 完成页面状态归并**
  - [x] 生成脚本成功后自动打开新脚本详情
  - [x] 状态更新后保持筛选与选中状态一致
  - [x] 项目切换后清空旧项目选中脚本并加载新列表
  - [x] API 不可用时禁用会写入的操作

- [x] **T4.3 响应式与可访问性检查**
  - [x] 桌面布局使用稳定分栏
  - [x] 表单控件有 label 或可访问名称
  - [x] loading 和错误状态可被屏幕阅读器感知

---

## Phase 5: 验证与文档

- [x] **T5.1 自动化验证**
  - [x] 运行 `docker exec ai-agent-api cargo test -p novex-api`
  - [x] 运行 `docker exec ai-agent-admin npm run lint`
  - [x] 运行 `docker exec ai-agent-admin npm run build`
  - [x] 运行新增前端测试命令

- [x] **T5.2 浏览器闭环验证**
  - [x] 从 `/server/docker-compose.yml` 启动相关服务
  - [x] 打开 `http://localhost:18182`
  - [x] 验证无项目状态和项目 API 可用
  - [x] 验证可以加载脚本并查看分镜
  - [x] 验证脚本状态筛选与状态更新控件可见

- [x] **T5.3 更新项目记忆和说明**
  - [x] 若工作台入口、设计规范或项目 API 成为稳定约定，更新 `MEMORY.md` 或对应 `docs/memory/*`
  - [x] 更新 `README.md` 中的前端工作台启动/验证说明
  - [x] 不记录一次性报错、临时探索或敏感信息
