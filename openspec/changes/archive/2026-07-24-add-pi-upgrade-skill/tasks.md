## 1. Skill 脚手架与元数据

- [x] 1.1 使用 `skill-creator` 初始化 `.agents/skills/upgrade-pi-runtime`，生成合规的 `SKILL.md`、`agents/openai.yaml` 和 `scripts/` 目录

## 2. 版本检查能力

- [x] 2.1 实现只读版本检查脚本，校验 manifest/lockfile 三包一致性，并支持 npm 与 GitHub 上游发布对照

## 3. 升级工作流

- [x] 3.1 编写 Skill 工作流，覆盖独立 OpenSpec change、发布产物差异审查、精确同步升级、隔离 lockfile、数据备份回滚和完整回归门禁

## 4. 验证

- [x] 4.1 运行 Skill 结构校验、脚本语法检查、离线检查和联网检查，确认只读输出与错误语义符合规格

## 5. OpenSpec 收口

- [x] 5.1 复核变更范围与工作区差异，重新执行 apply instructions 并确认状态达到 `all_done`
