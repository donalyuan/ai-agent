"""Canonical TimelineVersion compiler shared by preview and final FFmpeg rendering."""

from __future__ import annotations

from dataclasses import asdict
from typing import Any, cast
from uuid import uuid4

from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.exports import ExportSettings, RenderPlan
from video_agent_api.domain.timeline import TimelineVersion

_AUDIT_FIELDS = {
    "episode",
    "authorization",
    "license",
    "loudness",
    "models",
    "skillRevisions",
    "parameters",
    "usage",
    "cost",
}


def compile_render_plan(
    version: TimelineVersion, settings: ExportSettings | None = None
) -> RenderPlan:
    snapshot = version.cut_snapshot
    if snapshot.get("schema_version") != version.schema_version:
        raise ValidationDomainError("TimelineVersion schema version conflict")
    clips = snapshot.get("clips")
    cues = snapshot.get("soundCues", [])
    captions = snapshot.get("captions", [])
    ducking = snapshot.get("ducking")
    if not isinstance(clips, list) or not clips:
        raise ValidationDomainError("RenderPlan requires at least one Clip")
    if not isinstance(cues, list) or not isinstance(captions, list):
        raise ValidationDomainError("RenderPlan audio or captions are invalid")
    ordered_clips = tuple(sorted((dict(item) for item in clips), key=_clip_order))
    _validate_clips(ordered_clips)
    ordered_cues = tuple(sorted((dict(item) for item in cues), key=_cue_order))
    _validate_cues(ordered_cues)
    ordered_captions = tuple(sorted((dict(item) for item in captions), key=_caption_order))
    _validate_captions(ordered_captions)
    normalized_ducking = _validate_ducking(ducking)
    return RenderPlan(
        version.project_id,
        version.episode_id,
        version.id,
        ordered_clips,
        ordered_cues,
        ordered_captions,
        normalized_ducking,
        settings or ExportSettings(),
    )


def render_plan_from_snapshot(payload: dict[str, object], expected_hash: str) -> RenderPlan:
    if set(payload) != {
        "projectId",
        "episodeId",
        "timelineVersionId",
        "clips",
        "soundCues",
        "captions",
        "ducking",
        "settings",
    }:
        raise ValidationDomainError("frozen RenderPlan fields are incomplete or aliased")
    settings = payload["settings"]
    if not isinstance(settings, dict) or set(settings) != {
        "aspectRatio",
        "width",
        "height",
        "fps",
        "container",
        "videoCodec",
        "pixelFormat",
        "audioCodec",
        "sampleRate",
        "subtitleEncoding",
    }:
        raise ValidationDomainError("frozen RenderPlan settings are incomplete or aliased")
    clips = payload["clips"]
    cues = payload["soundCues"]
    captions = payload["captions"]
    if (
        not isinstance(clips, (list, tuple))
        or not isinstance(cues, (list, tuple))
        or not isinstance(captions, (list, tuple))
    ):
        raise ValidationDomainError("frozen RenderPlan collections are invalid")
    normalized_clips = tuple(dict(cast(dict[str, object], item)) for item in clips)
    normalized_cues = tuple(dict(cast(dict[str, object], item)) for item in cues)
    normalized_captions = tuple(dict(cast(dict[str, object], item)) for item in captions)
    _validate_clips(normalized_clips)
    _validate_cues(normalized_cues)
    _validate_captions(normalized_captions)
    ducking = _validate_ducking(payload["ducking"])
    plan = RenderPlan(
        str(payload["projectId"]),
        str(payload["episodeId"]),
        str(payload["timelineVersionId"]),
        normalized_clips,
        normalized_cues,
        normalized_captions,
        ducking,
        ExportSettings(
            aspect_ratio=cast(Any, settings["aspectRatio"]),
            width=cast(int, settings["width"]),
            height=cast(int, settings["height"]),
            fps=cast(int, settings["fps"]),
            container=cast(Any, settings["container"]),
            video_codec=cast(Any, settings["videoCodec"]),
            pixel_format=cast(Any, settings["pixelFormat"]),
            audio_codec=cast(Any, settings["audioCodec"]),
            sample_rate=cast(int, settings["sampleRate"]),
            subtitle_encoding=cast(Any, settings["subtitleEncoding"]),
        ),
    )
    if plan.render_plan_hash != expected_hash:
        raise ValidationDomainError("frozen RenderPlan hash is invalid")
    return plan


