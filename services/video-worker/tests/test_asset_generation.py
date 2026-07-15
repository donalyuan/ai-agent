import base64
import json
import logging
import subprocess
import sys
from io import BytesIO
from pathlib import Path
from urllib.error import HTTPError, URLError

import pytest

from video_worker import asset_generation
from video_worker.asset_generation import (
    AssetGenerationTask,
    FakeImageProvider,
    GeneratedImage,
    LocalAssetStorage,
    OpenAIImageProvider,
    PendingImageGenerationTask,
    PermanentProviderError,
    PostgresAssetGenerationStore,
    ProviderConfigError,
    SceneGenerationContext,
    TemporaryProviderError,
    default_json_post,
    image_provider_from_model,
    process_image_task,
    run_next_image_task,
)
from video_worker.model_registry import (
    ImageModelRuntimeConfig,
    ModelRegistryError,
)


PNG_BYTES = b"\x89PNG\r\n\x1a\nimage"
JPEG_BYTES = b"\xff\xd8\xff\xe0image"
WEBP_BYTES = b"RIFF\x0c\x00\x00\x00WEBPimage"


def image_task(candidate_count: int = 1) -> AssetGenerationTask:
    return AssetGenerationTask(
        task_id="task-1",
        provider="gpt-image-2",
        prompt="生成分镜候选图",
        candidate_count=candidate_count,
        reference_material_urls=[],
        reference_material_ids=[],
        generation_task_id="task-1",
        scene_id="scene-1",
        script_title_snapshot="测试脚本",
        scene_sequence=1,
    )


class MemoryAssetGenerationStore:
    def __init__(self, task: PendingImageGenerationTask | None):
        self.task = task
        self.created_materials: list[dict[str, object]] = []
        self.failed_candidates: list[dict[str, object]] = []
        self.completed: dict[str, object] | None = None
        self.model_snapshot: dict[str, object] | None = None

    def claim_next_image_task(self) -> PendingImageGenerationTask | None:
        task = self.task
        self.task = None
        return task

    def record_model_snapshot(
        self,
        task_id: str,
        model_snapshot: dict[str, object],
    ) -> None:
        self.model_snapshot = model_snapshot

    def create_generated_image_candidate(
        self,
        task: PendingImageGenerationTask,
        scene: SceneGenerationContext,
        material,
        rank: int,
    ) -> None:
        self.created_materials.append(
            {
                "task_id": task.task_id,
                "scene_id": scene.scene_id,
                "file_url": material.file_url,
                "file_name": material.file_name,
                "metadata": material.metadata,
                "rank": rank,
            }
        )

    def create_failed_image_candidate(
        self,
        task: PendingImageGenerationTask,
        scene: SceneGenerationContext,
        candidate_index: int,
        rank: int,
        error_message: str,
    ) -> None:
        self.failed_candidates.append(
            {
                "task_id": task.task_id,
                "scene_id": scene.scene_id,
                "candidate_index": candidate_index,
                "rank": rank,
                "error_message": error_message,
            }
        )

    def complete_image_task(
        self,
        task_id: str,
        status: str,
        result: dict[str, object],
        error_message: str | None,
        retry_count: int,
    ) -> None:
        self.completed = {
            "task_id": task_id,
            "status": status,
            "result": result,
            "error_message": error_message,
            "retry_count": retry_count,
        }


def pending_task() -> PendingImageGenerationTask:
    return PendingImageGenerationTask(
        task_id="task-1",
        project_id="project-1",
        script_id="script-1",
        model_id="model-1",
        provider="gpt-image-2",
        image_candidates_per_scene=2,
        reference_material_ids=["material-1"],
        reference_material_urls=["/assets/reference.png"],
        script_title_snapshot="别硬扛，用Debug解决烦心事",
        scenes=[
            SceneGenerationContext(
                scene_id="scene-1",
                sequence=1,
                narration="旁白",
                visual_description="人物站在窗边讲解 AI 图片生成",
                emotion="平静",
                duration_sec=8,
            )
        ],
    )


def two_scene_pending_task() -> PendingImageGenerationTask:
    task = pending_task()
    return PendingImageGenerationTask(
        task_id=task.task_id,
        project_id=task.project_id,
        script_id=task.script_id,
        model_id=task.model_id,
        provider=task.provider,
        image_candidates_per_scene=task.image_candidates_per_scene,
        reference_material_ids=task.reference_material_ids,
        reference_material_urls=task.reference_material_urls,
        script_title_snapshot=task.script_title_snapshot,
        scenes=[
            *task.scenes,
            SceneGenerationContext(
                scene_id="scene-2",
                sequence=2,
                narration="第二段旁白",
                visual_description="人物转身展示生成结果",
                emotion="平静",
                duration_sec=6,
            ),
        ],
    )


