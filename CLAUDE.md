# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 语言与沟通

1. 全程使用简体中文。
2. 结论优先，证据其次，非必要不展开。
3. 代码、命令、路径、协议、技术术语保持原文。
4. 同一轮回复中，不重复输出语义相同的方案、结论或问题；若需修正，直接基于上一次内容增量说明。

## 项目记忆

1. Novex 项目记忆采用"总索引 + 分文件"结构：`/server/video-agent/MEMORY.md` 作为项目级总索引和全局稳定约束，`/server/video-agent/docs/memory/README.md` 作为文档区索引，`/server/video-agent/docs/memory/*.md` 作为具体主题记忆。
2. 每次新会话开始前、每次上下文压缩后恢复继续执行前，必须先读取 `/server/video-agent/MEMORY.md`；涉及具体主题、长期决策或历史背景时，再读取 `/server/video-agent/docs/memory/` 或 `/server/video-agent/docs/requirements/` 下对应文件。
3. 写入 memory 时，优先更新对应主题文件；当新增主题、调整索引、变更全局约束或跨文件稳定规则时，再同步更新 `/server/video-agent/MEMORY.md`，必要时更新 `/server/video-agent/docs/memory/README.md`。
4. 只保存已确认并在后续仍可能复用的关键信息，包括长期偏好、稳定规则、历史决策，以及跨轮或压缩恢复后继续执行仍必需的上下文。
5. 禁止写入临时探索、一次性报错、未确认猜测、仅当前局部步骤短暂有效的信息，以及口令、密钥、令牌、隐私数据等敏感信息。
6. 需求明确或架构决策一旦确认，必须在当轮同步写入对应的主题 memory 文件；若影响总索引或全局约束，再同步更新 `/server/video-agent/MEMORY.md`。

## 当前仓库状态

1. 项目正在从根级 video-agent MVP 结构迁移为 Novex AI Agent Foundation monorepo；`apps/video-agent` 是首个业务应用。
2. 技术栈已确定：Rust + Axum + SQLx + PostgreSQL + Milvus + Redis + Python Worker + Next.js。
3. 后续实例在进入实现前，必须先重新检查仓库实际内容，以真实文件为准，不能从历史对话或其他项目推断当前实现状态。

## 运行环境

1. 当前项目默认运行与验证环境为 Docker Compose 统一编排。
2. 涉及本项目的 `cargo`、`pytest`、`npm` 及其他依赖项目运行时的命令，默认必须优先在容器内执行。
3. 本项目服务容器内项目路径为 `/app`；宿主机项目路径为 `/server/video-agent`。
4. 未经明确确认，不得为本项目在宿主机临时安装或依赖与项目运行环境不一致的替代运行时。

## 常用命令

### 仓库探查

```bash
git status --short
rg --files
find . -maxdepth 2 -type f | sort
```

### 构建、Lint、测试

启动开发环境：

```bash
docker compose -f /server/docker-compose.yml up -d --build novex-api novex-video-worker novex-admin
```

检查顶层 Compose 服务：

```bash
docker compose -f /server/docker-compose.yml config --services
```

Rust API 构建：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo build --workspace'
```

Rust API 全量测试：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'
```

Rust API 单个测试文件：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test health'
```

Python Worker 全量测试：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-video-worker sh -lc 'cd /app && pytest tests -q'
```

Python Worker 单个测试文件：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-video-worker sh -lc 'cd /app && pytest tests/test_health.py -q'
```

前端 lint：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-admin sh -lc 'cd /app && npm run lint'
```

前端构建：

```bash
docker compose -f /server/docker-compose.yml exec -T novex-admin sh -lc 'cd /app && npm run build'
```

## 测试与验证

1. 默认采用 TDD：先写测试，再写实现。
2. 声称完成前，必须运行直接相关测试或验证命令，并以实际输出为准。
3. 证据优先级：
   - 运行报错
   - 代码事实
   - 文档约定
4. 未验证的实现不得声称"已完成"。

## 代码可读性

1. 关键变量、关键方法、关键流程节点必须补充简洁备注，说明其业务意图、输入输出或存在原因。
2. 备注应解释"为什么这样设计"或"这个值/方法承担什么职责"，不得用无信息量的注释重复代码表面含义。
3. 复杂的 Agent 编排逻辑、视频生成流程、平台 API 对接必须有清晰的文档字符串。
4. 数据库 migration、JSONB 结构、枚举类型必须有明确注释。

## 风险控制

1. 若用户要求与协议、现有约束或代码事实冲突，必须按"结论｜证据｜可执行替代方案"明确指出。
2. 不接受临时兜底方案冒充彻底解决方案。
3. 对第三方平台 API（Runway、可灵、抖音、小红书）的非标准行为，必须在实现中明确标注其兼容性质。
4. 未经用户明确确认，不得擅自执行 `git add`、`git commit`、`git push`，或将文件提交到仓库。
5. 需要执行 `git commit` 时，提交备注必须使用简体中文，不得使用英文提交信息。
6. 涉及视频生成、平台发布等可能产生外部费用的操作，必须有明确的成本控制和错误重试机制。

