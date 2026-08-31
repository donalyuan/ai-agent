from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, cast

from video_agent_api.domain.errors import (
    EpisodeNotFoundError,
    ProjectNotFoundError,
    ValidationDomainError,
)
from video_agent_api.domain.scenes import (
    SCHEMA_VERSION,
    AcceptedMediaEligibility,
    ImmutableOwnerRef,
    Scene,
    SceneShotBatchHandoff,
    SceneShotOwnerAck,
    Shot,
    reorder_scenes,
    validate_review_decision,
)


@dataclass(frozen=True, slots=True)
class CreateSceneCommand:
    project_id: str
    episode_id: str


@dataclass(frozen=True, slots=True)
class CreateShotCommand:
    project_id: str
    episode_id: str
    scene_id: str


@dataclass(frozen=True, slots=True)
class ReorderScenesCommand:
    project_id: str
    episode_id: str
    scene_ids: list[str]
    expected_revision: int


@dataclass(frozen=True, slots=True)
class ReorderShotsCommand:
    project_id: str
    episode_id: str
    scene_id: str
    shot_ids: list[str]
    expected_revision: int


@dataclass(frozen=True, slots=True)
class ReviewMediaCommand:
    project_id: str
    episode_id: str
    shot_id: str
    decision: str
    candidate: dict[str, object] | None
    expected_shot_revision: int


@dataclass(frozen=True, slots=True)
class AttachContinuityCommand:
    project_id: str
    episode_id: str
    shot_id: str
    snapshot_id: str
    snapshot_revision: int
    snapshot_hash: str
    expected_shot_revision: int


def _ref(value: ImmutableOwnerRef | None, purpose: str) -> dict[str, object] | None:
    if value is None:
        return None
    return {
        "ownerId": value.id,
        "revision": value.revision,
        "contentHash": value.content_hash,
        "purpose": purpose,
    }


def _eligibility(value: AcceptedMediaEligibility | None) -> dict[str, object] | None:
    if value is None:
        return None
    return {
        "candidateId": value.candidate_id,
        "candidateRevision": value.candidate_revision,
        "projectId": value.project_id,
        "episodeId": value.episode_id,
        "targetId": value.target_id,
        "assetVersionId": value.asset_version_id,
        "assetVersionRevision": value.asset_version_revision,
        "assetVersionHash": value.asset_version_hash,
        "provenance": value.provenance,
        "mediaKind": value.media_kind,
        "shotSpecRevision": value.shot_spec_revision,
        "shotSpecHash": value.shot_spec_hash,
        "durationMs": value.duration_ms,
        "aspectRatio": value.aspect_ratio,
        "derivativeStatus": value.derivative_status,
        "status": "accepted_current",
        "timelineReady": value.timeline_ready,
    }


def _shot_projection(value: Shot) -> dict[str, object]:
    return {
        "id": value.id,
        "projectId": value.project_id,
        "episodeId": value.episode_id,
        "sceneId": value.scene_id,
        "number": value.display_number,
        "schemaVersion": value.schema_version,
        "revision": value.revision,
        "status": value.status,
        "specRef": _ref(value.spec_ref, "shotSpec"),
        "continuitySnapshot": _ref(value.continuity_snapshot, "continuitySnapshot"),
        "continuityTasks": [
            _ref(item, "continuityRevisionTask") for item in value.continuity_task_refs
        ],
        "currentImage": _eligibility(value.current_image),
        "currentVideo": _eligibility(value.current_video),
    }


def _scene_projection(value: Scene) -> dict[str, object]:
    return {
        "id": value.id,
        "projectId": value.project_id,
        "episodeId": value.episode_id,
        "number": value.display_number,
        "title": value.title,
        "schemaVersion": value.schema_version,
        "revision": value.revision,
        "status": value.status,
        "specRef": _ref(value.spec_ref, "sceneSpec"),
        "shots": [_shot_projection(shot) for shot in value.shots],
    }