def image_model_config(api_protocol: str = "openai_images") -> ImageModelRuntimeConfig:
    is_ark = api_protocol == "volcengine_ark_images"
    return ImageModelRuntimeConfig(
        model_id="model-1",
        display_name="测试图片模型",
        provider_name="test",
        api_protocol=api_protocol,
        protocol_version="v1",
        auth_scheme="bearer",
        request_base_url=(
            "https://ark.cn-beijing.volces.com/api/v3"
            if is_ark
            else "https://images.example/v1"
        ),
        upstream_model=("doubao-seedream-test" if is_ark else "test-image"),
        api_key="test-key",
        api_secret=None,
        timeout_seconds=45,
        settings=(
            {
                "supported_sizes": [],
                "default_size": None,
                "max_images_per_request": 1,
            }
            if is_ark
            else {
                "supported_sizes": ["1024x1024"],
                "default_size": "1024x1024",
                "max_images_per_request": 4,
            }
        ),
    )


class FakeModelRegistry:
    def __init__(self, result: ImageModelRuntimeConfig | Exception):
        self.result = result
        self.call_count = 0

    def resolve_enabled(self, model_id: str, expected_type: str):
        self.call_count += 1
        assert model_id == "model-1"
        assert expected_type == "image"
        if isinstance(self.result, Exception):
            raise self.result
        return self.result


class ScriptedPerCandidateProvider:
    request_mode = "per_candidate"

    def __init__(self, responses: list[GeneratedImage | Exception]):
        self.responses = list(responses)
        self.tasks: list[AssetGenerationTask] = []

    def generate_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        self.tasks.append(task)
        response = self.responses.pop(0)
        if isinstance(response, Exception):
            raise response
        return [response]


class FakeQueryResult:
    def __init__(self, *, row=None, rows=None):
        self.row = row
        self.rows = rows or []

    def fetchone(self):
        return self.row

    def fetchall(self):
        return self.rows


class FakeTransaction:
    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False


class FakePsycopgConnection:
    def __init__(self):
        self.queries: list[str] = []
        self.executions: list[tuple[str, object]] = []

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def transaction(self):
        return FakeTransaction()

    def execute(self, query, params=None):
        sql = str(query)
        self.queries.append(sql)
        self.executions.append((sql, params))
        if "UPDATE asset_generation_tasks task" in sql:
            return FakeQueryResult(
                row={
                    "id": "task-1",
                    "project_id": "project-1",
                    "script_id": "script-1",
                    "model_id": "model-1",
                    "provider": "gpt-image-2",
                    "candidate_count": 2,
                    "reference_material_ids": [],
                    "params": {
                        "scene_ids": ["scene-1"],
                        "image_candidates_per_scene": 2,
                    },
                    "script_title_snapshot": "领取时脚本标题",
                }
            )
        if "FROM scenes" in sql:
            return FakeQueryResult(
                rows=[
                    {
                        "id": "scene-1",
                        "sequence": 1,
                        "narration": "旁白",
                        "visual_description": "画面",
                        "emotion": "平静",
                        "duration_sec": 8,
                    }
                ]
            )
        return FakeQueryResult()


def test_worker_writes_generated_image_to_local_storage(tmp_path: Path):
    provider = FakeImageProvider(
        [GeneratedImage(filename="provider-name.bin", content=PNG_BYTES)]
    )
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(), provider, storage)

    assert result.status == "completed"
    assert result.retry_count == 0
    assert result.materials[0].file_url == (
        "/assets/generated/images/task-1/测试脚本-镜头01-第01张.png"
    )
    assert result.materials[0].file_name == "测试脚本-镜头01-第01张.png"
    assert result.materials[0].candidate_index == 1
    assert result.materials[0].metadata["storage_provider"] == "local"
    assert result.materials[0].metadata["source"] == "ai_generated"
    assert result.materials[0].metadata["generation_task_id"] == "task-1"
    assert (
        tmp_path
        / "generated"
        / "images"
        / "task-1"
        / "测试脚本-镜头01-第01张.png"
    ).read_bytes() == PNG_BYTES


