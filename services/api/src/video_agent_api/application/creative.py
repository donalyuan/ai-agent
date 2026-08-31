"""Projects owner 的创作配置应用服务。"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, cast
from uuid import uuid4

from video_agent_api.domain.creative import (
    CreationMode,
    CreativeBriefSourceBindingSnapshot,
    CreativeBriefVersion,
    ProjectCreativeSettingsVersion,
    ProjectEpisodeTextHandoff,
    ProjectEpisodeTextHandoffAck,
    _hash,
    ensure_revision,
)
from video_agent_api.domain.errors import ProjectNotFoundError, ValidationDomainError
from video_agent_api.domain.source_material import SourceMaterial, SourceMaterialVersion


@dataclass(frozen=True, slots=True)
class SaveCreativeBriefCommand:
    project_id: str
    creation_mode: CreationMode
    fields: dict[str, object]
    expected_project_revision: int
    expected_brief_revision: int | None = None


@dataclass(frozen=True, slots=True)
class SaveCreativeSettingsCommand:
    project_id: str
    threshold: dict[str, str] | None
    expected_project_revision: int
    expected_settings_revision: int | None = None


@dataclass(frozen=True, slots=True)
class BindCreativeSourceCommand:
    project_id: str
    snapshot: CreativeBriefSourceBindingSnapshot
    expected_project_revision: int
    expected_brief_revision: int


class CreativeService:
    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def get_projection(self, project_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(project_id)
            if project is None:
                raise ProjectNotFoundError(project_id)
            return {
                "projectId": project_id,
                "projectRevision": project.revision,
                "creationMode": getattr(project, "creation_mode", None)
                or uow.creative_modes.get(project_id),
                "creativeBrief": _public(
                    getattr(project, "creative_brief_current", None)
                    or uow.creative_brief_current.get(project_id)
                ),
                "creativeBriefHistory": [
                    _public(item)
                    for item in (
                        getattr(project, "creative_brief_history", [])
                        or uow.creative_briefs.get(project_id, [])
                    )
                ],
                "settings": _public(
                    getattr(project, "creative_settings_current", None)
                    or uow.creative_settings_current.get(project_id)
                ),
                "settingsHistory": [
                    _public(item)
                    for item in (
                        getattr(project, "creative_settings_history", [])
                        or uow.creative_settings.get(project_id, [])
                    )
                ],
                "sourceBinding": _public(
                    getattr(project, "source_binding_current", None)
                    or uow.source_bindings_current.get(project_id)
                ),
                "storySpecRef": getattr(project, "story_spec_ref", None),
            }

    async def save_brief(self, command: SaveCreativeBriefCommand) -> CreativeBriefVersion:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            if project is None:
                raise ProjectNotFoundError(command.project_id)
            ensure_revision(command.expected_project_revision, project.revision, project.id)
            current = getattr(
                project, "creative_brief_current", None
            ) or uow.creative_brief_current.get(project.id)
            if command.expected_brief_revision is not None and current is not None:
                ensure_revision(command.expected_brief_revision, current.revision, current.id)
            if command.creation_mode == "original" and (
                getattr(project, "source_binding_current", None)
                or uow.source_bindings_current.get(project.id)
            ):
                raise ValidationDomainError(
                    "original brief cannot retain adaptation source binding"
                )
            fields = command.fields
            brief = CreativeBriefVersion(
                creative_brief_id=current.creative_brief_id if current else str(uuid4()),
                project_id=project.id,
                subject=cast(str, fields.get("subject", "")),
                genre=cast(str, fields.get("genre", "")),
                audience=cast(str, fields.get("audience", "")),
                character_premise=cast(str, fields.get("characterPremise", "")),
                style=cast(str, fields.get("style", "")),
                episode_duration_seconds=cast(int, fields.get("episodeDurationSeconds", 0)),
                episode_count=cast(int, fields.get("episodeCount", 0)),
                scenes_per_episode=cast(int, fields.get("scenesPerEpisode", 0)),
                shots_per_scene=cast(int, fields.get("shotsPerScene", 0)),
                revision=current.revision + 1 if current else 1,
            )
            project.revision += 1
            project.creation_mode = command.creation_mode
            project.creative_brief_current = brief
            project.creative_brief_history = [
                *getattr(project, "creative_brief_history", []),
                brief,
            ]
            uow.creative_modes[project.id] = command.creation_mode
            uow.creative_brief_current[project.id] = brief
            uow.creative_briefs.setdefault(project.id, []).append(brief)
            await uow.projects.save(project)
            uow.audit_events.append(
                {
                    "action": "creative_brief_saved",
                    "projectId": project.id,
                    "revision": project.revision,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "creative.brief.saved",
                    "projectId": project.id,
                    "revision": project.revision,
                }
            )
            await uow.commit()
            return brief

    async def save_settings(
        self, command: SaveCreativeSettingsCommand
    ) -> ProjectCreativeSettingsVersion:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            if project is None:
                raise ProjectNotFoundError(command.project_id)
            ensure_revision(command.expected_project_revision, project.revision, project.id)
            current = getattr(
                project, "creative_settings_current", None
            ) or uow.creative_settings_current.get(project.id)
            if command.expected_settings_revision is not None and current is not None:
                ensure_revision(command.expected_settings_revision, current.revision, current.id)
            item = ProjectCreativeSettingsVersion(
                project.id, command.threshold, current.revision + 1 if current else 1
            )
            project.revision += 1
            project.creative_settings_current = item
            project.creative_settings_history = [
                *getattr(project, "creative_settings_history", []),
                item,
            ]
            uow.creative_settings_current[project.id] = item
            uow.creative_settings.setdefault(project.id, []).append(item)
            await uow.projects.save(project)
            uow.audit_events.append(
                {
                    "action": "creative_settings_saved",
                    "projectId": project.id,
                    "revision": project.revision,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "creative.settings.saved",
                    "projectId": project.id,
                    "revision": project.revision,
                }
            )
            await uow.commit()
            return item

    async def bind_source(
        self, command: BindCreativeSourceCommand
    ) -> CreativeBriefSourceBindingSnapshot:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            if project is None:
                raise ProjectNotFoundError(command.project_id)
            ensure_revision(command.expected_project_revision, project.revision, project.id)
            if (
                getattr(project, "creation_mode", None) or uow.creative_modes.get(project.id)
            ) != "adaptation":
                raise ValidationDomainError("source binding requires adaptation creationMode")
            brief = getattr(
                project, "creative_brief_current", None
            ) or uow.creative_brief_current.get(project.id)
            if brief is None:
                raise ValidationDomainError("creative brief is not configured")
            ensure_revision(command.expected_brief_revision, brief.revision, brief.id)
            snapshot = command.snapshot
            source = uow.source_materials.get(snapshot.source_material_id)
            if source is None:
                raw = next(
                    (
                        item
                        for item in getattr(project, "source_materials", [])
                        if isinstance(item, dict)
                        and str(item.get("id")) == snapshot.source_material_id
                    ),
                    None,
                )
                if isinstance(raw, dict):
                    versions = [
                        SourceMaterialVersion(**item)
                        for item in raw.get("versions", [])
                        if isinstance(item, dict)
                    ]
                    current_raw = raw.get("current")
                    current = (
                        SourceMaterialVersion(**current_raw)
                        if isinstance(current_raw, dict)
                        else None
                    )
                    source = SourceMaterial(
                        project.id,
                        cast(Any, str(raw.get("material_type", ""))),
                        cast(Any, str(raw.get("input_mode", ""))),
                        current,
                        int(raw.get("revision", 1)),
                        str(raw.get("id")),
                        versions,
                    )
            current_source = source.current if source is not None else None
            if (
                source is None
                or source.project_id != project.id
                or current_source is None
                or snapshot.source_material_revision != source.revision
                or snapshot.source_content_hash != current_source.content_hash
                or snapshot.parse_status != current_source.parse_status
                or snapshot.validation_status != current_source.validation_status
                or snapshot.binding_status != "bound"
                or snapshot.project_id != project.id
                or snapshot.creative_brief_id != brief.creative_brief_id
                or snapshot.creative_brief_payload_hash != brief.payload_hash
            ):
                raise ValidationDomainError("source binding does not match current brief")
            uow.source_bindings_current[project.id] = snapshot
            uow.source_bindings.setdefault(project.id, []).append(snapshot)
            project.source_binding_current = snapshot
            project.source_binding_history = [
                *getattr(project, "source_binding_history", []),
                snapshot,
            ]
            project.revision += 1
            await uow.projects.save(project)
            uow.audit_events.append(
                {
                    "action": "source_binding_saved",
                    "projectId": project.id,
                    "revision": project.revision,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "creative.source.bound",
                    "projectId": project.id,
                    "revision": project.revision,
                }
            )
            await uow.commit()
            return snapshot

    async def apply_handoff(
        self, handoff: ProjectEpisodeTextHandoff
    ) -> ProjectEpisodeTextHandoffAck:
        fingerprint = _hash(handoff)
        async with self._uow_factory() as uow:
            existing = uow.handoff_acks.get(handoff.handoff_id)
            if existing:
                if existing.fingerprint != fingerprint:
                    raise ValidationDomainError("handoff id fingerprint conflict")
                return cast(ProjectEpisodeTextHandoffAck, existing)
            project = await uow.projects.get(handoff.project_id)
            if project is None:
                raise ProjectNotFoundError(handoff.project_id)
            ensure_revision(handoff.project_revision, project.revision, project.id)
            episodes = {
                episode.id: episode for episode in await uow.episodes.list_by_project(project.id)
            }
            refs = sorted(
                handoff.episode_script_refs, key=lambda item: int(cast(int, item.get("number", 0)))
            )
            if set(str(item.get("episodeId")) for item in refs) != set(episodes):
                raise ValidationDomainError("handoff must include complete episode set")
            for item in refs:
                episode = episodes[str(item["episodeId"])]
                ensure_revision(
                    int(cast(int, item.get("expectedRevision", episode.revision))),
                    episode.revision,
                    episode.id,
                )
                script_ref = item.get("scriptSpecRef")
                if not isinstance(script_ref, dict):
                    raise ValidationDomainError("scriptSpecRef is required for every episode")
                if str(script_ref.get("projectId", handoff.project_id)) != handoff.project_id:
                    raise ValidationDomainError("scriptSpecRef project scope is invalid")
            story_ref = {
                "id": handoff.story_spec_id,
                "revision": handoff.story_spec_revision,
                "hash": handoff.story_spec_hash,
                "projectId": handoff.project_id,
            }
            project.story_spec_ref = story_ref
            project.story_spec_history = [*getattr(project, "story_spec_history", []), story_ref]
            for item in refs:
                episode = episodes[str(item["episodeId"])]
                script_ref = dict(cast(dict[str, object], item["scriptSpecRef"]))
                script_ref.setdefault("projectId", handoff.project_id)
                episode.script_spec_ref = script_ref
                episode.script_spec_history = [
                    *getattr(episode, "script_spec_history", []),
                    script_ref,
                ]
                episode.revision += 1
                await uow.episodes.save(episode)
            project.revision += 1
            await uow.projects.save(project)
            ack = ProjectEpisodeTextHandoffAck(
                handoff.handoff_id,
                fingerprint,
                project.revision,
                tuple((e.id, e.revision) for e in episodes.values()),
                handoff.correlation_id,
            )
            uow.handoff_acks[handoff.handoff_id] = ack
            uow.audit_events.append(
                {
                    "action": "text_handoff_applied",
                    "projectId": project.id,
                    "handoffId": handoff.handoff_id,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "creative.text.handoff.applied",
                    "projectId": project.id,
                    "handoffId": handoff.handoff_id,
                }
            )
            await uow.commit()
            return ack

    async def list_episodes_projection(self, project_id: str) -> list[dict[str, object]]:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(project_id)
            if project is None:
                raise ProjectNotFoundError(project_id)
            episodes = await uow.episodes.list_by_project(project_id)
            return [
                {
                    "episodeId": episode.id,
                    "number": episode.number,
                    "revision": episode.revision,
                    "scriptSpecRef": getattr(episode, "script_spec_ref", None),
                }
                for episode in sorted(episodes, key=lambda item: (item.number, item.id))
            ]


def _public(value: object) -> object:
    if value is None:
        return None
    if hasattr(value, "__dataclass_fields__"):
        from dataclasses import asdict

        return asdict(cast(Any, value))
    return value
