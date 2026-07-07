# Novex 项目记忆

> 本文件是 Novex AI Agent 基座项目级记忆统一入口，记录长期偏好、稳定规则、历史决策和跨会话背景。`apps/video-agent` 是当前第一个业务应用。

## 记忆文件索引

### 仓库约定
- 详见 `docs/memory/project-memory-structure.md` — 项目记忆结构规则
- 详见 `docs/memory/frontend-design-skill-requirement.md` — 前端设计约束

### 项目背景
- 详见 `docs/memory/project-tech-stack.md` — 技术栈与架构设计
- 详见 `docs/memory/video-agent-workspace-flow.md` — 视频工作台菜单、业务流、Agent 分层和开发阶段规划
- 详见 `docs/requirements/video-agent-mvp.md` — MVP需求边界与验收标准
- 详见 `docs/requirements/video-agent-database-schema.md` — 简化版数据库设计
- 详见 `docs/requirements/video-agent-full-spec.md` — video-agent 完整需求文档

## 核心决策（2026-07-01）

### 技术选型
- **后端**: Rust + Axum + SQLx + PostgreSQL
- **向量库**: Milvus Standalone（20万素材规模）
- **任务队列**: Redis + 简单Job表
- **Worker**: Python（FastAPI）处理视频生成和平台发布
- **前端**: Next.js 14 + TypeScript + shadcn/ui

### 当前架构决策
- 当前仓库定位已从根级 video-agent MVP 调整为 **Novex AI Agent Foundation monorepo**
- `video-agent` 保留为 `apps/video-agent` 下的首个业务应用
- OpenSpec change `align-novex-foundation-architecture` 已于 2026-07-01 归档
- `script-agent-mvp` 已在 Novex 基座结构下完成并归档，脚本生成、读取、列表、状态更新 API 已实现
- `apps/video-agent/` 是 `VEDIO-AGENT / 视频工作台` 的正式视频生产工作台边界；当前已迁入脚本 Agent 前端闭环，打通“生成脚本 -> 查看分镜 -> 更新状态”。素材匹配和视频生成编排作为后续 OpenSpec change 推进
- `admin/` 已收敛为 Novex 平台管理后台入口，承载用户、权限、模型、工具、MCP、任务、日志、运行状态、成本、限额和健康检查等控制面能力，不再承载日常视频内容生产流程
- `apps/video-agent/` 前端工作台的对外可见产品品牌名为 `VEDIO-AGENT`，展示名为“视频工作台”；原型、UI 和当前工作台设计文档不得使用 `Novex Admin` 作为展示品牌
- 视频工作台 Pencil 原型源文件统一保存在 `docs/prototypes/video-agent/video-agent.pen`；后续有关视频工作台的原型修改都以该文件为准，不再使用 `docs/prototypes/script-agent-workspace/` 截图目录
- 用户已确认视频工作台业务流程走向：内容策略 -> 脚本创作 -> 素材管理 -> 作品生产 -> 发布运营 -> 数据分析 -> 工作流任务；前端一级菜单和开发阶段规划详见 `docs/memory/video-agent-workspace-flow.md`
- 视频工作台导航应以数据库持久化菜单配置作为单一来源，一级菜单固定围绕业务流程组织；`apps/video-agent` 不得继续用 6 个 Agent 硬编码数组作为一级导航，Agent 状态只能作为二级菜单、模块状态或执行状态展示
- 视频工作台不是单一脚本 Agent 页面；选题、脚本、素材、视频、发布、优化六类 Agent 能力应映射到业务菜单下的二级入口、模块状态或执行状态，不再作为前端一级导航；当前 `script-agent-workspace` 只实现脚本创作下的脚本生成模块闭环
- `projects` 是内容项目/账号方向/内容生产边界，不是具体选题；当前脚本生成必须绑定真实 `project_id`，但选题尚无独立管理模型，只作为 `topic` 文本输入和脚本上下文保存。没有选题池前，不显示“当前选题”或选题管理入口；后续应在“内容策略/选题池”中确认选题，再进入脚本创作并让脚本引用 `topic_id` 或保存选题快照
- 用户已确认内容策略与选题池第一版走“选题池优先”：内容策略页展示项目策略摘要和选题池闭环，支持人工创建选题和选题 Agent 批量生成候选；选题状态为 `idea -> approved -> scripted -> archived`，Agent 生成候选自动入库为 `idea` 并记录批次。选题 Agent 第一版接入通用 Agent Runtime 的 `topic` adapter，输入只依赖项目定位和用户补充要求；脚本生成关联 `topic_id` 并保存 `scripts.content.topic_snapshot`，成功后选题状态更新为 `scripted`
- 用户已确认历史生成批次补充选题采用“同主题上下文 + 补充批次”语义：补充生成必须创建新的 `topic_generation_batches`，新批次 `supplement_of_batch_id` 指向原始批次，新选题 `content_topics.batch_id` 指向补充批次本身；对补充批次再次补充时归一到最初原始批次。前端查看选题时按主题组聚合展示原始批次和补充批次的选题，批次只作为生成来源和审计记录。
- 用户已确认账号/项目管理暂列为后续功能，不纳入当前内容策略完善范围；后续应将前端展示语义从“项目”统一调整为“账号”，并补充账号管理入口，支持新建、编辑账号定位、切换当前账号。当前阶段先继续完善“内容策略”
- 已建立第一版通用对话 Agent Runtime 后端基座：`agent_conversations` / `agent_messages` 承载连续对话，单轮消息继续写入 `agent_runs` / `agent_steps`，脚本 Agent 已接入对话式分镜修改能力；后续选题、素材、视频、发布、优化 Agent 应接入同一 Runtime/adapter 接口，不得各自实现孤立聊天逻辑。当前未实现前端聊天面板，`apps/video-agent` UI 接入仍需先走 Pencil 原型确认
- 用户已确认新的脚本创作产品约定：脚本生成也应走脚本 Agent 对话入口；后续 `apps/video-agent` 不应在右侧并列保留独立“生成脚本”大表单和“脚本 Agent 对话”输入框，而应使用单一脚本 Agent 对话承载无脚本时生成脚本、有脚本时修改脚本。该改造通过 OpenSpec change `conversational-script-generation` 推进，前端实现前仍需更新 `docs/prototypes/video-agent/video-agent.pen` 并获得确认
- 脚本智能体详情展示已选定“时间轴对照视图”：左侧表达分镜顺序和节奏节点，右侧并排展示旁白与画面指令；后续实现不要回退成纯卡片流或纯表格
- Video Agent 前端工作台当前仅覆盖桌面端运营后台，不涉及移动端原型、移动端适配或移动端验收；后续如需要移动端，应单独提出 OpenSpec change

