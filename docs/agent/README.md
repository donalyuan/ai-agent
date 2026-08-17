# 代理持久记忆

此目录保存可由后续会话复用、且能由仓库事实验证的项目记忆。开始仓库任务时固定读取 [项目事实](PROJECT.md)、[长期决策](DECISIONS.md) 与 [当前交接](HANDOFF.md)；诊断问题时再读取 [排障记录](TROUBLESHOOTING.md)。

| 文件 | 用途 |
| --- | --- |
| [PROJECT.md](PROJECT.md) | 当前可证实的项目事实、目录与已验证命令。 |
| [DECISIONS.md](DECISIONS.md) | 长期决策的简短索引与 ADR 链接。 |
| [HANDOFF.md](HANDOFF.md) | 当前分支、最近完成工作、验证和待确认事项。 |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | 仅记录已观察且可复现的问题与证据。 |

当前代码、测试、schema 和可执行配置始终优先于本目录内容。不得在此保存秘密、凭据、令牌、私有数据或设备相关绝对路径。