@pytest.mark.parametrize(
    "filename",
    ["../provider.png", "..", ".", "nested\\provider.png"],
)
def test_local_storage_rejects_provider_path_as_final_filename(
    tmp_path: Path,
    filename: str,
):
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    with pytest.raises(ValueError, match="safe basename"):
        storage.save_image(
            "task-1",
            GeneratedImage(filename=filename, content=PNG_BYTES),
        )


def test_postgres_store_claims_script_title_in_same_transaction(monkeypatch):
    connection = FakePsycopgConnection()
    monkeypatch.setattr("psycopg.connect", lambda *_args, **_kwargs: connection)

    task = PostgresAssetGenerationStore("postgres://test").claim_next_image_task()

    assert task is not None
    assert task.script_title_snapshot == "领取时脚本标题"
    assert "JOIN scripts" in connection.queries[0]
    assert "task.task_type = 'image_candidates'" in connection.queries[0]
    assert "video_draft" not in connection.queries[0]
    assert "video_generation" not in connection.queries[0]


def test_postgres_store_records_failed_candidate_naming_metadata(monkeypatch):
    connection = FakePsycopgConnection()
    monkeypatch.setattr("psycopg.connect", lambda *_args, **_kwargs: connection)
    task = pending_task()

    PostgresAssetGenerationStore("postgres://test").create_failed_image_candidate(
        task,
        task.scenes[0],
        candidate_index=2,
        rank=9002,
        error_message="upstream failed",
    )

    metadata = connection.executions[0][1][-1].obj
    assert metadata["script_title_snapshot"] == "别硬扛，用Debug解决烦心事"
    assert metadata["scene_sequence"] == 1
    assert metadata["candidate_index"] == 2


def test_worker_retries_temporary_error_once(tmp_path: Path):
    provider = FakeImageProvider(
        [
            TemporaryProviderError("timeout"),
            GeneratedImage(filename="provider-name.bin", content=PNG_BYTES),
        ]
    )
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(), provider, storage)

    assert result.status == "completed"
    assert result.retry_count == 1
    assert provider.call_count == 2
    assert result.materials[0].file_url == (
        "/assets/generated/images/task-1/测试脚本-镜头01-第01张.png"
    )


def test_worker_does_not_create_material_when_download_fails(tmp_path: Path):
    provider = FakeImageProvider([GeneratedImage(filename="bad.png", content=None)])
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(), provider, storage)

    assert result.status == "failed"
    assert result.materials == []
    assert result.error_message is not None
    assert not (tmp_path / "generated" / "images" / "task-1" / "bad.png").exists()


def test_worker_keeps_partial_success_when_one_image_fails(tmp_path: Path):
    provider = FakeImageProvider(
        [
            GeneratedImage(filename="one.bin", content=PNG_BYTES),
            GeneratedImage(filename="bad.png", content=None),
            GeneratedImage(filename="three.bin", content=WEBP_BYTES),
        ]
    )
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(candidate_count=3), provider, storage)

    assert result.status == "completed"
    assert [material.candidate_index for material in result.materials] == [1, 3]
    assert [material.file_name for material in result.materials] == [
        "测试脚本-镜头01-第01张.png",
        "测试脚本-镜头01-第03张.webp",
    ]
    assert result.failed_count == 1
    assert [failure.candidate_index for failure in result.failures] == [2]
    assert result.partial is True


def test_per_candidate_provider_executes_one_independent_call_per_candidate(
    tmp_path: Path,
):
    provider = ScriptedPerCandidateProvider(
        [
            GeneratedImage(filename="one.bin", content=PNG_BYTES),
            GeneratedImage(filename="two.bin", content=JPEG_BYTES),
            GeneratedImage(filename="three.bin", content=WEBP_BYTES),
        ]
    )

    result = process_image_task(
        image_task(candidate_count=3),
        provider,
        LocalAssetStorage(tmp_path),
    )

    assert result.status == "completed"
    assert [material.candidate_index for material in result.materials] == [1, 2, 3]
    assert [material.file_name for material in result.materials] == [
        "测试脚本-镜头01-第01张.png",
        "测试脚本-镜头01-第02张.jpg",
        "测试脚本-镜头01-第03张.webp",
    ]
    assert [task.candidate_count for task in provider.tasks] == [1, 1, 1]
    assert [task.task_id for task in provider.tasks] == [
        "task-1-1",
        "task-1-2",
        "task-1-3",
    ]


