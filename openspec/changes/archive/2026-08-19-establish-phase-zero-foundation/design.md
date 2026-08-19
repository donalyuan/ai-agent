## Context

当前仓库只有规则、基线和已归档的 OpenSpec 变更。产品目标是个人、本地优先的剧情短剧与小说改编工作台；阶段 0 必须把后续阶段共享的工程边界和数据契约固定下来，同时不能以真实凭据、付费调用或未验证的供应商能力作为启动前提。

利益相关者是单一开发者与后续阶段的实现任务。该阶段的验收对象是可重复的本地工程基础，而不是可生成、剪辑或发布视频的产品闭环。

## Goals / Non-Goals

**Goals:**

- 建立单仓库边界：`apps/web`、`services/api`、`workers/{agent,generation,media}`、`packages/contracts` 与 `infra/compose`。
- 以 JSON Schema Draft 2020-12 定义九个跨层文档，并在 API 中以 Pydantic 2 进行边界校验；数据库模型与迁移保存基础实体、稳定 ID、`revision`、状态和不可变版本引用。
- 建立六个业务 Port、Provider/Profile/Model 的数据驱动配置、Mock Provider、`LocalWorkspaceAdapter` 与可重复的无密钥启动。
- 建立本地 Compose 运行形态和可执行质量门，使后续功能可在同一契约上增量实现。

**Non-Goals:**

- 不实现真实文本、图片、视频、TTS、ASR 或 TOS 调用，不保存或猜测真实 `base_url`、模型、bucket、region、密钥或能力快照。
- 不实现完整画布、专业剪辑、生成编排、FFmpeg 渲染、素材入库、发布、TikTok、手机端、多人协作或多 Agent 产品行为。
- 不启用 semantic 模型或独立 semantic 服务；不把 Redis、MinIO、生产环境、Kubernetes 或外部 Skill 仓库作为阶段 0 依赖。

## Decisions

### 1. 单仓库加独立容器职责

Web、API、三类 Worker 与契约包保持同一版本库，Compose 启动它们和 PostgreSQL、Temporal。Worker 即使在阶段 0 尚未消费真实队列，也必须有独立入口、健康状态和最小配置边界。

选择该方案是为了在开发早期保持模块化单体的低运维成本，同时保留媒体与外部副作用的隔离。替代方案是先拆多个仓库或只建立 API；前者增加协调成本，后者会延后跨容器配置错误的发现。

### 2. JSON Schema 是跨语言文档契约的权威来源

`packages/contracts` 保存九份带显式 `schema_version` 的 Draft 2020-12 JSON Schema；Web 使用生成或校验所得的类型，API 的 Pydantic 边界模型不得维护第二套含义不同的文档结构。Pydantic/OpenAPI 继续是 HTTP 边界的来源。

选择此分层可让版本化领域文档跨 Web、API、Worker 与未来工程包一致。替代方案是只靠 ORM 或 Pydantic；它们无法直接成为前端和 Worker 的可移植文档契约。

### 3. 最小 DDD 持久化模型先固定身份与版本语义

Project、Episode、Scene、Shot、Asset、AssetVersion、WorkflowDraft、WorkflowVersion 与 TimelineDocument 是阶段 0 的命名契约。API/数据库至少保存稳定 ID、所属关系、`revision`、状态、`schema_version` 和必要的不可变版本引用；状态遵循 `draft -> generated -> pending_review -> approved/rejected -> superseded/archived`。草稿更新以 revision 防止静默覆盖，已发布或版本记录不得覆盖。

选择最小模型而非全表实现，能先锁定后续聚合边界和审计语义。替代方案是用无结构 JSONB 或一开始实现所有业务表；前者会丢失引用约束，后者超出阶段 0。

### 4. Provider、模型和存储从业务流中反转

业务服务仅依赖 `TextModelPort`、`ImageGenerationPort`、`VideoGenerationPort`、`TtsPort`、`AsrPort` 和 `StoragePort`。Provider/Profile/Model 的标识、启用状态、参数 Schema 与默认值来自配置或数据库记录；业务代码不得内嵌 model、`base_url`、bucket 或 region。阶段 0 仅接入能返回确定性结果的 Mock Provider。

这让真实适配器作为后续增量，而不是让供应商 SDK 污染领域服务。替代方案是直接接入首个 Provider；它会引入凭据、费用和未验证能力，违反本阶段无密钥要求。

