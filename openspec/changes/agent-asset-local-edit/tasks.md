## 1. Contract and Persistence

- [ ] 1.1 定义 image/video/audio selection、mask、time range、Plan、Execution、Candidate、Impact Schema
- [ ] 1.2 设计 owner 表、base AssetVersion revision/hash、scope、capability、budget 和 no-GC 约束
- [ ] 1.3 建立合法/非法 selection、范围、未知字段、跨项目和 stale fixtures

## 2. Plan and Execution

- [ ] 2.1 实现从上下文 turn 生成 Schema-valid 局部编辑计划
- [ ] 2.2 实现费用/能力确认、execute intent、Outbox 和稳定 operation key
- [ ] 2.3 实现 image/video/audio adapter capability mapping、submit/poll/result/reconcile
- [ ] 2.4 实现候选登记、输入输出 hash、impact/stale 分析和显式 accept/reject
- [ ] 2.5 实现 unsupported、unconfigured、quota unknown、409 和权限 fail-closed

## 3. UI and Verification

- [ ] 3.1 更新 Agent 上下文 UI 展示 selection/mask/range、费用、能力和影响范围
- [ ] 3.2 增加 execute/accept 前确认、冲突刷新和禁止隐式降级交互
- [ ] 3.3 增加版本冲突、重复提交、Provider unknown、权限和零副作用测试
- [ ] 3.4 完成候选生成到 owner 接受的 Playwright 闭环
- [ ] 3.5 完成 OpenSpec strict、历史 AssetVersion 兼容和 no-GC 验证
