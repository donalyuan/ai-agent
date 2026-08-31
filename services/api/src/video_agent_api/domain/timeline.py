"""Episode-owned current Cut and immutable TimelineVersion contracts."""

from __future__ import annotations

import hashlib
import json
from copy import deepcopy
from dataclasses import asdict, dataclass, field
from typing import Literal, cast
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

Track = Literal["dialogue", "music", "ambience", "effects"]
Trigger = Literal["manual", "scene_start", "shot_start", "shot_end"]
_TRACKS = {"dialogue", "music", "ambience", "effects"}
_DUCK_TARGETS = {"music", "ambience", "effects"}
_TRIGGERS = {"manual", "scene_start", "shot_start", "shot_end"}


def _hex64(value: object, field_name: str) -> str:
    if not isinstance(value, str) or len(value) != 64:
        raise ValidationDomainError(f"{field_name} is invalid")
    try:
        int(value, 16)
    except ValueError as error:
        raise ValidationDomainError(f"{field_name} is invalid") from error
    return value.lower()


def _integer_frame(value: object, field_name: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ValidationDomainError(f"{field_name} must be an integer frame")
    return value


@dataclass(frozen=True, slots=True)
class AssetSelection:
    """Small selector handoff; asset-center remains owner of library metadata."""

    project_id: str
    episode_id: str
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    derivative_fingerprint: str
    authorization_status: Literal["authorized"] = "authorized"
    license_status: Literal["approved"] = "approved"
    available_frames: int = 1
    accepted_current: bool = True
    shot_id: str | None = None

    def __post_init__(self) -> None:
        if not self.project_id or not self.episode_id or not self.asset_version_id:
            raise ValidationDomainError("asset selection scope is required")
        if self.asset_version_revision < 0 or self.available_frames < 1:
            raise ValidationDomainError("asset selection revision or frame count is invalid")
        _hex64(self.asset_version_hash, "assetVersionHash")
        _hex64(self.derivative_fingerprint, "derivativeFingerprint")
        if not self.accepted_current:
            raise ValidationDomainError("asset selection is not accepted current")


@dataclass(frozen=True, slots=True)
class SoundCue:
    track: Track
    asset_version_id: str
    start_frame: int
    duration_frames: int
    trigger: Trigger = "manual"
    trigger_ref: dict[str, object] | None = None
    priority: int = 0
    continuity_refs: tuple[dict[str, object], ...] = ()
    gain_db: float = 0.0
    mute: bool = False
    solo: bool = False
    fade_in_frames: int = 0
    fade_out_frames: int = 0
    asset_version_revision: int = 1
    asset_version_hash: str | None = None
    authorization_status: str = "authorized"
    license_status: str = "approved"
    id: str = field(default_factory=lambda: str(uuid4()))

    def __post_init__(self) -> None:
        _integer_frame(self.start_frame, "startFrame")
        _integer_frame(self.duration_frames, "durationFrames", minimum=1)
        _integer_frame(self.fade_in_frames, "fadeInFrames")
        _integer_frame(self.fade_out_frames, "fadeOutFrames")
        if self.track not in _TRACKS or self.trigger not in _TRIGGERS:
            raise ValidationDomainError("sound cue track or trigger is invalid")
        if not 0 <= self.priority <= 100 or self.asset_version_revision < 0:
            raise ValidationDomainError("sound cue priority or revision is invalid")
        if self.fade_in_frames + self.fade_out_frames > self.duration_frames:
            raise ValidationDomainError("sound cue fades exceed duration")
        if self.trigger == "manual" and self.trigger_ref is not None:
            raise ValidationDomainError("manual trigger must not contain triggerRef")
        if self.trigger != "manual" and not self.trigger_ref:
            raise ValidationDomainError("non-manual trigger requires triggerRef")
        if len(self.continuity_refs) > 8:
            raise ValidationDomainError("continuityRefs exceed the bounded limit")
        keys: list[tuple[object, object, object]] = []
        for item in self.continuity_refs:
            if set(item) != {"ownerType", "id", "revision", "hash"}:
                raise ValidationDomainError("continuityRef is incomplete")
            if item["ownerType"] not in {"asset_bible", "scene", "shot", "shot_spec"}:
                raise ValidationDomainError("continuityRef ownerType is invalid")
            if not isinstance(item["revision"], int) or item["revision"] < 1:
                raise ValidationDomainError("continuityRef revision is invalid")
            _hex64(item["hash"], "continuityRef hash")
            keys.append((item["ownerType"], item["id"], item["revision"]))
        if len(keys) != len(set(keys)):
            raise ValidationDomainError("continuityRefs must be unique")
        if self.asset_version_hash is not None:
            _hex64(self.asset_version_hash, "assetVersionHash")
        if self.authorization_status != "authorized" or self.license_status != "approved":
            raise ValidationDomainError("sound cue asset is unauthorized")


@dataclass(frozen=True, slots=True)
class Ducking:
    enabled: bool
    dialogue_intervals: tuple[tuple[int, int], ...]
    attenuation_db: float
    attack_frames: int
    release_frames: int
    target_tracks: tuple[str, ...]

    def __post_init__(self) -> None:
        _integer_frame(self.attack_frames, "attackFrames")
        _integer_frame(self.release_frames, "releaseFrames")
        if self.attenuation_db <= 0 or not self.target_tracks:
            raise ValidationDomainError("ducking parameters are invalid")
        if len(self.target_tracks) != len(set(self.target_tracks)) or any(
            track not in _DUCK_TARGETS for track in self.target_tracks
        ):
            raise ValidationDomainError("ducking targetTracks are invalid")
        object.__setattr__(self, "dialogue_intervals", _merge_intervals(self.dialogue_intervals))


def _merge_intervals(intervals: tuple[tuple[int, int], ...]) -> tuple[tuple[int, int], ...]:
    ordered: list[tuple[int, int]] = []
    for start, end in intervals:
        _integer_frame(start, "dialogue interval start")
        _integer_frame(end, "dialogue interval end", minimum=1)
        if end <= start:
            raise ValidationDomainError("dialogue interval is invalid")
        ordered.append((start, end))
    merged: list[list[int]] = []
    for start, end in sorted(ordered):
        if not merged or start > merged[-1][1]:
            merged.append([start, end])
        else:
            merged[-1][1] = max(merged[-1][1], end)
    return tuple((start, end) for start, end in merged)


@dataclass(slots=True)
class TimelineCut:
    episode_id: str
    project_id: str = ""
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = "1.0.0"
    clips: list[dict[str, object]] = field(default_factory=list)
    cues: list[SoundCue] = field(default_factory=list)
    captions: list[dict[str, object]] = field(default_factory=list)
    ducking: Ducking | None = None

    def edit(self, expected_revision: int, command: str, **payload: object) -> None:
        """Validate against detached state, then publish one atomic revision."""
        if expected_revision != self.revision:
            raise RevisionConflictError(self.episode_id, expected_revision, self.revision)
        if command in {
            "undo",
            "redo",
            "restore",
            "automation",
            "keyframes",
            "loop",
            "speed",
            "audio_crossfade",
            "wipe",
            "mask",
            "create_cut",
            "select_cut",
            "timeline_draft",
        }:
            raise ValidationDomainError("unsupported_feature")

        clips = deepcopy(self.clips)
        cues = list(self.cues)
        captions = deepcopy(self.captions)
        ducking = self.ducking
        if command == "add_clip":
            clip = dict(cast(dict[str, object], payload.get("clip", {})))
            _validate_clip(clip, project_id=self.project_id, episode_id=self.episode_id)
            if any(item.get("id") == clip["id"] for item in clips):
                raise ValidationDomainError("duplicate_clip")
            clips.append(clip)
            _validate_clip_layout(clips)
        elif command in {"trim_clip", "set_clip_transform"}:
            clip = _clip(clips, payload.get("clip_id"))
            if command == "trim_clip":
                in_frame = payload.get("in_frame")
                out_frame = payload.get("out_frame")
                _frame_range(in_frame, out_frame)
                if cast(int, out_frame) > cast(int, clip.get("sourceFrames", out_frame)):
                    raise ValidationDomainError("frame_out_of_bounds")
                clip["inFrame"], clip["outFrame"] = in_frame, out_frame
                clip["durationFrames"] = cast(int, out_frame) - cast(int, in_frame)
            else:
                transform = payload.get("transform")
                _validate_transform(transform)
                clip["transform"] = deepcopy(cast(dict[str, object], transform))
            _validate_clip_layout(clips)
        elif command == "split_clip":
            clip = _clip(clips, payload.get("clip_id"))
            split_frame = _integer_frame(payload.get("split_frame"), "splitFrame", minimum=1)
            in_frame, out_frame = cast(int, clip["inFrame"]), cast(int, clip["outFrame"])
            if not in_frame < split_frame < out_frame:
                raise ValidationDomainError("frame_out_of_bounds")
            first, second = dict(clip), dict(clip)
            first["id"], second["id"] = str(uuid4()), str(uuid4())
            first["outFrame"], first["durationFrames"] = split_frame, split_frame - in_frame
            second["inFrame"], second["durationFrames"] = split_frame, out_frame - split_frame
            second["timelineStart"] = cast(int, clip["timelineStart"]) + cast(
                int, first["durationFrames"]
            )
            position = clips.index(clip)
            clips[position : position + 1] = [first, second]
            _validate_clip_layout(clips)
        elif command == "delete_clip":
            clips.remove(_clip(clips, payload.get("clip_id")))
        elif command == "reorder_clips":
            ids = payload.get("clip_ids")
            existing_ids = [str(clip["id"]) for clip in clips]
            if (
                not isinstance(ids, list)
                or len(ids) != len(set(map(str, ids)))
                or set(map(str, ids)) != set(existing_ids)
            ):
                raise ValidationDomainError("reorder must include complete unique clip set")
            by_id = {str(clip["id"]): clip for clip in clips}
            clips = [by_id[str(item)] for item in ids]
            cursor = 0
            for clip in clips:
                clip["timelineStart"] = cursor
                cursor += cast(int, clip["durationFrames"])
        elif command == "replace_clip_source":
            _replace_source(
                _clip(clips, payload.get("clip_id")), payload, self.project_id, self.episode_id
            )
        elif command == "add_sound_cue":
            cue = payload.get("cue")
            if not isinstance(cue, SoundCue):
                raise ValidationDomainError("sound cue is required")
            if any(item.id == cue.id for item in cues):
                raise ValidationDomainError("duplicate_sound_cue")
            cues.append(cue)
            cues.sort(key=lambda item: (item.start_frame, item.track, -item.priority, item.id))
        elif command == "remove_sound_cue":
            cue_id = payload.get("cue_id")
            if not any(item.id == cue_id for item in cues):
                raise ValidationDomainError("sound_cue_not_found")
            cues = [item for item in cues if item.id != cue_id]
        elif command == "set_sound_cue_mix":
            cue = next((item for item in cues if item.id == payload.get("cue_id")), None)
            if cue is None:
                raise ValidationDomainError("sound_cue_not_found")
            if {"automation", "keyframes", "points"}.intersection(payload):
                raise ValidationDomainError("automation keyframes are unsupported")
            updated = asdict(cue)
            for key in ("gain_db", "mute", "solo", "fade_in_frames", "fade_out_frames"):
                if key in payload:
                    updated[key] = payload[key]
            updated["continuity_refs"] = tuple(updated["continuity_refs"])
            cues[cues.index(cue)] = SoundCue(**updated)
        elif command == "set_ducking":
            proposed = payload.get("ducking")
            if not isinstance(proposed, Ducking):
                raise ValidationDomainError("ducking is required")
            ducking = proposed
        elif command == "upsert_caption":
            caption = dict(cast(dict[str, object], payload.get("caption", {})))
            _validate_caption(caption)
            caption_id = caption.get("id") or str(uuid4())
            caption["id"] = caption_id
            existing = next((item for item in captions if item.get("id") == caption_id), None)
            if existing is None:
                captions.append(caption)
            else:
                existing.update(caption)
            captions.sort(key=lambda item: (cast(int, item["startFrame"]), str(item["id"])))
        else:
            raise ValidationDomainError("unsupported_feature")

        self.clips, self.cues, self.captions, self.ducking = clips, cues, captions, ducking
        self.revision += 1

    def fingerprint(self) -> str:
        value = {
            "id": self.id,
            "revision": self.revision,
            "schema_version": self.schema_version,
            "clips": self.clips,
            "cues": [asdict(item) for item in self.cues],
            "captions": self.captions,
            "ducking": asdict(self.ducking) if self.ducking else None,
        }
        return hashlib.sha256(
            json.dumps(value, sort_keys=True, separators=(",", ":"), default=str).encode()
        ).hexdigest()


@dataclass(frozen=True, slots=True)
class TimelineVersion:
    episode_id: str
    source_cut_revision: int
    name: str
    cut_snapshot: dict[str, object]
    project_id: str = ""
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        if not self.name.strip() or len(self.name) > 120 or self.source_cut_revision < 1:
            raise ValidationDomainError("timeline version name or revision is invalid")
        if self.cut_snapshot.get("schema_version") != self.schema_version:
            raise ValidationDomainError("timeline schema version conflict")


def _clip(clips: list[dict[str, object]], clip_id: object) -> dict[str, object]:
    clip = next((item for item in clips if item.get("id") == clip_id), None)
    if clip is None:
        raise ValidationDomainError("clip_not_found")
    return clip


def _frame_range(start: object, end: object) -> None:
    start_value = _integer_frame(start, "startFrame")
    end_value = _integer_frame(end, "endFrame", minimum=1)
    if end_value <= start_value:
        raise ValidationDomainError("frame_out_of_bounds")


def _validate_transform(transform: object) -> None:
    if not isinstance(transform, dict) or set(transform) != {"position", "scale", "opacity"}:
        raise ValidationDomainError("static transform requires position, scale and opacity")
    position = transform["position"]
    invalid_position_value = isinstance(position, dict) and any(
        isinstance(value, bool) or not isinstance(value, (int, float))
        for value in position.values()
    )
    if not isinstance(position, dict) or set(position) != {"x", "y"} or invalid_position_value:
        raise ValidationDomainError("position is invalid")
    scale, opacity = transform["scale"], transform["opacity"]
    if isinstance(scale, bool) or not isinstance(scale, (int, float)) or scale <= 0:
        raise ValidationDomainError("scale is invalid")
    if isinstance(opacity, bool) or not isinstance(opacity, (int, float)) or not 0 <= opacity <= 1:
        raise ValidationDomainError("opacity is invalid")


def _validate_caption(caption: dict[str, object]) -> None:
    _frame_range(caption.get("startFrame"), caption.get("endFrame"))
    text = caption.get("text")
    if not isinstance(text, str) or not text.strip():
        raise ValidationDomainError("caption text is required")
    if set(caption) - {"id", "text", "startFrame", "endFrame"}:
        raise ValidationDomainError("caption style is unsupported")


def _validate_clip(clip: dict[str, object], *, project_id: str = "", episode_id: str = "") -> None:
    required = {
        "id",
        "assetVersionId",
        "assetVersionRevision",
        "assetVersionHash",
        "derivativeFingerprint",
        "inFrame",
        "outFrame",
        "timelineStart",
    }
    if not required.issubset(clip):
        raise ValidationDomainError("clip fields are incomplete")
    _frame_range(clip["inFrame"], clip["outFrame"])
    _integer_frame(clip["timelineStart"], "timelineStart")
    _integer_frame(clip["assetVersionRevision"], "assetVersionRevision")
    _hex64(clip["assetVersionHash"], "assetVersionHash")
    _hex64(clip["derivativeFingerprint"], "derivativeFingerprint")
    if "sourceFrames" in clip and cast(int, clip["outFrame"]) > _integer_frame(
        clip["sourceFrames"], "sourceFrames", minimum=1
    ):
        raise ValidationDomainError("frame_out_of_bounds")
    if project_id and clip.get("projectId", project_id) != project_id:
        raise ValidationDomainError("clip belongs to another project")
    if episode_id and clip.get("episodeId", episode_id) != episode_id:
        raise ValidationDomainError("clip belongs to another episode")
    if (
        clip.get("acceptedCurrent", True) is not True
        or clip.get("derivativeStatus", "ready") != "ready"
    ):
        raise ValidationDomainError("clip source is not timeline eligible")
    if "transform" in clip:
        _validate_transform(clip["transform"])
    transition = clip.get("transition", {"type": "cut", "durationFrames": 0})
    if isinstance(transition, str):
        transition = {"type": transition, "durationFrames": 0}
        clip["transition"] = transition
    if not isinstance(transition, dict) or set(transition) != {"type", "durationFrames"}:
        raise ValidationDomainError("transition is invalid")
    kind, duration = transition["type"], transition["durationFrames"]
    _integer_frame(duration, "transition durationFrames")
    if kind not in {"cut", "crossfade"} or (kind == "cut" and duration != 0):
        raise ValidationDomainError("transition must be cut or crossfade")
    clip_duration = cast(int, clip["outFrame"]) - cast(int, clip["inFrame"])
    if kind == "crossfade" and (duration < 1 or duration >= clip_duration):
        raise ValidationDomainError("crossfade duration is out of bounds")
    clip["durationFrames"] = clip_duration


def _validate_clip_layout(clips: list[dict[str, object]]) -> None:
    ordered = sorted(clips, key=lambda item: cast(int, item["timelineStart"]))
    for index, clip in enumerate(ordered):
        _validate_clip(clip)
        if index == 0:
            continue
        previous = ordered[index - 1]
        previous_end = cast(int, previous["timelineStart"]) + cast(int, previous["durationFrames"])
        transition = cast(dict[str, object], clip["transition"])
        expected_start = previous_end - cast(int, transition["durationFrames"])
        if cast(int, clip["timelineStart"]) != expected_start:
            raise ValidationDomainError(
                "clips must be adjacent with only bounded crossfade overlap"
            )


def _replace_source(
    clip: dict[str, object], payload: dict[str, object], project_id: str, episode_id: str
) -> None:
    old = payload.get("old_source")
    new = payload.get("new_source")
    if not isinstance(old, dict) or not isinstance(new, AssetSelection):
        raise ValidationDomainError("replace source requires exact old and new fingerprints")
    expected = {
        "assetVersionId": clip.get("assetVersionId"),
        "assetVersionRevision": clip.get("assetVersionRevision"),
        "assetVersionHash": clip.get("assetVersionHash"),
        "derivativeFingerprint": clip.get("derivativeFingerprint"),
    }
    if old != expected:
        raise ValidationDomainError("old source fingerprint mismatch")
    if new.project_id != project_id or new.episode_id != episode_id:
        raise ValidationDomainError("replacement source scope mismatch")
    if clip.get("shotId") and new.shot_id != clip.get("shotId"):
        raise ValidationDomainError("replacement source shot mismatch")
    if cast(int, clip["inFrame"]) + cast(int, clip["durationFrames"]) > new.available_frames:
        raise ValidationDomainError("replacement source has insufficient frames")
    clip.update(
        {
            "assetVersionId": new.asset_version_id,
            "assetVersionRevision": new.asset_version_revision,
            "assetVersionHash": new.asset_version_hash,
            "derivativeFingerprint": new.derivative_fingerprint,
        }
    )
