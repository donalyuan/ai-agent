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

视频工作台原型源文件固定为 `docs/prototypes/video-agent/video-agent.pen`。涉及视频工作台的原型修改必须更新该文件，不再使用 `docs/prototypes/script-agent-workspace/` 截图目录。

- `awesome-design-md` 用于补齐 `DESIGN.md` 风格上下文。
- `awesome-design-systems` 用于引用真实设计系统案例，避免主观臆造。

**Why:** 前端原型确认不能只靠口头描述或主观审美，需要先有风格约束和真实设计参考，才能让原型讨论更稳定。

**How to apply:**
- 开始前端原型任务时，先加载这两个 skill。
- 若缺少 `DESIGN.md`，先补设计上下文或明确本次为什么不落地。
- 完成上下文后，再进入 `Pencil MCP` 原型确认。

相关：[[project-memory-structure]]

## 工作台 Select 统一规范

2026-07-21 用户确认视频工作台的标准单行 Select 必须统一使用同一套样式：高度 `36px`、白色背景、`#B8C2D1` 边框、`6px` 圆角、`13px` 输入值、固定右侧下拉箭头，并统一 hover、focus、disabled 和 expanded 状态。页面不得继续新增局部原生 Select 样式；标准 Select 应复用统一组件和样式令牌，segmented control、复选框、开关等不同语义控件不强制伪装为 Select。

音色等目录型选择器必须复用工作台统一的可搜索选择器结构：显示可用数量，提供名称/描述/标签搜索、语言筛选、声线筛选和可滚动结果列表，列表项展示名称、描述与语言/声线标签。选项必须来自当前模型的动态能力目录，禁止前端硬编码；切换模型后原选择失效时必须保留失效值、明确标记并阻止提交，禁止静默替换。

相关：[[video-agent-workspace-flow]]