### 5. 本地工作区是 StoragePort 的开发实现

`LocalWorkspaceAdapter` 只在配置的相对工作区根下管理临时对象、测试对象和清理策略；业务记录只保存抽象对象引用，绝不保存宿主绝对路径。真实 TOS 仅以接口和示例配置占位。

该方案提供可测试的文件边界并避免依赖外部对象存储。替代方案是启动 MinIO 或模拟 TOS SDK；两者增加非必要服务和行为差异。

### 6. SkillRegistry 与确定性路由优先

`SkillRegistry` 管理本地 manifest 的名称、版本、来源、许可证、启用状态、输入输出和允许工具。`SkillRouter` 执行固定顺序：`deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide`，记录候选、得分、选择和回退原因。semantic adapter 是可选接口，未配置时路由仍确定且可人工选择。

选择此方案避免语义模型、网络和许可证成为基础运行依赖。替代方案是阶段 0 直接安装第三方 Skill 或语义路由服务；它们不能满足已确认的可控边界。

### 7. Compose 与配置按无密钥模式失败可见

Compose 默认绑定 localhost，并包含 Web、API、PostgreSQL、Temporal、Agent、Generation 与 Media 容器。示例环境文件只能包含非敏感占位符；启动时选择 Mock Provider 和本地工作区。缺少真实 Provider 配置时必须暴露“未配置”状态，不能伪装成真实调用成功。

选择此模式能让首日启动完全离线于凭据，同时保留真实配置的显式路径。替代方案是以空字符串或硬编码回退到某个服务，会导致误调用或不可诊断的失败。

### 8. 质量门从 TDD 启动并保留 BDD/SDD 追溯

每个实现批次先增加失败的单元、契约或集成测试，再最小实现通过；schema 需正反样例，API 需健康和配置行为测试，迁移需升级验证。格式、类型、测试、Schema 校验、Alembic 和 `docker compose config` 构成基础门；可用时再验证 Compose 启动与健康。

这把 DDD 的所有权、BDD 的可观察场景、SDD 的 JSON Schema/Port 与 TDD 的测试顺序统一到同一任务清单。替代方案是先搭空壳后补检查；它会让基础契约在实现压力下漂移。

## Risks / Trade-offs

- [基础 Compose 镜像或本机 Docker 不可用] → 将 `docker compose config` 与静态检查作为最低证据，保留原始启动错误，不宣称容器健康。
- [九个 Schema 过早冻结] → 每个 Schema 带 `schema_version`，后续不兼容变更必须新增迁移器和 OpenSpec 变更。
- [ORM 与 JSON Schema 语义漂移] → 用共享样例、Pydantic 边界测试和 migration 测试验证最小公共字段。
- [Mock 行为掩盖真实供应商差异] → Mock 仅验证 Port 协议与配置选择；真实适配器和 capability snapshot 明确留给后续阶段。
- [本地工作区泄漏绝对路径或超范围文件] → adapter 限制根目录、拒绝路径逃逸，并仅持久化抽象对象标识。
- [可选语义适配器影响确定性] → adapter 不可用、低置信或并列时保留确定性排序并要求人工选择。

## Migration Plan

1. 创建目录、锁定依赖、示例配置、Compose 和健康端点，使 Mock 模式可启动。
2. 先加入九份 Schema 与正反样例，再加入最小 Pydantic/ORM/Alembic 模型和迁移验证。
3. 在没有真实凭据的环境中实现 Port、Mock、LocalWorkspaceAdapter、SkillRegistry 和 SkillRouter。
4. 将所有检查接入本地命令与 Compose 健康验证；后续阶段只能通过新增 OpenSpec 变更扩展真实适配器和产品流程。

回滚时停止 Compose、回退本次新增的应用与基础配置；Alembic migration 必须提供受控 downgrade 或在本地开发数据库重建。任何含真实数据或凭据的环境均不在本阶段迁移范围内。

## Open Questions

- 真实 Provider、TOS 和生产密钥的具体配置尚未提供；阶段 0 不猜测，后续接入时需单独确认。
- Temporal 的精确镜像标签与本机 Docker 可用性需在实现和 Compose 验证时以实际工具输出确认。
- 未来从 JSON Schema 生成 TypeScript/Pydantic 的具体工具链可在实现时选择，但不得改变本设计的权威边界。