## 架构判断

1. 项目定位为 Novex AI Agent Foundation，video-agent 是 `apps/video-agent` 下的首个业务应用。
2. 系统边界以 `ARCHITECTURE.md` 为长期基准：`backend/admin/apps/crates/services/templates/infra/docs`。
3. 可复用 AI 基建能力优先沉淀到 `crates/*`，业务应用放入 `apps/*`，Python sidecar/runtime 放入 `services/*`。
4. 如果后续建设 video-agent 业务，应先确认它是应用私有能力还是可复用基座能力，再选择落点。

## OpenSpec 规范

1. 本工作区涉及功能新增、行为修改、协议改造、测试规则变化时，必须优先遵循 OpenSpec 工作流。
2. 开始实现前，必须先检查 OpenSpec 当前状态：
   - `openspec list --json`
   - 若已有相关 change，优先在已有 change 上继续
   - 若没有相关 change，先新建 change
3. 用户提出需求后，默认先进入 OpenSpec 的需求澄清/设计阶段，不得跳过设计直接改代码。
4. 需求讨论阶段，内部必须先从 `DDD`、`BDD`、`SDD`、`TDD` 四个角度完成审视，再对外输出结论。
5. 需求讨论阶段对外回复时，必须显式按 `DDD`、`BDD`、`SDD`、`TDD` 四个标签组织内容；若某一角度当前无新增约束或影响，也必须明确写出，不得省略。
6. 四个角度的最小审视口径如下：
   - `DDD`：领域概念、边界、状态流转、规则归属。
   - `BDD`：用户场景、触发行为、可观察结果、验收口径。
   - `SDD`：规格、接口、数据结构、约束、兼容性、非目标。
   - `TDD`：测试入口、失败场景、回归范围、验证方式。
7. 需求边界不清时，必须先澄清；证据不足时，不得主观猜测后直接实现。
8. 确认方案后，必须在 `openspec/changes/<change-name>/` 下补齐当前 schema 需要的 artifacts，至少包括：
   - `proposal.md`
   - `design.md`
   - `specs/**/*.md`
   - `tasks.md`
9. 实施过程中，代码改动必须与对应 OpenSpec tasks 对齐；完成一项就同步勾选 `tasks.md`。
10. 实现完成后，必须再次执行：
    - `openspec instructions apply --change "<change-name>" --json`
    - 确认 `state` 为 `all_done` 或任务进度与实际一致
11. 若实现过程中发现设计或范围变化，必须先回写对应 OpenSpec artifacts，再继续编码。
12. 未经明确确认，不得绕过 OpenSpec 直接做"先改代码后补文档"。

## 前端原型确认

1. 每次新增或修改前端页面前，必须先使用项目级 `.claude/skills/awesome-design-md` 与 `.claude/skills/awesome-design-systems` 补齐设计上下文，再进入 `Pencil MCP` 原型阶段。
2. `awesome-design-md` 负责 `DESIGN.md` 风格上下文；若当前任务需要明确颜色、字体、组件与布局语言，而项目根暂无可用 `DESIGN.md`，必须先补充该上下文或明确说明本次暂不落地的原因。
3. `awesome-design-systems` 负责真实设计系统案例参考；设计时不得仅凭主观印象臆造参考对象。
4. 前端原型确认只允许使用 `Pencil MCP`；若当前会话没有可用的 `Pencil MCP`，必须先向用户说明当前不足以继续原型确认，禁止退回浏览器确认流程。
5. 用户提出原型修改意见后，必须先更新原型并说明本次改动点，再等待用户确认，不得跳过原型确认直接编码。
6. 进入前端正式开发前，必须取得用户的明确确认口令，例如"确认开发"、"按这个原型开发"或"这个版本通过"；模糊反馈不足以视为确认。
7. 若用户仅给出"差不多""先这样""再看看"等不明确反馈，按"不足以定论"处理，继续停留在原型阶段。
8. 用户若通过截图、标注图、手绘图等图片形式提出原型修改，默认通过当前对话直接上传图片；若图片已在本机且用户提供了明确绝对路径，也可按该路径读取。
9. 截图、标注图、手绘图仅作为视觉修改依据，不得直接视为可开发原型源文件；必须先将其转译为新的原型版本，再交由用户确认。
10. 接收到截图类修改后，必须先明确提炼"本次变更点"和"未覆盖范围"；若截图信息不足以定论，不得主观补全，必须先向用户澄清。

## 工作约束

1. 当用户只要求生成仓库约束文件时，只创建或修改 `CLAUDE.md`，不要顺带创建脚手架、示例代码、依赖清单或测试文件，除非用户明确要求。
2. 在当前这种尚处于规划阶段的仓库中，禁止凭空补完整的应用代码、未验证的配置或示例 schema。
3. 以后若仓库新增了 README、其他 AI 工具规则文件，更新 `CLAUDE.md` 时只提炼其中长期有效、会影响 Claude Code 行为的部分，避免重复抄录。
4. 把命令写进 `CLAUDE.md` 之前，必须先在本仓库实际运行并确认可用；没有验证过的命令不要写成既定事实。