def test_per_candidate_provider_retries_only_current_candidate_once(tmp_path: Path):
    provider = ScriptedPerCandidateProvider(
        [
            GeneratedImage(filename="one.bin", content=PNG_BYTES),
            TemporaryProviderError("retry candidate two"),
            GeneratedImage(filename="two.bin", content=JPEG_BYTES),
            GeneratedImage(filename="three.bin", content=WEBP_BYTES),
        ]
    )

    result = process_image_task(
        image_task(candidate_count=3),
        provider,
        LocalAssetStorage(tmp_path),
    )

    assert len(provider.tasks) == 4
    assert [task.task_id for task in provider.tasks] == [
        "task-1-1",
        "task-1-2",
        "task-1-2",
        "task-1-3",
    ]
    assert len(result.materials) == 3
    assert result.retry_count == 1
    assert result.failed_count == 0


def test_per_candidate_provider_continues_after_temporary_retry_is_exhausted(
    tmp_path: Path,
):
    provider = ScriptedPerCandidateProvider(
        [
            GeneratedImage(filename="one.bin", content=PNG_BYTES),
            TemporaryProviderError("candidate two temporary failure"),
            TemporaryProviderError("candidate two still unavailable"),
            GeneratedImage(filename="three.bin", content=WEBP_BYTES),
        ]
    )

    result = process_image_task(
        image_task(candidate_count=3),
        provider,
        LocalAssetStorage(tmp_path),
    )

    assert [task.task_id for task in provider.tasks] == [
        "task-1-1",
        "task-1-2",
        "task-1-2",
        "task-1-3",
    ]
    assert [material.candidate_index for material in result.materials] == [1, 3]
    assert result.materials[1].file_name == "测试脚本-镜头01-第03张.webp"
    assert result.retry_count == 1
    assert result.failed_count == 1
    assert [failure.candidate_index for failure in result.failures] == [2]
    assert result.partial is True
    assert result.fatal is False


def test_per_candidate_provider_stops_remaining_calls_after_permanent_error(
    tmp_path: Path,
):
    provider = ScriptedPerCandidateProvider(
        [
            GeneratedImage(filename="one.bin", content=PNG_BYTES),
            PermanentProviderError("invalid Ark request"),
            GeneratedImage(filename="must-not-run.bin", content=WEBP_BYTES),
        ]
    )

    result = process_image_task(
        image_task(candidate_count=3),
        provider,
        LocalAssetStorage(tmp_path),
    )

    assert [task.task_id for task in provider.tasks] == ["task-1-1", "task-1-2"]
    assert len(result.materials) == 1
    assert result.failed_count == 2
    assert [failure.candidate_index for failure in result.failures] == [2, 3]
    assert result.partial is True
    assert result.fatal is True


def test_run_next_image_task_claims_pending_task_and_writes_material_candidates(tmp_path: Path):
    provider = FakeImageProvider(
        [GeneratedImage(filename="provider-name.bin", content=PNG_BYTES)]
    )
    store = MemoryAssetGenerationStore(pending_task())

    processed = run_next_image_task(
        store,
        model_registry=FakeModelRegistry(image_model_config()),
        provider_factory=lambda config: provider,
        storage=LocalAssetStorage(tmp_path, public_prefix="/assets"),
    )

    assert processed is True
    assert store.created_materials[0]["scene_id"] == "scene-1"
    assert store.created_materials[0]["file_url"] == (
        "/assets/generated/images/task-1/"
        "别硬扛，用Debug解决烦心事-镜头01-第01张.png"
    )
    assert store.created_materials[0]["file_name"] == (
        "别硬扛，用Debug解决烦心事-镜头01-第01张.png"
    )
    assert store.created_materials[0]["metadata"]["source"] == "ai_generated"  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["generation_task_id"] == "task-1"  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["source_scene_id"] == "scene-1"  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["reference_material_ids"] == ["material-1"]  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["script_title_snapshot"] == (  # type: ignore[index]
        "别硬扛，用Debug解决烦心事"
    )
    assert store.created_materials[0]["metadata"]["scene_sequence"] == 1  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["candidate_index"] == 1  # type: ignore[index]
    assert store.created_materials[0]["rank"] == 1001
    assert [candidate["candidate_index"] for candidate in store.failed_candidates] == [2]
    assert store.failed_candidates[0]["rank"] == 9002
    assert store.completed is not None
    assert store.completed["status"] == "completed"
    assert store.completed["result"]["generated_count"] == 1  # type: ignore[index]
    assert store.model_snapshot is not None
    assert store.model_snapshot["model_id"] == "model-1"
    assert "api_key" not in store.model_snapshot


