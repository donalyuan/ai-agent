from __future__ import annotations

import base64
import binascii
import json
import logging
import os
import re
import shlex
import uuid
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Callable, Protocol
from urllib import error as urllib_error
from urllib import request as urllib_request

from video_worker.generated_image_filename import (
    DEFAULT_SCRIPT_TITLE,
    MAX_FILENAME_BYTES,
    generated_image_filename,
)
from video_worker.model_registry import ImageModelRuntimeConfig, PostgresModelRegistry


OPENAI_COMPATIBLE_USER_AGENT = "codex-cli/0.142.5"
LOGGER = logging.getLogger("uvicorn.error")


@dataclass(frozen=True)
class AssetGenerationTask:
    task_id: str
    provider: str
    prompt: str
    candidate_count: int
    reference_material_urls: list[str] = field(default_factory=list)
    reference_material_ids: list[str] = field(default_factory=list)
    generation_task_id: str | None = None
    scene_id: str | None = None
    script_title_snapshot: str = DEFAULT_SCRIPT_TITLE
    scene_sequence: int = 1
    candidate_index: int | None = None


@dataclass(frozen=True)
class SceneGenerationContext:
    scene_id: str
    sequence: int
    narration: str
    visual_description: str
    emotion: str
    duration_sec: int


@dataclass(frozen=True)
class PendingImageGenerationTask:
    task_id: str
    project_id: str
    script_id: str
    model_id: str
    provider: str
    image_candidates_per_scene: int
    reference_material_ids: list[str]
    reference_material_urls: list[str]
    scenes: list[SceneGenerationContext]
    script_title_snapshot: str = DEFAULT_SCRIPT_TITLE


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
    status: str
    materials: list[GeneratedMaterial]
    retry_count: int = 0
    failed_count: int = 0
    partial: bool = False
    error_message: str | None = None
    fatal: bool = False
    failures: list[FailedImageCandidate] = field(default_factory=list)


class TemporaryProviderError(RuntimeError):
    pass


class PermanentProviderError(RuntimeError):
    pass


class ProviderConfigError(RuntimeError):
    pass


class ImageProvider(Protocol):
    request_mode: str

    def generate_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        ...


class AssetGenerationStore(Protocol):
    def claim_next_image_task(self) -> PendingImageGenerationTask | None:
        ...

    def record_model_snapshot(
        self,
        task_id: str,
        model_snapshot: dict[str, object],
    ) -> None:
        ...

    def create_generated_image_candidate(
        self,
        task: PendingImageGenerationTask,
        scene: SceneGenerationContext,
        material: GeneratedMaterial,
        rank: int,
    ) -> None:
        ...

    def create_failed_image_candidate(
        self,
        task: PendingImageGenerationTask,
        scene: SceneGenerationContext,
        candidate_index: int,
        rank: int,
        error_message: str,
    ) -> None:
        ...

    def complete_image_task(
        self,
        task_id: str,
        status: str,
        result: dict[str, object],
        error_message: str | None,
        retry_count: int,
    ) -> None:
        ...


class FakeImageProvider:
    request_mode = "batch"

    def __init__(self, responses: list[GeneratedImage | Exception]):
        self._responses = list(responses)
        self.call_count = 0

    def generate_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        self.call_count += 1
        if not self._responses:
            return []
        response = self._responses.pop(0)
        if isinstance(response, Exception):
            raise response
        images = [response]
        while self._responses and not isinstance(self._responses[0], Exception):
            images.append(self._responses.pop(0))  # type: ignore[arg-type]
        return [
            replace(image, candidate_index=index)
            for index, image in enumerate(images[: task.candidate_count], start=1)
        ]


class LocalAssetStorage:
    def __init__(self, root: Path, public_prefix: str = "/assets"):
        self.root = Path(root)
        self.public_prefix = public_prefix.rstrip("/")

    def save_image(self, task_id: str, image: GeneratedImage) -> str:
        if image.content is None:
            raise ValueError("generated image content is empty")
        filename = image.filename.strip()
        if (
            not filename
            or filename in {".", ".."}
            or Path(filename).name != filename
            or "\\" in filename
        ):
            raise ValueError("generated image filename must be a safe basename")
        if len(filename.encode("utf-8")) > MAX_FILENAME_BYTES:
            raise ValueError("generated image filename exceeds 255 UTF-8 bytes")
        path = self.root / "generated" / "images" / task_id / filename
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(image.content)
        return f"{self.public_prefix}/generated/images/{task_id}/{filename}"