def compile_preview_manifest(plan: RenderPlan, cut_revision: int) -> dict[str, object]:
    return {
        "schema_version": "1.0.0",
        "projectId": plan.project_id,
        "episodeId": plan.episode_id,
        "timelineVersionId": plan.timeline_version_id,
        "cutRevision": cut_revision,
        "renderPlanHash": plan.render_plan_hash,
        "plan": plan.canonical_payload(),
    }


def compile_ffmpeg_filter_graph(plan: RenderPlan) -> tuple[str, str, str | None]:
    """Compile only allowlisted filters; returned labels are mapped by the adapter."""
    statements: list[str] = []
    video_labels: list[str] = []
    for index, clip in enumerate(plan.clips):
        start = cast(int, clip["inFrame"])
        end = cast(int, clip["outFrame"])
        label = f"v{index}"
        statements.append(
            f"[{index}:v]trim=start_frame={start}:end_frame={end},"
            f"setpts=PTS-STARTPTS,scale={plan.settings.width}:{plan.settings.height},"
            f"format=yuv420p[{label}]"
        )
        video_labels.append(label)
    current_video = video_labels[0]
    for index in range(1, len(video_labels)):
        transition = cast(dict[str, object], plan.clips[index]["transition"])
        next_label = f"vjoin{index}"
        if transition["type"] == "crossfade":
            duration_frames = cast(int, transition["durationFrames"])
            offset_frames = cast(int, plan.clips[index]["timelineStart"])
            statements.append(
                f"[{current_video}][{video_labels[index]}]xfade=transition=fade:"
                f"duration={duration_frames}/30:offset={offset_frames}/30[{next_label}]"
            )
        else:
            statements.append(
                f"[{current_video}][{video_labels[index]}]concat=n=2:v=1:a=0[{next_label}]"
            )
        current_video = next_label

    audio_labels: list[str] = []
    input_index = len(plan.clips)
    for cue_index, cue in enumerate(plan.cues):
        label = f"a{cue_index}"
        gain_value = cue.get("gainDb", 0.0)
        if isinstance(gain_value, bool) or not isinstance(gain_value, (int, float)):
            raise ValidationDomainError("SoundCue gainDb is invalid")
        gain_db = float(gain_value)
        if cue.get("mute") is True:
            gain_db = -120.0
        filters = [
            f"atrim=duration={cast(int, cue['durationFrames'])}/30",
            "asetpts=PTS-STARTPTS",
            f"volume={gain_db}dB",
        ]
        fade_in = cast(int, cue.get("fadeInFrames", 0))
        fade_out = cast(int, cue.get("fadeOutFrames", 0))
        if fade_in:
            filters.append(f"afade=t=in:st=0:d={fade_in}/30")
        if fade_out:
            duration = cast(int, cue["durationFrames"])
            filters.append(f"afade=t=out:st={duration - fade_out}/30:d={fade_out}/30")
        filters.extend(_ducking_filters(plan, cue))
        filters.append(
            f"adelay={cast(int, cue['startFrame']) * 1000 // 30}|"
            f"{cast(int, cue['startFrame']) * 1000 // 30}"
        )
        statements.append(f"[{input_index + cue_index}:a]{','.join(filters)}[{label}]")
        audio_labels.append(label)
    audio_map: str | None = None
    if audio_labels:
        joined = "".join(f"[{label}]" for label in audio_labels)
        statements.append(
            f"{joined}amix=inputs={len(audio_labels)}:normalize=0,loudnorm=I=-14:TP=-1:LRA=11[aout]"
        )
        audio_map = "[aout]"
    return ";".join(statements), f"[{current_video}]", audio_map


def render_srt(plan: RenderPlan) -> bytes:
    blocks: list[str] = []
    for index, caption in enumerate(plan.captions, start=1):
        blocks.append(
            f"{index}\n{_srt_time(cast(int, caption['startFrame']))} --> "
            f"{_srt_time(cast(int, caption['endFrame']))}\n{caption['text']}"
        )
    return ("\n\n".join(blocks) + ("\n" if blocks else "")).encode("utf-8")