def test_run_next_image_task_names_multiple_scenes_and_candidates(tmp_path: Path):
    provider = ScriptedPerCandidateProvider(
        [
            GeneratedImage(filename="ignored-1.bin", content=PNG_BYTES),
            GeneratedImage(filename="ignored-2.bin", content=JPEG_BYTES),
            GeneratedImage(filename="ignored-3.bin", content=PNG_BYTES),
            GeneratedImage(filename="ignored-4.bin", content=WEBP_BYTES),
        ]
    )
    store = MemoryAssetGenerationStore(two_scene_pending_task())

    processed = run_next_image_task(
        store,
        model_registry=FakeModelRegistry(image_model_config()),
        provider_factory=lambda config: provider,
        storage=LocalAssetStorage(tmp_path, public_prefix="/assets"),
    )

    assert processed is True
    assert [material["file_name"] for material in store.created_materials] == [
        "别硬扛，用Debug解决烦心事-镜头01-第01张.png",
        "别硬扛，用Debug解决烦心事-镜头01-第02张.jpg",
        "别硬扛，用Debug解决烦心事-镜头02-第01张.png",
        "别硬扛，用Debug解决烦心事-镜头02-第02张.webp",
    ]
    assert store.failed_candidates == []


def test_run_next_image_task_records_failed_candidates_without_materials(tmp_path: Path):
    provider = FakeImageProvider([GeneratedImage(filename="bad.png", content=None)])
    store = MemoryAssetGenerationStore(pending_task())

    processed = run_next_image_task(
        store,
        model_registry=FakeModelRegistry(image_model_config()),
        provider_factory=lambda config: provider,
        storage=LocalAssetStorage(tmp_path, public_prefix="/assets"),
    )

    assert processed is True
    assert store.created_materials == []
    assert len(store.failed_candidates) == 2
    assert [candidate["candidate_index"] for candidate in store.failed_candidates] == [1, 2]
    assert store.completed is not None
    assert store.completed["status"] == "failed"
    assert store.completed["result"]["failed_count"] == 2  # type: ignore[index]


def test_run_next_image_task_marks_disabled_model_failed_without_provider_call(tmp_path: Path):
    store = MemoryAssetGenerationStore(pending_task())
    provider_factory_calls = 0

    def provider_factory(config):
        nonlocal provider_factory_calls
        provider_factory_calls += 1
        return FakeImageProvider([])

    processed = run_next_image_task(
        store,
        model_registry=FakeModelRegistry(
            ModelRegistryError("model_disabled", "模型已停用或删除")
        ),
        provider_factory=provider_factory,
        storage=LocalAssetStorage(tmp_path, public_prefix="/assets"),
    )

    assert processed is True
    assert len(store.failed_candidates) == 2
    assert [candidate["candidate_index"] for candidate in store.failed_candidates] == [1, 2]
    assert store.completed is not None
    assert store.completed["status"] == "failed"
    assert store.completed["result"]["failed_count"] == 2  # type: ignore[index]
    assert "模型已停用" in str(store.completed["error_message"])
    assert provider_factory_calls == 0
    assert store.model_snapshot is None


def test_run_next_image_task_marks_permanent_provider_error_failed_without_retry(tmp_path: Path):
    provider = FakeImageProvider([RuntimeError("provider rejected request")])
    store = MemoryAssetGenerationStore(pending_task())

    processed = run_next_image_task(
        store,
        model_registry=FakeModelRegistry(image_model_config()),
        provider_factory=lambda config: provider,
        storage=LocalAssetStorage(tmp_path, public_prefix="/assets"),
    )

    assert processed is True
    assert provider.call_count == 1
    assert len(store.failed_candidates) == 2
    assert [candidate["candidate_index"] for candidate in store.failed_candidates] == [1, 2]
    assert store.completed is not None
    assert store.completed["status"] == "failed"
    assert "provider rejected request" in str(store.completed["error_message"])


