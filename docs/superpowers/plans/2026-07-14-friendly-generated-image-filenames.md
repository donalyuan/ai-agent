# AI 生成图片友好文件名 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让新生成图片使用“脚本标题 + 镜头序号 + 候选序号 + 实际扩展名”的真实文件名，并保持物理文件、素材记录和候选审计一致。

**Architecture:** 新建纯函数模块负责 NFC、非法字符清理和 UTF-8 字节安全命名；现有 Worker 编排显式传播任务标题快照和 1-based 候选槽位，最终统一在落盘前按 magic bytes 选择扩展名。Provider 继续只负责上游请求与内容解析，UUID 任务目录继续承担跨任务隔离。

**Tech Stack:** Python 3.12、dataclasses、pathlib、unicodedata、pytest、PostgreSQL/psycopg、Rust Axum/tower-http、OpenSpec。

**Repository constraint:** 未获得 `git add`、`git commit` 或 `git push` 授权，本计划不执行提交步骤；所有运行测试均在 Docker Compose 容器内，真实图片 Worker 在修改挂载代码前先安全停止。

---

### Task 1: 图片文件名纯函数

**Files:**

- Create: `services/video-worker/src/video_worker/generated_image_filename.py`
- Create: `services/video-worker/tests/test_generated_image_filename.py`

- [x] **Step 1: 写中文、清理、回退、格式和字节边界失败测试**

```python
import unicodedata

import pytest

from video_worker.generated_image_filename import generated_image_filename


def test_generated_image_filename_keeps_chinese_and_formats_positions():
    assert generated_image_filename(
        "别硬扛，用Debug解决烦心事", 1, 1, ".jpg"
    ) == "别硬扛，用Debug解决烦心事-镜头01-第01张.jpg"


def test_generated_image_filename_normalizes_and_removes_invalid_characters():
    result = generated_image_filename(
        '  Cafe\u0301/A\\B<C>D:E"F|G?H*I\x00.  ', 2, 3, ".png"
    )
    assert result == "CaféABCDEFGHI-镜头02-第03张.png"
    assert unicodedata.is_normalized("NFC", result)


@pytest.mark.parametrize("title", ["", "   ", "...", "<>:\"/\\|?*\x00"])
def test_generated_image_filename_falls_back_for_empty_cleaned_title(title: str):
    assert generated_image_filename(title, 2, 3, ".png") == (
        "未命名脚本-镜头02-第03张.png"
    )


def test_generated_image_filename_truncates_at_utf8_codepoint_boundary():
    result = generated_image_filename("测" * 200, 20, 4, ".webp")
    assert len(result.encode("utf-8")) <= 255
    assert result.endswith("-镜头20-第04张.webp")
    result.encode("utf-8").decode("utf-8")


@pytest.mark.parametrize("extension", [".gif", "png", "../x.png"])
def test_generated_image_filename_rejects_unsupported_extension(extension: str):
    with pytest.raises(ValueError):
        generated_image_filename("脚本", 1, 1, extension)
```

- [x] **Step 2: 运行纯函数测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker \
  sh -lc 'cd /app && pytest tests/test_generated_image_filename.py -q'
```

Expected: collection fails with `ModuleNotFoundError: video_worker.generated_image_filename`.

- [x] **Step 3: 实现最小命名模块**

```python
from __future__ import annotations

import unicodedata


DEFAULT_SCRIPT_TITLE = "未命名脚本"
MAX_FILENAME_BYTES = 255
SUPPORTED_EXTENSIONS = frozenset({".png", ".jpg", ".webp"})
WINDOWS_INVALID_FILENAME_CHARS = frozenset('<>:"/\\|?*')


def generated_image_filename(
    script_title: str,
    scene_sequence: int,
    candidate_index: int,
    extension: str,
) -> str:
    if scene_sequence < 1 or candidate_index < 1:
        raise ValueError("scene sequence and candidate index must be positive")
    if extension not in SUPPORTED_EXTENSIONS:
        raise ValueError(f"unsupported generated image extension: {extension}")

    suffix = f"-镜头{scene_sequence:02d}-第{candidate_index:02d}张{extension}"
    title = _clean_script_title(script_title)
    title_budget = MAX_FILENAME_BYTES - len(suffix.encode("utf-8"))
    if title_budget <= 0:
        raise ValueError("generated image suffix exceeds filename byte limit")
    title = _truncate_utf8(title, title_budget).rstrip(". ") or DEFAULT_SCRIPT_TITLE
    return f"{title}{suffix}"


def _clean_script_title(value: str) -> str:
    normalized = unicodedata.normalize("NFC", value)
    cleaned = "".join(
        character
        for character in normalized
        if character not in WINDOWS_INVALID_FILENAME_CHARS
        and unicodedata.category(character) != "Cc"
    )
    return cleaned.strip().rstrip(". ") or DEFAULT_SCRIPT_TITLE