class OpenAIImageProvider:
    request_mode = "batch"

    def __init__(
        self,
        api_key: str | None,
        model: str = "gpt-image-2",
        base_url: str = "https://api.openai.com/v1",
        http_post: Callable[
            [str, dict[str, str], dict[str, object]], dict[str, object]
        ] | None = None,
        http_multipart_post: Callable[
            [str, dict[str, str], dict[str, object], list[tuple[str, str, bytes, str]]],
            dict[str, object],
        ] | None = None,
        download_url: Callable[[str], bytes] | None = None,
        timeout_seconds: int = 60,
    ):
        if not api_key:
            raise ProviderConfigError("OPENAI_API_KEY is required for gpt-image-2")
        self.api_key = api_key
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.timeout_seconds = timeout_seconds
        self.http_post = http_post or (
            lambda url, headers, payload: default_json_post(
                url, headers, payload, timeout_seconds=self.timeout_seconds
            )
        )
        self.http_multipart_post = http_multipart_post or (
            lambda url, headers, fields, files: default_multipart_post(
                url,
                headers,
                fields,
                files,
                timeout_seconds=self.timeout_seconds,
            )
        )
        self.download_url = download_url or default_binary_get

    def generate_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        if task.reference_material_urls:
            return self._edit_images(task)
        payload: dict[str, object] = {
            "model": self.model,
            "prompt": task.prompt,
            "n": task.candidate_count,
        }
        response = self.http_post(
            f"{self.base_url}/images/generations",
            {
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
                "User-Agent": OPENAI_COMPATIBLE_USER_AGENT,
            },
            payload,
        )
        data = response.get("data")
        return self._images_from_response(task, data)

    def _edit_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        files = []
        for index, url in enumerate(task.reference_material_urls, start=1):
            files.append(
                (
                    "image[]",
                    f"reference-{index}.png",
                    self.download_url(url),
                    "image/png",
                )
            )
        response = self.http_multipart_post(
            f"{self.base_url}/images/edits",
            {
                "Authorization": f"Bearer {self.api_key}",
                "User-Agent": OPENAI_COMPATIBLE_USER_AGENT,
            },
            {"model": self.model, "prompt": task.prompt, "n": task.candidate_count},
            files,
        )
        data = response.get("data")
        return self._images_from_response(task, data)

    def _images_from_response(
        self,
        task: AssetGenerationTask,
        data: object,
    ) -> list[GeneratedImage]:
        if not isinstance(data, list):
            raise RuntimeError("OpenAI image response missing data")
        images: list[GeneratedImage] = []
        for index, item in enumerate(data, start=1):
            if not isinstance(item, dict):
                continue
            b64_json = item.get("b64_json")
            if not isinstance(b64_json, str) or not b64_json:
                continue
            images.append(
                GeneratedImage(
                    filename=f"{task.task_id}-{index}.png",
                    content=base64.b64decode(b64_json),
                    candidate_index=index,
                )
            )
        return images


class VolcengineArkImageProvider:
    request_mode = "per_candidate"

    def __init__(
        self,
        api_key: str | None,
        model: str = "doubao-seedream-5-0-260128",
        base_url: str = "https://ark.cn-beijing.volces.com/api/v3",
        http_post: Callable[
            [str, dict[str, str], dict[str, object]], dict[str, object]
        ]
        | None = None,
        download_url: Callable[[str], bytes] | None = None,
        timeout_seconds: int = 120,
    ):
        if not api_key:
            raise ProviderConfigError("Volcengine Ark API key is required")
        self.api_key = api_key
        self.model = model
        self.base_url = base_url.rstrip("/")
        self.timeout_seconds = timeout_seconds
        self.http_post = http_post or (
            lambda url, headers, payload: default_json_post(
                url,
                headers,
                payload,
                timeout_seconds=self.timeout_seconds,
            )
        )
        self.download_url = download_url or default_binary_get

    def generate_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        if task.candidate_count != 1:
            raise ProviderConfigError("Volcengine Ark provider requires one candidate")

        payload: dict[str, object] = {
            "model": self.model,
            "prompt": task.prompt,
            "sequential_image_generation": "disabled",
            "response_format": "b64_json",
            "stream": False,
            "watermark": False,
        }
        if task.reference_material_urls:
            payload["image"] = [
                image_data_url(self.download_url(url))
                for url in task.reference_material_urls
            ]

        url = f"{self.base_url}/images/generations"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        log_ark_request(task.task_id, url, payload)
        response = self.http_post(url, headers, payload)
        log_ark_response(task.task_id, response)
        return [parse_ark_image(task.task_id, response)]