def build_light_manifest(
    plan: RenderPlan,
    version: TimelineVersion,
    audit_facts: dict[str, object],
    mp4_artifact_id: str,
    srt_artifact_id: str,
    package_id: str | None = None,
) -> dict[str, object]:
    if set(audit_facts) != _AUDIT_FIELDS:
        raise ValidationDomainError("light manifest audit facts are incomplete or aliased")
    for key in _AUDIT_FIELDS:
        if audit_facts[key] in (None, "", {}, []):
            raise ValidationDomainError(f"light manifest audit fact is empty: {key}")
    cost = audit_facts["cost"]
    if not isinstance(cost, dict) or set(cost) != {"value", "currency", "status", "source"}:
        raise ValidationDomainError("light manifest cost fact is incomplete")
    if cost.get("value") == "unknown" and (
        cost.get("status") != "unknown" or not str(cost.get("source", "")).strip()
    ):
        raise ValidationDomainError("unknown cost requires explicit status and source")
    asset_refs: dict[str, dict[str, object]] = {}
    for clip in plan.clips:
        asset_id = str(clip["assetVersionId"])
        asset_refs[asset_id] = {
            "id": asset_id,
            "revision": clip["assetVersionRevision"],
            "hash": clip["assetVersionHash"],
            "authorization": {
                "status": clip.get("authorizationStatus", "authorized"),
                "source": "asset-owner",
                "recordId": str(clip.get("authorizationRecordId", f"auth:{asset_id}")),
            },
            "license": {
                "status": clip.get("licenseStatus", "approved"),
                "source": "asset-owner",
                "recordId": str(clip.get("licenseRecordId", f"license:{asset_id}")),
            },
        }
    for cue in plan.cues:
        asset_id = str(cue["assetVersionId"])
        asset_refs[asset_id] = {
            "id": asset_id,
            "revision": cue["assetVersionRevision"],
            "hash": cue["assetVersionHash"],
            "authorization": {
                "status": cue["authorizationStatus"],
                "source": "asset-owner",
                "recordId": f"auth:{asset_id}",
            },
            "license": {
                "status": cue["licenseStatus"],
                "source": "asset-owner",
                "recordId": f"license:{asset_id}",
            },
        }
    return {
        "id": package_id or str(uuid4()),
        "schema_version": version.schema_version,
        "manifestVersion": version.schema_version,
        "exportProfile": "light",
        "projectId": plan.project_id,
        "episode": audit_facts["episode"],
        "timelineVersion": {
            "id": version.id,
            "revision": version.revision,
            "hash": plan.render_plan_hash,
        },
        "assetVersionRefs": list(asset_refs.values()),
        "soundCues": [
            {
                "cueId": cue["id"],
                "track": cue["track"],
                "assetVersionId": cue["assetVersionId"],
                "startFrame": cue["startFrame"],
                "durationFrames": cue["durationFrames"],
            }
            for cue in plan.cues
        ],
        "authorization": audit_facts["authorization"],
        "license": audit_facts["license"],
        "loudness": audit_facts["loudness"],
        "models": audit_facts["models"],
        "skillRevisions": audit_facts["skillRevisions"],
        "parameters": audit_facts["parameters"],
        "usage": audit_facts["usage"],
        "cost": cost,
        "references": [
            {"artifactType": "mp4", "artifactId": mp4_artifact_id},
            {"artifactType": "srt", "artifactId": srt_artifact_id},
        ],
    }


def verify_parity(
    preview_plan_hash: str,
    ffmpeg_plan_hash: str,
    *,
    ssim: float,
    duration_delta_frames: int,
    caption_delta_frames: int,
    audio_delta_frames: int,
) -> dict[str, object]:
    if (
        preview_plan_hash != ffmpeg_plan_hash
        or ssim < 0.98
        or any(
            abs(value) > 1
            for value in (
                duration_delta_frames,
                caption_delta_frames,
                audio_delta_frames,
            )
        )
    ):
        raise ValidationDomainError("renderer parity gate failed")
    return {
        "renderPlanHash": preview_plan_hash,
        "ssim": ssim,
        "durationDeltaFrames": duration_delta_frames,
        "captionDeltaFrames": caption_delta_frames,
        "audioDeltaFrames": audio_delta_frames,
        "status": "passed",
    }