def scene_projection(value: Scene) -> dict[str, object]:
    return _scene_projection(value)


def shot_projection(value: Shot) -> dict[str, object]:
    return _shot_projection(value)


async def _accept_current_media(
    uow: Any,
    *,
    project_id: str,
    episode_id: str,
    shot_id: str,
    candidate: dict[str, object],
    expected_shot_revision: int,
    media_owner: Any | None = None,
) -> Shot:
    shot = uow.shots.get(shot_id)
    if shot is None or shot.project_id != project_id or shot.episode_id != episode_id:
        raise ValidationDomainError("shot scope is invalid")
    required = {
        "candidateId",
        "candidateRevision",
        "projectId",
        "episodeId",
        "targetId",
        "assetVersionId",
        "assetVersionRevision",
        "assetVersionHash",
        "provenance",
        "mediaKind",
    }
    if required - set(candidate):
        raise ValidationDomainError("accepted media candidate is required")
    if (
        candidate["projectId"] != shot.project_id
        or candidate["episodeId"] != shot.episode_id
        or candidate["targetId"] != shot.id
    ):
        raise ValidationDomainError("media candidate scope is stale or foreign")
    asset_version = await uow.asset_versions.get(str(candidate["assetVersionId"]))
    if (
        asset_version is None
        or asset_version.project_id != shot.project_id
        or asset_version.revision != candidate["assetVersionRevision"]
        or asset_version.content_hash != candidate["assetVersionHash"]
    ):
        raise ValidationDomainError("media candidate AssetVersion is stale or foreign")
    eligibility = AcceptedMediaEligibility(
        candidate_id=str(candidate["candidateId"]),
        candidate_revision=int(str(candidate["candidateRevision"])),
        project_id=str(candidate["projectId"]),
        episode_id=str(candidate["episodeId"]),
        target_id=str(candidate["targetId"]),
        asset_version_id=str(candidate["assetVersionId"]),
        asset_version_revision=int(str(candidate["assetVersionRevision"])),
        asset_version_hash=str(candidate["assetVersionHash"]),
        provenance=str(candidate["provenance"]),
        media_kind=str(candidate["mediaKind"]),  # type: ignore[arg-type]
        shot_spec_revision=(
            int(str(candidate["shotSpecRevision"]))
            if candidate.get("shotSpecRevision") is not None
            else None
        ),
        shot_spec_hash=(
            str(candidate["shotSpecHash"]) if candidate.get("shotSpecHash") is not None else None
        ),
        duration_ms=(
            int(str(candidate["durationMs"])) if candidate.get("durationMs") is not None else None
        ),
        aspect_ratio=(
            str(candidate["aspectRatio"]) if candidate.get("aspectRatio") is not None else None
        ),
        derivative_status=str(candidate.get("derivativeStatus", "pending")),
    )
    existing = shot.current_image if eligibility.media_kind == "image" else shot.current_video
    if existing is not None and existing.candidate_id == eligibility.candidate_id:
        if existing != eligibility:
            raise ValidationDomainError("media candidate idempotency conflict")
        return cast(Shot, shot)
    if shot.revision != expected_shot_revision:
        from video_agent_api.domain.errors import RevisionConflictError

        raise RevisionConflictError(shot.id, expected_shot_revision, shot.revision)
    if shot.continuity_snapshot is None or shot.continuity_task_refs:
        raise ValidationDomainError(
            "accepted continuity snapshot without pending tasks is required"
        )
    if eligibility.media_kind == "image":
        shot.current_image = eligibility
    else:
        if shot.spec_ref is None:
            raise ValidationDomainError("video candidate requires current ShotSpec")
        if (
            eligibility.shot_spec_revision != shot.spec_ref.revision
            or eligibility.shot_spec_hash != shot.spec_ref.content_hash
        ):
            raise ValidationDomainError("video candidate ShotSpec is stale")
        shot.current_video = eligibility
    shot.revision += 1
    uow.audit_events.append(
        {
            "type": "media.reviewed",
            "shotId": shot.id,
            "decision": "accept",
            "candidateId": eligibility.candidate_id,
        }
    )
    uow.outbox_events.append({"type": "media.reviewed", "shotId": shot.id, "decision": "accept"})
    if media_owner is not None:
        # The review command is an eligibility projection, not the candidate owner
        # record.  The producer gate needs the transaction's accepted state too.
        accepted_projection = {**candidate, "status": "accepted"}
        await media_owner.produce_generated_candidate(
            uow, candidate=accepted_projection, asset_version=asset_version
        )
    return cast(Shot, shot)


