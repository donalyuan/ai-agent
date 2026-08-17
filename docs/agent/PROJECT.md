# 项目事实

## 当前状态

- 记录日期：2026-08-17。
- 项目处于规则、OpenSpec 与持久记忆初始化阶段。
- 初始核对时，已跟踪文件为 `AGENTS.md` 与 `CLAUDE.md`；`openspec/config.yaml` 已存在且 schema 为 `spec-driven`。
- 初始核对已验证 `git status --short`、`git branch --show-current`、`git log -1 --oneline`、`openspec list --json` 与 `openspec --version` 可执行；当时分支为 `main`，OpenSpec 版本为 `1.3.1`，且无既有 change。

## 当前可证实目录

- `docs/agent/`：本目录，保存项目持久记忆。
- `docs/adr/`：架构决策记录。
- `openspec/`：OpenSpec 配置与变更 artifacts。

## 待确认

以下信息当前没有可验证的项目文件，必须在得到证据或用户确认后再记录为事实：

- 产品目的与用户范围。
- 技术栈、应用或服务目录、依赖与数据存储。
- 安装、运行、测试、lint、build、部署命令。
- 外部服务、凭据管理与运行环境。

## 使用规则

此文件只记录稳定且已确认的项目事实。与当前代码、测试、schema 或可执行配置冲突时，以后者为准并更新本文件。
