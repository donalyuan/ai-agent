from __future__ import annotations

from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.sqlalchemy_models import (
    Asset,
    AssetVersion,
    Base,
    CredentialMetadata,
    Episode,
    Model,
    Project,
    Provider,
    ProviderProfile,
    Scene,
    Shot,
    TimelineDocument,
    VideoOperationModel,
    VideoTakeCandidateModel,
    WorkflowDraft,
    WorkflowVersion,
)


def test_phase_zero_models_expose_all_persistent_contracts() -> None:
    expected = {
        Project,
        Episode,
        Scene,
        Shot,
        Asset,
        AssetVersion,
        WorkflowDraft,
        WorkflowVersion,
        TimelineDocument,
        Provider,
        ProviderProfile,
        Model,
        CredentialMetadata,
    }
    assert len(expected) == 13
    assert VideoOperationModel.__tablename__ == "video_operations"
    assert VideoTakeCandidateModel.__tablename__ == "video_take_candidates"
    assert Project.__tablename__ == "projects"
    assert "ciphertext" in CredentialMetadata.__table__.columns
    assert "nonce" in CredentialMetadata.__table__.columns
    assert "tag" in CredentialMetadata.__table__.columns
    assert "endpoint" in ProviderProfile.__table__.columns
    assert "bucket" in ProviderProfile.__table__.columns
    assert "region" in ProviderProfile.__table__.columns


def test_versioned_models_preserve_the_minimum_contract_fields() -> None:
    contract_models = (
        Project,
        Episode,
        Scene,
        Shot,
        Asset,
        AssetVersion,
        WorkflowDraft,
        WorkflowVersion,
        TimelineDocument,
    )
    for model in contract_models:
        default = model.__table__.columns["schema_version"].default
        assert default is not None
        assert default.arg == "1.0.0"

    for model in (AssetVersion, WorkflowVersion):
        columns = model.__table__.columns
        assert columns["revision"].default.arg == 0
        assert columns["status"].default.arg == "draft"


async def test_minimal_project_hierarchy_persists_with_stable_relationships() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        session_factory = async_sessionmaker(engine, expire_on_commit=False)
        async with session_factory() as session:
            project = Project(id="project-1", name="Project", status="draft")
            episode = Episode(
                id="episode-1", project_id=project.id, display_number=1, status="draft"
            )
            scene = Scene(
                id="scene-1",
                project_id=project.id,
                episode_id=episode.id,
                display_number=1,
                status="draft",
            )
            shot = Shot(
                id="shot-1",
                project_id=project.id,
                episode_id=episode.id,
                scene_id=scene.id,
                display_number=1,
                status="draft",
            )
            session.add_all([project, episode, scene, shot])
            await session.commit()
            assert await session.get(Shot, shot.id) is not None
    finally:
        await engine.dispose()