class ScenesService:
    def __init__(self, uow_factory: Any, media_owner: Any | None = None) -> None:
        self._uow_factory = uow_factory
        self._media_owner = media_owner

    async def create_scene(self, command: CreateSceneCommand) -> Scene:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            episode = await uow.episodes.get(command.episode_id)
            if project is None:
                raise ProjectNotFoundError(command.project_id)
            if episode is None or episode.project_id != project.id:
                raise EpisodeNotFoundError(command.episode_id)
            existing = uow.scenes_by_episode.setdefault(episode.id, [])
            uow.scene_order_revisions.setdefault(episode.id, 1)
            item = Scene(
                project.id,
                episode.id,
                len(existing) + 1,
                title=f"Scene {len(existing) + 1}",
            )
            existing.append(item)
            uow.scenes[item.id] = item
            uow.audit_events.append({"type": "scene.created", "sceneId": item.id})
            uow.outbox_events.append({"type": "scene.created", "sceneId": item.id})
            await uow.commit()
            return item

    async def create_shot(self, command: CreateShotCommand) -> Shot:
        async with self._uow_factory() as uow:
            scene = uow.scenes.get(command.scene_id)
            if (
                scene is None
                or scene.project_id != command.project_id
                or scene.episode_id != command.episode_id
            ):
                raise ValidationDomainError("scene scope is invalid")
            item = Shot(scene.id, scene.project_id, scene.episode_id, len(scene.shots) + 1)
            scene.shots.append(item)
            uow.shots[item.id] = item
            scene.revision += 1
            uow.audit_events.append({"type": "shot.created", "shotId": item.id})
            uow.outbox_events.append({"type": "shot.created", "shotId": item.id})
            await uow.commit()
            return item

    async def list_episode(self, project_id: str, episode_id: str) -> list[dict[str, object]]:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
            if episode is None or episode.project_id != project_id:
                raise EpisodeNotFoundError(episode_id)
            scenes = sorted(
                uow.scenes_by_episode.get(episode_id, []), key=lambda item: item.display_number
            )
            order_revision = uow.scene_order_revisions.get(episode_id, 1)
            return [
                {**_scene_projection(scene), "sceneOrderRevision": order_revision}
                for scene in scenes
            ]

    async def workflow_scope(self, project_id: str, episode_id: str) -> dict[str, object]:
        scenes = await self.list_episode(project_id, episode_id)
        return {
            "projectId": project_id,
            "episodeId": episode_id,
            "schemaVersion": SCHEMA_VERSION,
            "scenes": scenes,
        }

    async def reorder_shots(self, command: ReorderShotsCommand) -> list[Shot]:
        async with self._uow_factory() as uow:
            scene = uow.scenes.get(command.scene_id)
            if (
                scene is None
                or scene.project_id != command.project_id
                or scene.episode_id != command.episode_id
            ):
                raise ValidationDomainError("scene scope is invalid")
            scene.reorder_shots(command.shot_ids, command.expected_revision)
            uow.audit_events.append({"type": "shots.reordered", "sceneId": scene.id})
            uow.outbox_events.append({"type": "shots.reordered", "sceneId": scene.id})
            await uow.commit()
            return cast(list[Shot], scene.shots)

    async def append_spec(
        self,
        project_id: str,
        episode_id: str,
        scene_id: str,
        payload: dict[str, object],
        shot_id: str | None = None,
    ) -> object:
        async with self._uow_factory() as uow:
            scene = uow.scenes.get(scene_id)
            if scene is None or scene.project_id != project_id or scene.episode_id != episode_id:
                raise ValidationDomainError("scene scope is invalid")
            if shot_id is not None:
                shot = next((item for item in scene.shots if item.id == shot_id), None)
                if shot is None:
                    raise ValidationDomainError("shot scope is invalid")
                spec = scene.append_shot_spec(shot, payload)
            else:
                spec = scene.append_spec(payload)
            uow.audit_events.append({"type": "spec.appended", "specId": spec.id})
            uow.outbox_events.append({"type": "spec.appended", "specId": spec.id})
            await uow.commit()
            return spec

    async def reorder_scenes(self, command: ReorderScenesCommand) -> list[Scene]:
        async with self._uow_factory() as uow:
            scenes = uow.scenes_by_episode.get(command.episode_id, [])
            if any(item.project_id != command.project_id for item in scenes):
                raise ValidationDomainError("scene scope is foreign")
            current_revision = uow.scene_order_revisions.get(command.episode_id, 1)
            uow.scene_order_revisions[command.episode_id] = reorder_scenes(
                scenes, command.scene_ids, command.expected_revision, current_revision
            )
            uow.audit_events.append({"type": "scenes.reordered", "episodeId": command.episode_id})
            uow.outbox_events.append({"type": "scenes.reordered", "episodeId": command.episode_id})
            await uow.commit()
            return cast(list[Scene], scenes)

    async def review_media(self, command: ReviewMediaCommand) -> Shot:
        action = validate_review_decision(command.decision)
        async with self._uow_factory() as uow:
            if action == "accept":
                shot = await _accept_current_media(
                    uow,
                    project_id=command.project_id,
                    episode_id=command.episode_id,
                    shot_id=command.shot_id,
                    candidate=command.candidate or {},
                    expected_shot_revision=command.expected_shot_revision,
                    media_owner=self._media_owner,
                )
                await uow.commit()
                return shot
            shot = uow.shots.get(command.shot_id)
            if (
                shot is None
                or shot.project_id != command.project_id
                or shot.episode_id != command.episode_id
            ):
                raise ValidationDomainError("shot scope is invalid")
            if shot.revision != command.expected_shot_revision:
                from video_agent_api.domain.errors import RevisionConflictError

                raise RevisionConflictError(shot.id, command.expected_shot_revision, shot.revision)
            uow.audit_events.append(
                {
                    "type": "media.reviewed",
                    "shotId": shot.id,
                    "decision": action,
                    "candidateId": (command.candidate or {}).get("candidateId"),
                }
            )
            uow.outbox_events.append(
                {"type": "media.reviewed", "shotId": shot.id, "decision": action}
            )
            await uow.commit()
            return cast(Shot, shot)

    async def accept_current_media_in_transaction(
        self,
        uow: Any,
        *,
        project_id: str,
        episode_id: str,
        shot_id: str,
        candidate: dict[str, object],
        expected_shot_revision: int,
    ) -> Shot:
        return await _accept_current_media(
            uow,
            project_id=project_id,
            episode_id=episode_id,
            shot_id=shot_id,
            candidate=candidate,
            expected_shot_revision=expected_shot_revision,
            media_owner=self._media_owner,
        )

    async def update_derivative_in_transaction(
        self,
        uow: Any,
        *,
        project_id: str,
        shot_id: str,
        candidate_id: str,
        derivative_status: str,
    ) -> Shot:
        if derivative_status not in {"pending", "ready", "failed", "stale"}:
            raise ValidationDomainError("media derivative status is invalid")
        shot = uow.shots.get(shot_id)
        if shot is None or shot.project_id != project_id:
            raise ValidationDomainError("shot scope is invalid")
        current = next(
            (
                item
                for item in (shot.current_image, shot.current_video)
                if item is not None and item.candidate_id == candidate_id
            ),
            None,
        )
        if current is None:
            raise ValidationDomainError("accepted current media candidate is stale or foreign")
        updated = replace(current, derivative_status=derivative_status)
        if current == updated:
            return cast(Shot, shot)
        if current.media_kind == "image":
            shot.current_image = updated
        else:
            shot.current_video = updated
        shot.revision += 1
        uow.audit_events.append(
            {
                "type": "media.derivative.updated",
                "shotId": shot.id,
                "candidateId": candidate_id,
                "status": derivative_status,
            }
        )
        uow.outbox_events.append(
            {
                "type": "media.derivative.updated",
                "shotId": shot.id,
                "candidateId": candidate_id,
                "status": derivative_status,
            }
        )
        return cast(Shot, shot)

    async def attach_continuity(self, command: AttachContinuityCommand) -> Shot:
        async with self._uow_factory() as uow:
            shot = uow.shots.get(command.shot_id)
            if (
                shot is None
                or shot.project_id != command.project_id
                or shot.episode_id != command.episode_id
            ):
                raise ValidationDomainError("shot scope is invalid")
            if shot.revision != command.expected_shot_revision:
                from video_agent_api.domain.errors import RevisionConflictError

                raise RevisionConflictError(shot.id, command.expected_shot_revision, shot.revision)
            snapshot = uow.asset_bible_snapshots.get(command.snapshot_id)
            if (
                snapshot is None
                or snapshot.project_id != shot.project_id
                or snapshot.target_id != shot.id
                or snapshot.status != "accepted"
                or snapshot.revision != command.snapshot_revision
                or snapshot.content_hash != command.snapshot_hash
            ):
                raise ValidationDomainError("continuity snapshot is incomplete, stale or foreign")
            pending_tasks = [
                task
                for task in uow.asset_bible_tasks.values()
                if task.project_id == shot.project_id
                and task.target_id == shot.id
                and task.status in {"pending", "acknowledged"}
            ]
            shot.continuity_snapshot = ImmutableOwnerRef(
                snapshot.id, snapshot.revision, snapshot.content_hash
            )
            shot.continuity_task_refs = [
                ImmutableOwnerRef(
                    task.id,
                    task.revision,
                    task.snapshot_hash or snapshot.content_hash,
                )
                for task in pending_tasks
            ]
            shot.revision += 1
            uow.audit_events.append(
                {
                    "type": "shot.continuity-attached",
                    "shotId": shot.id,
                    "snapshotId": snapshot.id,
                    "pendingTaskIds": [task.id for task in pending_tasks],
                }
            )
            uow.outbox_events.append(
                {
                    "type": "shot.continuity-attached",
                    "shotId": shot.id,
                    "snapshotId": snapshot.id,
                }
            )
            await uow.commit()
            return cast(Shot, shot)

    async def review_video(
        self, shot_id: str, decision: str, candidate: dict[str, object] | None = None
    ) -> Shot:
        """Compatibility method with an intentionally impossible implicit scope.

        The HTTP API uses ``review_media``.  Older callers can only exercise verb
        validation; valid writes require explicit owner scope and expected revision.
        """

        validate_review_decision(decision)
        raise ValidationDomainError("explicit media review scope is required")

    async def apply_text_handoff(self, handoff: SceneShotBatchHandoff) -> SceneShotOwnerAck:
        async with self._uow_factory() as uow:
            previous = uow.scene_handoff_acks.get(handoff.handoff_id)
            if previous is not None:
                if previous.payload_hash != handoff.payload_hash:
                    raise ValidationDomainError("scene/shot handoff idempotency conflict")
                return cast(SceneShotOwnerAck, previous)
            project = await uow.projects.get(handoff.project_id)
            episode = await uow.episodes.get(handoff.episode_id)
            if project is None or episode is None or episode.project_id != handoff.project_id:
                raise ValidationDomainError("scene/shot handoff scope is invalid")
            if episode.status in {"archived", "published"}:
                raise ValidationDomainError("scene/shot handoff parent is immutable")

            prepared: list[
                tuple[Scene, list[tuple[Shot, dict[str, object]]], dict[str, object]]
            ] = []
            seen_candidates: set[str] = set()
            for scene_number, item in enumerate(handoff.scenes, 1):
                candidate_id = str(item.get("candidateId", ""))
                source_hash = str(item.get("sourceHash", ""))
                payload = item.get("payload")
                shots = item.get("shots")
                if (
                    not candidate_id
                    or len(source_hash) != 64
                    or not isinstance(payload, dict)
                    or not isinstance(shots, list)
                    or not shots
                    or candidate_id in seen_candidates
                ):
                    raise ValidationDomainError("scene/shot handoff candidate is invalid")
                seen_candidates.add(candidate_id)
                scene_id = str(item.get("sceneId") or __import__("uuid").uuid4())
                if item.get("expectedRevision", 0) != 0 or scene_id in uow.scenes:
                    raise ValidationDomainError("initial scene handoff revision conflicts")
                scene = Scene(
                    handoff.project_id,
                    handoff.episode_id,
                    scene_number,
                    title=str(payload.get("title") or f"Scene {scene_number}"),
                    id=scene_id,
                )
                shot_values: list[tuple[Shot, dict[str, object]]] = []
                for shot_number, shot_item in enumerate(shots, 1):
                    if not isinstance(shot_item, dict):
                        raise ValidationDomainError("shot handoff candidate is invalid")
                    shot_candidate_id = str(shot_item.get("candidateId", ""))
                    shot_source_hash = str(shot_item.get("sourceHash", ""))
                    shot_payload = shot_item.get("payload")
                    if (
                        not shot_candidate_id
                        or shot_candidate_id in seen_candidates
                        or len(shot_source_hash) != 64
                        or not isinstance(shot_payload, dict)
                        or shot_item.get("expectedRevision", 0) != 0
                    ):
                        raise ValidationDomainError("shot handoff candidate is invalid")
                    seen_candidates.add(shot_candidate_id)
                    shot_id = str(shot_item.get("shotId") or __import__("uuid").uuid4())
                    if shot_id in uow.shots:
                        raise ValidationDomainError("initial shot handoff revision conflicts")
                    shot_values.append(
                        (
                            Shot(
                                scene.id,
                                handoff.project_id,
                                handoff.episode_id,
                                shot_number,
                                id=shot_id,
                            ),
                            shot_payload,
                        )
                    )
                prepared.append((scene, shot_values, payload))

            scene_ids: list[str] = []
            shot_ids: list[str] = []
            for scene, shot_values, scene_payload in prepared:
                scene.append_spec(scene_payload)
                for shot, shot_payload in shot_values:
                    scene.shots.append(shot)
                    scene.append_shot_spec(shot, shot_payload)
                    uow.shots[shot.id] = shot
                    shot_ids.append(shot.id)
                uow.scenes[scene.id] = scene
                uow.scenes_by_episode.setdefault(handoff.episode_id, []).append(scene)
                uow.scene_order_revisions.setdefault(handoff.episode_id, 1)
                scene_ids.append(scene.id)
            ack = SceneShotOwnerAck(
                handoff.handoff_id,
                handoff.project_id,
                handoff.episode_id,
                tuple(scene_ids),
                tuple(shot_ids),
                handoff.payload_hash,
                handoff.correlation_id,
            )
            uow.scene_handoff_acks[handoff.handoff_id] = ack
            uow.audit_events.append(
                {"type": "scene-shot.handoff.applied", "handoffId": handoff.handoff_id}
            )
            uow.outbox_events.append(
                {"type": "scene-shot.handoff.applied", "handoffId": handoff.handoff_id}
            )
            await uow.commit()
            return ack

    async def unsupported_structure_edit(self) -> None:
        raise ValidationDomainError("unsupported_feature")
