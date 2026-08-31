## ADDED Requirements

### Requirement:批量 operation group 固定目标和快照
跨集生成、审核、导出、QC、备份和恢复 SHALL 创建 operation group，冻结 project/episode/owner revision、目标集合、权限、预算和 capability snapshot；执行结果逐项幂等记录。

#### Scenario:批量导出不扩大范围
- **WHEN** 用户提交三集的显式 TimelineVersion 列表
- **THEN** 系统只处理这三集并分别生成 artifact；新增集必须另建 operation group

#### Scenario:批量项部分失败可重试
- **WHEN** operation group 中一项因 quota 或 renderer 失败
- **THEN** 失败项保留 diagnostic 和 operation key，可单项重试，不重跑已 succeeded 项

### Requirement:自动备份恢复和通知可审计
系统 SHALL 按保留策略自动生成 PostgreSQL/object manifest/config/key reference backup，通知只发送脱敏状态；恢复必须经过隔离校验、operator 确认和 checksum/ETag gate。

#### Scenario:备份容量不足
- **WHEN** backup 预计占用超过硬阈值
- **THEN** 任务保持 blocked 并记录 observed/required capacity，不删除历史审计或偷偷切换目标

#### Scenario:恢复完成通知
- **WHEN** 隔离恢复通过所有 hash、权限和归属校验
- **THEN** 记录 restore correlation、operator UUID 和结果，通知可定位但不包含密钥或对象私有 URL

#### Scenario:操作台重试失败项
- **WHEN** 用户在 Operations UI 查看固定目标集合并仅重试失败项
- **THEN** succeeded 项保持不变，失败项沿用原 snapshot/idempotency contract；无权限、过期会话或容量阻断时只显示可读诊断，不创建新 ProviderCall 或恢复写入
