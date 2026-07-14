# AI 生成图片友好文件名设计

## 结论

新生成图片的实际文件名统一为：

```text
{脚本名称}-镜头{两位序号}-第{两位候选序号}张.{实际扩展名}
```

示例：

```text
别硬扛，用Debug解决烦心事-镜头01-第01张.jpg
```

UUID 生成任务目录继续保留；`materials.file_name` 与物理 basename 一致。只影响新生成图片，不改历史文件，不修改前端，不改变供应商调用和计费规则。

## DDD

- 文件命名属于 Worker 生成编排与存储边界，不属于 OpenAI Images 或 Ark provider adapter。
- 任务领取时读取 `ScriptTitleSnapshot`；本任务后续始终使用该快照，脚本改名不追改历史素材。
- `CandidateIndex` 是单镜头内 1-based 请求槽位，不是成功结果序号。
- 任务 UUID 提供跨任务唯一目录；标题、镜头序号和候选序号提供任务内可读性与唯一性。

## BDD

- 中文标题直接保留；镜头 1、候选 1 格式化为 `镜头01-第01张`。
- 多镜头、多候选分别使用各自序号。
- 候选 2 失败而候选 3 成功时，候选 3 仍命名为 `第03张`。
- batch 与 `per_candidate` 两条路径遵循同一编号规则。
- 脚本标题在任务领取后修改，不影响当前任务和历史文件名；后续任务使用新快照。

## SDD

### 标题与长度

1. 对标题执行 Unicode NFC 规范化。
2. 删除路径分隔符、Windows 非法字符 `< > : \" / \\ | ? *` 和 Unicode 控制字符。
3. 去除首尾空白及结尾点和空格。
4. 清理后为空时回退为 `未命名脚本`。
5. 为 `-镜头NN-第NN张.ext` 预留空间后按 UTF-8 字节截断标题，不切开 Unicode code point。
6. 完整 basename 不超过 255 UTF-8 字节。

### 执行上下文

`PendingImageGenerationTask` 增加 `script_title_snapshot`。任务领取事务通过 `script_id` 查询 `scripts.title`，避免候选执行期间重复查询当前标题。

内部结果显式携带 `candidate_index`。OpenAI batch 使用原始响应槽位；Ark `per_candidate` 使用外层候选循环槽位。最终命名不得解析 provider 文件名，也不得对成功结果重新编号。

### 存储与 metadata

新文件保存到：

```text
/app/storage/assets/generated/images/<generation-task-uuid>/<friendly-basename>
```

`file_url` 使用对应 `/assets/...` 相对路径。扩展名依据实际图片字节规范为 `.png`、`.jpg` 或 `.webp`。

成功素材与对应候选 metadata 增加：

- `script_title_snapshot`
- `scene_sequence`
- `candidate_index`

失败候选也记录这三个字段。现有来源、任务 ID、分镜 ID、参考素材和候选状态字段保持不变。

## TDD

- 命名纯函数：中文、NFC、非法字符、控制字符、首尾点/空格、空标题、UTF-8 255 字节边界。
- 图片格式：PNG、JPEG、WebP。
- 编排：多镜头、多候选、OpenAI batch、Ark `per_candidate`、中间失败不重排、重试不改序号。
- 一致性：物理文件、`file_url`、`materials.file_name`、素材 metadata、候选 metadata。
- 存储访问：百分号编码的中文 `/assets/...` 路径可读取。
- 自动化测试全部使用 fake provider，不发送真实 OpenAI 或 Ark 请求。

## 非目标

- 不批量重命名或补写现有素材。
- 不新增数据库字段或 migration。
- 不修改 API DTO、前端页面或 Pencil 原型。
- 不改变候选数量、供应商请求次数、重试、停止或计费边界。