def process_image_task(
    task: AssetGenerationTask,
    provider: ImageProvider,
    storage: LocalAssetStorage,
) -> ImageTaskResult:
    if provider.request_mode == "per_candidate":
        return process_per_candidate_image_task(task, provider, storage)
    if provider.request_mode != "batch":
        error_message = (
            f"unsupported image provider request mode: {provider.request_mode}"
        )
        failures = [
            FailedImageCandidate(candidate_index=index, error_message=error_message)
            for index in _task_candidate_indices(task)
        ]
        return ImageTaskResult(
            status="failed",
            materials=[],
            failed_count=len(failures),
            error_message=error_message,
            fatal=True,
            failures=failures,
        )
    return process_batch_image_task(task, provider, storage)


def process_batch_image_task(
    task: AssetGenerationTask,
    provider: ImageProvider,
    storage: LocalAssetStorage,
) -> ImageTaskResult:
    requested_indices = _task_candidate_indices(task)
    retry_count = 0
    try:
        images = provider.generate_images(task)
    except TemporaryProviderError:
        retry_count = 1
        try:
            images = provider.generate_images(task)
        except TemporaryProviderError as error:
            return _failed_image_task_result(
                requested_indices,
                error,
                retry_count=retry_count,
            )
        except Exception as error:
            return _failed_image_task_result(
                requested_indices,
                error,
                retry_count=retry_count,
                fatal=True,
            )
    except Exception as error:
        return _failed_image_task_result(
            requested_indices,
            error,
            retry_count=retry_count,
            fatal=True,
        )

    materials: list[GeneratedMaterial] = []
    failures: list[FailedImageCandidate] = []
    error_messages: list[str] = []
    seen_indices: set[int] = set()
    for image in images[: task.candidate_count]:
        candidate_index = task.candidate_index or image.candidate_index
        if candidate_index not in requested_indices or candidate_index in seen_indices:
            error_messages.append(
                f"图片供应商返回无效候选序号: {candidate_index}"
            )
            continue
        seen_indices.add(candidate_index)
        try:
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
                replace(
                    image,
                    filename=filename,
                    candidate_index=candidate_index,
                ),
            )
        except (OSError, ValueError, PermanentProviderError) as error:
            failures.append(
                FailedImageCandidate(
                    candidate_index=candidate_index,
                    error_message=str(error),
                )
            )
            error_messages.append(str(error))
            continue
        materials.append(
            GeneratedMaterial(
                file_url=file_url,
                file_name=filename,
                metadata={
                    "storage_provider": "local",
                    "source": "ai_generated",
                    "generation_task_id": task.generation_task_id or task.task_id,
                    "source_scene_id": task.scene_id,
                    "reference_material_ids": task.reference_material_ids,
                    "reference_material_urls": task.reference_material_urls,
                    "candidate_status": "candidate",
                    "script_title_snapshot": task.script_title_snapshot,
                    "scene_sequence": task.scene_sequence,
                    "candidate_index": candidate_index,
                },
                candidate_index=candidate_index,
            )
        )

    for candidate_index in requested_indices:
        if candidate_index in seen_indices:
            continue
        error_message = "图片生成未返回有效文件"
        failures.append(
            FailedImageCandidate(
                candidate_index=candidate_index,
                error_message=error_message,
            )
        )
        error_messages.append(error_message)

    failures.sort(key=lambda failure: failure.candidate_index)
    failed_count = len(failures)

    return ImageTaskResult(
        status="completed" if materials else "failed",
        materials=materials,
        retry_count=retry_count,
        failed_count=failed_count,
        partial=bool(materials and failed_count),
        error_message="; ".join(error_messages) or None,
        failures=failures,
    )


