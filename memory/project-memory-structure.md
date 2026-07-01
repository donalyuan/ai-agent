---
name: project-memory-structure
description: 项目记忆采用总索引 + 分文件结构的长期约定
metadata:
  type: project
---

# 项目记忆结构

video-agent 的持久记忆分三层：

- `/server/video-agent/MEMORY.md`：项目级总索引和全局稳定约束。
- `/server/video-agent/memory/MEMORY.md`：子目录索引。
- `/server/video-agent/memory/*.md`：具体主题记忆。

## 规则

- 新增长期规则时，优先写入对应主题文件。
- 当规则影响多个主题、跨文件约定或全局约束时，再同步更新根 `MEMORY.md`，必要时更新 `memory/MEMORY.md`。
- 临时探索、一次性报错、未确认猜测、敏感信息不写入。

**Why:** 避免把所有长期信息压在单一文件里，也避免索引和主题记忆脱节。分层后更容易维护，也更容易让后续会话快速定位稳定约定。

**How to apply:**
- 新需求先判断属于哪个主题记忆。
- 如果是新稳定规则，先写主题文件，再视影响范围更新两个索引。
- 需要回忆历史决策时，先读根 `MEMORY.md`，再读对应主题文件。

相关：[[project-tech-stack]]、[[mvp-requirements]]、[[database-schema]]
