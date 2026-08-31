## Why

## 设置投影与操作门

设置页展示八项 Skill candidate 的 provenance、approval、enabled：仅 `novel-writing` 和 `drama-skills` 为 `verified_snapshot`/`approved`/enabled，且默认 `drama-mvp-a-default` 只绑定这两项；另六项为 `pending_provenance`/disabled，不能成为启动或默认 Run 前置，只有 node `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 都满足后按需读取。

设置页的首次 connection-test/probe 仅对已安装、catalog `approval=approved`、`featureGate=MVP-A` 且用户提供 explicit live opt-in、已选 profile、可解析 credential、timeout 的 operation 暴露，成功后冻结 capability snapshot，不能要求既有 snapshot 或 `runnable=true`，也不因 disabled-for-run 阻断。snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default/Run resolution/live invocation，后者还要求该成功 snapshot 与 `runnable=true`。MVP-B/uninstalled/not-approved 或缺 opt-in/profile/credential/timeout 的 operation 不发 probe/网络；TTS/ASR、MiniMax H3、Seedance 2.5、Agnes 未选中 mode 保持不可运行。默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），并保持 explicit live opt-in；Local 不是 TOS 失败 fallback，运行开始后 Adapter/Profile 冻结。

Provider、Profile、Model、SkillRevision 和 capability snapshot 的配置必须由用户显式查看、比较和确认，尤其不能从浏览设置时暴露密钥或意外触发真实调用。当前阶段 0 只有最小壳层，缺少集中、安全且可审计的设置工作流。

## What Changes

- 新增 Provider/Profile/Model/SkillRevision/capability snapshot 的设置导航、列表、详情和参数 Schema 表单契约。
- 定义密钥 replace/rotate 的掩码输入与绝不回显规则；模型同步只显示 diff，且须人工逐项接受。
- 呈现 system、project、workflow 覆盖层级和生效来源；连接测试与 capability probe 必须是显式命令。
- 新增 projects owner 项目文本费用阈值、图片/视频批量生成确认、`cost=unknown` 二次确认、`run_id + logical_operation` 绑定状态和 `retention_policy/version/hold` 的设置/审计视图；确认不得在其他 Run、重试或参数变更后复用。
- 明确设置页只管理 SkillRevision/lifecycle，不承担某次运行的 Skill 路由人工裁决；路由候选、原因和选择在 Workbench 展示并由 text/Agent runtime 保存。
- 规定 `Mock Provider +` 显式 Local test/offline profile、owner DTO/Zod/缓存边界与失败状态；访问设置或编辑草稿不得自动调用 Provider、同步模型、探测能力或创建/切换 profile。
- 在设置页提供 Provider/Profile/Model/Skill 的显式创建、编辑、启用和停用入口；所有 mutation 携带 `expectedRevision`/`If-Match`，历史 `SkillRevision` 与已冻结 snapshot 不被覆盖。
- 新增独立 StorageProfile 设置页面与表单：`StorageProfile/BucketBinding` 的 Bucket、Region、Endpoint、private policy、credential reference/status、timeout、presign TTL、project scope；支持 create/edit/enable/disable、`expectedRevision`/`If-Match` 409、显式 connection test 和掩码 credential 状态。
- 新增 Provider/Profile 的 per-operation 并发/限流配置和 quota `known|unknown|exhausted` 只读状态；429/`Retry-After` 显示 owner 诊断。Model 有历史引用时 UI 只提供停用，不提供或不得成功执行物理删除。

## Capabilities

### New Capabilities
- `provider-model-skill-settings-ui`: Provider、模型、Skill、密钥状态、能力探测与覆盖说明的桌面设置 UI contract。

### Modified Capabilities
- 无。

## Impact

- 后续实现将修改 `apps/web`，并消费 `implement-provider-model-skill-catalog` 及各 Provider change 的 owner contract。
- 使用 React 19/Vite/React Router/Lucide 和后续引入的 TanStack Query、Zustand、Zod、shadcn/Radix、Tailwind；不修改 Provider adapter、凭据存储或真实调用路径。

## Probe matrix boundary

默认 browser E2E 一律使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；live `1x1x1`（1 Episode x 1 Scene x 1 Shot）仅是显式 opt-in provider/storage/renderer probe。未配置必须保留 `unconfigured`，设置页不得把 probe 启动为默认 E2E、创建或切换 profile，或伪造成功。

## Catalog/Credential UI 合同

**DDD**：设置页不拥有 catalog/credential/StorageProfile 状态，只编排 catalog storage-config owner 的 Provider/Profile/Model/Skill/StorageProfile 生命周期命令。**BDD**：通用资源与 StorageProfile 的创建/编辑/启停、409 conflict、connection-test、masked credential、503 master-key unavailable 与 rotate/re-encrypt failure 可见。**SDD**：StorageProfile 表单严格消费 owner 字段和 `expectedRevision`/`If-Match`，绝不回显 envelope、key、object URI。**TDD**：验证 StorageProfile CRUD/connection-test 的成功/失败/零隐式调用，enable/disable 不改历史 snapshot，Skill 内容编辑只产生新 revision。

## Operation policy/quota UI

设置页只编辑 owner 的 operation policy revision，显示 max concurrency、rate window/limit 和 quota source/capturedAt/status；不在客户端计算 active lease 或剩余额度。Model delete 先读取 owner reference proof，`model_in_use`/proof unavailable 时只显示停用 command；任何 UI 隐藏不能替代后端保护。

## 阶段一表格与动态表单边界

本 change MUST 使用 TanStack Table 展示 Provider、Model、Skill（含 revision/provenance/approval/enabled、operation policy 和 quota）列表，并使用 React Hook Form + Zod 生成 Provider/Model/Skill 动态参数表单。表格、字段错误、dirty/submit 状态和 409 恢复只消费 shared/ui 封装；不得新增第二套表格、表单或组件库。表格筛选/排序为读取，保存仍由 owner `expectedRevision`/`If-Match` command 决定。

验收必须覆盖未知参数 schema、字段级 Zod 错误、masked credential、unknown quota、model disable-only、candidate diff 和显式 probe；读取设置和动态表单渲染不得触发 ProviderCall、网络 probe 或 Run。