def process_per_candidate_image_task(
    task: AssetGenerationTask,
    provider: ImageProvider,
    storage: LocalAssetStorage,
) -> ImageTaskResult:
    materials: list[GeneratedMaterial] = []
    failures: list[FailedImageCandidate] = []
    retry_count = 0
    error_messages: list[str] = []
    fatal = False

    for candidate_index in range(1, task.candidate_count + 1):
        single_task = replace(
            task,
            task_id=f"{task.task_id}-{candidate_index}",
            candidate_count=1,
            candidate_index=candidate_index,
        )
        result = process_batch_image_task(single_task, provider, storage)
        materials.extend(result.materials)
        failures.extend(result.failures)
        retry_count += result.retry_count
        if result.error_message:
            error_messages.append(result.error_message)

        if result.fatal:
            fatal = True
            fatal_error = result.error_message or "图片供应商返回永久错误"
            failures.extend(
                FailedImageCandidate(
                    candidate_index=remaining_index,
                    error_message=fatal_error,
                )
                for remaining_index in range(
                    candidate_index + 1,
                    task.candidate_count + 1,
                )
            )
            break

    failures.sort(key=lambda failure: failure.candidate_index)
    failed_count = len(failures)
    return ImageTaskResult(
        status="completed" if materials else "failed",
        materials=materials,
        retry_count=retry_count,
        failed_count=failed_count,
        partial=bool(materials and failed_count),
        error_message="; ".join(error_messages) or None,
        fatal=fatal,
        failures=failures,
    )


def _task_candidate_indices(task: AssetGenerationTask) -> list[int]:
    if task.candidate_index is not None:
        return [task.candidate_index]
    return list(range(1, task.candidate_count + 1))


def _failed_image_task_result(
    candidate_indices: list[int],
    error: Exception,
    *,
    retry_count: int,
    fatal: bool = False,
) -> ImageTaskResult:
    error_message = str(error) or error.__class__.__name__
    failures = [
        FailedImageCandidate(
            candidate_index=candidate_index,
            error_message=error_message,
        )
        for candidate_index in candidate_indices
    ]
    return ImageTaskResult(
        status="failed",
        materials=[],
        retry_count=retry_count,
        failed_count=len(failures),
        partial=False,
        error_message=error_message,
        fatal=fatal,
        failures=failures,
    )


def run_next_image_task(
    store: AssetGenerationStore,
    model_registry: PostgresModelRegistry,
    provider_factory: Callable[[ImageModelRuntimeConfig], ImageProvider],
    storage: LocalAssetStorage,
) -> bool:
    task = store.claim_next_image_task()
    if task is None:
        return False

    try:
        model_config = model_registry.resolve_enabled(task.model_id, "image")
        store.record_model_snapshot(task.task_id, model_config.snapshot())
        provider = provider_factory(model_config)
    except Exception as error:
        error_message = str(error) or error.__class__.__name__
        failed_count = 0
        for scene in task.scenes:
            for candidate_index in range(1, task.image_candidates_per_scene + 1):
                failed_count += 1
                store.create_failed_image_candidate(
                    task,
                    scene,
                    candidate_index=candidate_index,
                    rank=9000 + candidate_index,
                    error_message=error_message,
                )
        store.complete_image_task(
            task.task_id,
            status="failed",
            result={"generated_count": 0, "failed_count": failed_count, "partial": False},
            error_message=error_message,
            retry_count=0,
        )
        return True
    generated_count = 0
    failed_count = 0
    retry_count = 0
    error_messages: list[str] = []

    for scene_index, scene in enumerate(task.scenes):
        image_task = AssetGenerationTask(
            task_id=f"{task.task_id}-{scene.scene_id}",
            provider=task.provider,
            prompt=scene_image_prompt(scene),
            candidate_count=task.image_candidates_per_scene,
            reference_material_urls=list(task.reference_material_urls),
            reference_material_ids=list(task.reference_material_ids),
            generation_task_id=task.task_id,
            scene_id=scene.scene_id,
            script_title_snapshot=task.script_title_snapshot,
            scene_sequence=scene.sequence,
        )
        result = process_image_task(image_task, provider, storage)
        retry_count += result.retry_count
        if result.error_message:
            error_messages.append(result.error_message)

        for material in result.materials:
            generated_count += 1
            store.create_generated_image_candidate(
                task,
                scene,
                material,
                rank=1000 + material.candidate_index,
            )

        failed_count += len(result.failures)
        for failure in result.failures:
            store.create_failed_image_candidate(
                task,
                scene,
                candidate_index=failure.candidate_index,
                rank=9000 + failure.candidate_index,
                error_message=failure.error_message,
            )

        if result.fatal:
            fatal_error = result.error_message or "图片供应商返回永久错误"
            for remaining_scene in task.scenes[scene_index + 1 :]:
                for candidate_index in range(
                    1,
                    task.image_candidates_per_scene + 1,
                ):
                    failed_count += 1
                    store.create_failed_image_candidate(
                        task,
                        remaining_scene,
                        candidate_index=candidate_index,
                        rank=9000 + candidate_index,
                        error_message=fatal_error,
                    )
            break

    status = "completed" if generated_count else "failed"
    result_payload: dict[str, object] = {
        "generated_count": generated_count,
        "failed_count": failed_count,
        "partial": bool(generated_count and failed_count),
    }
    store.complete_image_task(
        task.task_id,
        status=status,
        result=result_payload,
        error_message="; ".join(error_messages) or None,
        retry_count=retry_count,
    )

    return True