def test_run_next_image_task_stops_after_first_permanent_http_error(
    tmp_path: Path,
    monkeypatch,
):
    call_count = 0

    def forbidden(*args, **kwargs):
        nonlocal call_count
        call_count += 1
        raise HTTPError(
            "https://proxy.example/v1/images/edits",
            403,
            "Forbidden",
            {},
            BytesIO(b'{"error":{"message":"image access denied"}}'),
        )

    monkeypatch.setattr(
        "video_worker.asset_generation.urllib_request.urlopen",
        forbidden,
    )
    provider = OpenAIImageProvider(
        api_key="test-key",
        base_url="https://proxy.example/v1",
        download_url=lambda url: b"reference",
    )
    store = MemoryAssetGenerationStore(two_scene_pending_task())

    processed = run_next_image_task(
        store,
        model_registry=FakeModelRegistry(image_model_config()),
        provider_factory=lambda config: provider,
        storage=LocalAssetStorage(tmp_path, public_prefix="/assets"),
    )

    assert processed is True
    assert call_count == 1
    assert len(store.failed_candidates) == 4
    assert store.completed is not None
    assert store.completed["status"] == "failed"
    assert store.completed["result"]["failed_count"] == 4  # type: ignore[index]
    assert "HTTP 403" in str(store.completed["error_message"])
    assert "image access denied" in str(store.completed["error_message"])


def test_openai_image_provider_builds_generation_request_and_decodes_base64():
    requests: list[dict[str, object]] = []

    def fake_post(url: str, headers: dict[str, str], payload: dict[str, object]) -> dict[str, object]:
        requests.append({"url": url, "headers": headers, "payload": payload})
        return {
            "data": [
                {
                    "b64_json": "cG5n",
                }
            ]
        }

    provider = OpenAIImageProvider(api_key="test-key", http_post=fake_post)

    images = provider.generate_images(image_task(candidate_count=2))

    assert images == [GeneratedImage(filename="task-1-1.png", content=b"png")]
    assert requests[0]["url"] == "https://api.openai.com/v1/images/generations"
    assert requests[0]["headers"]["Authorization"] == "Bearer test-key"  # type: ignore[index]
    assert requests[0]["headers"]["User-Agent"] == "codex-cli/0.142.5"  # type: ignore[index]
    assert requests[0]["payload"]["model"] == "gpt-image-2"  # type: ignore[index]
    assert requests[0]["payload"]["n"] == 2  # type: ignore[index]
    assert "response_format" not in requests[0]["payload"]  # type: ignore[index]
    assert "reference_material_urls" not in requests[0]["payload"]  # type: ignore[index]


def test_openai_image_provider_uses_edit_endpoint_for_reference_images():
    requests: list[dict[str, object]] = []

    def fake_multipart_post(
        url: str,
        headers: dict[str, str],
        fields: dict[str, object],
        files: list[tuple[str, str, bytes, str]],
    ) -> dict[str, object]:
        requests.append(
            {"url": url, "headers": headers, "fields": fields, "files": files}
        )
        return {"data": [{"b64_json": "cG5n"}]}

    provider = OpenAIImageProvider(
        api_key="test-key",
        http_multipart_post=fake_multipart_post,
        download_url=lambda url: b"reference",
    )
    task = image_task(candidate_count=1)
    task.reference_material_urls.append("https://assets.example.com/ref.png")

    images = provider.generate_images(task)

    assert images == [GeneratedImage(filename="task-1-1.png", content=b"png")]
    assert requests[0]["url"] == "https://api.openai.com/v1/images/edits"
    assert requests[0]["headers"]["User-Agent"] == "codex-cli/0.142.5"  # type: ignore[index]
    assert requests[0]["fields"]["model"] == "gpt-image-2"  # type: ignore[index]
    assert requests[0]["fields"]["prompt"] == "生成分镜候选图"  # type: ignore[index]
    assert requests[0]["files"][0][0] == "image[]"  # type: ignore[index]


def test_openai_image_provider_requires_api_key():
    try:
        OpenAIImageProvider(api_key="")
    except ProviderConfigError as error:
        assert "OPENAI_API_KEY" in str(error)
    else:
        raise AssertionError("missing api key should fail fast")


def test_openai_image_provider_is_built_from_database_model_config():
    provider = image_provider_from_model(image_model_config())

    assert isinstance(provider, OpenAIImageProvider)
    assert provider.base_url == "https://images.example/v1"
    assert provider.model == "test-image"


