## MODIFIED Requirements

### Requirement:MVP-A `light` ProjectPackage
系统 SHALL 继续为 MVP-A 导出 `exportProfile=light` 的 manifest/reference-only `ProjectPackage`，并新增阶段二 `exportProfile=portable` 的完整媒体载荷和可回导清单。`light` MUST 保持不可回导且字段兼容；`portable` 必须遵守 phase-two-portable-package 的预检、加密、hash、license 和冲突处理要求。两个 profile 共用 `schema_version`、TimelineVersion、AssetVersion、音频/字幕、models、skillRevisions、usage/cost 和 retention/hold 字段，不得形成第二版本源。

#### Scenario:阶段一 light 包继续可读
- **WHEN** 用户读取历史 light artifact
- **THEN** 系统按原 schema 返回引用和 provenance，禁止导入或把它升级为 portable

#### Scenario:阶段二 portable 包可回导
- **WHEN** portable manifest 和全部媒体 hash 通过预检且用户确认导入
- **THEN** 系统创建新项目/版本并保留原项目历史，不覆盖任何 owner fact
