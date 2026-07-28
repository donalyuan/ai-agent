"""作品生成 Worker：支持显式 fake/real 模式与可恢复的 provider attempt。"""

from __future__ import annotations

import os
import hashlib
import json
import subprocess
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Protocol


WORK_GENERATION_CLAIM_SQL = """SELECT s.id, s.run_id, s.step_type, s.status, s.depends_on,
          s.result_material_ids, s.output_snapshot, s.input_snapshot,
          s.model_snapshot AS step_model_snapshot, s.resource_usage AS step_resource_usage,
          r.status AS run_status, r.work_id, r.work_version_id,
          r.model_snapshot AS run_model_snapshot,
          r.capability_snapshot, r.voice_snapshot, r.prompt_snapshot,
          r.timeline_snapshot, r.parameter_snapshot, r.resource_usage AS run_resource_usage,
          w.project_id, w.title AS work_title, wv.input_snapshot AS work_input_snapshot,
          (SELECT v.languages FROM voice_catalog_entries v
             WHERE v.id=NULLIF(r.voice_snapshot->>'voice_id','')::uuid) AS voice_languages,
          (SELECT v.catalog_version FROM voice_catalog_entries v
             WHERE v.id=NULLIF(r.voice_snapshot->>'voice_id','')::uuid) AS voice_catalog_version,
          (SELECT v.is_available FROM voice_catalog_entries v
             WHERE v.id=NULLIF(r.voice_snapshot->>'voice_id','')::uuid) AS voice_available,
          (SELECT COUNT(*) FROM work_generation_steps rs
             WHERE rs.run_id=r.id AND rs.step_type='asr' AND rs.is_required) AS asr_step_count
   FROM work_generation_steps s
   JOIN work_generation_runs r ON r.id=s.run_id
   JOIN works w ON w.id=r.work_id
   JOIN work_versions wv ON wv.id=r.work_version_id
   WHERE ((r.status='cancelling' AND s.status='running'
     AND EXISTS (SELECT 1 FROM work_generation_attempts a
       WHERE a.step_id=s.id AND a.status='running'
         AND a.upstream_task_id IS NOT NULL))
     OR (r.status IN ('queued','running') AND s.status='queued'
     AND NOT EXISTS (SELECT 1 FROM work_generation_steps d
       WHERE d.id = ANY(SELECT jsonb_array_elements_text(s.depends_on)::uuid)
         AND d.status <> 'succeeded'))
     OR (r.status IN ('queued','running') AND s.status='running'
       AND EXISTS (SELECT 1 FROM work_generation_attempts a
       WHERE a.step_id=s.id AND a.status='running'
         AND a.lease_expires_at < NOW())))
   ORDER BY CASE
       WHEN r.status='cancelling' THEN 0
       WHEN s.status='queued' THEN 1
       ELSE 2
   END, s.step_no
   FOR UPDATE OF s SKIP LOCKED LIMIT 1"""


@dataclass
class WorkAttempt:
    attempt_id: str
    step_id: str
    attempt_no: int
    status: str = "queued"
    upstream_task_id: str | None = None
    error_category: str | None = None
    error_summary: str | None = None
    provider_cancel_supported: bool = False
    cancel_requested_at: datetime | None = None
    cancel_response: str | None = None
    lease_expired: bool = False
    claimed_from_queue: bool = False
    input_snapshot: dict[str, object] = field(default_factory=dict)
    output_snapshot: dict[str, object] = field(default_factory=dict)
    error_code: str | None = None


@dataclass
class WorkStep:
    step_id: str
    run_id: str
    step_type: str
    status: str = "queued"
    depends_on: list[str] = field(default_factory=list)
    attempts: list[WorkAttempt] = field(default_factory=list)
    cancel_requested: bool = False
    result_material_ids: list[str] = field(default_factory=list)
    output_snapshot: dict[str, object] = field(default_factory=dict)
    input_snapshot: dict[str, object] = field(default_factory=dict)
    model_snapshot: dict[str, object] = field(default_factory=dict)
    resource_usage: dict[str, object] = field(default_factory=dict)
    run_model_snapshot: dict[str, object] = field(default_factory=dict)
    capability_snapshot: dict[str, object] = field(default_factory=dict)
    voice_snapshot: dict[str, object] = field(default_factory=dict)
    prompt_snapshot: dict[str, object] = field(default_factory=dict)
    timeline_snapshot: dict[str, object] = field(default_factory=dict)
    parameter_snapshot: dict[str, object] = field(default_factory=dict)
    run_resource_usage: dict[str, object] = field(default_factory=dict)
    work_input_snapshot: dict[str, object] = field(default_factory=dict)
    work_id: str | None = None
    work_version_id: str | None = None
    project_id: str | None = None
    work_title: str = ""
    asr_step_count: int = 0
    voice_languages: list[object] = field(default_factory=list)
    voice_catalog_version: int | None = None
    voice_available: bool = False
    error_category: str | None = None
    error_summary: str | None = None


class WorkGenerationStore(Protocol):
    def claim_next_step(self) -> WorkStep | None: ...
    def save_attempt(self, attempt: WorkAttempt) -> None: ...
    def update_step(self, step: WorkStep) -> None: ...

    def persist_upstream_task(self, attempt: WorkAttempt) -> None: ...

    def materialize_compose_artifact(self, step: WorkStep) -> None: ...

    def materialize_video_segment_artifact(self, step: WorkStep) -> None: ...


class FakeWorkProvider:
    """无外部费用的确定性 provider，用于本地验收和流程联调。"""

    supports_cancel = False

    def submit(self, step: WorkStep) -> str:
        return f"fake-{step.step_id}"

    def query(self, upstream_task_id: str) -> str:
        return "succeeded"

    def cancel(self, upstream_task_id: str) -> str:
        raise RuntimeError("当前 provider 不支持取消")


class TemporaryWorkProviderError(RuntimeError):
    pass


class UnknownSubmissionError(RuntimeError):
    pass


