"""SourceMaterial import/verification orchestration owned by text and storage boundaries."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, cast

from video_agent_api.domain.errors import ProjectNotFoundError, ValidationDomainError
from video_agent_api.domain.source_material import (
    SourceMaterial,
    SourceMaterialUploadIntent,
    SourceMaterialVersion,
    VerifiedStoredObjectHandoff,
)


@dataclass(frozen=True, slots=True)
class CreateSourceMaterialCommand:
    project_id: str
    material_type: str
    input_mode: str


@dataclass(frozen=True, slots=True)
class AppendSourceMaterialCommand:
    source_material_id: str
    expected_revision: int
    input_mode: str
    content: bytes | None = None
    content_hash: str | None = None
    asset_version_id: str | None = None


class SourceMaterialService:
    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def create(self, command: CreateSourceMaterialCommand) -> SourceMaterial:
        if command.input_mode not in {"inline_text", "uploaded_file"}:
            raise ValidationDomainError("source material input mode is invalid")
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            if project is None:
                raise ProjectNotFoundError(command.project_id)
            if (
                getattr(project, "creation_mode", None)
                or uow.creative_modes.get(command.project_id)
            ) != "adaptation":
                raise ValidationDomainError("SourceMaterial requires adaptation creationMode")
            source = SourceMaterial(
                command.project_id,
                command.material_type,  # type: ignore[arg-type]
                command.input_mode,  # type: ignore[arg-type]
            )
            uow.source_materials[source.id] = source
            project.source_materials = [
                *getattr(project, "source_materials", []),
                _source_json(source),
            ]
            project.revision += 1
            await uow.projects.save(project)
            await uow.commit()
        return source

    async def append(self, command: AppendSourceMaterialCommand) -> SourceMaterialVersion:
        async with self._uow_factory() as uow:
            source = uow.source_materials.get(command.source_material_id)
            if source is None:
                project = await _project_for_source(uow, command.source_material_id)
                if project is not None:
                    source = _restore_source(project, command.source_material_id)
                    if source is not None:
                        uow.source_materials[source.id] = source
            if source is None:
                raise ValidationDomainError("source material not found")
            if command.input_mode != source.input_mode:
                raise ValidationDomainError("source material input mode is immutable")
            if command.input_mode == "uploaded_file":
                version = (
                    await uow.asset_versions.get(command.asset_version_id)
                    if command.asset_version_id
                    else None
                )
                asset = await uow.assets.get(version.asset_id) if version is not None else None
                if (
                    version is None
                    or version.project_id != source.project_id
                    or asset is None
                    or asset.project_id != source.project_id
                    or asset.authorization_status in {"restricted", "expired"}
                    or version.status not in {"draft", "ready", "accepted", "verified"}
                    or version.content_hash is None
                    or version.content_hash != version.storage_object.checksum
                ):
                    raise ValidationDomainError(
                        "uploaded source AssetVersion is unverified or foreign"
                    )
                if command.content is not None or (
                    command.content_hash is not None
                    and command.content_hash != version.content_hash
                ):
                    raise ValidationDomainError(
                        "uploaded source hash must be derived from AssetVersion"
                    )
                command = AppendSourceMaterialCommand(
                    source_material_id=command.source_material_id,
                    expected_revision=command.expected_revision,
                    input_mode=command.input_mode,
                    content=None,
                    content_hash=version.content_hash,
                    asset_version_id=version.id,
                )
            version = source.append(
                expected_revision=command.expected_revision,
                input_mode=command.input_mode,
                content=command.content,
                content_hash=command.content_hash,
                asset_version_id=command.asset_version_id,
            )
            project = await _project_for_source(uow, source.id)
            if project is not None:
                project.source_materials = [
                    _source_json(source) if item.get("id") == source.id else item
                    for item in getattr(project, "source_materials", [])
                ]
                project.revision += 1
                await uow.projects.save(project)
            await uow.commit()
            return cast(SourceMaterialVersion, version)

    async def create_upload_intent(
        self,
        source_material_id: str,
        reservation_id: str,
        expected_revision: int,
        content_hash: str,
    ) -> SourceMaterialUploadIntent:
        async with self._uow_factory() as uow:
            source = uow.source_materials.get(source_material_id)
            if source is None:
                project = await _project_for_source(uow, source_material_id)
                source = _restore_source(project, source_material_id) if project else None
            if source is None or source.revision != expected_revision:
                raise ValidationDomainError("source material revision is stale")
            if source.input_mode != "uploaded_file":
                raise ValidationDomainError("source material upload intent requires uploaded_file")
            intent = SourceMaterialUploadIntent(
                source.project_id,
                source.id,
                source.revision,
                source.material_type,
                "uploaded_file",
                content_hash,
                f"source-material-upload:{source.project_id}:{source.id}:{source.revision}",
                reservation_id,
                project_scope=source.project_id,
            )
            await uow.commit()
            return intent

    async def verify_handoff(self, handoff: VerifiedStoredObjectHandoff) -> dict[str, object]:
        if handoff.status != "verified":
            return {"status": handoff.status, "diagnostic": "source_upload_not_verified"}
        async with self._uow_factory() as uow:
            source = uow.source_materials.get(handoff.source_material_id)
            if source is None or source.project_id != handoff.project_id:
                raise ValidationDomainError("source handoff scope is invalid")
            if source.revision != handoff.source_material_revision:
                raise ValidationDomainError("source handoff revision is stale")
            return {
                "status": "verified",
                "projectId": handoff.project_id,
                "sourceMaterialId": handoff.source_material_id,
                "sourceMaterialRevision": handoff.source_material_revision,
                "objectRef": handoff.object_ref,
                "checksum": handoff.checksum,
                "sizeBytes": handoff.size_bytes,
                "mimeType": handoff.mime_type,
                "profileRevision": handoff.profile_revision,
            }


async def _project_for_source(uow: Any, source_id: str) -> Any | None:
    for project in await uow.projects.list():
        if any(
            str(item.get("id")) == source_id for item in getattr(project, "source_materials", [])
        ):
            return project
    return None


def _restore_source(project: Any, source_id: str) -> SourceMaterial | None:
    raw = next(
        (
            item
            for item in getattr(project, "source_materials", [])
            if str(item.get("id")) == source_id
        ),
        None,
    )
    if not isinstance(raw, dict):
        return None
    versions = [SourceMaterialVersion(**item) for item in raw.get("versions", [])]
    current_raw = raw.get("current")
    current = SourceMaterialVersion(**current_raw) if isinstance(current_raw, dict) else None
    return SourceMaterial(
        project.id,
        cast(Any, str(raw["material_type"])),
        cast(Any, str(raw["input_mode"])),
        current,
        int(raw.get("revision", 1)),
        str(raw["id"]),
        versions,
    )


def _source_json(source: SourceMaterial) -> dict[str, object]:
    from dataclasses import asdict

    return asdict(source)
