## Context

当前 Worker 直接使用 provider 返回的 `GeneratedImage.filename` 落盘：OpenAI Images 生成任务 UUID 派生的 `.png` 名称，Ark 生成任务 UUID 派生且按 magic bytes 选择扩展名的名称。`LocalAssetStorage.save_image()` 再通过仅允许 ASCII 的 `safe_filename()` 清理，导致物理文件名和 `materials.file_name` 都无法表达脚本、镜头和候选位置。

现有数据边界为：`scripts.title VARCHAR(200)`、`scenes.sequence` 为 1-20、单镜头图片候选为 1-4、`materials.file_name VARCHAR(255)`，本地文件系统单个路径段也需要控制在 255 UTF-8 字节内。静态文件由 API 的 `/assets` `ServeDir` 暴露，前端继续消费相对 `file_url`。

## Goals / Non-Goals

**Goals:**

- 新生成图片使用 `{脚本名称}-镜头{两位序号}-第{两位候选序号}张.{实际扩展名}` 作为真实物理文件名。
- 中文标题可读，非法字符、超长标题和空标题得到确定性处理。
- batch 与 `per_candidate` 两条执行路径都保留原始 1-based 候选槽位，部分失败不重排后续成功图片。
- `materials.file_name`、物理文件名、`file_url` 和 metadata 可相互核对。
- 保留 UUID 任务目录，历史文件和历史素材记录保持不变。

**Non-Goals:**

- 不修改前端页面、API DTO、数据库 schema 或供应商请求协议。
- 不批量重命名既有文件，不追随脚本后续改名更新历史素材。
- 不改变候选数量、调用次数、错误分类、重试或计费规则。

## DDD

### 领域概念

- `ScriptTitleSnapshot`：Worker 领取任务并进入 `processing` 时读取的脚本标题原值，只服务于本次任务的命名与审计。
- `CandidateIndex`：单个镜头内从 1 开始的请求槽位，不是成功结果在返回列表中的顺序。
- `GeneratedImageFileName`：由标题快照、镜头序号、候选槽位和图片实际扩展名共同形成的存储值对象。

### 规则归属

- Provider adapter 只负责请求和解析图片内容，不决定最终业务文件名。
- Worker 编排层拥有脚本、镜头和候选上下文，负责形成最终文件名并传给本地存储。
- 本地存储只按已经校验的最终文件名落盘并返回稳定 `file_url`，不得再次把中文替换为 ASCII 连字符。
- 素材仓储把同一个最终 basename 写入 `materials.file_name`，并把命名快照写入素材与候选 metadata。

## BDD

### 中文脚本生成多个候选

脚本《别硬扛，用Debug解决烦心事》的镜头 1 生成两个候选时，新文件分别为：

```text
别硬扛，用Debug解决烦心事-镜头01-第01张.jpg
别硬扛，用Debug解决烦心事-镜头01-第02张.jpg
```

两个文件位于本次生成任务的 UUID 目录中，素材库文件名与物理 basename 完全一致。

### 部分失败

一个镜头请求候选 1、2、3，候选 2 失败而候选 3 成功时，候选 3 文件名仍为 `第03张`，不得压缩为 `第02张`。该规则同时适用于 OpenAI batch 结果、Ark `per_candidate` 调用和落盘失败。

### 标题变更

Worker 领取任务后，即使脚本标题被修改，本任务仍使用领取时标题快照完成命名；已生成文件不重命名。后续新任务使用其各自领取时的新标题快照。

## SDD

### 任务标题快照

`PendingImageGenerationTask` 增加 `script_title_snapshot`。`claim_next_image_task()` 在同一个领取事务中通过 `asset_generation_tasks.script_id -> scripts.id` 读取 `scripts.title`，与任务从 `pending` 切换为 `processing` 一起形成稳定快照。实现不得在每个候选保存前重复查询当前标题。

### 候选槽位贯穿

内部生成结果必须显式携带 1-based `candidate_index`：

- batch provider 以响应数组中的原始槽位标记结果；跳过无效结果时不得改变后续槽位。
- `per_candidate` 编排使用外层循环的 `candidate_index + 1`，不得使用单候选 provider 响应中的局部序号 `1` 作为最终序号。
- 存储失败和 provider 失败也保留对应槽位，成功候选 rank 与 metadata 使用原始槽位。

最终文件名只能从这个结构化槽位读取，不得解析 provider 文件名或按成功结果重新 `enumerate()` 推断。

