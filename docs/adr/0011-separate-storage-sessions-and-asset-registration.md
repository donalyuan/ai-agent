# ADR-0011：分离存储会话与素材版本登记

- 状态：已接受
- 日期：2026-08-23

## 决策

Storage 只拥有 `StorageProfile`、private bucket binding、upload operation/session/part、
verified immutable `StoredObjectRef`、reference proof 与 recovery record。Assets owner 先创建
`AssetVersionReservation`，所有通用上传复用
`asset-upload:{projectId}:{assetId}:{reservationId}`，对象校验后再由 Assets application 在自己的
UoW 幂等登记一个 `AssetVersion`。SourceMaterial、ExportArtifact 与 audio selection 保持各自 owner
的显式 binding/register/select 步骤。

`LocalWorkspaceAdapter` 将 multipart session 与 part 保存到受控 workspace，可在 API/Worker 重启后
恢复并流式 complete/stat；成功/失败临时文件分别保留 24 小时/7 天，业务对象不进入 cleaner。
delete 只有在 AssetVersion、Run、Timeline、Package/manifest 四类 owner 都给出 no-reference 证明后
才执行。TOS 只消费 catalog `CredentialResolver`，当前没有已批准 SDK、账号、private bucket、Docker
Secret 或凭据，因此 production adapter 和 connection test 返回明确 `unconfigured`，不发网络请求、
不 fallback Local，也不伪造 live 成功。

## 结果

- Alembic `0019_storage_owner` 只增加 storage owner 的规范化元数据表，不保存媒体 bytes，也不改写
  AssetVersion/objectKey。
- 默认测试使用 Mock Provider 与显式 `local_workspace` profile；deterministic TOS transport fake 只能
  由测试显式注入。
- 阶段退出的 2 GiB 证据使用不入仓库的有效 PNG fixture，验证 actual bytes、multipart 重启恢复、
  complete/stat/hash/MIME/ETag、单一 AssetVersion 和 Media Worker proxy；报告位于临时目录。
- capacity/admission 与 backup/restore/checksum drill 仍由 operations-resilience owner 决策，Storage
  只提供 capability 和 transport/object facts。
