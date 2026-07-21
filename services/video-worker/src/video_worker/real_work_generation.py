"""真实作品生成执行器；所有收费提交都受运行级硬限制约束。"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import uuid
from pathlib import Path
from urllib import request as urllib_request
from urllib.parse import urlsplit
from uuid import UUID

from video_worker.model_registry import PostgresModelRegistry
from video_worker.seedance import (
    ArkSeedanceProvider,
    SeedanceImageInput,
    SeedanceProviderError,
    SeedanceRequest,
)
from video_worker.speech_generation import (
    SpeechProviderError,
    TtsSynthesisRequest,
    VolcengineTtsV3Provider,
    inspect_audio_file,
)
from video_worker.tos_staging import TosMediaStaging, TosStagingConfig
from video_worker.work_generation import (
    PollingTemporaryError,
    RealWorkGenerationLimits,
    UnknownSubmissionError,
    WorkGenerationConfigurationError,
    WorkStep,
)


class LocalWorkGenerationStorage:
    def __init__(self, root: Path, public_prefix: str = "/assets") -> None:
        self.root = root.resolve()
        self.public_prefix = public_prefix.rstrip("/")

    def source_path(self, file_url: str) -> Path:
        prefix = f"{self.public_prefix}/"
        if not file_url.startswith(prefix):
            raise WorkGenerationConfigurationError("参考素材不属于自管存储")
        relative = Path(file_url[len(prefix) :])
        if relative.is_absolute() or ".." in relative.parts:
            raise WorkGenerationConfigurationError("参考素材路径无效")
        path = (self.root / relative).resolve()
        if self.root not in path.parents or not path.is_file():
            raise WorkGenerationConfigurationError("参考素材缺失或路径越界")
        return path

    def artifact_path(self, step: WorkStep, role: str, extension: str) -> tuple[Path, str, UUID]:
        if not step.project_id:
            raise WorkGenerationConfigurationError("作品步骤缺少项目 ID")
        material_id = uuid.uuid5(uuid.NAMESPACE_URL, f"novex:{role}:{step.step_id}")
        safe_extension = extension.lower().lstrip(".")
        path = self.root / "generated" / "artifacts" / step.project_id / f"{material_id}.{safe_extension}"
        path.parent.mkdir(parents=True, exist_ok=True)
        public_url = f"{self.public_prefix}/generated/artifacts/{step.project_id}/{path.name}"
        return path, public_url, material_id

    @staticmethod
    def atomic_write(path: Path, content: bytes) -> None:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
            handle.write(content)
            temporary = Path(handle.name)
        os.replace(temporary, path)


class RealWorkProvider:
    supports_cancel = False

    def __init__(
        self,
        store,
        registry: PostgresModelRegistry,
        limits: RealWorkGenerationLimits,
        storage: LocalWorkGenerationStorage,
        *,
        seedance_factory=None,
        tts_factory=None,
        tos_factory=None,
        signed_url_reader=None,
        video_downloader=None,
    ) -> None:
        self.store = store
        self.registry = registry
        self.limits = limits
        self.storage = storage
        self.seedance_factory = seedance_factory or _seedance_from_config
        self.tts_factory = tts_factory or _tts_from_config
        self.tos_factory = tos_factory or _tos_from_config
        self.signed_url_reader = signed_url_reader or _read_signed_url
        self.video_downloader = video_downloader or _download_video
        self.step: WorkStep | None = None
        self._seedance = None
        self._submission_audit: dict[str, object] = {}
        self._output_audit: dict[str, object] = {}

    def bind_step(self, step: WorkStep) -> None:
        self.limits.validate_step(step)
        if step.step_type == "asr":
            raise WorkGenerationConfigurationError("真实验证禁止 ASR 调用")
        self.step = step
        self._seedance = None
        self.supports_cancel = step.step_type == "video_segment"
        self._submission_audit = {}
        self._output_audit = {}

    def submission_audit(self) -> dict[str, object]:
        return dict(self._submission_audit)

    def output_audit(self) -> dict[str, object]:
        return dict(self._output_audit)

    def submit(self, step: WorkStep) -> str:
        if step is not self.step:
            self.bind_step(step)
        if step.step_type == "video_segment":
            return self._submit_video(step)
        if step.step_type == "tts":
            self._run_tts(step)
            return f"local-tts-{step.step_id}"
        if step.step_type == "subtitle":
            self._run_subtitle(step)
            return f"local-subtitle-{step.step_id}"
        if step.step_type in {"mix", "compose"}:
            return f"local-{step.step_type}-{step.step_id}"
        raise WorkGenerationConfigurationError(f"真实 Worker 不支持步骤类型 {step.step_type}")

    def query(self, upstream_task_id: str) -> str:
        step = self._require_step()
        if upstream_task_id.startswith("local-"):
            return "succeeded"
        provider = self._seedance_provider(step)
        try:
            task = provider.get(upstream_task_id)
        except Exception as error:
            raise PollingTemporaryError(
                f"Seedance 查询暂时失败: {error.__class__.__name__}"
            ) from error
        self._output_audit = {
            "provider": "volcengine_ark_video",
            "upstream_task_id": task.task_id,
            "status": task.status,
            "has_video_url": bool(task.output_url),
            "error": task.error,
        }
        if task.status == "succeeded":
            if not task.output_url:
                raise WorkGenerationConfigurationError("Seedance 成功结果缺少 video_url")
            if not step.result_material_ids:
                path, public_url, material_id = self.storage.artifact_path(step, "video_segment", "mp4")
                inspection = self.video_downloader(task.output_url, path, 256 * 1024 * 1024)
                self.store.register_generated_material(
                    step,
                    material_id=material_id,
                    material_type="video",
                    artifact_role="video_segment",
                    file_url=public_url,
                    file_name=f"{step.work_title} 视频片段.mp4",
                    file_path=path,
                    media_metadata=inspection,
                    tags=["作品生成", "视频片段"],
                )
            self._output_audit["material_ids"] = list(step.result_material_ids)
        if task.status in {"succeeded", "failed", "cancelled"}:
            self._cleanup_reference_images(step)
        return task.status

    def cancel(self, upstream_task_id: str) -> str:
        step = self._require_step()
        if step.step_type != "video_segment":
            raise WorkGenerationConfigurationError("当前真实步骤不支持取消")
        try:
            task = self._seedance_provider(step).cancel(upstream_task_id)
        except Exception as error:
            raise PollingTemporaryError(
                f"Seedance 取消暂时失败: {error.__class__.__name__}"
            ) from error
        self._output_audit = {"status": task.status, "upstream_task_id": task.task_id}
        if task.status in {"succeeded", "failed", "cancelled"}:
            self._cleanup_reference_images(step)
        return task.status

    def _submit_video(self, step: WorkStep) -> str:
        config = self._resolve_video(step)
        references = step.input_snapshot.get("reference_image_ids")
        reference_count = len(references) if isinstance(references, list) else 0
        image_roles = _seedance_image_roles(config.upstream_model, reference_count)
        image_urls, staging_audit = self._stage_reference_images(step)
        output = step.parameter_snapshot.get("output")
        parameters = output if isinstance(output, dict) else step.parameter_snapshot
        duration = int(step.input_snapshot.get("duration_seconds") or 0)
        ratio = str(parameters.get("aspect_ratio") or "")
        resolution = str(parameters.get("resolution") or "")
        audio_mode = str(step.timeline_snapshot.get("audio_mode") or "independent_tts")
        request = SeedanceRequest(
            prompt=str(step.input_snapshot.get("prompt") or ""),
            images=[
                SeedanceImageInput(url, role)
                for url, role in zip(image_urls, image_roles, strict=True)
            ],
            duration_seconds=duration,
            aspect_ratio=ratio,
            resolution=resolution,
            generate_audio=_seedance_generate_audio(audio_mode),
            model=config.upstream_model,
        )
        request.validate()
        if ratio not in config.settings["aspect_ratios"] or resolution not in config.settings["resolutions"]:
            raise WorkGenerationConfigurationError("视频输出规格与锁定模型能力不一致")
        self._submission_audit = {
            "provider": "volcengine_ark_video",
            "model": config.snapshot(),
            "duration": duration,
            "ratio": ratio,
            "resolution": resolution,
            "generate_audio": request.generate_audio,
            "reference_images": staging_audit,
            "prompt_sha256": hashlib.sha256(request.prompt.encode()).hexdigest(),
        }
        try:
            task = self._seedance_provider(step).create(request)
        except SeedanceProviderError as error:
            self._output_audit = {
                "provider": "volcengine_ark_video",
                **error.audit_snapshot(),
            }
            if error.code == "unknown_submission":
                raise UnknownSubmissionError(str(error)) from error
            self._cleanup_reference_images(step, staging_audit)
            raise
        return task.task_id

    def _run_tts(self, step: WorkStep) -> None:
        override = step.work_input_snapshot.get("narration_override")
        text = (
            str(override).strip()
            if isinstance(override, str) and override.strip()
            else "".join(
                str(scene.get("narration") or "").strip()
                for scene in _ordered_scenes(step.work_input_snapshot)
            )
        )
        if not text or len(text) > self.limits.max_tts_characters:
            raise WorkGenerationConfigurationError("TTS 文本为空或超过已批准字符上限")
        locked_catalog_version = _required_int(step.voice_snapshot, "catalog_version")
        if not step.voice_available or step.voice_catalog_version != locked_catalog_version:
            raise WorkGenerationConfigurationError("TTS 音色已停用或目录版本发生变化")
        language = _select_tts_language(text, step.voice_languages)
        version = _required_int(step.run_model_snapshot, "tts_registry_version")
        protocol = str(step.run_model_snapshot.get("tts_api_protocol") or "")
        model_id = str(step.run_model_snapshot.get("tts_model_id") or "")
        config = self.registry.resolve_speech(model_id, protocol, version)
        voice_type = str(step.voice_snapshot.get("voice_type") or "").strip()
        if not voice_type:
            raise WorkGenerationConfigurationError("TTS 音色快照缺失")
        audio_format = str(config.settings.get("default_audio_format") or "mp3")
        request = TtsSynthesisRequest(
            request_id=UUID(step.step_id),
            text=text,
            voice_type=voice_type,
            language=language,
            parameters={
                "audio_format": audio_format,
                "sample_rate": int(config.settings.get("default_sample_rate") or 24000),
                "speed_ratio": 1.0,
            },
        )
        self._submission_audit = {
            "provider": config.api_protocol,
            "model": config.snapshot(),
            "voice_type": voice_type,
            "language": language,
            "text_characters": len(text),
            "text_sha256": hashlib.sha256(text.encode()).hexdigest(),
        }
        try:
            result = self.tts_factory(config).synthesize(request)
        except SpeechProviderError as error:
            self._output_audit = {
                "error_code": error.code,
                "retryable": error.retryable,
                "error_details": error.error_details,
                "upstream_log_id": error.upstream_log_id,
            }
            if error.retryable:
                raise UnknownSubmissionError(
                    f"TTS 提交结果不确定: {error.code}"
                ) from error
            raise
        except Exception as error:
            raise UnknownSubmissionError(
                f"TTS 提交结果不确定: {error.__class__.__name__}"
            ) from error
        path, public_url, material_id = self.storage.artifact_path(step, "tts_audio", result.audio_format)
        self.storage.atomic_write(path, result.audio_content)
        inspection = inspect_audio_file(path)
        self.store.register_generated_material(
            step,
            material_id=material_id,
            material_type="audio",
            artifact_role="tts_audio",
            file_url=public_url,
            file_name=f"{step.work_title} 配音.{result.audio_format}",
            file_path=path,
            media_metadata={
                "duration_ms": inspection.duration_ms,
                "audio_codec": inspection.audio_codec,
                "sample_rate_hz": inspection.sample_rate_hz,
                "channel_count": inspection.channel_count,
                "source_sha256": inspection.source_sha256,
            },
            tags=["作品生成", "TTS"],
        )
        self._output_audit = {**result.audit_snapshot(), "material_ids": list(step.result_material_ids)}

    def _run_subtitle(self, step: WorkStep) -> None:
        scenes = _ordered_scenes(step.work_input_snapshot)
        duration_ms = int(step.timeline_snapshot.get("duration_seconds") or 0) * 1000
        override = step.work_input_snapshot.get("narration_override")
        if duration_ms <= 0:
            raise WorkGenerationConfigurationError("字幕时间轴输入缺失")
        if isinstance(override, str) and override.strip():
            scenes = [{"sequence": 1, "narration": override.strip()}]
        if not scenes:
            raise WorkGenerationConfigurationError("字幕文本输入缺失")
        weights = [max(1, len(str(scene.get("narration") or ""))) for scene in scenes]
        total = sum(weights)
        start = 0
        blocks: list[str] = []
        for index, (scene, weight) in enumerate(zip(scenes, weights), start=1):
            end = duration_ms if index == len(scenes) else start + round(duration_ms * weight / total)
            text = str(scene.get("narration") or "").strip()
            blocks.extend([str(index), f"{_srt_time(start)} --> {_srt_time(end)}", text, ""])
            start = end
        content = "\n".join(blocks).encode("utf-8")
        path, public_url, material_id = self.storage.artifact_path(step, "subtitle", "srt")
        self.storage.atomic_write(path, content)
        self.store.register_generated_material(
            step,
            material_id=material_id,
            material_type="subtitle",
            artifact_role="subtitle",
            file_url=public_url,
            file_name=f"{step.work_title} 字幕.srt",
            file_path=path,
            media_metadata={"format": "srt", "cue_count": len(scenes)},
            tags=["作品生成", "字幕"],
        )

    def _stage_reference_images(self, step: WorkStep) -> tuple[list[str], list[dict[str, object]]]:
        references = step.input_snapshot.get("reference_image_ids")
        if not isinstance(references, list) or not 1 <= len(references) <= 9:
            raise WorkGenerationConfigurationError("Seedance 参考图必须为 1~9 张")
        scenes = _ordered_scenes(step.work_input_snapshot)
        by_material = {str(scene.get("image_material_id")): scene for scene in scenes}
        config_id = str(step.run_model_snapshot.get("tos_staging_config_id") or "")
        config_version = _required_int(step.run_model_snapshot, "tos_staging_config_version")
        tos_config = self.registry.resolve_tos_staging(config_id, config_version)
        staging = self.tos_factory(tos_config)
        urls: list[str] = []
        audits: list[dict[str, object]] = []
        for reference in references:
            scene = by_material.get(str(reference))
            if scene is None:
                raise WorkGenerationConfigurationError("参考图不属于锁定作品输入")
            path = self.storage.source_path(str(scene.get("image_url") or ""))
            content = path.read_bytes()
            extension, content_type = _image_type(path, content)
            staged = staging.stage_media(
                project_id=UUID(step.project_id or ""),
                task_id=UUID(step.step_id),
                content=content,
                extension=extension,
                content_type=content_type,
            )
            downloaded = self.signed_url_reader(staged.signed_get_url, len(content) + 1)
            if hashlib.sha256(downloaded).hexdigest() != staged.source_sha256:
                raise WorkGenerationConfigurationError("TOS 签名 URL 内容校验失败")
            urls.append(staged.signed_get_url)
            audits.append(staged.audit_snapshot())
        return urls, audits

    def _resolve_video(self, step: WorkStep):
        version = _required_int(step.run_model_snapshot, "video_registry_version")
        model_id = str(step.run_model_snapshot.get("video_model_id") or "")
        return self.registry.resolve_video(model_id, version)

    def _cleanup_reference_images(
        self,
        step: WorkStep,
        references: list[dict[str, object]] | None = None,
    ) -> None:
        if references is None:
            attempt = next(
                (
                    item
                    for item in reversed(step.attempts)
                    if isinstance(item.input_snapshot.get("reference_images"), list)
                ),
                None,
            )
            references = (
                attempt.input_snapshot.get("reference_images") if attempt else None
            )
        if not isinstance(references, list):
            return
        config_id = str(step.run_model_snapshot.get("tos_staging_config_id") or "")
        config_version = _required_int(step.run_model_snapshot, "tos_staging_config_version")
        staging = self.tos_factory(
            self.registry.resolve_tos_staging(config_id, config_version)
        )
        failures = 0
        for reference in references:
            object_key = reference.get("object_key") if isinstance(reference, dict) else None
            if not isinstance(object_key, str):
                continue
            try:
                staging.cleanup(object_key)
            except Exception:
                failures += 1
        self._output_audit["staging_cleanup"] = (
            "succeeded" if failures == 0 else f"failed:{failures}"
        )

    def _seedance_provider(self, step: WorkStep):
        if self._seedance is None:
            self._seedance = self.seedance_factory(self._resolve_video(step))
        return self._seedance

    def _require_step(self) -> WorkStep:
        if self.step is None:
            raise RuntimeError("真实 provider 尚未绑定步骤")
        return self.step


def _seedance_from_config(config):
    return ArkSeedanceProvider(
        api_key=config.api_key,
        base_url=config.request_base_url,
        timeout_seconds=config.timeout_seconds,
    )


def _seedance_generate_audio(audio_mode: str) -> bool:
    return audio_mode in {"seedance_original", "seedance_original_and_tts"}


def _seedance_image_roles(model: str, reference_count: int) -> list[str]:
    """在任何暂存和收费调用前锁定模型家族对应的图片 role。"""

    if model.startswith("doubao-seedance-1-5-"):
        if reference_count == 1:
            return ["first_frame"]
        if reference_count == 2:
            return ["first_frame", "last_frame"]
        raise WorkGenerationConfigurationError("Seedance 1.5 最多 2 张首尾帧图片")
    if model.startswith("doubao-seedance-2-0-"):
        if 1 <= reference_count <= 9:
            return ["reference_image"] * reference_count
        raise WorkGenerationConfigurationError("Seedance 2.0 参考图必须为 1~9 张")
    raise WorkGenerationConfigurationError("当前 Seedance 模型家族未实现")


def _tts_from_config(config):
    if config.api_protocol != "volcengine_tts_v3":
        raise WorkGenerationConfigurationError("作品真实 TTS 仅支持 volcengine_tts_v3")
    return VolcengineTtsV3Provider(
        api_key=config.api_key,
        base_url=config.request_base_url,
        resource_id=str(config.settings.get("resource_id") or ""),
        timeout_seconds=config.timeout_seconds,
    )


def _tos_from_config(config):
    return TosMediaStaging(
        TosStagingConfig(
            endpoint=config.endpoint,
            region=config.region,
            bucket=config.bucket,
            object_prefix=config.object_prefix,
            signed_url_ttl_seconds=config.signed_url_ttl_seconds,
            max_file_bytes=config.max_file_bytes,
            access_key=config.access_key,
            secret_key=config.secret_key,
        )
    )


def _ordered_scenes(snapshot: dict[str, object]) -> list[dict[str, object]]:
    scenes = snapshot.get("scenes")
    if not isinstance(scenes, list) or not all(isinstance(scene, dict) for scene in scenes):
        return []
    return sorted(scenes, key=lambda scene: int(scene.get("sequence") or 0))


def _required_int(snapshot: dict[str, object], key: str) -> int:
    value = snapshot.get(key)
    if isinstance(value, bool):
        raise WorkGenerationConfigurationError(f"运行锁定快照缺少 {key}")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise WorkGenerationConfigurationError(f"运行锁定快照缺少 {key}") from error
    if parsed <= 0:
        raise WorkGenerationConfigurationError(f"运行锁定快照缺少 {key}")
    return parsed


def _select_tts_language(text: str, languages: list[object]) -> str:
    codes = [
        str(item.get("Language") or item.get("language") or "").lower()
        for item in languages
        if isinstance(item, dict)
    ]
    contains_chinese = any("\u4e00" <= char <= "\u9fff" for char in text)
    if contains_chinese:
        chinese = next((code for code in codes if code.startswith("zh")), None)
        if chinese is None:
            raise WorkGenerationConfigurationError("所选 TTS 音色不支持中文旁白")
        return chinese
    if not codes:
        raise WorkGenerationConfigurationError("TTS 音色语言能力快照缺失")
    return codes[0]


def _image_type(path: Path, content: bytes) -> tuple[str, str]:
    suffix = path.suffix.lower().lstrip(".")
    if suffix == "jpeg":
        suffix = "jpg"
    signatures = {
        "png": content.startswith(b"\x89PNG\r\n\x1a\n"),
        "jpg": content.startswith(b"\xff\xd8\xff"),
        "webp": content.startswith(b"RIFF") and content[8:12] == b"WEBP",
    }
    if not signatures.get(suffix):
        raise WorkGenerationConfigurationError("参考图扩展名与文件内容不匹配")
    return suffix, {"png": "image/png", "jpg": "image/jpeg", "webp": "image/webp"}[suffix]


def _read_signed_url(url: str, max_bytes: int) -> bytes:
    if urlsplit(url).scheme != "https":
        raise WorkGenerationConfigurationError("TOS 签名 URL 必须使用 HTTPS")
    with urllib_request.urlopen(url, timeout=30) as response:
        content = response.read(max_bytes + 1)
    if len(content) > max_bytes:
        raise WorkGenerationConfigurationError("TOS 签名 URL 响应大小异常")
    return content


def _download_video(url: str, path: Path, max_bytes: int) -> dict[str, object]:
    if urlsplit(url).scheme != "https":
        raise WorkGenerationConfigurationError("Seedance 视频 URL 必须使用 HTTPS")
    written = 0
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        try:
            with urllib_request.urlopen(url, timeout=120) as response:
                while chunk := response.read(1024 * 1024):
                    written += len(chunk)
                    if written > max_bytes:
                        raise WorkGenerationConfigurationError("Seedance 视频超过下载大小限制")
                    handle.write(chunk)
        except Exception:
            temporary.unlink(missing_ok=True)
            raise
    if written == 0:
        temporary.unlink(missing_ok=True)
        raise WorkGenerationConfigurationError("Seedance 视频为空")
    inspection = _inspect_video(temporary)
    os.replace(temporary, path)
    return inspection


def _inspect_video(path: Path) -> dict[str, object]:
    try:
        completed = subprocess.run(
            [
                "ffprobe", "-v", "error", "-show_streams", "-show_format",
                "-of", "json", str(path),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        payload = json.loads(completed.stdout)
    except (subprocess.CalledProcessError, json.JSONDecodeError) as error:
        raise WorkGenerationConfigurationError("Seedance 输出不是有效媒体") from error
    streams = payload.get("streams") if isinstance(payload, dict) else None
    video = next(
        (item for item in streams or [] if isinstance(item, dict) and item.get("codec_type") == "video"),
        None,
    )
    if video is None:
        raise WorkGenerationConfigurationError("Seedance 输出不包含视频流")
    format_payload = payload.get("format") if isinstance(payload.get("format"), dict) else {}
    duration = float(video.get("duration") or format_payload.get("duration") or 0)
    if not 3.5 <= duration <= 16:
        raise WorkGenerationConfigurationError("Seedance 输出时长异常")
    return {
        "mime_type": "video/mp4",
        "format": str(format_payload.get("format_name") or ""),
        "duration_sec": duration,
        "width": int(video.get("width") or 0),
        "height": int(video.get("height") or 0),
        "video_codec": str(video.get("codec_name") or ""),
        "file_size_bytes": path.stat().st_size,
        "source_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def _srt_time(milliseconds: int) -> str:
    hours, remainder = divmod(milliseconds, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    seconds, millis = divmod(remainder, 1_000)
    return f"{hours:02}:{minutes:02}:{seconds:02},{millis:03}"


def build_real_compose_command(
    videos: list[Path],
    audio: Path,
    subtitle: Path,
    output: Path,
    duration: int,
) -> list[str]:
    if not videos or any(not path.is_file() for path in videos):
        raise FileNotFoundError("真实合成缺少视频片段")
    if not audio.is_file() or not subtitle.is_file():
        raise FileNotFoundError("真实合成缺少 TTS 或字幕")
    if not 4 <= duration <= 60:
        raise ValueError("真实合成时长必须为 4~60 秒")
    command = ["ffmpeg", "-y"]
    for video in videos:
        command.extend(["-i", str(video)])
    command.extend(["-i", str(audio)])
    audio_index = len(videos)
    escaped_subtitle = str(subtitle).replace("\\", "\\\\").replace(":", "\\:").replace("'", "\\'")
    if len(videos) == 1:
        command.extend(
            [
                "-vf", f"subtitles='{escaped_subtitle}'",
                "-map", "0:v:0", "-map", f"{audio_index}:a:0",
            ]
        )
    else:
        video_inputs = "".join(f"[{index}:v:0]" for index in range(len(videos)))
        filters = (
            f"{video_inputs}concat=n={len(videos)}:v=1:a=0[vcat];"
            f"[vcat]subtitles='{escaped_subtitle}'[vout]"
        )
        command.extend(
            ["-filter_complex", filters, "-map", "[vout]", "-map", f"{audio_index}:a:0"]
        )
    command.extend(
        [
            "-t", str(duration), "-c:v", "libx264", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-movflags", "+faststart", str(output),
        ]
    )
    return command


def build_real_silent_compose_command(
    videos: list[Path],
    output: Path,
    duration: int,
) -> list[str]:
    if not videos or any(not path.is_file() for path in videos):
        raise FileNotFoundError("静音合成缺少视频片段")
    if not 4 <= duration <= 60:
        raise ValueError("静音合成时长必须为 4~60 秒")
    command = ["ffmpeg", "-y"]
    for video in videos:
        command.extend(["-i", str(video)])
    command.extend(
        [
            "-f", "lavfi", "-i",
            "anullsrc=channel_layout=stereo:sample_rate=48000",
        ]
    )
    audio_index = len(videos)
    if len(videos) == 1:
        command.extend(["-map", "0:v:0", "-map", f"{audio_index}:a:0"])
    else:
        video_inputs = "".join(f"[{index}:v:0]" for index in range(len(videos)))
        filters = f"{video_inputs}concat=n={len(videos)}:v=1:a=0[vout]"
        command.extend(
            ["-filter_complex", filters, "-map", "[vout]", "-map", f"{audio_index}:a:0"]
        )
    command.extend(
        [
            "-t", str(duration), "-c:v", "libx264", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-movflags", "+faststart", str(output),
        ]
    )
    return command