def test_default_binary_get_reads_managed_asset_and_rejects_escape(
    tmp_path: Path,
    monkeypatch,
):
    asset_root = tmp_path / "assets"
    reference_path = asset_root / "existing" / "reference.png"
    reference_path.parent.mkdir(parents=True)
    reference_path.write_bytes(b"managed-reference")
    monkeypatch.setenv("ASSET_STORAGE_ROOT", str(asset_root))
    monkeypatch.setenv("ASSET_PUBLIC_PREFIX", "/assets")

    assert (
        asset_generation.default_binary_get("/assets/existing/reference.png")
        == b"managed-reference"
    )
    with pytest.raises(
        asset_generation.PermanentProviderError,
        match="escapes asset storage root",
    ):
        asset_generation.default_binary_get("/assets/../outside.png")


def test_image_provider_factory_rejects_openai_responses():
    with pytest.raises(
        ProviderConfigError,
        match="unsupported image protocol: openai_responses",
    ):
        image_provider_from_model(image_model_config("openai_responses"))


def test_volcengine_ark_image_provider_is_built_from_database_model_config():
    provider = image_provider_from_model(image_model_config("volcengine_ark_images"))

    assert provider.__class__.__name__ == "VolcengineArkImageProvider"
    assert provider.base_url == "https://ark.cn-beijing.volces.com/api/v3"
    assert provider.model == "doubao-seedream-test"


def test_default_json_post_preserves_permanent_http_error_status_and_summary(monkeypatch):
    def forbidden(*args, **kwargs):
        raise HTTPError(
            "https://proxy.example/v1/images/generations",
            403,
            "Forbidden",
            {},
            BytesIO(b'{"error":{"message":"insufficient permissions"}}'),
        )

    monkeypatch.setattr(
        "video_worker.asset_generation.urllib_request.urlopen",
        forbidden,
    )

    with pytest.raises(Exception) as error:
        default_json_post("https://proxy.example", {}, {"prompt": "test"})

    assert error.type.__name__ == "PermanentProviderError"
    assert "HTTP 403" in str(error.value)
    assert "insufficient permissions" in str(error.value)


@pytest.mark.parametrize("status", [429, 503])
def test_default_json_post_classifies_retryable_http_errors_as_temporary(
    monkeypatch,
    status: int,
):
    def retryable(*args, **kwargs):
        raise HTTPError(
            "https://proxy.example/v1/images/generations",
            status,
            "retry later",
            {},
            BytesIO(b'{"error":{"message":"temporarily unavailable"}}'),
        )

    monkeypatch.setattr(
        "video_worker.asset_generation.urllib_request.urlopen",
        retryable,
    )

    with pytest.raises(TemporaryProviderError, match=f"HTTP {status}"):
        default_json_post("https://proxy.example", {}, {"prompt": "test"})


def test_default_json_post_classifies_network_errors_as_temporary(monkeypatch):
    def unavailable(*args, **kwargs):
        raise URLError("connection reset")

    monkeypatch.setattr(
        "video_worker.asset_generation.urllib_request.urlopen",
        unavailable,
    )

    with pytest.raises(TemporaryProviderError, match="connection reset"):
        default_json_post("https://proxy.example", {}, {"prompt": "test"})


def test_volcengine_ark_provider_builds_single_candidate_request_and_decodes_png(
    caplog,
):
    requests: list[dict[str, object]] = []
    png = b"\x89PNG\r\n\x1a\nimage"
    encoded = base64.b64encode(png).decode("ascii")

    def fake_post(url, headers, payload):
        requests.append({"url": url, "headers": headers, "payload": payload})
        return {"data": [{"b64_json": encoded}]}

    provider = asset_generation.VolcengineArkImageProvider(
        api_key="test-key",
        model="doubao-seedream-test",
        base_url="https://ark.cn-beijing.volces.com/api/v3",
        http_post=fake_post,
    )

    with caplog.at_level(logging.INFO, logger=asset_generation.LOGGER.name):
        images = provider.generate_images(image_task(candidate_count=1))

    assert images == [GeneratedImage(filename="task-1-1.png", content=png)]
    assert requests[0]["url"] == (
        "https://ark.cn-beijing.volces.com/api/v3/images/generations"
    )
    assert requests[0]["headers"] == {  # type: ignore[comparison-overlap]
        "Authorization": "Bearer test-key",
        "Content-Type": "application/json",
    }
    payload = requests[0]["payload"]
    assert payload == {  # type: ignore[comparison-overlap]
        "model": "doubao-seedream-test",
        "prompt": "生成分镜候选图",
        "sequential_image_generation": "disabled",
        "response_format": "b64_json",
        "stream": False,
        "watermark": False,
    }
    assert "n" not in payload  # type: ignore[operator]

    log_text = caplog.text
    assert "ark_image_request" in log_text
    assert "ark_image_response" in log_text
    assert "Bearer ***" in log_text
    assert "test-key" not in log_text
    assert encoded not in log_text
    response_log = next(
        json.loads(record.message)
        for record in caplog.records
        if "ark_image_response" in record.message
    )
    assert response_log["image_bytes"] == len(png)


