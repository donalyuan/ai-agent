## ADDED Requirements

### Requirement:统一审核队列是投影而非事实源
系统 SHALL 聚合文本、Scene/Shot、AssetVersion、TimelineVersion 和 QC 任务到按 project scope 过滤的 Review Inbox；accept/reject/retake MUST 转换为各 owner 的 typed command，Review Inbox 不得复制或覆盖 owner 版本。

#### Scenario:从队列接受媒体候选
- **WHEN** reviewer 对仍匹配 source revision/hash 的媒体候选执行 accept
- **THEN** 系统调用对应 owner command 并更新投影状态，历史候选和审计保持可读

#### Scenario:投影滞后时安全重读
- **WHEN** 队列事件缺失、重复或 owner revision 已变化
- **THEN** 系统标记 projection_stale 并要求重读 owner，不执行第二次 accept 或外部调用

### Requirement:版本锚定评论、时间码与批量决策
评论 MUST 锚定稳定 owner ID、revision 和可选 frame/timecode；批量决策 MUST 固定目标集合和 operation group，权限、CAS、license 和预算检查逐项执行。

#### Scenario:批量驳回保持逐项结果
- **WHEN** reviewer 对固定集合提交批量 reject，且其中一项已过期
- **THEN** 系统按幂等键记录逐项成功/失败和原因，过期项不被覆盖，不扩大目标集合

#### Scenario:评论不泄漏敏感内容
- **WHEN** 评论或通知通过 SSE 推送
- **THEN** 只发送脱敏引用、revision 和摘要；原始密钥、提示词全文和持久 URL 不出现在事件中

### Requirement:审核任务分派与超期治理
系统 SHALL 支持 ReviewTask 的 `open|assigned|in_review|completed|reopened|cancelled|overdue` 生命周期，以及分派、改派、认领、撤销分派、完成后 reopen 和取消命令，并记录操作人、revision 和幂等键。认领形成可过期锁定，完成、取消或锁超时必须释放锁。任务 MUST 支持 `dueAt`、超期状态、提醒升级和按 assignee/due 状态筛选；分派、生命周期和决策权限 MUST 按项目角色及 owner scope 校验。

#### Scenario:认领已分派任务
- **WHEN** reviewer 认领仍处于待审核且未被其他人锁定的任务
- **THEN** 任务以 CAS 原子绑定该 reviewer；重复认领或过期 revision 返回冲突且不改变任务

#### Scenario:超期任务升级提醒
- **WHEN** ReviewTask 超过 dueAt 且仍未完成
- **THEN** 投影为 overdue 并生成脱敏升级通知；通知重复消费不产生重复提醒

#### Scenario:完成后重新打开任务
- **WHEN** owner revision 发生变化或 reviewer 发现完成结论需要复核，具备权限的用户对 `completed` 任务提交 reopen
- **THEN** 任务以 CAS 进入 `reopened` 并锚定新的 owner revision；重复 reopen、无权限或过期 revision 返回稳定错误且不重复创建任务或 ProviderCall

#### Scenario:取消任务释放锁
- **WHEN** owner 或 reviewer 按权限取消未完成的 ReviewTask
- **THEN** 任务进入 `cancelled`，认领锁和提醒被释放/抑制，历史决策和评论保持可读，重复取消幂等

### Requirement:可配置 QC 策略与人工覆盖
系统 SHALL 持久化语义/视觉 QC policy、阈值、证据引用和 revision，并区分 `candidate|passed|failed|unavailable|overridden` 状态。QC 失败 MUST 支持显式重跑或转人工审核；人工 override 必须具备权限、理由和审计，不得静默改变 owner 事实或重复外部调用。

#### Scenario:QC 失败转人工
- **WHEN** QC 结果低于冻结阈值且 reviewer 选择转人工审核
- **THEN** 生成一个幂等 ReviewTask，保留原始 QC 证据，不自动接受候选

#### Scenario:人工覆盖 QC
- **WHEN** 具备权限的 reviewer 提交带理由的 override
- **THEN** 只更新 QC 投影和审计状态；无权限或重复 override 返回稳定错误且零 owner/Provider 写入
