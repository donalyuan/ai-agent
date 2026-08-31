## Context

阶段一使用模块化单体、SQLAlchemy collection-backed owner document、Temporal durable start ledger 和 `LocalWorkspaceAdapter`。review 证明这些边界在并发、跨容器和伪造输入下没有闭合。真实 Provider/TOS/FFmpeg 仍保持显式未配置，本 change 只修复 Mock/Local 默认路径和安全合同。

## Goals / Non-Goals

**Goals:**

- 让文本生成 activity 使用同一持久 UoW 和 catalog owner，成功后通过 `RunsService.enter_text_review` 进入审核状态。
- 使 collection document、multipart session 和 owner snapshot 的更新采用加载 revision CAS。
- 统一 HTTP project scope header、SourceMaterial/AssetVersion 事实校验和 canonical object key。
- 让 API、Agent、Media Worker 共享 Compose Local workspace，并让 Local storage 在没有跨 owner 引用索引时 fail closed。

**Non-Goals:**

- 不引入真实 Provider/TOS/FFmpeg，不改变现有外部适配器能力边界。
- 不把媒体二进制写入数据库，不改变 AssetVersion append-only owner 模型。
- 不把 scope header 伪装成用户认证；身份认证仍由后续部署边界负责。

## Decisions

1. **Activity 内执行 application service。** Temporal workflow 继续保持确定性，只选择 activity；activity 从数据库加载冻结 Run/Brief/Source binding，构造 `GenerateTextBatchCommand`，生成后调用 `enter_text_review`。这样 Provider 副作用仍在 activity，状态转换仍归 Runs owner。
2. **PhaseOneDocument 使用加载值 CAS。** 每个 collection 进入 UoW 时记录 revision 和编码 payload。提交时只有 payload 变化才执行 `UPDATE ... WHERE collection = ? AND revision = loaded_revision`，新 revision 为 loaded+1；rowcount 非 1 抛 `RevisionConflictError`，绝不覆盖并发事实。
3. **Scope 依赖在 router mount 统一执行。** creative、scenes、text、video、agent-edit 和 Asset Center router 使用同一个 FastAPI dependency 检查非空 `X-Project-Scope`；路径携带 project ID 时在读取 owner 前比较，ID-only 路由把已验证 scope 传入 application service 并与实体真实 owner 比较。
4. **Multipart binding 全字段冻结。** `_LocalUpload` 持久化 project/profile/object key/size/checksum/MIME；operation key 命中时比较全部字段，`complete_multipart` 也比较完整 `UploadSessionRef`，避免跨项目引用复用。
5. **来源事实由 owner 派生。** `bind_source` 只能绑定 UoW 中同项目的 SourceMaterial current version，hash、revision、parse/validation 状态均从 owner 读取；uploaded source 只能引用同项目、完整元数据且处于允许状态的 AssetVersion，并从其 `content_hash` 派生 verified parse 状态。
6. **ProviderCall 使用 durable claim。** Text service 用 canonical prompt digest 作为 request fingerprint，先登记 `pending` ProviderCall，再通过 revision CAS claim 为 `unknown` 后调用 Provider；返回后先保存受控 usage/request id，batch 成功后终结为 `succeeded`，失败保存类型化脱敏 diagnostic。重试发现已有 batch 时补终结，发现无 batch 的 ambiguous call 时进入 reconciliation 且不重复提交；没有 catalog 注入的纯单元 service 保持未配置错误。
7. **Storage proof 由 composite owner 提供。** Local adapter 的 `prove_no_references` 永不返回成功证明；调用者必须注入 `CompositeStorageReferenceProof` 或其他真实 owner proof port。

## Risks / Trade-offs

- [并发冲突增加] 过去静默覆盖会变为明确 409/冲突，需要上层重试；→ 保留稳定 revision 错误并补并发测试。
- [旧 multipart manifest 缺少冻结字段] 历史 session 无法安全证明绑定；→ 缺失字段只允许在 intent 同样未声明时继续，完成时仍强制比较可用字段，冲突则 fail closed。
- [旧 HTTP 客户端未传 scope] 将收到 403；→ 同步 Web/API 调用统一发送 header，并在契约测试中固定该行为。
- [Activity 依赖数据库] Worker 无 `DATABASE_URL` 或 schema 未升级时会明确失败；→ Compose 已为 Agent 注入数据库并由 health/迁移流程负责可用性。
