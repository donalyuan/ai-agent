## Why

现有项目规则引用了当前仓库无法证实或当前会话不可用的设计、原型和第三方平台能力，容易让后续实现被无效前置条件阻塞，并把未确认的业务与数据约束误写为项目事实。

现在需要将规则收敛为仓库可维护的通用约束，并为确实可用的设计与浏览器验证能力建立准确、授权受限的路由。

## What Changes

- 整理 `AGENTS.md` 与 `CLAUDE.md`：保留中文沟通、项目记忆、事实优先、OpenSpec、验证及 Git 授权边界。
- 将 UI 新建或重构路由到 `frontend-design`，将浏览器交互和端到端检查路由到 `playwright`；当前项目前端开发、调试或验证所需的当前项目浏览器页面或应用窗口可自动截图，其他截图仍须事前向用户索要明确授权。
- 删除对失效的 `.claude/skills/awesome-design-*`、`Pencil MCP`、Runway、可灵、抖音、小红书及无仓库证据的 migration、JSONB、视频费用和 GitNexus 规则的引用。
- 更新当前交接，使其反映规则已与可用能力对齐，同时继续明确产品和技术栈待确认。

## Capabilities

### New Capabilities

- `project-rule-governance`: 规定项目助手规则如何以仓库事实为依据、准确路由当前能力并维护交接信息。

### Modified Capabilities

- 无。

## Impact

- 受影响文件：`AGENTS.md`、`CLAUDE.md`、`docs/agent/HANDOFF.md` 及本 change 的 OpenSpec artifacts。
- 不新增依赖、应用代码、产品架构或外部服务集成。