def _truncate_utf8(value: str, max_bytes: int) -> str:
    encoded = value.encode("utf-8")
    if len(encoded) <= max_bytes:
        return value
    return encoded[:max_bytes].decode("utf-8", errors="ignore")
```

- [x] **Step 4: 运行纯函数测试并确认 GREEN**

Run the Task 1 Step 2 command. Expected: all tests pass.

### Task 2: 候选槽位与真实扩展名编排

**Files:**

- Modify: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `services/video-worker/tests/test_asset_generation.py`

- [x] **Step 1: 补充有效图片字节和候选槽位失败测试**

在测试文件定义：

```python
PNG_BYTES = b"\x89PNG\r\n\x1a\nimage"
JPEG_BYTES = b"\xff\xd8\xff\xe0image"
WEBP_BYTES = b"RIFF\x0c\x00\x00\x00WEBPimage"
```

新增/修改断言，使单任务结果要求：

```python
assert result.materials[0].file_name == "测试脚本-镜头01-第01张.png"
assert result.materials[0].file_url == (
    "/assets/generated/images/task-1/测试脚本-镜头01-第01张.png"
)
assert result.materials[0].candidate_index == 1
```

新增 batch 中间落盘失败用例，provider 返回候选 1、2、3，其中候选 2 `content=None`；断言成功材料候选序号为 `[1, 3]`，文件名包含 `第01张`、`第03张`，失败结果槽位为 `[2]`。

新增 `per_candidate` 中间临时错误耗尽后候选 3 成功用例；断言材料槽位为 `[1, 3]`，候选 3 文件名不重排，provider 调用与现有重试上限保持一致。

- [x] **Step 2: 运行聚焦测试并确认 RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker \
  sh -lc 'cd /app && pytest tests/test_asset_generation.py -q \
  -k "local_storage or partial_success or per_candidate"'
```

Expected: 文件名仍为 provider 名称、`GeneratedMaterial` 没有 `candidate_index` 或候选 3 被重排。

- [x] **Step 3: 增加结构化候选结果**

在 `asset_generation.py` 中引入 `generated_image_filename`，并调整数据结构：

```python
@dataclass(frozen=True)
class AssetGenerationTask:
    # existing fields...
    script_title_snapshot: str = "未命名脚本"
    scene_sequence: int = 1
    candidate_index: int | None = None


@dataclass(frozen=True)
class GeneratedImage:
    filename: str
    content: bytes | None
    candidate_index: int = 1


@dataclass(frozen=True)
class GeneratedMaterial:
    file_url: str
    file_name: str
    metadata: dict[str, object]
    candidate_index: int


@dataclass(frozen=True)
class FailedImageCandidate:
    candidate_index: int
    error_message: str


@dataclass(frozen=True)
class ImageTaskResult:
    # existing fields...
    failures: list[FailedImageCandidate] = field(default_factory=list)
```

OpenAI batch 解析按原响应数组写入 `candidate_index=index`；Fake batch provider 同样用 `replace(image, candidate_index=index)` 模拟槽位。`per_candidate` 的 `single_task.candidate_index` 使用外层 1-based 索引。

- [x] **Step 4: 在落盘前形成最终文件名和失败槽位**

`process_batch_image_task()` 对每个请求槽位：

```python
candidate_index = task.candidate_index or image.candidate_index
if image.content is None:
    raise ValueError("generated image content is empty")
_, extension = detect_image_type(image.content)
filename = generated_image_filename(
    task.script_title_snapshot,
    task.scene_sequence,
    candidate_index,
    extension,
)
storage_task_id = task.generation_task_id or task.task_id
file_url = storage.save_image(
    storage_task_id,
    replace(image, filename=filename, candidate_index=candidate_index),
)
```

结果为每个缺失、解析失败、落盘失败槽位创建 `FailedImageCandidate`；`per_candidate` 永久错误为剩余槽位补失败结果。`LocalAssetStorage` 不再调用 ASCII-only `safe_filename()`，但必须拒绝空 basename、路径分隔和超过 255 UTF-8 字节的输入。

- [x] **Step 5: 运行聚焦测试并确认 GREEN**

Run the Task 2 Step 2 command. Expected: all selected tests pass and现有 provider 调用次数断言不变。

### Task 3: 标题快照、素材与失败候选审计

**Files:**

- Modify: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `services/video-worker/tests/test_asset_generation.py`

- [x] **Step 1: 写编排和仓储契约失败测试**

更新 `pending_task()` 使用 `script_title_snapshot="别硬扛，用Debug解决烦心事"`，更新内存仓储失败方法接收 `candidate_index` 并保存。断言：

```python
material = store.created_materials[0]
assert material["file_name"] == "别硬扛，用Debug解决烦心事-镜头01-第01张.png"
assert material["file_url"] == (
    "/assets/generated/images/task-1/"
    "别硬扛，用Debug解决烦心事-镜头01-第01张.png"
)
assert material["metadata"]["script_title_snapshot"] == "别硬扛，用Debug解决烦心事"
assert material["metadata"]["scene_sequence"] == 1
assert material["metadata"]["candidate_index"] == 1
assert store.failed_candidates[0]["candidate_index"] == 2
```

