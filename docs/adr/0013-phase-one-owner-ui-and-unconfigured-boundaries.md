# ADR-0013：阶段一 owner UI 与未配置外部能力边界

## 状态

已接受（2026-08-23）

## 决策

阶段一 UI 只消费 owner projection，并以 project/Episode scope、stable ID、revision、content hash 和 expectedRevision/CAS 发送显式 commands。Timeline 只编辑每集唯一 current Cut，published TimelineVersion 只读比较；Exports 只接受显式 Episode/TimelineVersion 集合并按集生成 MP4/SRT/light owner jobs。Provider、Model、Skill、StorageProfile 设置只显示 masked credential、provenance、quota、retention 和 capability facts，连接测试/模型同步/启停均需用户 command。

页面读取、路由切换、筛选、比较、会话恢复和诊断读取不得触发 Provider、Agent、Storage、Worker、Timeline、Export 或付费 mutation。缺少真实凭据、TOS account 或 `FFMPEG_PATH`/`FFPROBE_PATH` 时必须返回 `unconfigured`、`renderer_unconfigured` 或 503，并继续使用显式 `Mock Provider + Local offline`（`adapter identity=local_workspace`）验证不依赖外部凭据的路径。

## 依据

- `apps/web/src/timeline/api.ts`、`apps/web/src/pages/TimelineEditorPage.tsx`：Timeline command 与版本发布均绑定 owner revision。
- `apps/web/src/pages/ProviderSettingsPage.tsx`、`apps/web/src/pages/StorageProfilePage.tsx`：密钥不回显，probe/connection-test 为显式动作。
- `services/api/tests/test_timeline_export_slice.py`、`services/api/tests/test_catalog_slice.py`、`services/api/tests/test_storage_provider.py`：409、renderer、master-key 和 foreign scope 拒绝无副作用。
- `output/playwright/` 与 `docs/evidence/`：真实 localhost 导航和 2 GiB/resilience/observability 证据。
