from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest
from alembic.config import Config
from fastapi.testclient import TestClient
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from alembic import command
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.adapters.sqlalchemy import SQLAlchemyUnitOfWork
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.app import create_app
from video_agent_api.application.catalog import (
    AppendSkillRevisionCommand,
    AuditSkillAccessCommand,
    CatalogService,
    ConfirmCostCommand,
    CreateModelCommand,
    CreateProfileCommand,
    CreateProviderCommand,
    ModelSyncCommand,
    RecordProviderCallCommand,
    ReplaceCredentialCommand,
    SetQuotaCommand,
    UpdateCatalogCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.domain.errors import (
    CredentialMasterKeyUnavailableError,
    RevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.ports.credentials import CredentialKeyring


def test_catalog_owner_tables_are_normalized_and_append_only(tmp_path) -> None:
    database_url = f"sqlite:///{tmp_path / 'catalog-owner.db'}"
    api_root = Path(__file__).parents[1]
    config = Config(str(api_root / "alembic.ini"))
    config.set_main_option("script_location", str(api_root / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    tables = set(inspect(engine).get_table_names())
    assert {
        "capability_snapshots",
        "skill_revisions",
        "provider_calls",
        "provider_quota_snapshots",
        "provider_operation_policies",
        "cost_confirmations",
        "model_sync_candidates",
        "skill_access_audits",
    } <= tables
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
            == "0023_export_dispatch_owner"
        )
    engine.dispose()


async def test_catalog_bootstrap_and_cas_lifecycle() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    await service.bootstrap()
    assert len(uow.skills) == 8
    assert {item.name for item in uow.skills if item.enabled} == {
        "drama-skills",
        "novel-writing",
    }
    provider = next(iter(uow.providers.values()))
    updated = await service.update_provider(
        UpdateCatalogCommand(provider.id, provider.revision, {"name": "Mock Provider Local"})
    )
    before = (updated.name, updated.revision, len(uow.audit_events))
    with pytest.raises(RevisionConflictError):
        await service.update_provider(
            UpdateCatalogCommand(updated.id, 1, {"name": "stale overwrite"})
        )
    current = uow.providers[provider.id]
    assert (current.name, current.revision, len(uow.audit_events)) == before


@pytest.mark.asyncio
async def test_sqlalchemy_catalog_round_trip_uses_normalized_owner_tables() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        factory = async_sessionmaker(engine, expire_on_commit=False)
        service = CatalogService(lambda: SQLAlchemyUnitOfWork(factory))
        await service.bootstrap()
        await service.bootstrap()
        async with SQLAlchemyUnitOfWork(factory) as loaded:
            provider = next(iter(loaded.providers.values()))
            profile = next(iter(loaded.profiles.values()))
            assert len(loaded.models) == 1
            assert set(profile.capability_snapshots) == {
                "text.generate",
                "image.generate",
                "video.submit",
            }
        model = await service.create_model(CreateModelCommand(profile.id, "local-model"))
        await service.update_operation_policy(
            profile.id,
            "text.generate",
            profile.revision,
            {"maxConcurrency": 2, "rateLimit": 4, "rateWindowSeconds": 60},
        )
        await service.set_quota(
            SetQuotaCommand(profile.id, "text.generate", "known", 3, "later", "local")
        )
        snapshot = await service.snapshot(profile.id, "text.generate")
        projects = ProjectsEpisodesService(lambda: SQLAlchemyUnitOfWork(factory))
        project = await projects.create_project("Catalog round trip")
        call = await service.record_provider_call(
            RecordProviderCallCommand(
                project.id,
                "run-1",
                None,
                "text.generate:1",
                "text.generate",
                provider.id,
                profile.id,
                model.id,
                "f" * 64,
                capability_snapshot_id=snapshot.id,
                status="succeeded",
                cost_status="known",
                cost_value="0",
                cost_currency="USD",
                native_usage={"inputTokens": 2},
            )
        )
        assert call.capability_snapshot_id == snapshot.id
        confirmation = await service.confirm_cost(
            ConfirmCostCommand(
                project.id,
                "run-1",
                "text.generate:1",
                "f" * 64,
                "user-1",
                "threshold-1",
                1,
                "0",
                "known",
                "text",
                1,
            )
        )
        async with SQLAlchemyUnitOfWork(factory) as reloaded:
            assert set(reloaded.providers) == {provider.id}
            assert reloaded.profiles[profile.id].operation_policies["text.generate"] == {
                "maxConcurrency": 2,
                "rateLimit": 4,
                "rateWindowSeconds": 60,
            }
            assert reloaded.profiles[profile.id].quota_snapshots["text.generate"].remaining == 3
            assert (
                reloaded.profiles[profile.id].capability_snapshots["text.generate"].id
                == snapshot.id
            )
            assert reloaded.provider_calls[call.id].request_fingerprint == "f" * 64
            assert reloaded.cost_confirmations[("run-1", "text.generate:1")].id == confirmation.id
            assert not reloaded._phase_one_collections.get("providers")
        async with factory() as session:
            assert await session.scalar(text("SELECT COUNT(*) FROM providers")) == 1
            assert await session.scalar(text("SELECT COUNT(*) FROM provider_calls")) == 1
            assert await session.scalar(text("SELECT COUNT(*) FROM capability_snapshots")) == 4
            assert (
                await session.scalar(
                    text(
                        "SELECT COUNT(*) FROM phase_one_documents "
                        "WHERE collection IN ('providers', 'profiles', 'models', 'skills', "
                        "'provider_calls', 'provider_call_keys', 'cost_confirmations', "
                        "'credential_envelopes', 'model_sync_candidates', 'skill_access_audits', "
                        "'catalog_overrides', 'usage_audits')"
                    )
                )
                == 0
            )
    finally:
        await engine.dispose()


async def test_credential_envelope_rotation_is_masked_and_atomic() -> None:
    uow = InMemoryUnitOfWork()
    source = CredentialKeyring(b"a" * 32, "v1")
    service = CatalogService(lambda: uow, source)
    provider = await service.create_provider(CreateProviderCommand("Live", "live"))
    profile = await service.create_profile(
        CreateProfileCommand(provider.id, "Live profile", "live")
    )
    status = await service.replace_credential(
        ReplaceCredentialCommand(profile.id, "api-key", "secret-123456", profile.revision)
    )
    assert status == {
        "status": "configured",
        "maskedPrefix": "secr",
        "last4": "3456",
        "keyVersion": "v1",
    }
    assert "secret-123456" not in repr(uow.credential_envelopes[profile.id])
    rotated = await service.rotate_credentials(CredentialKeyring(b"b" * 32, "v2"))
    assert rotated == {"status": "rotated", "count": 1, "keyVersion": "v2"}
    assert (await service.credential_status(profile.id))["keyVersion"] == "v2"

    unavailable = CatalogService(lambda: uow)
    with pytest.raises(CredentialMasterKeyUnavailableError):
        await unavailable.replace_credential(
            ReplaceCredentialCommand(profile.id, "replacement", "new-secret", profile.revision)
        )
    assert (await service.credential_status(profile.id))["keyVersion"] == "v2"


async def test_model_sync_requires_explicit_accept_and_skill_revisions_append() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    provider = await service.create_provider(CreateProviderCommand("P", "mock"))
    profile = await service.create_profile(CreateProfileCommand(provider.id, "Local"))
    existing = await service.create_model(CreateModelCommand(profile.id, "old-model"))
    candidate = await service.preview_model_sync(ModelSyncCommand(profile.id, ("new-model",)))
    assert candidate.added == ("new-model",) and candidate.removed == ("old-model",)
    assert {item.model_key for item in uow.models.values()} == {"old-model"}
    accepted = await service.decide_model_sync(candidate.id, 1, "accept")
    assert accepted.status == "accepted"
    assert uow.models[existing.id].enabled is False
    assert {item.model_key for item in uow.models.values()} == {"old-model", "new-model"}

    original = next(item for item in await _skills(service, uow) if item.name == "drama-skills")
    successor = await service.append_skill_revision(
        AppendSkillRevisionCommand(
            original.name,
            "2.0.0",
            original.revision,
            "git:https://example.test/repo;commit:abc123",
            "f" * 64,
            "git",
            "verified",
            ("text.generate",),
        )
    )
    assert successor.revision == 2 and original.version == "1.0.0"
    with pytest.raises(ValidationDomainError, match="not_authorized"):
        await service.audit_skill_access(
            AuditSkillAccessCommand(successor.id, "run", "node", "network", selected=True)
        )
    assert uow.skill_access_audits[-1].allowed is False


async def test_historically_referenced_model_is_disable_only() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    provider = await service.create_provider(CreateProviderCommand("P", "mock"))
    profile = await service.create_profile(CreateProfileCommand(provider.id, "Local"))
    model = await service.create_model(CreateModelCommand(profile.id, "model"))
    uow.provider_calls["call"] = type(
        "Call",
        (),
        {"model_id": model.id},
    )()
    with pytest.raises(ValidationDomainError, match="model_in_use"):
        await service.delete_model(model.id, model.revision)
    disabled = await service.disable_model(model.id, model.revision)
    assert disabled.enabled is False and disabled.id == model.id


@pytest.mark.asyncio
async def test_model_delete_proof_covers_snapshot_run_and_workflow_default() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    provider = await service.create_provider(CreateProviderCommand("P", "mock"))
    provider.approval = "approved"
    provider.adapter_installed = True
    profile = await service.create_profile(CreateProfileCommand(provider.id, "Local"))
    model = await service.create_model(CreateModelCommand(profile.id, "model"))

    snapshot = await service.snapshot(profile.id, "text.generate")
    profile.capability_snapshots["text.generate"] = replace(snapshot, model_id=model.id)
    with pytest.raises(ValidationDomainError, match="model_in_use"):
        await service.delete_model(model.id, model.revision)
    profile.capability_snapshots.clear()

    uow.workflow_runs["run"] = type("Run", (), {"selection_snapshot": {"modelId": model.id}})()
    with pytest.raises(ValidationDomainError, match="model_in_use"):
        await service.delete_model(model.id, model.revision)
    uow.workflow_runs.clear()

    uow.workflow_by_project["workflow"] = type(
        "Workflow", (), {"definition": {"nodes": [{"modelId": model.id}]}}
    )()
    with pytest.raises(ValidationDomainError, match="model_in_use"):
        await service.delete_model(model.id, model.revision)
    uow.workflow_by_project.clear()

    uow.workflow_bindings["project"] = type("Binding", (), {"model_id": model.id})()
    with pytest.raises(ValidationDomainError, match="model_in_use"):
        await service.delete_model(model.id, model.revision)


@pytest.mark.asyncio
async def test_model_delete_without_references_removes_normalized_row() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    provider = await service.create_provider(CreateProviderCommand("P", "mock"))
    profile = await service.create_profile(CreateProfileCommand(provider.id, "Local"))
    model = await service.create_model(CreateModelCommand(profile.id, "unused"))
    await service.delete_model(model.id, model.revision)
    assert model.id not in uow.models


async def _skills(service: CatalogService, uow: InMemoryUnitOfWork):
    await service.bootstrap()
    return uow.skills


async def test_provider_call_is_unique_redacted_and_not_a_run_event() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    command = RecordProviderCallCommand(
        "project",
        "run",
        "node",
        "image.generate:1",
        "image.generate",
        "provider",
        "profile",
        "model",
        "fingerprint",
        status="failed",
        cost_status="unknown",
        native_usage={"inputTokens": 5, "prompt": "must not project"},
    )
    call = await service.record_provider_call(command)
    retry = await service.record_provider_call(command)
    assert retry.id == call.id and len(uow.provider_calls) == 1
    with pytest.raises(ValidationDomainError, match="fingerprint conflict"):
        await service.record_provider_call(replace(command, project_id="foreign"))
    assert uow.run_events == {}
    summary = (await service.provider_call_summaries("project", "run"))[0]
    assert summary["cost"]["status"] == "unknown"  # type: ignore[index]
    assert "prompt" not in summary and "objectKey" not in summary
    assert summary["nativeUsage"] == {"inputTokens": 5}
    cleanup = await service.cleanup_audit_facts()
    assert cleanup["providerCalls"] == 1 and len(uow.provider_calls) == 1


async def test_probe_keeps_live_unconfigured_and_local_mock_runnable() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    await service.bootstrap()
    local_profile = next(iter(uow.profiles.values()))
    snapshot = await service.snapshot(local_profile.id, "text.generate")
    assert snapshot.runnable is True

    live = await service.create_provider(CreateProviderCommand("Live", "live"))
    live.approval = "approved"
    live.adapter_installed = True
    profile = await service.create_profile(CreateProfileCommand(live.id, "Live", "live"))
    profile.explicit_live_opt_in = True
    profile.credential_status = "configured"
    with pytest.raises(ValidationDomainError, match="transport_unconfigured"):
        await service.snapshot(profile.id, "image.generate")
    assert profile.capability_snapshots == {}


def test_catalog_http_rejects_plaintext_without_master_key() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    with TestClient(app) as client:
        catalog = client.get("/v1/catalog").json()
        profile = catalog["profiles"][0]
        response = client.put(
            f"/v1/catalog/profiles/{profile['id']}/credential",
            json={
                "credentialId": "api-key",
                "value": "must-not-leak",
                "expectedRevision": profile["revision"],
            },
        )
    assert response.status_code == 503
    assert response.json()["detail"]["type"] == "credential_master_key_unavailable"
    assert "must-not-leak" not in response.text


def test_catalog_projection_exposes_owner_parameter_schemas_without_probe() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    with TestClient(app) as client:
        response = client.get("/v1/catalog")
        assert response.status_code == 200
        payload = response.json()
        profile = payload["profiles"][0]
        assert payload["schemaVersion"] == "1.0.0"
        assert payload["profileParameterSchemas"][profile["id"]] == {}

        uow.models["owner-model"] = SimpleNamespace(
            id="owner-model",
            profile_id=profile["id"],
            enabled=True,
            parameter_schema={
                "image.generate": {
                    "type": "object",
                    "properties": {"prompt": {"type": "string"}},
                }
            },
        )
        response = client.get("/v1/catalog")

    assert response.status_code == 200
    payload = response.json()
    schema = payload["profileParameterSchemas"][profile["id"]]["image.generate"]
    assert schema["properties"]["prompt"]["type"] == "string"
    assert "maxConcurrency" not in schema["properties"]
    assert uow.audit_events == [] and uow.provider_calls == {}


@pytest.mark.asyncio
async def test_catalog_lifecycle_cas_and_immutable_skill_toggle() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    await service.bootstrap()
    provider = next(iter(uow.providers.values()))
    model = next(iter(uow.models.values()))
    provider_revision = provider.revision
    disabled_provider = await service.set_provider_enabled(provider.id, provider_revision, False)
    assert disabled_provider.enabled is False
    with pytest.raises(RevisionConflictError):
        await service.set_provider_enabled(provider.id, provider_revision, True)
    await service.set_provider_enabled(provider.id, disabled_provider.revision, True)
    disabled_model = await service.set_model_enabled(model.id, model.revision, False)
    enabled_model = await service.set_model_enabled(model.id, disabled_model.revision, True)
    assert enabled_model.enabled is True

    original = next(item for item in uow.skills if item.name == "drama-skills")
    toggled = await service.set_skill_enabled(original.id, original.revision, False)
    assert original.enabled is True
    assert toggled.enabled is False and toggled.revision == original.revision + 1
    assert original.id != toggled.id
    projection = await service.projection()
    current = [item for item in projection["skills"] if item.name == original.name]
    assert len(current) == 1 and current[0].id == toggled.id
    with pytest.raises(ValidationDomainError, match="not_authorized"):
        await service.audit_skill_access(
            AuditSkillAccessCommand(original.id, "run", "node", "content", selected=True)
        )


@pytest.mark.asyncio
async def test_model_sync_requires_explicit_input_source() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    provider = await service.create_provider(CreateProviderCommand("P", "mock"))
    profile = await service.create_profile(CreateProfileCommand(provider.id, "Local"))
    with pytest.raises(ValidationDomainError, match="explicit_input"):
        await service.preview_model_sync(
            ModelSyncCommand(profile.id, ("model",), profile.revision, "adapter_discovery")
        )


@pytest.mark.asyncio
async def test_probe_requires_current_profile_revision() -> None:
    uow = InMemoryUnitOfWork()
    service = CatalogService(lambda: uow)
    await service.bootstrap()
    profile = next(iter(uow.profiles.values()))
    with pytest.raises(RevisionConflictError, match="revision conflict"):
        await service.snapshot(profile.id, "image.generate", expected_revision=profile.revision - 1)