def scene_image_prompt(scene: SceneGenerationContext) -> str:
    return (
        f"为短视频第 {scene.sequence} 个分镜生成候选图。"
        f"画面：{scene.visual_description}。"
        f"旁白：{scene.narration}。"
        f"情绪：{scene.emotion}。"
        f"时长：{scene.duration_sec} 秒。"
    )


def image_provider_from_model(config: ImageModelRuntimeConfig) -> ImageProvider:
    if config.api_protocol == "openai_images":
        return OpenAIImageProvider(
            api_key=config.api_key,
            model=config.upstream_model,
            base_url=config.request_base_url,
            timeout_seconds=config.timeout_seconds,
        )
    if config.api_protocol == "volcengine_ark_images":
        return VolcengineArkImageProvider(
            api_key=config.api_key,
            model=config.upstream_model,
            base_url=config.request_base_url,
            timeout_seconds=config.timeout_seconds,
        )
    raise ProviderConfigError(f"unsupported image protocol: {config.api_protocol}")


class PostgresAssetGenerationStore:
    def __init__(self, database_url: str):
        self.database_url = database_url

    @classmethod
    def from_env(cls) -> "PostgresAssetGenerationStore":
        return cls(
            os.getenv(
                "DATABASE_URL",
                "postgres://postgres:postgres@biga-postgres:5432/video_agent",
            )
        )

    def claim_next_image_task(self) -> PendingImageGenerationTask | None:
        import psycopg
        from psycopg.rows import dict_row

        with psycopg.connect(self.database_url, row_factory=dict_row) as connection:
            with connection.transaction():
                row = connection.execute(
                    """
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
                    SET status = 'processing',
                        updated_at = NOW()
                    FROM candidate
                    WHERE task.id = candidate.id
                    RETURNING task.id, task.project_id, task.script_id, task.model_id,
                              task.provider,
                              task.candidate_count, task.reference_material_ids, task.params,
                              candidate.script_title_snapshot
                    """,
                ).fetchone()
                if row is None:
                    return None

                params = row["params"] or {}
                scene_ids = params.get("scene_ids") or [params.get("scene_id")]
                scene_ids = [str(scene_id) for scene_id in scene_ids if scene_id]
                scenes = self._load_scenes(connection, scene_ids)
                if not scenes:
                    raise RuntimeError("image generation task has no scenes")
                per_scene = int(
                    params.get("image_candidates_per_scene")
                    or max(1, int(row["candidate_count"]) // len(scenes))
                )
                reference_material_ids = [
                    str(material_id) for material_id in row["reference_material_ids"]
                ]
                reference_urls = self._load_reference_urls(
                    connection,
                    project_id=str(row["project_id"]),
                    material_ids=reference_material_ids,
                )

                return PendingImageGenerationTask(
                    task_id=str(row["id"]),
                    project_id=str(row["project_id"]),
                    script_id=str(row["script_id"]),
                    model_id=str(row["model_id"] or ""),
                    provider=str(row["provider"]),
                    image_candidates_per_scene=per_scene,
                    reference_material_ids=reference_material_ids,
                    reference_material_urls=reference_urls,
                    scenes=scenes,
                    script_title_snapshot=str(row["script_title_snapshot"]),
                )

    def record_model_snapshot(
        self,
        task_id: str,
        model_snapshot: dict[str, object],
    ) -> None:
        import psycopg
        from psycopg.types.json import Jsonb

        with psycopg.connect(self.database_url) as connection:
            connection.execute(
                """
                UPDATE asset_generation_tasks
                SET model_snapshot = %s,
                    updated_at = NOW()
                WHERE id = %s
                  AND status = 'processing'
                """,
                (Jsonb(model_snapshot), task_id),
            )

    def create_generated_image_candidate(
        self,
        task: PendingImageGenerationTask,
        scene: SceneGenerationContext,
        material: GeneratedMaterial,
        rank: int,
    ) -> None:
        import psycopg
        from psycopg.types.json import Jsonb

        with psycopg.connect(self.database_url) as connection:
            with connection.transaction():
                material_id = connection.execute(
                    """
                    INSERT INTO materials (
                        project_id, material_type, file_url, file_name, tags, metadata, status
                    )
                    VALUES (%s, 'image', %s, %s, ARRAY['AI生成']::text[], %s, 'active')
                    RETURNING id
                    """,
                    (
                        task.project_id,
                        material.file_url,
                        material.file_name,
                        Jsonb(material.metadata),
                    ),
                ).fetchone()[0]
                connection.execute(
                    """
                    INSERT INTO scene_asset_candidates (
                        project_id, script_id, scene_id, material_id, candidate_type,
                        source, status, rank, generation_task_id, metadata
                    )
                    VALUES (%s, %s, %s, %s, 'image', 'ai_generated', 'candidate', %s, %s, %s)
                    """,
                    (
                        task.project_id,
                        task.script_id,
                        scene.scene_id,
                        material_id,
                        rank,
                        task.task_id,
                        Jsonb(material.metadata),
                    ),
                )

    def create_failed_image_candidate(
        self,
        task: PendingImageGenerationTask,
        scene: SceneGenerationContext,
        candidate_index: int,
        rank: int,
        error_message: str,
    ) -> None:
        import psycopg
        from psycopg.types.json import Jsonb

        metadata = {
            "source": "ai_generated",
            "generation_task_id": task.task_id,
            "source_scene_id": scene.scene_id,
            "reference_material_ids": task.reference_material_ids,
            "candidate_status": "failed",
            "error_message": error_message,
            "script_title_snapshot": task.script_title_snapshot,
            "scene_sequence": scene.sequence,
            "candidate_index": candidate_index,
        }
        with psycopg.connect(self.database_url) as connection:
            connection.execute(
                """
                INSERT INTO scene_asset_candidates (
                    project_id, script_id, scene_id, material_id, candidate_type,
                    source, status, rank, generation_task_id, metadata
                )
                VALUES (%s, %s, %s, NULL, 'image', 'ai_generated', 'failed', %s, %s, %s)
                """,
                (
                    task.project_id,
                    task.script_id,
                    scene.scene_id,
                    rank,
                    task.task_id,
                    Jsonb(metadata),
                ),
            )

    def complete_image_task(
        self,
        task_id: str,
        status: str,
        result: dict[str, object],
        error_message: str | None,
        retry_count: int,
    ) -> None:
        import psycopg
        from psycopg.types.json import Jsonb

        with psycopg.connect(self.database_url) as connection:
            connection.execute(
                """
                UPDATE asset_generation_tasks
                SET status = %s,
                    result = %s,
                    error_message = %s,
                    retry_count = retry_count + %s,
                    updated_at = NOW()
                WHERE id = %s
                """,
                (status, Jsonb(result), error_message, retry_count, task_id),
            )

    def _load_scenes(self, connection, scene_ids: list[str]) -> list[SceneGenerationContext]:
        rows = connection.execute(
            """
            SELECT id, sequence, narration, visual_description, emotion, duration_sec
            FROM scenes
            WHERE id = ANY(%s::uuid[])
            ORDER BY sequence ASC
            """,
            (scene_ids,),
        ).fetchall()
        return [
            SceneGenerationContext(
                scene_id=str(row["id"]),
                sequence=int(row["sequence"]),
                narration=str(row["narration"]),
                visual_description=str(row["visual_description"]),
                emotion=str(row["emotion"]),
                duration_sec=int(row["duration_sec"]),
            )
            for row in rows
        ]

    def _load_reference_urls(
        self,
        connection,
        project_id: str,
        material_ids: list[str],
    ) -> list[str]:
        if not material_ids:
            return []
        rows = connection.execute(
            """
            SELECT file_url
            FROM materials
            WHERE project_id = %s
              AND id = ANY(%s::uuid[])
              AND material_type = 'image'
              AND status = 'active'
            ORDER BY created_at ASC
            """,
            (project_id, material_ids),
        ).fetchall()
        return [str(row["file_url"]) for row in rows]


def safe_filename(filename: str) -> str:
    cleaned = Path(filename).name.strip()
    if not cleaned:
        return "generated.png"
    return re.sub(r"[^A-Za-z0-9._-]", "-", cleaned)


def default_json_post(
    url: str,
    headers: dict[str, str],
    payload: dict[str, object],
    timeout_seconds: int = 60,
) -> dict[str, object]:
    data = json.dumps(payload).encode("utf-8")
    request = urllib_request.Request(url, data=data, headers=headers, method="POST")
    return _send_json_request(request, timeout=timeout_seconds)


def default_multipart_post(
    url: str,
    headers: dict[str, str],
    fields: dict[str, object],
    files: list[tuple[str, str, bytes, str]],
    timeout_seconds: int = 120,
) -> dict[str, object]:
    boundary = f"----novex-{uuid.uuid4().hex}"
    body = multipart_body(boundary, fields, files)
    request_headers = {
        **headers,
        "Content-Type": f"multipart/form-data; boundary={boundary}",
    }
    request = urllib_request.Request(
        url,
        data=body,
        headers=request_headers,
        method="POST",
    )
    return _send_json_request(request, timeout=timeout_seconds)


def _send_json_request(
    request: urllib_request.Request,
    timeout: int,
) -> dict[str, object]:
    try:
        with urllib_request.urlopen(request, timeout=timeout) as response:
            raw = response.read().decode("utf-8")
    except urllib_error.HTTPError as error:
        raise _provider_error_from_http(error) from error
    except (urllib_error.URLError, TimeoutError) as error:
        raise TemporaryProviderError(str(error)) from error

    parsed = json.loads(raw)
    if not isinstance(parsed, dict):
        raise RuntimeError("JSON response must be an object")
    return parsed


def _provider_error_from_http(error: urllib_error.HTTPError) -> RuntimeError:
    response_summary = _http_error_response_summary(error)
    reason = str(error.reason).strip() if error.reason else ""
    message = f"HTTP {error.code}"
    if reason:
        message = f"{message} {reason}"
    if response_summary:
        message = f"{message}: {response_summary}"

    if error.code == 429 or 500 <= error.code <= 599:
        return TemporaryProviderError(message)
    return PermanentProviderError(message)


def _http_error_response_summary(error: urllib_error.HTTPError) -> str:
    try:
        raw = error.read(4096)
    except OSError:
        return ""
    if not raw:
        return ""

    text = raw.decode("utf-8", errors="replace").strip()
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        summary = text
    else:
        error_payload = payload.get("error") if isinstance(payload, dict) else None
        if isinstance(error_payload, dict) and isinstance(error_payload.get("message"), str):
            summary = error_payload["message"]
        elif isinstance(error_payload, str):
            summary = error_payload
        else:
            summary = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
    return " ".join(summary.split())[:1000]


def multipart_body(
    boundary: str,
    fields: dict[str, object],
    files: list[tuple[str, str, bytes, str]],
) -> bytes:
    chunks: list[bytes] = []
    for name, value in fields.items():
        chunks.extend(
            [
                f"--{boundary}\r\n".encode("utf-8"),
                f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode("utf-8"),
                str(value).encode("utf-8"),
                b"\r\n",
            ]
        )
    for field_name, filename, content, content_type in files:
        chunks.extend(
            [
                f"--{boundary}\r\n".encode("utf-8"),
                (
                    f'Content-Disposition: form-data; name="{field_name}"; '
                    f'filename="{safe_filename(filename)}"\r\n'
                ).encode("utf-8"),
                f"Content-Type: {content_type}\r\n\r\n".encode("utf-8"),
                content,
                b"\r\n",
            ]
        )
    chunks.append(f"--{boundary}--\r\n".encode("utf-8"))
    return b"".join(chunks)


def default_binary_get(url: str) -> bytes:
    public_prefix = os.getenv("ASSET_PUBLIC_PREFIX", "/assets").rstrip("/")
    if url == public_prefix or url.startswith(f"{public_prefix}/"):
        storage_root = Path(
            os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")
        ).resolve()
        relative_path = url[len(public_prefix) :].lstrip("/")
        local_path = (storage_root / relative_path).resolve()
        try:
            local_path.relative_to(storage_root)
        except ValueError as error:
            raise PermanentProviderError(
                "reference material path escapes asset storage root"
            ) from error
        return local_path.read_bytes()
    with urllib_request.urlopen(url, timeout=60) as response:
        return response.read()


def detect_image_type(content: bytes) -> tuple[str, str]:
    if content.startswith(b"\x89PNG\r\n\x1a\n"):
        return ("image/png", ".png")
    if content.startswith(b"\xff\xd8\xff"):
        return ("image/jpeg", ".jpg")
    if len(content) >= 12 and content.startswith(b"RIFF") and content[8:12] == b"WEBP":
        return ("image/webp", ".webp")
    raise PermanentProviderError("unsupported image type")


def image_data_url(content: bytes) -> str:
    media_type, _ = detect_image_type(content)
    encoded = base64.b64encode(content).decode("ascii")
    return f"data:{media_type};base64,{encoded}"


def parse_ark_image(task_id: str, response: dict[str, object]) -> GeneratedImage:
    data = response.get("data")
    if not isinstance(data, list) or len(data) != 1 or not isinstance(data[0], dict):
        raise PermanentProviderError(
            "Ark image response must contain exactly one data item"
        )
    item = data[0]
    if item.get("error") is not None:
        summary = json.dumps(
            item["error"], ensure_ascii=False, separators=(",", ":")
        )
        raise PermanentProviderError(f"Ark image response error: {summary}")
    encoded = item.get("b64_json")
    if not isinstance(encoded, str) or not encoded:
        raise PermanentProviderError("Ark image response missing b64_json")
    try:
        content = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError) as error:
        raise PermanentProviderError("invalid Ark image base64") from error
    _, extension = detect_image_type(content)
    return GeneratedImage(filename=f"{task_id}-1{extension}", content=content)


