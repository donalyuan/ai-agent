from io import BytesIO
from pathlib import Path
from urllib.error import HTTPError, URLError

import pytest

from video_worker.asset_generation import (
    AssetGenerationTask,
    FakeImageProvider,
    GeneratedImage,
    JimengImageProvider,
    LocalAssetStorage,
    OpenAIImageProvider,
    PendingImageGenerationTask,
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
        rank: int,
        error_message: str,
    ) -> None:
        self.failed_candidates.append(
            {
                "task_id": task.task_id,
                "scene_id": scene.scene_id,
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
    return ImageModelRuntimeConfig(
        model_id="model-1",
        display_name="测试图片模型",
        provider_name="test",
        api_protocol=api_protocol,
        protocol_version="v1",
        auth_scheme="access_key_secret" if api_protocol == "jimeng_visual" else "bearer",
        request_base_url="https://images.example/v1",
        upstream_model="test-image",
        api_key="test-key",
        api_secret="test-secret" if api_protocol == "jimeng_visual" else None,
        timeout_seconds=45,
        settings={
            "default_size": "1328x1328" if api_protocol == "jimeng_visual" else "1024x1024",
            "request_key": "test-request-key",
        },
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


def test_worker_writes_generated_image_to_local_storage(tmp_path: Path):
    provider = FakeImageProvider(
        [GeneratedImage(filename="scene-1.png", content=b"png")]
    )
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(), provider, storage)

    assert result.status == "completed"
    assert result.retry_count == 0
    assert result.materials[0].file_url.startswith("/assets/")
    assert result.materials[0].metadata["storage_provider"] == "local"
    assert result.materials[0].metadata["source"] == "ai_generated"
    assert result.materials[0].metadata["generation_task_id"] == "task-1"
    assert (tmp_path / "generated" / "images" / "task-1" / "scene-1.png").read_bytes() == b"png"


def test_worker_retries_temporary_error_once(tmp_path: Path):
    provider = FakeImageProvider(
        [TemporaryProviderError("timeout"), GeneratedImage(filename="ok.png", content=b"png")]
    )
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(), provider, storage)

    assert result.status == "completed"
    assert result.retry_count == 1
    assert provider.call_count == 2
    assert result.materials[0].file_url == "/assets/generated/images/task-1/ok.png"


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
            GeneratedImage(filename="ok.png", content=b"png"),
            GeneratedImage(filename="bad.png", content=None),
        ]
    )
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")

    result = process_image_task(image_task(candidate_count=2), provider, storage)

    assert result.status == "completed"
    assert len(result.materials) == 1
    assert result.failed_count == 1
    assert result.partial is True


def test_run_next_image_task_claims_pending_task_and_writes_material_candidates(tmp_path: Path):
    provider = FakeImageProvider(
        [GeneratedImage(filename="scene-1.png", content=b"png")]
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
    assert store.created_materials[0]["file_url"] == "/assets/generated/images/task-1-scene-1/scene-1.png"
    assert store.created_materials[0]["metadata"]["source"] == "ai_generated"  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["generation_task_id"] == "task-1"  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["source_scene_id"] == "scene-1"  # type: ignore[index]
    assert store.created_materials[0]["metadata"]["reference_material_ids"] == ["material-1"]  # type: ignore[index]
    assert store.completed is not None
    assert store.completed["status"] == "completed"
    assert store.completed["result"]["generated_count"] == 1  # type: ignore[index]
    assert store.model_snapshot is not None
    assert store.model_snapshot["model_id"] == "model-1"
    assert "api_key" not in store.model_snapshot


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


def test_jimeng_image_provider_is_built_from_database_model_config():
    provider = image_provider_from_model(image_model_config("jimeng_visual"))

    assert isinstance(provider, JimengImageProvider)
    assert provider.req_key == "test-request-key"
    assert provider.width == 1328
    assert provider.height == 1328


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


def test_jimeng_image_provider_builds_visual_sdk_form_and_downloads_urls():
    forms: list[dict[str, object]] = []

    def fake_submit(form: dict[str, object]) -> dict[str, object]:
        forms.append(form)
        return {"data": {"image_urls": ["https://tmp.example.com/image.png"]}}

    def fake_download(url: str) -> bytes:
        assert url == "https://tmp.example.com/image.png"
        return b"png"

    provider = JimengImageProvider(
        access_key="ak",
        secret_key="sk",
        submit_task=fake_submit,
        download_url=fake_download,
    )

    images = provider.generate_images(image_task(candidate_count=1))

    assert images == [GeneratedImage(filename="task-1-1.png", content=b"png")]
    assert forms[0]["req_key"] == "high_aes_general_v30l_zt2i"
    assert forms[0]["prompt"] == "生成分镜候选图"
    assert forms[0]["return_url"] is True
    assert forms[0]["width"] == 1328
    assert forms[0]["height"] == 1328
    assert forms[0]["logo_info"] == {"add_logo": False}


def test_jimeng_image_provider_requires_credentials():
    try:
        JimengImageProvider(access_key="", secret_key="")
    except ProviderConfigError as error:
        assert "JIMENG_ACCESS_KEY" in str(error)
    else:
        raise AssertionError("missing Jimeng credentials should fail fast")
