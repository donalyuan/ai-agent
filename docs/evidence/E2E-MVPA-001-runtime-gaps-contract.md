# E2E-MVPA-001 runtime gaps contract

记录日期：2026-08-26。此文件是 `close-phase-one-mvp-a-runtime-gaps` 的 1.x 追溯制品，不声明 credentialed sandbox 已通过。默认验证仍是 Mock Provider + `local_workspace`，任何 live Profile/TOS/renderer 未配置均为 `unconfigured` 或 `not_ready`。

## Gap-to-task matrix

| 权威来源 | 可观察义务 | 当前 owner / 输入快照 | 状态与幂等身份 | 本 change 的任务与不变量 |
| --- | --- | --- | --- | --- |
| 产品需求文档 | Text -> review -> image/video candidate -> accepted current -> Media -> Timeline -> MP4/SRT/light，并可在 API/Worker 重启后恢复 | Run/NodeRun、TextReview、Scenes/Shot、Media、Export；冻结 project scope、Run input、candidate provenance、TimelineVersion | ProviderCall `pending|succeeded|failed|unknown|cancelled`；VideoOperation `pending|submitted|running|submission_unknown|succeeded|failed|cancelled`；`run_id + logical_operation` | 2.2-2.5、3.1-3.5。未接受、stale 或 foreign provenance 在 intent/outbox/external submit 前零副作用；no-lookup 的 ambiguous submit 不重提。 |
| 技术架构 | API 不等待 Provider/TOS/FFmpeg；副作用留在 adapter/activity；owner 经 typed handoff 交接 | catalog composition、Provider/Storage ports、Temporal activity、Assets/Export services | ProviderCall/VideoOperation/UploadSession/ExportJob 各自 ledger；禁止统一跨 owner 表或状态机 | 1.2、2.1-2.9、3.2-3.7。Storage 只返回 immutable `StoredObjectRef`；Assets/Export owner 才可 append 最终事实。 |
| 技术实施方案 | catalog 驱动 selection、显式 live opt-in、AES-256-GCM credential、resource/capacity pre-intent admission、TOS multipart | Provider/Profile/Model/CapabilitySnapshot、CredentialResolver、StorageProfile/BucketBinding、RuntimeResourceSnapshot/CapacitySnapshot | profile/model/capability/policy/resource/capacity revision/hash 必须随 operation 冻结；upload 使用 canonical owner operation key | 2.1-2.9、3.6、3.8。probe 与 live invocation 分离；硬限额/能力缺失在 ProviderCall、UploadSession、ExportJob、AssetVersion、Outbox 前拒绝。 |
| 集成记录 | 显式 probe、SDK/账号能力隔离、TOS 私有 bucket、无隐式 Local fallback、证据无密钥 | injected Provider/TOS transport、catalog snapshot、credential reference、masked diagnostic | outbound correlation 派生自冻结 owner identity；remote lookup 只能使用该 correlation/remote id | 1.3、2.3-2.7、3.5-3.6。未知 SDK 字段、账号、区域、credentialed sandbox 均保持 `unconfigured`/`not_ready`，禁止猜测或真实请求。 |

## Versioned contract inventory

| Contract | 当前权威实现 | 版本化/稳定字段 | 验证入口 |
| --- | --- | --- | --- |
| Runtime selection | `runtime.py`、`application/runtime_composition.py` | explicit provider/storage/profile/renderer references；catalog profile/capability/policy revisions | `tests/test_runtime_composition.py` |
| Provider ledger | `application/catalog.py`、`domain/provider_ops.py`、Alembic `0024_provider_outbound_corr` | `run_id + logical_operation`、request fingerprint、outbound correlation、remote request id、lookup outcome | `tests/test_catalog_slice.py`、`tests/test_phase_one_contracts.py` |
| Image command | `application/image_generation.py`、`interfaces/http/image_generation.py` | project scope、target/CAS、continuity snapshot、provider selection、logical operation | `tests/test_phase_one_contracts.py`、`tests/test_runtime_composition.py` |
| Storage/Assets handoff | `application/storage_handoffs.py`、`application/assets.py` | StorageProfile/BucketBinding snapshot、operation key、verified immutable `StoredObjectRef` | `tests/test_storage_provider.py` |
| Export handoff | `application/exports.py`、`application/export_worker.py` | ExportJob, packaging subphase, verified `StoredObjectRef`, append-only ExportArtifact | `tests/test_timeline_export_slice.py` |
| Resource admission | `resilience.py` | RuntimeResourceSnapshot/CapacitySnapshot revision, operation key, correlation id, warning/diagnostic | `tests/test_resilience_observability.py` |

## Credentialed sandbox evidence schema

Runtime-gap sandbox evidence is an additive document with this shape. All `secret`, `authorization`, complete remote response and media byte fields are forbidden.

```json
{
  "schemaVersion": "1.1.0",
  "reportId": "E2E-MVPA-001",
  "result": "ready|not_ready|unconfigured|failed",
  "sandbox": {"profileId": "opaque-id", "allowlistId": "opaque-id"},
  "stages": [{
    "id": "S05",
    "owner": "image candidate owner",
    "prerequisites": ["accepted continuity snapshot"],
    "operation": {"runId": "opaque-id", "logicalOperation": "image.generate:1"},
    "admission": {
      "catalog": {"id": "opaque-id", "revision": 1, "hash": "sha256"},
      "policy": {"id": "opaque-id", "revision": 1, "hash": "sha256"},
      "resource": {"revision": 1, "hash": "sha256"},
      "capacity": {"revision": 1, "hash": "sha256"}
    },
    "observedResult": "pending|succeeded|failed|unknown|submission_unknown",
    "outbound": {"correlation": "sha256", "lookupOutcome": "not_attempted"},
    "failure": {"code": "redacted diagnostic", "noSideEffect": true},
    "restartReconcile": {"restarted": false, "outcome": "not_applicable"},
    "artifacts": [{"kind": "mp4", "sha256": "sha256"}]
  }]
}
```

The report is `ready` only after all configured capabilities and the end-to-end sandbox sequence have passed. `unconfigured` is evidence of a controlled refusal, never an MVP-A exit success.
