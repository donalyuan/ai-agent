## ADDED Requirements

### Requirement:portable manifest 与媒体载荷完整可验证
系统 SHALL 导出 `exportProfile=portable` 的 manifest、完整媒体载荷、checksum/ETag、AssetVersion/TimelineVersion/Provider/Skill/授权依赖和 schema version；manifest 不得包含密钥或不可解析私有 URL。

#### Scenario:生成可迁移工程包
- **WHEN** 用户选择固定项目/集/TimelineVersion 并通过容量、license 和 renderer preflight
- **THEN** 系统生成可校验 portable artifact，记录所有对象和依赖，原 light artifact 不变

### Requirement:导入先预检后提交
导入 MUST 在隔离 workspace 执行 schema、hash、路径穿越、容量、病毒/格式、许可证和 owner 冲突预检；只有用户显式确认后才创建新项目或新版本，禁止覆盖既有事实。

#### Scenario:导入冲突需要显式解决
- **WHEN** 包含同 ID 不同 hash 的 AssetVersion 或 WorkflowVersion
- **THEN** 预检返回 conflict report 和可选映射，零项目、资产、时间线或 Provider mutation

#### Scenario:导入恢复可重复
- **WHEN** 导入任务中断后以同一 package hash 和 operation key 恢复
- **THEN** 系统复用已验证对象并追加一次性新版本，不重复上传或登记

### Requirement:portable 加密容器与密钥隔离
portable 载荷 MUST 使用 B3 设计阶段冻结的加密容器和 authenticated encryption；manifest 只能保存算法、密钥引用和版本，不得保存明文密钥、token 或永久私有 URL。导出、导入、轮换和恢复 MUST 在密钥缺失、密文篡改或认证失败时 fail-closed，并保留可定位诊断。

#### Scenario:密钥缺失阻断导入
- **WHEN** 导入端无法解析 package 的密钥引用
- **THEN** 预检保持 blocked，零对象上传、owner mutation、ProviderCall 或成功状态写入

#### Scenario:密文篡改被拒绝
- **WHEN** portable 分片或 manifest 的认证标签校验失败
- **THEN** 导入返回 integrity_failed，清理未登记的临时载荷，不影响既有项目事实