def _clip_order(clip: object) -> tuple[int, str]:
    if not isinstance(clip, dict):
        raise ValidationDomainError("RenderPlan Clip is invalid")
    return cast(int, clip.get("timelineStart", -1)), str(clip.get("id", ""))


def _cue_order(cue: object) -> tuple[int, str, int, str]:
    if not isinstance(cue, dict):
        raise ValidationDomainError("RenderPlan SoundCue is invalid")
    return (
        cast(int, cue.get("startFrame", -1)),
        str(cue.get("track", "")),
        -cast(int, cue.get("priority", 0)),
        str(cue.get("id", "")),
    )


def _caption_order(caption: object) -> tuple[int, str]:
    if not isinstance(caption, dict):
        raise ValidationDomainError("RenderPlan Caption is invalid")
    return cast(int, caption.get("startFrame", -1)), str(caption.get("id", ""))


def _validate_clips(clips: tuple[dict[str, object], ...]) -> None:
    previous_end = 0
    for index, clip in enumerate(clips):
        required = {
            "id",
            "assetVersionId",
            "assetVersionRevision",
            "assetVersionHash",
            "derivativeFingerprint",
            "derivativeStatus",
            "inFrame",
            "outFrame",
            "durationFrames",
            "timelineStart",
            "transition",
        }
        if not required.issubset(clip) or clip["derivativeStatus"] != "ready":
            raise ValidationDomainError("RenderPlan Clip is incomplete or derivative is unready")
        integers = (
            "assetVersionRevision",
            "inFrame",
            "outFrame",
            "durationFrames",
            "timelineStart",
        )
        for name in integers:
            frame = clip[name]
            if isinstance(frame, bool) or not isinstance(frame, int) or frame < 0:
                raise ValidationDomainError("RenderPlan Clip frame is invalid")
        in_frame = cast(int, clip["inFrame"])
        out_frame = cast(int, clip["outFrame"])
        clip_duration = cast(int, clip["durationFrames"])
        timeline_start = cast(int, clip["timelineStart"])
        if out_frame - in_frame != clip_duration:
            raise ValidationDomainError("RenderPlan Clip duration mismatch")
        transition = clip["transition"]
        if not isinstance(transition, dict) or set(transition) != {"type", "durationFrames"}:
            raise ValidationDomainError("RenderPlan transition is invalid")
        duration = transition["durationFrames"]
        if isinstance(duration, bool) or not isinstance(duration, int) or duration < 0:
            raise ValidationDomainError("RenderPlan transition duration is invalid")
        if transition["type"] not in {"cut", "crossfade"}:
            raise ValidationDomainError("RenderPlan transition is unsupported")
        expected_start = 0 if index == 0 else previous_end - duration
        if timeline_start != expected_start:
            raise ValidationDomainError("RenderPlan clips violate adjacency/overlap")
        if transition["type"] == "cut" and duration != 0:
            raise ValidationDomainError("cut transition duration must be zero")
        if transition["type"] == "crossfade" and not 0 < duration < clip_duration:
            raise ValidationDomainError("crossfade duration is out of bounds")
        previous_end = timeline_start + clip_duration


def _validate_cues(cues: tuple[dict[str, object], ...]) -> None:
    for cue in cues:
        if set(cue) - {
            "id",
            "track",
            "assetVersionId",
            "assetVersionRevision",
            "assetVersionHash",
            "startFrame",
            "durationFrames",
            "trigger",
            "triggerRef",
            "priority",
            "continuityRefs",
            "gainDb",
            "mute",
            "solo",
            "fadeInFrames",
            "fadeOutFrames",
            "authorizationStatus",
            "licenseStatus",
        }:
            raise ValidationDomainError("SoundCue contains unsupported automation fields")
        if cue.get("track") not in {"dialogue", "music", "ambience", "effects"}:
            raise ValidationDomainError("SoundCue track is invalid")
        for field in ("startFrame", "durationFrames", "priority", "fadeInFrames", "fadeOutFrames"):
            value = cue.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise ValidationDomainError(f"SoundCue {field} is invalid")
        if cast(int, cue["durationFrames"]) < 1 or cast(int, cue["priority"]) > 100:
            raise ValidationDomainError("SoundCue duration or priority is invalid")