class PollingTemporaryError(RuntimeError):
    pass


class WorkGenerationConfigurationError(RuntimeError):
    pass


@dataclass(frozen=True)
class RealWorkGenerationLimits:
    allowed_run_ids: set[str]
    max_video_tasks: int = 1
    max_video_seconds: int = 15
    max_tts_characters: int = 398
    max_asr_tasks: int = 0
    max_concurrency: int = 1
    submit_retries: int = 0

    @classmethod
    def from_environment(cls) -> "RealWorkGenerationLimits":
        allowed = {
            item.strip()
            for item in os.getenv("WORK_GENERATION_REAL_RUN_ALLOWLIST", "").split(",")
            if item.strip()
        }
        limits = cls(
            allowed_run_ids=allowed,
            max_video_tasks=int(os.getenv("WORK_GENERATION_REAL_MAX_VIDEO_TASKS", "1")),
            max_video_seconds=int(os.getenv("WORK_GENERATION_REAL_MAX_VIDEO_SECONDS", "15")),
            max_tts_characters=int(os.getenv("WORK_GENERATION_REAL_MAX_TTS_CHARACTERS", "398")),
            max_asr_tasks=int(os.getenv("WORK_GENERATION_REAL_MAX_ASR_TASKS", "0")),
            max_concurrency=int(os.getenv("WORK_GENERATION_REAL_MAX_CONCURRENCY", "1")),
            submit_retries=int(os.getenv("WORK_GENERATION_REAL_SUBMIT_RETRIES", "0")),
        )
        if not allowed:
            raise WorkGenerationConfigurationError("真实作品生成必须配置非空运行 allowlist")
        if limits.max_concurrency != 1:
            raise WorkGenerationConfigurationError("真实作品生成并发必须为 1")
        if limits.submit_retries != 0:
            raise WorkGenerationConfigurationError("真实作品生成自动提交重试必须为 0")
        if (
            limits.max_video_tasks != 1
            or limits.max_video_seconds > 15
            or limits.max_tts_characters > 398
            or limits.max_asr_tasks != 0
        ):
            raise WorkGenerationConfigurationError("真实作品生成成本上限超过已批准边界")
        return limits

    def validate_step(self, step: WorkStep) -> None:
        if step.run_id not in self.allowed_run_ids:
            raise WorkGenerationConfigurationError("运行不在真实生成 allowlist")
        usage = step.run_resource_usage
        if int(usage.get("video_task_count") or 0) > self.max_video_tasks:
            raise WorkGenerationConfigurationError("视频任务数超过真实生成上限")
        if int(usage.get("video_seconds") or 0) > self.max_video_seconds:
            raise WorkGenerationConfigurationError("视频总时长超过真实生成上限")
        if int(usage.get("tts_characters") or 0) > self.max_tts_characters:
            raise WorkGenerationConfigurationError("TTS 字符数超过真实生成上限")
        if step.asr_step_count > self.max_asr_tasks or int(usage.get("asr_seconds") or 0) > 0:
            raise WorkGenerationConfigurationError("真实验证禁止 ASR 调用")


def validate_work_generation_mode(
    *, fake_enabled: bool, real_enabled: bool, worker_enabled: bool
) -> None:
    if worker_enabled and fake_enabled and real_enabled:
        raise WorkGenerationConfigurationError("作品生成 fake 与 real 模式不能同时启用")
    if worker_enabled and not fake_enabled and not real_enabled:
        raise WorkGenerationConfigurationError("作品生成 Worker 已启用但未选择 provider 模式")


def _work_artifact_identity(artifact_role: str, file_name: str) -> tuple[str, str]:
    """将 Worker 产物收敛到 Work Library 的受控角色和稳定 MIME。"""
    extension = os.path.splitext(file_name)[1].lower()
    if artifact_role in {"video_segment", "final_video"} and extension == ".mp4":
        role = "reusable_intermediate" if artifact_role == "video_segment" else "final_video"
        return role, "video/mp4"
    if artifact_role == "tts_audio":
        mime_types = {
            ".mp3": "audio/mpeg",
            ".wav": "audio/wav",
            ".m4a": "audio/mp4",
            ".ogg": "audio/ogg",
            ".aac": "audio/aac",
            ".pcm": "audio/L16",
            ".raw": "audio/L16",
        }
        if extension in mime_types:
            return "audio_track", mime_types[extension]
    if artifact_role == "subtitle" and extension == ".srt":
        return "subtitle", "application/x-subrip"
    raise WorkGenerationConfigurationError(
        f"Worker 产物 {artifact_role}/{extension or 'unknown'} 不支持登记为 WorkArtifact"
    )