增加领取查询契约断言，确认 SQL 通过 `scripts` 获取 `script_title_snapshot` 并构造 `PendingImageGenerationTask`。

- [x] **Step 2: 运行 `run_next` 聚焦测试并确认 RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker \
  sh -lc 'cd /app && pytest tests/test_asset_generation.py -q -k "run_next"'
```

Expected: 缺少标题快照、metadata 字段或失败候选槽位。

- [x] **Step 3: 实现领取时快照与 metadata**

`PendingImageGenerationTask` 增加：

```python
script_title_snapshot: str
```

领取 CTE 连接 `scripts` 并返回标题：

```sql
WITH candidate AS (
    SELECT task.id, script.title AS script_title_snapshot
    FROM asset_generation_tasks task
    JOIN scripts script ON script.id = task.script_id
    WHERE task.task_type = 'image_candidates'
      AND task.status = 'pending'
    ORDER BY task.created_at ASC, task.id ASC
    FOR UPDATE OF task SKIP LOCKED
    LIMIT 1
)
UPDATE asset_generation_tasks task
SET status = 'processing', updated_at = NOW()
FROM candidate
WHERE task.id = candidate.id
RETURNING task.id, task.project_id, task.script_id, task.model_id,
          task.provider, task.candidate_count, task.reference_material_ids,
          task.params, candidate.script_title_snapshot
```

构造场景任务时传入标题快照和镜头序号；成功 metadata 与失败 metadata 都加入：

```python
"script_title_snapshot": task.script_title_snapshot,
"scene_sequence": scene.sequence,
"candidate_index": candidate_index,
```

成功 rank 使用 `1000 + candidate_index`，失败 rank 使用 `9000 + candidate_index`。

- [x] **Step 4: 运行 `run_next` 聚焦测试并确认 GREEN**

Run the Task 3 Step 2 command. Expected: all selected tests pass.

### Task 4: 中文静态素材路径回归

**Files:**

- Modify: `backend/tests/asset_generation_routes.rs`

- [x] **Step 1: 把现有静态素材测试改为中文物理名和百分号编码 URI**

物理文件名使用：

```rust
.join("别硬扛，用Debug解决烦心事-镜头01-第01张.png");
```

请求 URI 使用浏览器等价百分号编码：

```text
/assets/generated/images/task-1/%E5%88%AB%E7%A1%AC%E6%89%9B%EF%BC%8C%E7%94%A8Debug%E8%A7%A3%E5%86%B3%E7%83%A6%E5%BF%83%E4%BA%8B-%E9%95%9C%E5%A4%B401-%E7%AC%AC01%E5%BC%A0.png
```

- [x] **Step 2: 运行 API 聚焦测试确认现有 `ServeDir` 行为**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api \
  sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api \
  --test asset_generation_routes assets_route_serves_generated_files_from_configured_storage_root'
```

Expected: PASS；若失败，先以运行报错为证据更新 OpenSpec 设计，再修正静态路由，不增加前端兼容逻辑。

### Task 5: 全量验证、OpenSpec 同步与安全部署

**Files:**

- Modify: `openspec/changes/friendly-generated-image-filenames/tasks.md`

- [x] **Step 1: 运行 Worker 全量测试**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker \
  sh -lc 'cd /app && pytest tests -q'
```

Expected: 现有 50 项加新增测试全部通过，0 failed；测试只使用 fake transport。

- [x] **Step 2: 运行 API 静态资源聚焦测试和文档校验**

运行 Task 4 Step 2 命令，然后运行：

```bash
openspec validate friendly-generated-image-filenames --strict --no-interactive
git diff --check -- \
  services/video-worker/src/video_worker/generated_image_filename.py \
  services/video-worker/src/video_worker/asset_generation.py \
  services/video-worker/tests/test_generated_image_filename.py \
  services/video-worker/tests/test_asset_generation.py \
  backend/tests/asset_generation_routes.rs \
  openspec/changes/friendly-generated-image-filenames \
  docs/superpowers
```

Expected: OpenSpec valid，`git diff --check` exit 0。

- [x] **Step 3: 同步 OpenSpec tasks 与 apply 状态**

只勾选具有运行证据的任务，再执行：

```bash
openspec instructions apply --change friendly-generated-image-filenames --json
```

Expected: 所有实施与自动化验证完成后 `state=all_done`；自然任务观察项若尚未发生，保持未勾选并如实报告。

- [x] **Step 4: 安全部署 Worker**

先查询 `asset_generation_tasks` 中 `pending/processing + image_candidates` 数量；确认无 `processing` 后重建 Worker：

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-video-worker
curl -fsS http://localhost:18181/health
```

Expected: Worker 健康、自动执行开关仍为已确认的 `true`。不创建额外图片任务；下一条用户自然任务再用于真实文件名观察。
