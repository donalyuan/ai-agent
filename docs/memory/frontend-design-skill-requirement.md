---
name: frontend-design-skill-requirement
description: 前端原型前必须先使用 awesome-design-md 和 awesome-design-systems
metadata:
  type: project
---

# 前端设计上下文约束

video-agent 仓库已引入两个前端设计参考 skill：

- `.claude/skills/awesome-design-md`
- `.claude/skills/awesome-design-systems`

前端页面新增或修改时，必须先使用这两个 skill 补齐设计上下文，再进入 `Pencil MCP` 原型阶段。

- `awesome-design-md` 用于补齐 `DESIGN.md` 风格上下文。
- `awesome-design-systems` 用于引用真实设计系统案例，避免主观臆造。

**Why:** 前端原型确认不能只靠口头描述或主观审美，需要先有风格约束和真实设计参考，才能让原型讨论更稳定。

**How to apply:**
- 开始前端原型任务时，先加载这两个 skill。
- 若缺少 `DESIGN.md`，先补设计上下文或明确本次为什么不落地。
- 完成上下文后，再进入 `Pencil MCP` 原型确认。

相关：[[project-memory-structure]]