### 标题清理与长度

新增图片命名专用函数，不放宽通用 `safe_filename()`：

1. 对标题执行 Unicode NFC 规范化。
2. 删除 `/`、`\\`、Windows 非法字符 `< > : \" | ? *` 和 Unicode 控制字符。
3. 去除首尾空白，并去除结尾的点和空格。
4. 清理后为空时使用 `未命名脚本`。
5. 先构造 `-镜头{sequence:02d}-第{candidate_index:02d}张{extension}` 后缀，再按剩余 UTF-8 字节预算截断标题；截断不得切开 Unicode code point。
6. 最终 basename（含扩展名）不得超过 255 UTF-8 字节；截断后再次去除结尾的点和空格，若为空仍使用回退标题。

不使用随机摘要替代标题。目录中的任务 UUID 已提供跨任务隔离，镜头序号和候选序号提供任务内唯一性。

### 扩展名与存储路径

扩展名由图片实际字节类型确定并规范为 `.png`、`.jpg` 或 `.webp`，不得信任上游提供的任意文件名，也不得统一硬编码 `.png`。不支持的图片内容继续按生成失败处理。

新文件路径为：

```text
<storage-root>/generated/images/<generation-task-uuid>/<friendly-basename>
```

对应 `file_url` 为：

```text
/assets/generated/images/<generation-task-uuid>/<friendly-basename>
```

浏览器可以对中文路径执行百分号编码，API 静态资源路由必须能返回对应物理文件；数据库中的 `materials.file_name` 保存未编码 basename，不保存 URL 编码结果。

### Metadata

成功素材的 `materials.metadata` 和对应 `scene_asset_candidates.metadata` 增加：

- `script_title_snapshot`：领取任务时的脚本标题原值。
- `scene_sequence`：1-based 镜头序号。
- `candidate_index`：1-based 候选槽位。

失败候选 metadata 同样记录这三个字段，便于在部分失败时核对槽位。现有 `source`、`generation_task_id`、`source_scene_id`、`reference_material_ids` 和 `candidate_status` 保持不变。

## TDD

先补失败测试，再修改实现：

- 纯函数测试覆盖中文、NFC 等价字符、路径分隔符、Windows 非法字符、控制字符、首尾点/空格、空标题回退和 255 UTF-8 字节边界。
- 命名测试覆盖多镜头、多候选和 PNG/JPEG/WebP 实际扩展名。
- batch 测试覆盖中间候选无效或落盘失败后，后续成功候选不重排。
- `per_candidate` 测试覆盖临时错误重试、永久错误停止和部分成功时槽位不重排。
- Worker 编排测试断言物理 basename、`file_url`、`materials.file_name`、素材 metadata 和候选 metadata 一致。
- PostgreSQL 任务领取测试断言标题在领取事务中成为快照。
- API 静态资源测试使用百分号编码的中文文件名请求并断言返回文件内容。
- Worker 全量自动化只使用 fake provider，不触发 OpenAI 或 Ark 真实请求。

## Risks / Trade-offs

- [中文 URL 在不同客户端中编码形式不同] -> 数据库存相对 IRI，增加 API 百分号编码路径回归测试，物理 basename 与 `materials.file_name` 始终保存 Unicode 原值。
- [标题过长导致文件系统 `ENAMETOOLONG`] -> 以完整 basename 的 UTF-8 字节数而非字符数限制为 255。
- [部分失败导致候选序号漂移] -> 在内部结果模型中显式保存槽位，禁止从成功列表位置反推。
- [任务领取后标题改变] -> 以领取事务中的标题为快照，metadata 保留原值；不追改历史文件。
- [同名脚本或重复生成产生同名文件] -> 每次生成任务使用独立 UUID 目录，不依赖文件名承担全局唯一性。

## Migration Plan

1. 在测试环境以 fake provider 验证新任务落盘、数据库写入和中文静态访问。
2. 部署 Worker；API 仅需现有静态路由通过回归验证，不需要数据库 migration。
3. 新规则只作用于部署后领取并生成的新图片，已有文件和记录不扫描、不改写。
4. 回退 Worker 时保留已生成的友好文件名和记录；旧版本按已保存 `file_url` 仍可读取，不需要反向迁移。

## Open Questions

无。文件格式、快照时点、清理规则、候选编号、历史兼容和非目标均已确认。
