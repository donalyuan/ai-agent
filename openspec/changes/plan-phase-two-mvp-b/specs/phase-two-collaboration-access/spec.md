## ADDED Requirements

### Requirement:受控 LAN 身份和项目角色
系统 SHALL 默认关闭非 localhost 监听；显式开启 LAN 后要求短期会话 token、Origin/CSRF 校验、撤销和审计，并提供 owner/editor/reviewer/viewer 角色。每个写命令 MUST 验证 project scope 和角色权限。

#### Scenario:默认不暴露 LAN
- **WHEN** 未配置显式 LAN opt-in
- **THEN** 服务仅监听 localhost/127.0.0.1，远程请求不可达且不创建会话

#### Scenario:viewer 尝试写入
- **WHEN** viewer 对项目发送编辑、接受或导出命令
- **THEN** 返回 forbidden，零 owner、Outbox、ProviderCall 或 storage mutation

### Requirement:协作冲突可解释且可恢复
共享编辑、评论和 presence SHALL 通过事件流广播稳定 ID、revision 和脱敏摘要；revision 冲突返回 409 和差异，客户端必须重读后重新提交，不得静默覆盖。

#### Scenario:两个编辑者同时修改
- **WHEN** 两个 editor 使用同一 revision 提交不同命令
- **THEN** 一个成功、另一个收到 conflict diff；历史命令和 owner revision 均可追溯

### Requirement:成员与会话管理可审计
项目 owner SHALL 能邀请、移除成员、变更 `owner|editor|reviewer|viewer` 角色并撤销其会话；系统 MUST 保护最后一个 owner，成员变更使用 expectedRevision/幂等键并使权限缓存立即失效。协作 UI SHALL 展示 LAN 监听诊断、成员角色、会话状态、锁状态和冲突重读入口。

#### Scenario:移除成员立即撤销访问
- **WHEN** owner 移除项目成员或撤销其会话
- **THEN** 目标会话立即失效，后续写请求返回 unauthorized/forbidden，历史操作和成员变更审计保持可读

#### Scenario:保护最后一个 owner
- **WHEN** owner 尝试移除或降级项目最后一个 owner
- **THEN** 返回 `last_owner_required`，成员关系、会话和权限缓存均不改变