### 架构原则
1. `backend/` 承担控制面 API 和业务编排入口
2. 可复用 AI 能力沉淀到 `crates/*`
3. Python 只做 `services/*` sidecar/runtime
4. 业务应用放入 `apps/*`
5. video-agent 业务范围仍参考 `docs/requirements/video-agent-mvp.md`

### 开发环境
- 环境初始化必须从 `/server/docker-compose.yml` 进入，并 include `/server/ai-agent/docker-compose.yml`
- 已复用现有 PostgreSQL 服务 `biga-postgres`，本项目使用独立数据库 `video_agent`
- 已复用现有 Redis 服务 `bs-redis`，本项目使用 Redis DB index `/2`
- 当前服务端口：API `18180->8080`，Video Worker `18181->8081`，Admin `18182->3000`，Video Agent 工作台 `18183->3000`
- Compose 服务名：`ai-agent-api`、`ai-agent-video-worker`、`ai-agent-admin`、`ai-agent-video-agent`
- 本项目服务容器内工作目录统一为 `/app`

### 六大Agent
1. **选题Agent**: 热点分析 + 爆款选题生成
2. **脚本Agent**: 结构化脚本 + 分镜生成
3. **素材Agent**: 语义检索 + 智能匹配
4. **视频Agent**: 多平台视频生成编排
5. **发布Agent**: 多平台自动发布
6. **优化Agent**: 数据回流 + 策略优化（Month 4）

## 记忆文件约定

1. 本文件是统一入口，具体主题记忆位于 `docs/memory/`，产品与需求文档位于 `docs/requirements/`
2. 每次新会话开始前、上下文压缩后恢复时，必须先读取本文件
3. 只记录已确认且后续会复用的信息，禁止写入临时探索、一次性报错、敏感信息
4. 重大决策变更时，同步更新本文件和对应的详细记忆文件
5. `docs/memory/` 与 `docs/requirements/` 跟随项目，可跨机器同步
