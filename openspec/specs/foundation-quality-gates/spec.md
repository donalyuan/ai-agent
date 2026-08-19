# foundation-quality-gates Specification

## Purpose
TBD - created by archiving change establish-phase-zero-foundation. Update Purpose after archive.
## Requirements
### Requirement: R8 TDD 优先的基础测试
阶段 0 实现 SHALL 以失败测试先于最小实现的顺序交付。测试套件 SHALL 覆盖 Schema 正反样例、Pydantic/ORM 边界、Alembic upgrade、Port Mock、LocalWorkspaceAdapter、SkillRegistry、SkillRouter、健康端点和无密钥配置。

#### Scenario: 运行阶段 0 基础测试
- **WHEN** 开发者运行约定的测试命令
- **THEN** 测试报告显示上述基础行为的通过或原始失败，不跳过未配置的真实 Provider 行为来报告成功

### Requirement: R8 类型、格式和契约检查
Web 与 Python SHALL 分别提供可重复的类型和格式检查。JSON Schema SHALL 使用 Draft 2020-12 元 Schema 与正反样例验证；Alembic SHALL 验证从空数据库升级；Compose SHALL 至少通过配置解析验证。

#### Scenario: 执行静态质量门
- **WHEN** 开发者运行阶段 0 的格式、类型、Schema、迁移和 Compose 检查
- **THEN** 每项命令输出可判定结果，失败时保留原始错误和受影响检查

### Requirement: R8 BDD 和 SDD 可追溯性
阶段 0 的可观察验收 SHALL 由 OpenSpec 场景、JSON Schema、Port 协议和测试名称共同追溯。每项实现任务 SHALL 链接其 DDD 所有权、BDD 场景、SDD 契约或 TDD 测试中的至少一项；产品非目标 SHALL 有明确的范围检查。

#### Scenario: 检查范围和追溯
- **WHEN** 审阅阶段 0 的 OpenSpec 与实现清单
- **THEN** R1-R8 均能追溯到规范、测试和验收命令，且没有真实付费调用、完整生成、专业剪辑、多人、手机端、发布、TikTok、多 Agent 产品能力或 semantic 模型任务