@pytest.mark.parametrize(
    ("content", "media_type"),
    [
        (b"\x89PNG\r\n\x1a\nreference", "image/png"),
        (b"\xff\xd8\xff\xe0reference", "image/jpeg"),
        (b"RIFF\x0c\x00\x00\x00WEBPreference", "image/webp"),
    ],
)
def test_volcengine_ark_provider_encodes_reference_images_as_data_urls(
    content: bytes,
    media_type: str,
    caplog,
):
    requests: list[dict[str, object]] = []
    png = b"\x89PNG\r\n\x1a\nresult"

    def fake_post(url, headers, payload):
        requests.append(payload)
        return {"data": [{"b64_json": base64.b64encode(png).decode("ascii")}]}

    provider = asset_generation.VolcengineArkImageProvider(
        api_key="test-key",
        model="doubao-seedream-test",
        base_url="https://ark.cn-beijing.volces.com/api/v3",
        http_post=fake_post,
        download_url=lambda _url: content,
    )
    task = image_task(candidate_count=1)
    task.reference_material_urls.extend(["/assets/reference.png"])

    with caplog.at_level(logging.INFO, logger=asset_generation.LOGGER.name):
        provider.generate_images(task)

    expected = base64.b64encode(content).decode("ascii")
    assert requests[0]["image"] == [f"data:{media_type};base64,{expected}"]
    assert expected not in caplog.text
    assert "<redacted:1 reference image(s)>" in caplog.text


@pytest.mark.parametrize(
    ("content", "extension"),
    [
        (b"\x89PNG\r\n\x1a\nresult", ".png"),
        (b"\xff\xd8\xff\xe0result", ".jpg"),
        (b"RIFF\x0c\x00\x00\x00WEBPresult", ".webp"),
    ],
)
def test_volcengine_ark_provider_uses_response_magic_bytes_for_extension(
    content: bytes,
    extension: str,
):
    provider = asset_generation.VolcengineArkImageProvider(
        api_key="test-key",
        http_post=lambda *_args: {
            "data": [{"b64_json": base64.b64encode(content).decode("ascii")}]
        },
    )

    images = provider.generate_images(image_task(candidate_count=1))

    assert images[0].filename.endswith(extension)
    assert images[0].content == content


@pytest.mark.parametrize(
    ("response", "message"),
    [
        ({"data": [{"error": {"code": "InvalidParameter"}}]}, "InvalidParameter"),
        ({"data": [{"b64_json": "%%%"}]}, "base64"),
        (
            {
                "data": [
                    {"b64_json": base64.b64encode(b"not-an-image").decode("ascii")}
                ]
            },
            "image type",
        ),
    ],
)
def test_volcengine_ark_provider_rejects_invalid_response_contract(response, message):
    provider = asset_generation.VolcengineArkImageProvider(
        api_key="test-key",
        http_post=lambda *_args: response,
    )

    with pytest.raises(PermanentProviderError, match=message):
        provider.generate_images(image_task(candidate_count=1))


def test_volcengine_ark_provider_requires_key_and_one_candidate():
    with pytest.raises(ProviderConfigError, match="API key"):
        asset_generation.VolcengineArkImageProvider(api_key="")

    provider = asset_generation.VolcengineArkImageProvider(
        api_key="test-key",
        http_post=lambda *_args: {},
    )
    with pytest.raises(ProviderConfigError, match="one candidate"):
        provider.generate_images(image_task(candidate_count=2))


def test_ark_logger_emits_info_with_uvicorn_runtime_configuration():
    completed = subprocess.run(
        [
            sys.executable,
            "-c",
            (
                "from uvicorn import Config; "
                "from video_worker.asset_generation import LOGGER; "
                "Config('video_worker.main:app').configure_logging(); "
                "LOGGER.info('ark-runtime-log-probe')"
            ),
        ],
        check=True,
        capture_output=True,
        text=True,
    )

    assert "ark-runtime-log-probe" in completed.stderr