def _sha256_file(path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _local_asset_path(file_url: str, storage_root: str) -> str:
    prefix = "/assets/"
    if not file_url.startswith(prefix):
        raise WorkGenerationConfigurationError("生成素材不是受控本地 asset URL")
    root = os.path.realpath(storage_root)
    path = os.path.realpath(os.path.join(root, file_url[len(prefix):]))
    if os.path.commonpath([root, path]) != root:
        raise WorkGenerationConfigurationError("生成素材路径越界")
    return path


class InMemoryWorkGenerationStore:
    def __init__(self, steps: list[WorkStep]):
        self.steps = steps
        self.attempts: list[WorkAttempt] = []

    def claim_next_step(self) -> WorkStep | None:
        for step in self.steps:
            if step.cancel_requested and step.status == "running":
                return step
        for step in self.steps:
            if step.status != "queued" or any(
                next((item.status for item in self.steps if item.step_id == dependency), "blocked") != "succeeded"
                for dependency in step.depends_on
            ):
                continue
            if any(attempt.status == "running" for attempt in step.attempts):
                continue
            queued_attempt = next(
                (attempt for attempt in reversed(step.attempts) if attempt.status == "queued"),
                None,
            )
            if queued_attempt is not None:
                queued_attempt.status = "running"
                queued_attempt.claimed_from_queue = True
            step.status = "running"
            return step
        for step in self.steps:
            if step.status == "running" and any(
                item.status == "running" and item.lease_expired for item in step.attempts
            ):
                return step
        return None

    def save_attempt(self, attempt: WorkAttempt) -> None:
        self.attempts.append(attempt)
        step = next(item for item in self.steps if item.step_id == attempt.step_id)
        step.attempts.append(attempt)

    def update_step(self, step: WorkStep) -> None:
        return None


def process_next_work_generation(
    store: WorkGenerationStore,
    provider: FakeWorkProvider | None = None,
) -> bool:
    step = store.claim_next_step()
    if step is None:
        return False
    provider = provider or FakeWorkProvider()
    previous_failures = sum(1 for attempt in step.attempts if attempt.status == "failed")
    attempt = next((item for item in step.attempts if item.status == "running"), None)
    if attempt is None:
        attempt = WorkAttempt(
            str(uuid.uuid4()),
            step.step_id,
            max((item.attempt_no for item in step.attempts), default=0) + 1,
            "running",
            provider_cancel_supported=provider.supports_cancel,
        )
        store.save_attempt(attempt)
        if not any(item.attempt_id == attempt.attempt_id for item in step.attempts):
            step.attempts.append(attempt)
    elif not attempt.upstream_task_id and not attempt.claimed_from_queue:
        attempt.status = "waiting_manual"
        attempt.error_category = "unknown_submission"
        attempt.error_summary = "Worker 恢复时无法确认上游是否已创建任务"
        step.status = "waiting_manual"
        store.update_step(step)
        return True
    attempt.claimed_from_queue = False
    try:
        bind_step = getattr(provider, "bind_step", None)
        if bind_step is not None:
            bind_step(step)
        attempt.provider_cancel_supported = provider.supports_cancel
        if step.cancel_requested:
            if not attempt.upstream_task_id:
                raise UnknownSubmissionError("取消时缺少可恢复的上游任务 ID")
            if not provider.supports_cancel:
                raise RuntimeError("当前 provider 不支持运行中取消")
            if attempt.cancel_requested_at is None:
                cancel_result = provider.cancel(attempt.upstream_task_id)
                attempt.cancel_response = str(cancel_result)
                attempt.cancel_requested_at = datetime.now(timezone.utc)
                attempt.status = (
                    str(cancel_result)
                    if str(cancel_result) in {"cancelled", "failed", "succeeded"}
                    else provider.query(attempt.upstream_task_id)
                )
            else:
                attempt.status = provider.query(attempt.upstream_task_id)
        elif attempt.upstream_task_id:
            attempt.status = provider.query(attempt.upstream_task_id)
        else:
            attempt.upstream_task_id = provider.submit(step)
            submission_audit = getattr(provider, "submission_audit", None)
            if submission_audit is not None:
                attempt.input_snapshot = submission_audit()
            persist_upstream_task = getattr(store, "persist_upstream_task", None)
            if persist_upstream_task is not None:
                persist_upstream_task(attempt)
            attempt.status = provider.query(attempt.upstream_task_id)
        output_audit = getattr(provider, "output_audit", None)
        if output_audit is not None:
            attempt.output_snapshot = output_audit()
    except PollingTemporaryError as error:
        attempt.status = "running"
        attempt.error_category = "polling_temporary"
        attempt.error_code = str(getattr(error, "code", "polling_temporary"))
        attempt.error_summary = str(error)
        step.status = "running"
    except TemporaryWorkProviderError as error:
        if previous_failures < 1:
            attempt.status = "failed"
            attempt.error_category = "temporary"
            attempt.error_code = str(getattr(error, "code", "temporary"))
            attempt.error_summary = str(error)
            step.status = "queued"
        else:
            attempt.status = "waiting_manual"
            attempt.error_category = "retry_exhausted"
            attempt.error_code = str(getattr(error, "code", "retry_exhausted"))
            attempt.error_summary = str(error)
            step.status = "waiting_manual"
    except UnknownSubmissionError as error:
        attempt.status = "waiting_manual"
        attempt.error_category = "unknown_submission"
        attempt.error_code = "unknown_submission"
        attempt.error_summary = str(error)
        step.status = "waiting_manual"
    except Exception as error:  # pragma: no cover - provider boundary
        attempt.status = "failed"
        attempt.error_category = "provider"
        attempt.error_code = str(getattr(error, "code", "provider_error"))
        attempt.error_summary = str(error)
        step.status = "failed"
    else:
        if attempt.status in {"queued", "running"}:
            step.status = "running"
        elif attempt.status == "succeeded":
            step.status = "succeeded"
        elif attempt.status == "cancelled":
            step.status = "cancelled"
        elif attempt.status == "waiting_manual":
            step.status = "waiting_manual"
        else:
            step.status = "failed"
    submission_audit = getattr(provider, "submission_audit", None)
    if submission_audit is not None:
        snapshot = submission_audit()
        if snapshot:
            attempt.input_snapshot = snapshot
    output_audit = getattr(provider, "output_audit", None)
    if output_audit is not None:
        snapshot = output_audit()
        if snapshot:
            attempt.output_snapshot = snapshot
    if step.status == "succeeded" and step.step_type == "video_segment":
        materialize_segment = getattr(store, "materialize_video_segment_artifact", None)
        if materialize_segment is not None:
            try:
                materialize_segment(step)
            except Exception as error:  # pragma: no cover - filesystem/provider boundary
                step.status = "failed"
                step.attempts[-1].status = "failed"
                step.attempts[-1].error_category = "artifact_materialization"
                step.attempts[-1].error_summary = str(error)
                step.error_category = "artifact_materialization"
                step.error_summary = str(error)
    if step.status == "succeeded" and step.step_type == "compose":
        materialize = getattr(store, "materialize_compose_artifact", None)
        if materialize is None:
            step.status = "failed"
            step.attempts[-1].status = "failed"
            step.attempts[-1].error_category = "artifact_missing"
            step.attempts[-1].error_summary = "合成步骤完成但未登记成品素材"
            step.error_category = "artifact_missing"
            step.error_summary = "合成步骤完成但未登记成品素材"
        else:
            try:
                materialize(step)
            except Exception as error:  # pragma: no cover - filesystem/provider boundary
                step.status = "failed"
                step.attempts[-1].status = "failed"
                step.attempts[-1].error_category = "artifact_materialization"
                step.attempts[-1].error_summary = str(error)
                step.error_category = "artifact_materialization"
                step.error_summary = str(error)
    store.update_step(step)
    return True


class PostgresWorkGenerationStore:
    def __init__(self, database_url: str, *, real_mode: bool = False):
        self.database_url = database_url
        self.worker_id = f"work-generation-{uuid.uuid4()}"
        self.real_mode = real_mode

    def _connect(self):
        import psycopg
        from psycopg.rows import dict_row

        return psycopg.connect(self.database_url, row_factory=dict_row)

    def claim_next_step(self) -> WorkStep | None:
        with self._connect() as connection, connection.transaction():
            row = connection.execute(
                WORK_GENERATION_CLAIM_SQL
            ).fetchone()
            if not row:
                return None
            connection.execute("UPDATE work_generation_steps SET status='running' WHERE id=%s", (row["id"],))
            attempt_rows = connection.execute(
                """SELECT id, attempt_no, status, upstream_task_id, error_category,
                          error_code, error_summary, provider_cancel_supported, cancel_requested_at,
                          cancel_response, input_snapshot, output_snapshot,
                          status='running' AND lease_expires_at < NOW() AS lease_expired
                   FROM work_generation_attempts
                   WHERE step_id=%s ORDER BY attempt_no""",
                (row["id"],),
            ).fetchall()
            attempts = [
                WorkAttempt(
                    str(item["id"]),
                    str(row["id"]),
                    item["attempt_no"],
                    item["status"],
                    item["upstream_task_id"],
                    item["error_category"],
                    item["error_summary"],
                    item["provider_cancel_supported"],
                    item["cancel_requested_at"],
                    item["cancel_response"],
                    item["lease_expired"],
                    False,
                    dict(item.get("input_snapshot") or {}),
                    dict(item.get("output_snapshot") or {}),
                    item.get("error_code"),
                )
                for item in attempt_rows
            ]
            queued_attempt = next(
                (attempt for attempt in reversed(attempts) if attempt.status == "queued"),
                None,
            )
            if queued_attempt is not None:
                queued_attempt.status = "running"
                queued_attempt.claimed_from_queue = True
                connection.execute(
                    "UPDATE work_generation_attempts SET status='running', lease_owner=%s, lease_expires_at=NOW()+INTERVAL '2 minutes' WHERE id=%s AND status='queued'",
                    (self.worker_id, queued_attempt.attempt_id),
                )
            else:
                running_attempt = next(
                    (attempt for attempt in reversed(attempts) if attempt.status == "running"),
                    None,
                )
                if running_attempt is not None:
                    connection.execute(
                        "UPDATE work_generation_attempts SET lease_owner=%s, lease_expires_at=NOW()+INTERVAL '2 minutes' WHERE id=%s",
                        (self.worker_id, running_attempt.attempt_id),
                    )
            return WorkStep(
                step_id=str(row["id"]),
                run_id=str(row["run_id"]),
                step_type=row["step_type"],
                status="running",
                depends_on=[str(item) for item in row["depends_on"]],
                attempts=attempts,
                cancel_requested=row["run_status"] == "cancelling",
                result_material_ids=list(row.get("result_material_ids") or []),
                output_snapshot=dict(row.get("output_snapshot") or {}),
                input_snapshot=dict(row.get("input_snapshot") or {}),
                model_snapshot=dict(row.get("step_model_snapshot") or {}),
                resource_usage=dict(row.get("step_resource_usage") or {}),
                run_model_snapshot=dict(row.get("run_model_snapshot") or {}),
                capability_snapshot=dict(row.get("capability_snapshot") or {}),
                voice_snapshot=dict(row.get("voice_snapshot") or {}),
                prompt_snapshot=dict(row.get("prompt_snapshot") or {}),
                timeline_snapshot=dict(row.get("timeline_snapshot") or {}),
                parameter_snapshot=dict(row.get("parameter_snapshot") or {}),
                run_resource_usage=dict(row.get("run_resource_usage") or {}),
                work_input_snapshot=dict(row.get("work_input_snapshot") or {}),
                work_id=str(row["work_id"]),
                work_version_id=str(row["work_version_id"]),
                project_id=str(row["project_id"]),
                work_title=str(row["work_title"]),
                asr_step_count=int(row.get("asr_step_count") or 0),
                voice_languages=list(row.get("voice_languages") or []),
                voice_catalog_version=(int(row["voice_catalog_version"]) if row.get("voice_catalog_version") is not None else None),
                voice_available=bool(row.get("voice_available")),
            )

    def materialize_video_segment_artifact(self, step: WorkStep) -> None:
        """fake 模式生成可复验分段；real 模式已由 provider 登记时保持幂等。"""
        if step.result_material_ids:
            return
        if self.real_mode:
            raise RuntimeError("真实 video_segment 成功但未登记正式产物")
        duration = int(step.input_snapshot.get("duration_seconds") or 0)
        if not 4 <= duration <= 30:
            raise RuntimeError("fake 视频分段时长无效")
        parameters = step.parameter_snapshot.get("output")
        output = parameters if isinstance(parameters, dict) else step.parameter_snapshot
        width, height = _fake_output_size(
            str(output.get("aspect_ratio") or "16:9"),
            str(output.get("resolution") or "1080p"),
        )
        material_id = uuid.uuid4()
        root = os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")
        directory = os.path.join(root, "generated", "artifacts", str(step.project_id))
        os.makedirs(directory, exist_ok=True)
        file_name = f"{material_id}.mp4"
        absolute_path = os.path.join(directory, file_name)
        required_scenes = {
            str(scene_id) for scene_id in (step.input_snapshot.get("scene_ids") or [])
        }
        snapshot = dict(step.work_input_snapshot)
        snapshot["scenes"] = [
            scene
            for scene in (step.work_input_snapshot.get("scenes") or [])
            if str(scene.get("scene_id")) in required_scenes
        ]
        image_paths = _resolve_scene_images(snapshot, root)
        command = (
            _build_slideshow_command(image_paths, absolute_path, width, height, duration)
            if image_paths
            else [
                "ffmpeg", "-y", "-f", "lavfi", "-i",
                f"testsrc2=size={width}x{height}:rate=25:duration={duration}",
                "-an", "-c:v", "libx264", "-preset", "veryfast", "-crf", "30",
                "-pix_fmt", "yuv420p", "-movflags", "+faststart", absolute_path,
            ]
        )
        subprocess.run(command, check=True, capture_output=True)
        public_url = f"/assets/generated/artifacts/{step.project_id}/{file_name}"
        self.register_generated_material(
            step,
            material_id=material_id,
            material_type="video",
            artifact_role="video_segment",
            file_url=public_url,
            file_name=f"{step.work_title} 视频片段.mp4",
            file_path=absolute_path,
            media_metadata={
                "duration_ms": duration * 1000,
                "width": width,
                "height": height,
            },
            tags=["作品生成", "视频片段", "fake"],
        )

    def materialize_compose_artifact(self, step: WorkStep) -> None:
        """按当前显式模式合成作品；真实模式禁止回退 fake 幻灯片。"""
        if self.real_mode:
            self._materialize_real_compose_artifact(step)
            return
        with self._connect() as connection:
            context = connection.execute(
                """SELECT r.id AS run_id, r.work_id, r.work_version_id,
                          w.project_id, r.timeline_snapshot, r.parameter_snapshot,
                          wv.input_snapshot, w.title
                   FROM work_generation_runs r
                   JOIN works w ON w.id=r.work_id
                   JOIN work_versions wv ON wv.id=r.work_version_id
                   WHERE r.id=(SELECT run_id FROM work_generation_steps WHERE id=%s)""",
                (step.step_id,),
            ).fetchone()
            if not context:
                raise RuntimeError("合成步骤所属运行不存在")
            existing = connection.execute(
                """SELECT id,file_url,metadata FROM materials
                   WHERE metadata->>'source'='work_generation'
                     AND metadata->>'generation_step_id'=%s
                     AND metadata->>'artifact_role'='final_video'
                   LIMIT 1""",
                (step.step_id,),
            ).fetchone()
            if existing:
                root = os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")
                existing_path = _local_asset_path(str(existing["file_url"]), root)
                if not os.path.isfile(existing_path) or os.path.getsize(existing_path) <= 0:
                    raise RuntimeError("既有 fake 成片文件不存在或为空")
                existing_metadata = dict(existing["metadata"] or {})
                existing_metadata["generation_attempt_id"] = step.attempts[-1].attempt_id
                work_artifact_id = self._register_work_artifact(
                    connection,
                    step=step,
                    material_id=existing["id"],
                    role="final_video",
                    file_name=os.path.basename(existing_path),
                    storage_path=str(existing["file_url"]),
                    mime_type="video/mp4",
                    size_bytes=os.path.getsize(existing_path),
                    sha256=_sha256_file(existing_path),
                    metadata=existing_metadata,
                )
                step.result_material_ids = [str(existing["id"])]
                step.output_snapshot = {
                    "material_id": str(existing["id"]),
                    "file_url": str(existing["file_url"]),
                    "artifact_role": "final_video",
                    "work_artifact_id": str(work_artifact_id),
                }
                return

            timeline = context["timeline_snapshot"] or {}
            parameters = context["parameter_snapshot"] or {}
            duration = int(timeline.get("duration_seconds") or 15)
            duration = max(4, min(60, duration))
            aspect_ratio = str(parameters.get("aspect_ratio") or "16:9")
            resolution = str(parameters.get("resolution") or "1080p")
            width, height = _fake_output_size(aspect_ratio, resolution)
            artifact_id = uuid.uuid4()
            root = os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")
            directory = os.path.join(root, "generated", "artifacts", str(context["project_id"]))
            os.makedirs(directory, exist_ok=True)
            file_name = f"{artifact_id}.mp4"
            absolute_path = os.path.join(directory, file_name)
            image_paths = _resolve_scene_images(context["input_snapshot"] or {}, root)
            command = _build_slideshow_command(image_paths, absolute_path, width, height, duration) if image_paths else [
                "ffmpeg", "-y", "-f", "lavfi", "-i", f"testsrc2=size={width}x{height}:rate=25:duration={duration}",
                "-f", "lavfi", "-i", f"anullsrc=channel_layout=stereo:sample_rate=48000",
                "-t", str(duration), "-c:v", "libx264", "-preset", "veryfast", "-crf", "30", "-pix_fmt", "yuv420p", "-c:a", "aac",
                "-shortest", "-movflags", "+faststart", absolute_path,
            ]
            subprocess.run(command, check=True, capture_output=True)
            thumbnail_name = f"{artifact_id}.jpg"
            thumbnail_path = os.path.join(directory, thumbnail_name)
            subprocess.run(
                [
                    "ffmpeg", "-y", "-ss", "0", "-i", absolute_path,
                    "-frames:v", "1", "-vf", "scale=640:-2", thumbnail_path,
                ],
                check=True,
                capture_output=True,
            )
            thumbnail_url = f"/assets/generated/artifacts/{context['project_id']}/{thumbnail_name}"
            metadata = {
                "source": "work_generation",
                "storage_provider": "local",
                "artifact_role": "final_video",
                "work_id": str(context["work_id"]),
                "work_version_id": str(context["work_version_id"]),
                "generation_run_id": str(context["run_id"]),
                "generation_step_id": step.step_id,
                "generation_attempt_id": step.attempts[-1].attempt_id,
                "model_snapshot": {},
                "voice_snapshot": dict((timeline.get("voice_snapshot") or {})),
                "prompt_snapshot": {},
                "timeline_snapshot": timeline,
                "resource_usage": {"video_seconds": duration},
                "mime_type": "video/mp4",
                "format": "mp4",
                "file_size_bytes": os.path.getsize(absolute_path),
                "sha256": _sha256_file(absolute_path),
                "duration_sec": duration,
                "width": width,
                "height": height,
                "thumbnail_url": thumbnail_url,
            }
            material = connection.execute(
                """INSERT INTO materials
                   (id, project_id, material_type, file_url, file_name, tags, metadata, status)
                   VALUES (%s,%s,'video',%s,%s,%s,%s,'active')
                   RETURNING id""",
                (
                    artifact_id,
                    context["project_id"],
                    f"/assets/generated/artifacts/{context['project_id']}/{file_name}",
                    f"{context['title']} 成片.mp4",
                    ["作品生成", "成片"],
                    json.dumps(metadata),
                ),
            ).fetchone()
            work_artifact_id = self._register_work_artifact(
                connection,
                step=step,
                material_id=material["id"],
                role="final_video",
                file_name=file_name,
                storage_path=f"/assets/generated/artifacts/{context['project_id']}/{file_name}",
                mime_type="video/mp4",
                size_bytes=os.path.getsize(absolute_path),
                sha256=_sha256_file(absolute_path),
                metadata=metadata,
            )
            connection.commit()
            step.result_material_ids = [str(material["id"])]
            step.output_snapshot = {
                "material_id": str(material["id"]),
                "file_url": f"/assets/generated/artifacts/{context['project_id']}/{file_name}",
                "artifact_role": "final_video",
                "work_artifact_id": str(work_artifact_id),
                "thumbnail_url": thumbnail_url,
            }

    def register_generated_material(
        self,
        step: WorkStep,
        *,
        material_id,
        material_type: str,
        artifact_role: str,
        file_url: str,
        file_name: str,
        file_path,
        media_metadata: dict[str, object],
        tags: list[str],
    ) -> None:
        """同事务登记 Material 与 WorkArtifact，避免媒体存在但 provenance 缺失。"""
        if not step.work_id or not step.work_version_id:
            raise WorkGenerationConfigurationError("生成步骤缺少锁定 Work/WorkVersion")
        if not os.path.isfile(file_path) or os.path.getsize(file_path) <= 0:
            raise RuntimeError("真实生成产物文件不存在或为空")
        work_role, mime_type = _work_artifact_identity(artifact_role, str(file_path))
        size_bytes = os.path.getsize(file_path)
        sha256 = _sha256_file(file_path)
        attempt_id = next(
            (
                attempt.attempt_id
                for attempt in reversed(step.attempts)
                if attempt.status in {"running", "succeeded"}
            ),
            None,
        )
        if attempt_id is None:
            raise WorkGenerationConfigurationError("生成步骤缺少当前 provider attempt")
        with self._connect() as connection, connection.transaction():
            existing = connection.execute(
                """SELECT id, file_url FROM materials
                   WHERE metadata->>'source'='work_generation'
                     AND metadata->>'generation_step_id'=%s
                     AND metadata->>'artifact_role'=%s
                   LIMIT 1""",
                (step.step_id, artifact_role),
            ).fetchone()
            metadata = {
                "source": "work_generation",
                "storage_provider": "local",
                "artifact_role": artifact_role,
                "work_id": step.work_id,
                "work_version_id": step.work_version_id,
                "generation_run_id": step.run_id,
                "generation_step_id": step.step_id,
                "generation_attempt_id": attempt_id,
                "model_snapshot": step.model_snapshot,
                "voice_snapshot": step.voice_snapshot,
                "prompt_snapshot": step.prompt_snapshot,
                "timeline_snapshot": step.timeline_snapshot,
                "resource_usage": step.resource_usage,
                "mime_type": mime_type,
                "file_size_bytes": size_bytes,
                "sha256": sha256,
                **media_metadata,
            }
            if material_type == "audio":
                metadata["audio_usage"] = "tts" if artifact_role == "tts_audio" else "other"
            if material_type == "subtitle":
                source = connection.execute(
                    """SELECT dependency.step_type,material.id
                       FROM work_generation_steps dependency
                       JOIN LATERAL jsonb_array_elements_text(dependency.result_material_ids)
                         result(value) ON TRUE
                       JOIN materials material ON material.id=result.value::uuid
                       WHERE dependency.id=ANY(%s::uuid[])
                       ORDER BY dependency.step_no,material.created_at LIMIT 1""",
                    (step.depends_on,),
                ).fetchone()
                if source is None:
                    raise WorkGenerationConfigurationError("字幕产物缺少正式来源音频")
                metadata["alignment_source"] = (
                    "asr" if source["step_type"] == "asr" else "tts_timestamp"
                )
                metadata["source_audio_material_id"] = str(source["id"])
            if existing:
                material_id = existing["id"]
                file_url = str(existing["file_url"])
            else:
                connection.execute(
                    """INSERT INTO materials
                       (id, project_id, material_type, file_url, file_name, tags, metadata, status)
                       VALUES (%s,%s,%s,%s,%s,%s,%s,'active')""",
                    (
                        material_id,
                        step.project_id,
                        material_type,
                        file_url,
                        file_name,
                        tags,
                        json.dumps(metadata),
                    ),
                )
            work_artifact_id = self._register_work_artifact(
                connection,
                step=step,
                material_id=material_id,
                role=work_role,
                file_name=os.path.basename(str(file_path)),
                storage_path=file_url,
                mime_type=mime_type,
                size_bytes=size_bytes,
                sha256=sha256,
                metadata=metadata,
            )
            step.result_material_ids = [str(material_id)]
            step.output_snapshot = {
                "material_id": str(material_id),
                "file_url": file_url,
                "artifact_role": artifact_role,
                "work_artifact_id": str(work_artifact_id),
                "sha256": sha256,
            }

    def _register_work_artifact(
        self,
        connection,
        *,
        step: WorkStep,
        material_id,
        role: str,
        file_name: str,
        storage_path: str,
        mime_type: str,
        size_bytes: int,
        sha256: str,
        metadata: dict[str, object],
    ):
        connection.execute(
            """INSERT INTO work_artifacts
               (work_version_id,role,material_id,generation_step_id,file_name,
                storage_path,mime_type,size_bytes,sha256,metadata)
               VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)
               ON CONFLICT (work_version_id,role,file_name) DO NOTHING""",
            (
                step.work_version_id,
                role,
                material_id,
                step.step_id,
                file_name,
                storage_path,
                mime_type,
                size_bytes,
                sha256,
                json.dumps(metadata),
            ),
        )
        artifact = connection.execute(
            """SELECT id,material_id,generation_step_id,storage_path,mime_type,size_bytes,sha256
               FROM work_artifacts
               WHERE work_version_id=%s AND role=%s AND file_name=%s""",
            (step.work_version_id, role, file_name),
        ).fetchone()
        expected = (
            str(material_id),
            step.step_id,
            storage_path,
            mime_type,
            size_bytes,
            sha256,
        )
        actual = (
            str(artifact["material_id"]) if artifact else None,
            str(artifact["generation_step_id"]) if artifact else None,
            str(artifact["storage_path"]) if artifact else None,
            str(artifact["mime_type"]) if artifact else None,
            int(artifact["size_bytes"]) if artifact else None,
            str(artifact["sha256"]) if artifact else None,
        )
        if actual != expected:
            raise RuntimeError("WorkArtifact 幂等身份与当前生成事实冲突")
        return artifact["id"]

    def _materialize_real_compose_artifact(self, step: WorkStep) -> None:
        from pathlib import Path

        from video_worker.real_work_generation import (
            LocalWorkGenerationStorage,
            build_real_compose_command,
            build_real_silent_compose_command,
        )

        storage = LocalWorkGenerationStorage(
            Path(os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")),
            os.getenv("ASSET_PUBLIC_PREFIX", "/assets"),
        )
        with self._connect() as connection:
            rows = connection.execute(
                """SELECT s.step_type, m.id, m.file_url, m.metadata
                   FROM work_generation_steps s
                   JOIN LATERAL jsonb_array_elements_text(s.result_material_ids) rid(value) ON TRUE
                   JOIN materials m ON m.id=rid.value::uuid
                   WHERE s.run_id=%s AND s.status='succeeded'
                   ORDER BY s.step_no""",
                (step.run_id,),
            ).fetchall()
        videos = [storage.source_path(str(row["file_url"])) for row in rows if row["step_type"] == "video_segment"]
        audio = next(
            (storage.source_path(str(row["file_url"])) for row in rows if row["step_type"] == "tts"),
            None,
        )
        subtitle = next(
            (storage.source_path(str(row["file_url"])) for row in rows if row["step_type"] == "subtitle"),
            None,
        )
        audio_mode = str(step.timeline_snapshot.get("audio_mode") or "independent_tts")
        if not videos:
            raise RuntimeError("真实合成缺少视频素材")
        if audio_mode != "silent" and (audio is None or subtitle is None):
            raise RuntimeError("真实合成缺少 TTS 或字幕素材")
        duration = int(step.timeline_snapshot.get("duration_seconds") or 0)
        if not 4 <= duration <= 60:
            raise RuntimeError("真实合成时长无效")
        output_path, output_url, material_id = storage.artifact_path(step, "final_video", "mp4")
        command = (
            build_real_silent_compose_command(videos, output_path, duration)
            if audio_mode == "silent"
            else build_real_compose_command(videos, audio, subtitle, output_path, duration)
        )
        subprocess.run(command, check=True, capture_output=True)
        thumbnail_path, thumbnail_url, _ = storage.artifact_path(step, "final_thumbnail", "jpg")
        subprocess.run(
            ["ffmpeg", "-y", "-ss", "0", "-i", str(output_path), "-frames:v", "1", "-vf", "scale=640:-2", str(thumbnail_path)],
            check=True,
            capture_output=True,
        )
        metadata = {
            "mime_type": "video/mp4",
            "format": "mp4",
            "duration_sec": duration,
            "file_size_bytes": os.path.getsize(output_path),
            "thumbnail_url": thumbnail_url,
            "audio_mode": audio_mode,
        }
        self.register_generated_material(
            step,
            material_id=material_id,
            material_type="video",
            artifact_role="final_video",
            file_url=output_url,
            file_name=f"{step.work_title} 成片.mp4",
            file_path=output_path,
            media_metadata=metadata,
            tags=["作品生成", "成片"],
        )
        step.output_snapshot["thumbnail_url"] = thumbnail_url

    def save_attempt(self, attempt: WorkAttempt) -> None:
        with self._connect() as connection, connection.transaction():
            connection.execute(
                """INSERT INTO work_generation_attempts
                   (id, step_id, attempt_no, status, upstream_task_id, error_category,
                    error_code, error_summary, provider_cancel_supported, lease_owner, lease_expires_at)
                   VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,NOW()+INTERVAL '2 minutes')""",
                (
                    attempt.attempt_id,
                    attempt.step_id,
                    attempt.attempt_no,
                    attempt.status,
                    attempt.upstream_task_id,
                    attempt.error_category,
                    attempt.error_code,
                    attempt.error_summary,
                    attempt.provider_cancel_supported,
                    self.worker_id,
                ),
            )

    def persist_upstream_task(self, attempt: WorkAttempt) -> None:
        """创建响应一旦返回 task ID，必须先单独提交事务再发起首次查询。"""
        if not attempt.upstream_task_id:
            raise ValueError("上游 task ID 不能为空")
        with self._connect() as connection, connection.transaction():
            updated = connection.execute(
                """UPDATE work_generation_attempts
                   SET upstream_task_id=%s, provider_cancel_supported=%s,
                       input_snapshot=%s, output_snapshot=%s,
                       lease_owner=%s, lease_expires_at=NOW()+INTERVAL '2 minutes'
                   WHERE id=%s AND status='running'
                   RETURNING id""",
                (
                    attempt.upstream_task_id,
                    attempt.provider_cancel_supported,
                    json.dumps(attempt.input_snapshot),
                    json.dumps(attempt.output_snapshot),
                    self.worker_id,
                    attempt.attempt_id,
                ),
            ).fetchone()
            if updated is None:
                raise RuntimeError("无法持久化作品生成上游 task ID")

    def update_step(self, step: WorkStep) -> None:
        with self._connect() as connection, connection.transaction():
            attempt = step.attempts[-1] if step.attempts else None
            if attempt:
                connection.execute(
                    """UPDATE work_generation_attempts
                       SET status=%s, upstream_task_id=%s, error_category=%s,
                           error_code=%s, error_summary=%s, provider_cancel_supported=%s,
                           cancel_requested_at=%s, cancel_response=%s,
                           input_snapshot=%s, output_snapshot=%s,
                           lease_owner=CASE WHEN %s='running' THEN %s ELSE NULL END,
                           lease_expires_at=CASE WHEN %s='running' THEN NOW()+INTERVAL '2 minutes' ELSE NULL END
                       WHERE id=%s""",
                    (
                        attempt.status,
                        attempt.upstream_task_id,
                        attempt.error_category,
                        attempt.error_code,
                        attempt.error_summary,
                        attempt.provider_cancel_supported,
                        attempt.cancel_requested_at,
                        attempt.cancel_response,
                        json.dumps(attempt.input_snapshot),
                        json.dumps(attempt.output_snapshot),
                        attempt.status,
                        self.worker_id,
                        attempt.status,
                        attempt.attempt_id,
                    ),
                )
            connection.execute("UPDATE work_generation_steps SET status=%s, external_task_id=%s, error_category=%s, error_code=%s, error_summary=%s, result_material_ids=%s, output_snapshot=%s WHERE id=%s", (step.status, step.attempts[-1].upstream_task_id if step.attempts else None, step.attempts[-1].error_category if step.attempts else None, step.attempts[-1].error_code if step.attempts else None, step.attempts[-1].error_summary if step.attempts else None, json.dumps(step.result_material_ids), json.dumps(step.output_snapshot), step.step_id))


def default_process_next_work_generation() -> bool:
    fake_enabled = os.getenv("WORK_GENERATION_FAKE_PROVIDER_ENABLED", "false").lower() == "true"
    real_enabled = os.getenv("WORK_GENERATION_REAL_PROVIDER_ENABLED", "false").lower() == "true"
    validate_work_generation_mode(
        fake_enabled=fake_enabled,
        real_enabled=real_enabled,
        worker_enabled=True,
    )
    database_url = os.getenv("DATABASE_URL", "postgres://postgres:postgres@biga-postgres:5432/video_agent")
    if not fake_enabled and not real_enabled:
        return False
    store = PostgresWorkGenerationStore(database_url, real_mode=real_enabled)
    if fake_enabled:
        return process_next_work_generation(store, FakeWorkProvider())
    from pathlib import Path

    from video_worker.model_registry import PostgresModelRegistry
    from video_worker.real_work_generation import LocalWorkGenerationStorage, RealWorkProvider

    provider = RealWorkProvider(
        store,
        PostgresModelRegistry(database_url),
        RealWorkGenerationLimits.from_environment(),
        LocalWorkGenerationStorage(
            Path(os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")),
            os.getenv("ASSET_PUBLIC_PREFIX", "/assets"),
        ),
    )
    return process_next_work_generation(store, provider)


def _fake_output_size(aspect_ratio: str, resolution: str) -> tuple[int, int]:
    long_edge = 1920 if resolution == "1080p" else 1280
    if aspect_ratio == "9:16":
        return (long_edge * 9 // 16, long_edge)
    if aspect_ratio == "1:1":
        return (long_edge, long_edge)
    return (long_edge, long_edge * 9 // 16)


def _resolve_scene_images(input_snapshot: dict[str, object], storage_root: str) -> list[tuple[str, int]]:
    scenes = input_snapshot.get("scenes")
    if not isinstance(scenes, list):
        return []
    resolved: list[tuple[str, int]] = []
    for scene in scenes:
        if not isinstance(scene, dict):
            continue
        image_url = scene.get("image_url")
        if not isinstance(image_url, str) or not image_url.startswith("/assets/"):
            continue
        image_path = os.path.join(storage_root, image_url.removeprefix("/assets/"))
        if os.path.isfile(image_path):
            try:
                seconds = max(1, int(scene.get("duration_seconds") or 1))
            except (TypeError, ValueError):
                seconds = 1
            resolved.append((image_path, seconds))
    return resolved


def _build_slideshow_command(images: list[tuple[str, int]], output_path: str, width: int, height: int, duration: int) -> list[str]:
    total = sum(seconds for _, seconds in images) or 1
    durations = [max(1, round(seconds * duration / total)) for _, seconds in images]
    durations[-1] += duration - sum(durations)
    command = ["ffmpeg", "-y"]
    for (path, _), seconds in zip(images, durations):
        command.extend(["-loop", "1", "-t", str(seconds), "-i", path])
    filters = []
    for index in range(len(images)):
        filters.append(f"[{index}:v]scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1[v{index}]")
    filters.append("".join(f"[v{index}]" for index in range(len(images))) + f"concat=n={len(images)}:v=1:a=0[vout]")
    command.extend(["-f", "lavfi", "-i", "anullsrc=channel_layout=stereo:sample_rate=48000", "-filter_complex", ";".join(filters), "-map", "[vout]", "-map", f"{len(images)}:a:0", "-t", str(duration), "-c:v", "libx264", "-preset", "veryfast", "-crf", "30", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", "-movflags", "+faststart", output_path])
    return command
