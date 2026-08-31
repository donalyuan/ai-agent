from __future__ import annotations

import hashlib

import pytest

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.creative import CreativeService, SaveCreativeBriefCommand
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.source_material import (
    AppendSourceMaterialCommand,
    CreateSourceMaterialCommand,
    SourceMaterialService,
)
from video_agent_api.application.storage_profiles import (
    CreateStorageProfileCommand,
    StorageProfileService,
)
from video_agent_api.domain.errors import ProjectAccessForbiddenError, ValidationDomainError
from video_agent_api.domain.source_material import SourceMaterial, SourceMaterialVersion


def _brief() -> dict[str, object]:
    return {
        "subject": "Subject",
        "genre": "Drama",
        "audience": "Adult",
        "characterPremise": "Premise",
        "style": "Grounded",
        "episodeDurationSeconds": 60,
        "episodeCount": 1,
        "scenesPerEpisode": 1,
        "shotsPerScene": 1,
    }


@pytest.mark.asyncio
async def test_source_material_commands_require_matching_scope_and_do_not_mutate_project() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("Adaptation")
    brief = await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(project.id, "adaptation", _brief(), project.revision)
    )
    service = SourceMaterialService(lambda: uow)
    with pytest.raises(ValidationDomainError, match="scope"):
        await service.create(CreateSourceMaterialCommand(project.id, "novel", "inline_text"))
    source = await service.create(
        CreateSourceMaterialCommand(project.id, "novel", "inline_text", project.id)
    )
    assert project.revision == brief.revision + 1
    assert project.source_materials == []
    with pytest.raises(ProjectAccessForbiddenError, match="project access forbidden"):
        await service.append(
            AppendSourceMaterialCommand(
                source.id, 1, "inline_text", content=b"body", project_scope="foreign"
            )
        )
    version = await service.append(
        AppendSourceMaterialCommand(
            source.id, 1, "inline_text", content=b"body", project_scope=project.id
        )
    )
    assert version.content_hash == hashlib.sha256(b"body").hexdigest()
    assert project.source_materials == []


def test_inline_source_rejects_hash_without_content_and_non_hex_hash() -> None:
    source = SourceMaterial("project", "novel", "inline_text")
    with pytest.raises(ValidationDomainError, match="content"):
        source.append(expected_revision=1, input_mode="inline_text", content_hash="a" * 64)
    with pytest.raises(ValidationDomainError, match="hash"):
        SourceMaterialVersion("project", "novel", "inline_text", "g" * 64)


@pytest.mark.asyncio
async def test_storage_profile_service_rejects_missing_and_foreign_scope() -> None:
    uow = InMemoryUnitOfWork()
    project = await ProjectsEpisodesService(lambda: uow).create_project("Storage")
    service = StorageProfileService(lambda: uow)
    profile = await service.create(
        CreateStorageProfileCommand(
            project.id, "https://tos.invalid", "private", "cn-test", project_scope=(project.id,)
        )
    )
    with pytest.raises(ValidationDomainError, match="scope"):
        await service.get(profile.id)
    with pytest.raises(ProjectAccessForbiddenError, match="project access forbidden"):
        await service.get(profile.id, "foreign")
    assert (await service.get(profile.id, project.id)).id == profile.id