def log_ark_request(task_id: str, url: str, payload: dict[str, object]) -> None:
    safe_payload = dict(payload)
    references = safe_payload.get("image")
    reference_count = len(references) if isinstance(references, list) else 0
    if reference_count:
        safe_payload["image"] = f"<redacted:{reference_count} reference image(s)>"
    safe_json = json.dumps(safe_payload, ensure_ascii=False, separators=(",", ":"))
    curl = " ".join(
        [
            "curl",
            "-X",
            "POST",
            shlex.quote(url),
            "-H",
            shlex.quote("Authorization: Bearer ***"),
            "-H",
            shlex.quote("Content-Type: application/json"),
            "--data-raw",
            shlex.quote(safe_json),
        ]
    )
    LOGGER.info(
        json.dumps(
            {
                "event": "ark_image_request",
                "task_id": task_id,
                "url": url,
                "headers": {
                    "Authorization": "Bearer ***",
                    "Content-Type": "application/json",
                },
                "payload": safe_payload,
                "curl": curl,
            },
            ensure_ascii=False,
            separators=(",", ":"),
        )
    )


def log_ark_response(task_id: str, response: dict[str, object]) -> None:
    data = response.get("data")
    items = data if isinstance(data, list) else []
    image_bytes = sum(
        base64_decoded_size(item.get("b64_json", ""))
        for item in items
        if isinstance(item, dict) and isinstance(item.get("b64_json"), str)
    )
    error_count = sum(
        1 for item in items if isinstance(item, dict) and item.get("error") is not None
    )
    LOGGER.info(
        json.dumps(
            {
                "event": "ark_image_response",
                "task_id": task_id,
                "data_count": len(items),
                "error_count": error_count,
                "image_bytes": image_bytes,
            },
            ensure_ascii=False,
            separators=(",", ":"),
        )
    )


def base64_decoded_size(encoded: str) -> int:
    padding = len(encoded) - len(encoded.rstrip("="))
    return max(0, len(encoded) * 3 // 4 - padding)
