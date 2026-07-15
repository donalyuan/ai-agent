# 素材库扩展到作品生产 Design

## Context

素材库当前以 `Material` 统一承载 `video/image/audio/subtitle`，支持上传、归档和 AI 图片入库。作品生产将产生 TTS、字幕、混音以及可复用的中间媒体；这些结果如果只挂在作品任务上，将无法统一检索、复用和保留历史。

## Goals / Non-Goals

**Goals:**

- 所有可复用作品生产结果继续以 `Material` 为统一资产。
- 生成来源和版本快照可追溯，重新生成不覆盖旧素材。
- 已有声音素材可被作品时间轴引用，归档规则一致。

**Non-Goals:**

- 不负责声音、字幕或视频的实际生成。
- 不把最终作品、作品版本或任务审计塞入素材生命周期。
- 不做未选定接口的 AI 音乐和音效生成。

## DDD

`Material` 继续是素材管理聚合根，负责文件身份、类型、状态、标签、稳定 URL 和素材级 metadata。`Work`、`WorkVersion` 和生成任务属于作品生产上下文；素材库只保存稳定外键/快照，不接管其状态机。

作品生成结果采用新增素材语义：每次 TTS、字幕、混音或局部重生成成功都创建新 `Material`。历史作品版本引用具体素材 ID 和快照，素材归档不会改写历史版本。

音频用途是素材分类，不是新的素材类型。建议标准化为 `tts`、`bgm`、`ambient`、`action_sfx`、`mixed`、`other`；历史音频允许为空并显示“未分类”。

## BDD

操作者在素材库中能够按音频用途或来源作品筛选 TTS、BGM、环境音、动作音效和混音结果。打开生成素材详情时可以看到来源作品版本、模型、音色、时长和任务追踪信息，但看不到密钥。

作品生产重新生成 TTS 或字幕时，素材库出现新资产，旧资产继续存在。归档声音素材后，它不再用于新作品选择，但引用它的历史作品仍可回放和审计。

## SDD

生成素材 metadata 至少包含：

- `source=work_generation`
- `work_id`、`work_version_id`、`generation_run_id`、`generation_step_id`
- `artifact_role` 或 `audio_usage`
- 模型 ID、上游模型和能力版本快照
- TTS 音色、语言、风格和参数快照
- 字幕对齐来源 `tts_timestamp | asr`、语言、格式和时间轴版本
- 请求追踪 ID、时长、字符数等非金额资源用量

密钥、token、完整鉴权 headers 不得进入 metadata 或日志。生成服务先将文件写入自管存储并校验成功，再原子登记素材；落盘失败不得创建伪成功素材。

筛选 API 需要支持 `material_type`、`audio_usage`、`source`、`work_id`、`work_version_id` 和现有状态/关键词/标签组合。历史未分类音频必须保持兼容。

作品生产上下文的数据表由后续独立 change 创建，本 change 不提前创建空壳 `works`、`work_versions` 或任务表，也不建立指向尚不存在表的外键。作品来源 ID 以 UUID 字符串快照保存在 metadata，并建立表达式索引；后续作品表落地后再由对应 change 增加引用完整性约束。

统一登记接口采用 `POST /api/projects/:project_id/materials/generated` multipart 请求，在一个用例内完成项目校验、文件内容检查、媒体探测、写入自管存储和素材登记。请求中的 `generation` JSON 使用以下稳定字段：

- `work_id`、`work_version_id`、`generation_run_id`、`generation_step_id`
- `artifact_role`，以及音频产物可选的 `audio_usage=tts|bgm|ambient|action_sfx|mixed|other`
- `model_snapshot`、`voice_snapshot`、`prompt_snapshot`、`timeline_snapshot` 和 `resource_usage`
- `request_trace_id`；字幕产物必须提供 `alignment_source=tts_timestamp|asr` 与 `source_audio_material_id`

登记接口只接受 `audio`、`subtitle` 和 `video` 作品产物；客户端不得指定 `source`，服务端固定写入 `source=work_generation`、`storage_provider=local` 和经探测得到的文件事实。重新登记始终生成新的物理文件和素材 ID。文件校验或落盘失败不写数据库；数据库写入失败时删除本次文件。

普通音频上传允许附带可选 `audio_usage`，用于登记已有 BGM、环境音、动作音效或其他声音；非音频上传不得携带该字段。作品生产生成的音频必须指定用途。`resource_usage` 只允许保存时长、字符数、任务数等非金额用量，拒绝 amount、cost、price、fee 和 currency 字段。

素材详情继续返回素材 metadata，但在响应边界执行递归脱敏；数据库同时通过递归 JSONB 敏感键约束拒绝 `api_key`、鉴权 header、token、secret、password、cookie 和 credentials 等键，防止其他写入路径绕过应用层。

## TDD

- Repository 测试覆盖新素材登记、重新生成不覆盖、作品来源筛选和归档行为。
- API 测试覆盖组合筛选、只读生成快照、密钥脱敏和历史未分类音频。
- 前端测试覆盖声音用途筛选、生成来源详情和归档素材不可新选。
- 集成测试使用本地媒体 fixture，验证文件落盘失败不入库、历史版本引用不因归档失效。

## Decisions

### 继续使用 Material 聚合

复用既有素材类型和生命周期，避免为 TTS、字幕和混音建立三套孤立资产表。作品任务细节仍留在任务上下文，素材 metadata 只保留稳定来源和必要快照。

### 音频用途作为分类

BGM、环境音、动作音效和 TTS 都是 `audio`，用用途字段区分比扩展素材类型更稳定，也便于后续新增声音分类。

## Risks / Trade-offs

- [metadata 过度膨胀] -> 只保存可审计快照和稳定引用，完整步骤日志留在任务上下文。
- [归档素材影响历史播放] -> 归档只控制新选择，不删除文件或历史引用。
- [作品产物与素材重复] -> 作品版本保存 artifact 引用，素材库保存可复用资产身份，职责分离。

## Migration Plan

1. 为现有音频兼容空用途，不强制回填猜测值。
2. 增加作品来源和音频用途筛选后，再接入后续生成模块的产物登记。
3. 回滚时停止写入新增 metadata 字段，保留已经生成的素材和文件。

## Open Questions

无阻塞问题。AI 音乐和音效生成继续由未来独立 change 定义。
