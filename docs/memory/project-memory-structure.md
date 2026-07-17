---
name: project-memory-structure
description: 项目记忆采用总索引 + 分文件结构的长期约定
metadata:
  type: project
---

# 项目记忆结构

Novex 的持久记忆与需求上下文分四层：

- `/server/ai-agent/MEMORY.md`：项目级总索引和全局稳定约束。
- `/server/ai-agent/docs/memory/README.md`：文档区记忆索引。
- `/server/ai-agent/docs/memory/*.md`：具体主题记忆。
- `/server/ai-agent/docs/requirements/*.md`：产品、需求、数据库等需求类文档。

## 规则

- 新增长期规则时，优先写入对应主题文件。
- 当规则影响多个主题、跨文件约定或全局约束时，再同步更新根 `MEMORY.md`，必要时更新 `docs/memory/README.md`。
- 临时探索、一次性报错、未确认猜测、敏感信息不写入。
- 主题文件只保留当前有效决策；被覆盖的旧口径应删除或明确标为历史，不得并列造成冲突。
- OpenSpec 任务数字、临时 Worker 开关等易变化状态以仓库事实为准，不在根 `MEMORY.md` 复制快照。

**Why:** 避免把所有长期信息压在单一文件里，也避免索引和主题记忆脱节。分层后更容易维护，也更容易让后续会话快速定位稳定约定。

**How to apply:**
- 新需求先判断属于哪个主题记忆。
- 如果是新稳定规则，先写主题文件，再视影响范围更新两个索引。
- 需要回忆历史决策时，先读根 `MEMORY.md`，再读对应主题文件。

相关：[[project-tech-stack]]、[`video-agent-mvp`](../requirements/video-agent-mvp.md)、[`video-agent-database-schema`](../requirements/video-agent-database-schema.md)