def _validate_captions(captions: tuple[dict[str, object], ...]) -> None:
    for caption in captions:
        if set(caption) != {"id", "text", "startFrame", "endFrame"}:
            raise ValidationDomainError("Caption contains unsupported style fields")
        start, end = caption["startFrame"], caption["endFrame"]
        if (
            isinstance(start, bool)
            or not isinstance(start, int)
            or isinstance(end, bool)
            or not isinstance(end, int)
            or start < 0
            or end <= start
            or not isinstance(caption["text"], str)
            or not caption["text"].strip()
        ):
            raise ValidationDomainError("Caption is invalid")


def _validate_ducking(value: object) -> dict[str, object] | None:
    if value is None:
        return None
    if not isinstance(value, dict) or set(value) != {
        "enabled",
        "dialogueIntervals",
        "attenuationDb",
        "attackFrames",
        "releaseFrames",
        "targetTracks",
    }:
        raise ValidationDomainError("ducking is incomplete or aliased")
    intervals = value["dialogueIntervals"]
    if not isinstance(intervals, list):
        raise ValidationDomainError("ducking intervals are invalid")
    previous_end = -1
    for interval in intervals:
        if (
            not isinstance(interval, list)
            or len(interval) != 2
            or any(isinstance(item, bool) or not isinstance(item, int) for item in interval)
            or interval[0] < 0
            or interval[1] <= interval[0]
            or interval[0] <= previous_end
        ):
            raise ValidationDomainError("ducking intervals must be merged integer frames")
        previous_end = interval[1]
    if value["enabled"] is True and not intervals:
        raise ValidationDomainError("enabled ducking requires dialogue intervals")
    if not isinstance(value["attenuationDb"], (int, float)) or value["attenuationDb"] <= 0:
        raise ValidationDomainError("ducking attenuation must be positive")
    if set(cast(list[str], value["targetTracks"])) - {"music", "ambience", "effects"}:
        raise ValidationDomainError("ducking target tracks are invalid")
    return dict(value)


def _srt_time(frame: int) -> str:
    total_ms = frame * 1000 // 30
    hours, remainder = divmod(total_ms, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    seconds, milliseconds = divmod(remainder, 1000)
    return f"{hours:02d}:{minutes:02d}:{seconds:02d},{milliseconds:03d}"


def _ducking_filters(plan: RenderPlan, cue: dict[str, object]) -> list[str]:
    policy = plan.ducking
    if policy is None or policy["enabled"] is not True:
        return []
    if cue["track"] not in cast(list[str], policy["targetTracks"]):
        return []
    attenuation = float(cast(float, policy["attenuationDb"]))
    gain = 10 ** (-attenuation / 20)
    attack = cast(int, policy["attackFrames"]) / 30
    release = cast(int, policy["releaseFrames"]) / 30
    cue_start = cast(int, cue["startFrame"]) / 30
    cue_end = cue_start + cast(int, cue["durationFrames"]) / 30
    filters: list[str] = []
    for interval in cast(list[list[int]], policy["dialogueIntervals"]):
        start = max(cue_start, interval[0] / 30) - cue_start
        end = min(cue_end, interval[1] / 30) - cue_start
        if end <= 0 or start >= cue_end - cue_start:
            continue
        start = max(0.0, start)
        attack_start = max(0.0, start - attack)
        release_end = min(cue_end - cue_start, end + release)
        attack_expr = (
            f"1-(1-{gain:.8f})*(t-{attack_start:.8f})/{attack:.8f}" if attack > 0 else f"{gain:.8f}"
        )
        release_expr = (
            f"{gain:.8f}+(1-{gain:.8f})*(t-{end:.8f})/{release:.8f}" if release > 0 else "1"
        )
        expression = (
            f"if(between(t,{attack_start:.8f},{start:.8f}),{attack_expr},"
            f"if(between(t,{start:.8f},{end:.8f}),{gain:.8f},"
            f"if(between(t,{end:.8f},{release_end:.8f}),{release_expr},1)))"
        )
        filters.append(f"volume='{expression}':eval=frame")
    return filters


def render_plan_debug(plan: RenderPlan) -> dict[str, object]:
    return {"plan": plan.canonical_payload(), "settings": asdict(plan.settings)}
