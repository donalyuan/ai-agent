"""Timeline commands, owner resolvers and current/version projections."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from typing import Any, cast
from uuid import uuid4

from video_agent_api.domain.errors import (
    AssetVersionNotFoundError,
    EpisodeNotFoundError,
    RevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.domain.media import source_fingerprint
from video_agent_api.domain.timeline import (
    AssetSelection,
    Ducking,
    SoundCue,
    TimelineCut,
    TimelineVersion,
)


@dataclass(frozen=True, slots=True)
class EditTimelineCommand:
    project_id: str
    episode_id: str
    expected_revision: int
    operation: str
    payload: dict[str, object]


@dataclass(frozen=True, slots=True)
class PublishTimelineCommand:
    project_id: str
    episode_id: str
    expected_revision: int
    name: str


@dataclass(frozen=True, slots=True)
class TimelinePublishPreflight:
    """Read-only owner result used before a user confirms publication."""

    cut_id: str
    expected_revision: int
    timeline_fingerprint: str


def timeline_cut_projection(cut: TimelineCut) -> dict[str, object]:
    return {
        "id": cut.id,
        "projectId": cut.project_id,
        "episodeId": cut.episode_id,
        "schemaVersion": cut.schema_version,
        "revision": cut.revision,
        "fps": 30,
        "clips": [dict(item) for item in cut.clips],
        "soundCues": [_sound_cue_projection(item) for item in cut.cues],
        "captions": [dict(item) for item in cut.captions],
        "ducking": _ducking_projection(cut.ducking),
        "timelineFingerprint": cut.fingerprint(),
    }


def timeline_version_projection(version: TimelineVersion) -> dict[str, object]:
    return {
        "id": version.id,
        "projectId": version.project_id,
        "episodeId": version.episode_id,
        "schemaVersion": version.schema_version,
        "revision": version.revision,
        "sourceCutRevision": version.source_cut_revision,
        "name": version.name,
        "snapshot": version.cut_snapshot,
    }


def _sound_cue_projection(cue: SoundCue) -> dict[str, object]:
    value = asdict(cue)
    return {
        "id": value["id"],
        "track": value["track"],
        "assetVersionId": value["asset_version_id"],
        "assetVersionRevision": value["asset_version_revision"],
        "assetVersionHash": value["asset_version_hash"],
        "startFrame": value["start_frame"],
        "durationFrames": value["duration_frames"],
        "trigger": value["trigger"],
        "triggerRef": value["trigger_ref"],
        "priority": value["priority"],
        "continuityRefs": value["continuity_refs"],
        "gainDb": value["gain_db"],
        "mute": value["mute"],
        "solo": value["solo"],
        "fadeInFrames": value["fade_in_frames"],
        "fadeOutFrames": value["fade_out_frames"],
        "authorizationStatus": value["authorization_status"],
        "licenseStatus": value["license_status"],
    }


def _ducking_projection(ducking: Ducking | None) -> dict[str, object] | None:
    if ducking is None:
        return None
    return {
        "enabled": ducking.enabled,
        "dialogueIntervals": [list(item) for item in ducking.dialogue_intervals],
        "attenuationDb": ducking.attenuation_db,
        "attackFrames": ducking.attack_frames,
        "releaseFrames": ducking.release_frames,
        "targetTracks": list(ducking.target_tracks),
    }


def _integer(value: object, field_name: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ValidationDomainError(f"{field_name} must be an integer")
    return value


class TimelineService:
    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def get_cut(self, episode_id: str, project_id: str | None = None) -> TimelineCut:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
            if episode is None or (project_id is not None and episode.project_id != project_id):
                raise EpisodeNotFoundError(episode_id)
            cut = uow.timeline_cuts.get(episode_id)
            # The first owner command persists the unique Cut; reads remain side-effect free.
            return cut if cut is not None else TimelineCut(episode_id, episode.project_id)

    async def edit(
        self,
        episode_id: str,
        expected_revision: int,
        command: str,
        payload: dict[str, object] | None = None,
        project_id: str | None = None,
    ) -> TimelineCut:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
            if episode is None or (project_id is not None and episode.project_id != project_id):
                raise EpisodeNotFoundError(episode_id)
            cut = uow.timeline_cuts.get(episode_id)
            if cut is None:
                cut = TimelineCut(episode_id, episode.project_id)
            elif cut.project_id and cut.project_id != episode.project_id:
                raise ValidationDomainError("current Cut owner scope is invalid")
            prepared = await self._prepare_payload(uow, cut, command, payload or {})
            cut.edit(expected_revision, command, **prepared)
            uow.timeline_cuts[episode_id] = cut
            uow.audit_events.append(
                {
                    "type": "timeline.edited",
                    "projectId": episode.project_id,
                    "episodeId": episode_id,
                    "cutId": cut.id,
                    "revision": cut.revision,
                    "command": command,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "timeline.edited",
                    "episodeId": episode_id,
                    "cutId": cut.id,
                    "revision": cut.revision,
                }
            )
            await uow.commit()
            return cast(TimelineCut, cut)

    async def publish(
        self,
        episode_id: str,
        name: str,
        expected_revision: int,
        project_id: str | None = None,
    ) -> TimelineVersion:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
            if episode is None or (project_id is not None and episode.project_id != project_id):
                raise EpisodeNotFoundError(episode_id)
            cut = uow.timeline_cuts.get(episode_id)
            if cut is None:
                raise ValidationDomainError("current Cut has not been persisted")
            if expected_revision != cut.revision:
                raise RevisionConflictError(cut.id, expected_revision, cut.revision)
            self._preflight(cut)
            snapshot = {
                "schema_version": cut.schema_version,
                "cutId": cut.id,
                "timelineFingerprint": cut.fingerprint(),
                "clips": [dict(item) for item in cut.clips],
                "soundCues": [_sound_cue_projection(cue) for cue in cut.cues],
                "captions": [dict(item) for item in cut.captions],
                "ducking": _ducking_projection(cut.ducking),
                "masterLimiter": {"integratedLufs": -14, "truePeakDbtp": -1},
            }
            version = TimelineVersion(
                episode_id=cut.episode_id,
                source_cut_revision=cut.revision,
                name=name,
                cut_snapshot=snapshot,
                project_id=episode.project_id,
                schema_version=cut.schema_version,
            )
            uow.timeline_versions[version.id] = version
            uow.audit_events.append(
                {
                    "type": "timeline.published",
                    "episodeId": episode_id,
                    "timelineVersionId": version.id,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "timeline.published",
                    "episodeId": episode_id,
                    "timelineVersionId": version.id,
                }
            )
            await uow.commit()
            return version

    async def preflight_publish(
        self,
        episode_id: str,
        expected_revision: int,
        project_id: str | None = None,
    ) -> TimelinePublishPreflight:
        """Validate the exact current Cut without creating a TimelineVersion."""
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
            if episode is None or (project_id is not None and episode.project_id != project_id):
                raise EpisodeNotFoundError(episode_id)
            cut = uow.timeline_cuts.get(episode_id)
            if cut is None:
                raise ValidationDomainError("current Cut has not been persisted")
            if expected_revision != cut.revision:
                raise RevisionConflictError(cut.id, expected_revision, cut.revision)
            self._preflight(cut)
            return TimelinePublishPreflight(
                cut_id=cut.id,
                expected_revision=cut.revision,
                timeline_fingerprint=cut.fingerprint(),
            )

    async def get_version(
        self, project_id: str, episode_id: str, version_id: str
    ) -> TimelineVersion:
        async with self._uow_factory() as uow:
            version = uow.timeline_versions.get(version_id)
            if (
                version is None
                or version.project_id != project_id
                or version.episode_id != episode_id
            ):
                raise ValidationDomainError("timeline version not found in owner scope")
            return cast(TimelineVersion, version)

    async def list_versions(self, project_id: str, episode_id: str) -> list[TimelineVersion]:
        async with self._uow_factory() as uow:
            values = [
                value
                for value in uow.timeline_versions.values()
                if value.project_id == project_id and value.episode_id == episode_id
            ]
            return sorted(values, key=lambda item: (item.source_cut_revision, item.id))

    async def _prepare_payload(
        self,
        uow: Any,
        cut: TimelineCut,
        command: str,
        payload: dict[str, object],
    ) -> dict[str, object]:
        prepared = dict(payload)
        if command == "add_clip":
            raw = prepared.get("clip")
            if not isinstance(raw, dict):
                raise ValidationDomainError("clip is required")
            asset_version = await self._asset_version(uow, raw)
            await self._validate_clip_eligibility(uow, cut, raw, asset_version.id)
            raw = dict(raw)
            raw.setdefault("projectId", cut.project_id)
            raw.setdefault("episodeId", cut.episode_id)
            raw.setdefault("assetVersionRevision", asset_version.revision)
            raw.setdefault("assetVersionHash", asset_version.content_hash)
            prepared["clip"] = raw
        elif command == "replace_clip_source":
            raw = prepared.get("new_source")
            if not isinstance(raw, dict):
                raise ValidationDomainError("new source selector is required")
            asset_version = await self._asset_version(uow, raw)
            await self._validate_clip_eligibility(uow, cut, raw, asset_version.id)
            prepared["new_source"] = AssetSelection(
                project_id=str(raw.get("projectId", "")),
                episode_id=str(raw.get("episodeId", "")),
                asset_version_id=asset_version.id,
                asset_version_revision=int(raw.get("assetVersionRevision", -1)),
                asset_version_hash=str(raw.get("assetVersionHash", "")),
                derivative_fingerprint=str(raw.get("derivativeFingerprint", "")),
                authorization_status=cast(Any, raw.get("authorizationStatus", "")),
                license_status=cast(Any, raw.get("licenseStatus", "")),
                available_frames=int(raw.get("availableFrames", 0)),
                accepted_current=raw.get("acceptedCurrent") is True,
                shot_id=str(raw["shotId"]) if raw.get("shotId") is not None else None,
            )
        elif command == "add_sound_cue":
            raw = prepared.get("cue")
            if not isinstance(raw, dict):
                raise ValidationDomainError("sound cue is required")
            expected_fields = {
                "id",
                "projectId",
                "episodeId",
                "assetVersionId",
                "assetVersionRevision",
                "assetVersionHash",
                "explicitSelection",
                "storageVerified",
                "authorizationStatus",
                "licenseStatus",
                "track",
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
            }
            if set(raw) != expected_fields:
                raise ValidationDomainError("sound cue fields are incomplete or aliased")
            asset_version = await self._asset_version(uow, raw)
            await self._validate_audio_selection(uow, cut, raw, asset_version.id)
            self._validate_continuity_refs(
                uow, cut, cast(list[dict[str, object]], raw.get("continuityRefs", []))
            )
            start_frame, trigger_ref = self._resolve_trigger(uow, cut, raw)
            duration_frames = _integer(raw.get("durationFrames"), "durationFrames", minimum=1)
            timeline_end = max(
                (
                    _integer(clip["timelineStart"], "clip timelineStart")
                    + _integer(clip["durationFrames"], "clip durationFrames", minimum=1)
                    for clip in cut.clips
                ),
                default=0,
            )
            if start_frame + duration_frames > timeline_end:
                raise ValidationDomainError("sound cue frame range is outside the timeline")
            prepared["cue"] = SoundCue(
                track=cast(Any, raw.get("track")),
                asset_version_id=asset_version.id,
                start_frame=start_frame,
                duration_frames=duration_frames,
                trigger=cast(Any, raw.get("trigger", "manual")),
                trigger_ref=trigger_ref,
                priority=_integer(raw.get("priority", 0), "priority"),
                continuity_refs=tuple(cast(list[dict[str, object]], raw.get("continuityRefs", []))),
                gain_db=float(raw.get("gainDb", 0.0)),
                mute=raw.get("mute", False) is True,
                solo=raw.get("solo", False) is True,
                fade_in_frames=_integer(raw.get("fadeInFrames", 0), "fadeInFrames"),
                fade_out_frames=_integer(raw.get("fadeOutFrames", 0), "fadeOutFrames"),
                asset_version_revision=asset_version.revision,
                asset_version_hash=asset_version.content_hash,
                authorization_status=str(raw.get("authorizationStatus", "")),
                license_status=str(raw.get("licenseStatus", "")),
                id=str(raw["id"]) if raw.get("id") else str(uuid4()),
            )
        elif command == "set_ducking":
            raw = prepared.get("ducking")
            if not isinstance(raw, dict):
                raise ValidationDomainError("ducking is required")
            if set(raw) != {
                "enabled",
                "dialogueIntervals",
                "attenuationDb",
                "attackFrames",
                "releaseFrames",
                "targetTracks",
            }:
                raise ValidationDomainError("ducking payload is incomplete")
            prepared["ducking"] = Ducking(
                enabled=raw["enabled"] is True,
                dialogue_intervals=tuple(
                    (
                        _integer(cast(list[object], item)[0], "dialogue interval start"),
                        _integer(
                            cast(list[object], item)[1],
                            "dialogue interval end",
                            minimum=1,
                        ),
                    )
                    for item in cast(list[object], raw["dialogueIntervals"])
                ),
                attenuation_db=float(raw["attenuationDb"]),
                attack_frames=_integer(raw["attackFrames"], "attackFrames"),
                release_frames=_integer(raw["releaseFrames"], "releaseFrames"),
                target_tracks=tuple(cast(list[str], raw["targetTracks"])),
            )
        return prepared

    async def _asset_version(self, uow: Any, raw: dict[str, object]) -> Any:
        version_id = str(raw.get("assetVersionId", ""))
        version = await uow.asset_versions.get(version_id)
        if version is None:
            raise AssetVersionNotFoundError(version_id)
        asset = await uow.assets.get(version.asset_id)
        if asset is None or asset.project_id != raw.get("projectId"):
            raise ValidationDomainError("asset selection belongs to another project")
        if (
            version.project_id != raw.get("projectId")
            or version.revision != raw.get("assetVersionRevision")
            or version.content_hash != raw.get("assetVersionHash")
        ):
            raise ValidationDomainError("asset selection is stale or foreign")
        if asset.authorization_status != "verified" or not asset.license:
            raise ValidationDomainError("asset authorization or license is incomplete")
        return version

    async def _validate_clip_eligibility(
        self, uow: Any, cut: TimelineCut, raw: dict[str, object], version_id: str
    ) -> None:
        if raw.get("projectId") != cut.project_id or raw.get("episodeId") != cut.episode_id:
            raise ValidationDomainError("clip selection scope is stale or foreign")
        shot_id = str(raw.get("shotId", ""))
        shot = uow.shots.get(shot_id)
        eligibility = None if shot is None else (shot.current_video or shot.current_image)
        raw_revision = raw.get("assetVersionRevision", -1)
        if isinstance(raw_revision, bool) or not isinstance(raw_revision, int):
            raise ValidationDomainError("asset selection revision is invalid")
        expected_fingerprint = source_fingerprint(
            version_id,
            raw_revision,
            str(raw.get("assetVersionHash", "")),
        )
        derivative = next(
            (
                item
                for item in uow.media_derivatives.values()
                if item.project_id == cut.project_id
                and item.asset_version_id == version_id
                and item.asset_version_revision == raw.get("assetVersionRevision")
                and item.source_hash == raw.get("assetVersionHash")
                and item.source_fingerprint == expected_fingerprint
                and item.source_fingerprint == raw.get("derivativeFingerprint")
                and item.kind == "proxy"
                and item.status == "ready"
            ),
            None,
        )
        if (
            shot is None
            or shot.project_id != cut.project_id
            or shot.episode_id != cut.episode_id
            or eligibility is None
            or eligibility.asset_version_id != version_id
            or not eligibility.accepted
            or eligibility.derivative_status != "ready"
            or raw.get("acceptedCurrent") is not True
            or raw.get("derivativeStatus") != "ready"
            or derivative is None
        ):
            raise ValidationDomainError("clip source is not accepted current with ready derivative")

    async def _validate_audio_selection(
        self, uow: Any, cut: TimelineCut, raw: dict[str, object], version_id: str
    ) -> None:
        version = await uow.asset_versions.get(version_id)
        asset = None if version is None else await uow.assets.get(version.asset_id)
        if raw.get("projectId") != cut.project_id or raw.get("episodeId") != cut.episode_id:
            raise ValidationDomainError("audio selection scope is stale or foreign")
        if (
            version is None
            or asset is None
            or asset.kind != "audio"
            or raw.get("explicitSelection") is not True
            or raw.get("storageVerified") is not True
        ):
            raise ValidationDomainError("audio requires explicit verified selector handoff")
        if raw.get("authorizationStatus") != "authorized" or raw.get("licenseStatus") != "approved":
            raise ValidationDomainError("audio authorization or license is incomplete")

    def _resolve_trigger(
        self, uow: Any, cut: TimelineCut, raw: dict[str, object]
    ) -> tuple[int, dict[str, object] | None]:
        trigger = raw.get("trigger", "manual")
        if trigger == "manual":
            return _integer(raw.get("startFrame"), "startFrame"), None
        trigger_ref = raw.get("triggerRef")
        if not isinstance(trigger_ref, dict) or set(trigger_ref) != {
            "ownerType",
            "id",
            "revision",
            "startFrame",
            "offsetFrames",
        }:
            raise ValidationDomainError("triggerRef is incomplete")
        if (trigger == "scene_start" and trigger_ref["ownerType"] != "scene") or (
            trigger in {"shot_start", "shot_end"} and trigger_ref["ownerType"] != "shot"
        ):
            raise ValidationDomainError("sound cue trigger ownerType does not match trigger")
        owner = (
            uow.scenes.get(trigger_ref["id"])
            if trigger == "scene_start"
            else uow.shots.get(trigger_ref["id"])
        )
        if (
            owner is None
            or owner.episode_id != cut.episode_id
            or owner.project_id != cut.project_id
            or owner.revision != trigger_ref["revision"]
        ):
            raise ValidationDomainError("sound cue trigger target is stale or foreign")
        owner_clips = [
            clip
            for clip in cut.clips
            if (
                clip.get("shotId") == owner.id
                if trigger in {"shot_start", "shot_end"}
                else (
                    (shot := uow.shots.get(str(clip.get("shotId", "")))) is not None
                    and shot.scene_id == owner.id
                )
            )
        ]
        if not owner_clips:
            raise ValidationDomainError("sound cue trigger target has no current Cut position")
        if trigger == "shot_end":
            base_frame = max(
                _integer(clip["timelineStart"], "clip timelineStart")
                + _integer(clip["durationFrames"], "clip durationFrames", minimum=1)
                for clip in owner_clips
            )
        else:
            base_frame = min(
                _integer(clip["timelineStart"], "clip timelineStart") for clip in owner_clips
            )
        claimed_frame = _integer(trigger_ref["startFrame"], "trigger startFrame")
        if claimed_frame != base_frame:
            raise ValidationDomainError("sound cue trigger frame is stale")
        start = base_frame + _integer(trigger_ref["offsetFrames"], "trigger offsetFrames")
        return start, dict(trigger_ref)

    def _validate_continuity_refs(
        self,
        uow: Any,
        cut: TimelineCut,
        refs: list[dict[str, object]],
    ) -> None:
        for ref in refs:
            owner_type = ref.get("ownerType")
            owner_id = str(ref.get("id", ""))
            revision = ref.get("revision")
            content_hash = ref.get("hash")
            if owner_type == "asset_bible":
                version = next(
                    (
                        version
                        for entry in uow.asset_bible_entries.values()
                        for version in entry.versions
                        if version.id == owner_id
                    ),
                    None,
                )
                valid = (
                    version is not None
                    and version.project_id == cut.project_id
                    and version.revision == revision
                    and version.content_hash == content_hash
                )
            elif owner_type in {"scene", "shot"}:
                owner = (
                    uow.scenes.get(owner_id) if owner_type == "scene" else uow.shots.get(owner_id)
                )
                valid = (
                    owner is not None
                    and owner.project_id == cut.project_id
                    and owner.episode_id == cut.episode_id
                    and owner.revision == revision
                    and _timeline_owner_hash(owner) == content_hash
                )
            elif owner_type == "shot_spec":
                spec = next(
                    (
                        spec
                        for shot in uow.shots.values()
                        for spec in shot.spec_versions
                        if spec.id == owner_id
                    ),
                    None,
                )
                valid = (
                    spec is not None
                    and spec.project_id == cut.project_id
                    and spec.episode_id == cut.episode_id
                    and spec.revision == revision
                    and spec.content_hash == content_hash
                )
            else:
                valid = False
            if not valid:
                raise ValidationDomainError("continuityRef owner is stale or foreign")

    def _preflight(self, cut: TimelineCut) -> None:
        if not cut.clips:
            raise ValidationDomainError("timeline preflight requires at least one clip")
        if any(cue.start_frame + cue.duration_frames < 1 for cue in cut.cues):
            raise ValidationDomainError("sound cue is outside the timeline")


def _timeline_owner_hash(owner: object) -> str:
    value = cast(Any, owner)
    return hashlib.sha256(
        json.dumps(
            {
                "id": value.id,
                "revision": value.revision,
                "projectId": value.project_id,
                "episodeId": value.episode_id,
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
    ).hexdigest()
