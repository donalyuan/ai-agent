from __future__ import annotations

from pathlib import Path

import pytest
import yaml
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.image_generation import ImageGenerationOperation
from video_agent_api.application.runtime_composition import (
    CatalogRuntimeComposition,
    CatalogRuntimeResolver,
)
from video_agent_api.db import CURRENT_MIGRATION_HEAD
from video_agent_api.domain.catalog import CapabilitySnapshot, Model, Provider, ProviderProfile
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.ports.contracts import AdapterNotConfiguredError, ModelSelection
from video_agent_api.ports.mocks import DeterministicMockProvider
from video_agent_api.ports.storage import LocalWorkspaceAdapter, StorageProfile, TOSAdapter
from video_agent_api.providers.gpt_image import GPTImageProvider
from video_agent_api.runtime import RuntimeSettings, UnconfiguredLiveProvider, build_runtime


def _example_environment() -> dict[str, str]:
    path = Path(__file__).parents[3] / ".env.example"
    return dict(
        line.split("=", 1)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    )


def test_example_environment_selects_mock_and_local_workspace() -> None:
    settings = RuntimeSettings.from_mapping(_example_environment())
    runtime = build_runtime(settings)

    assert _example_environment()["RENDERER_REQUIRED"] == "false"
    assert runtime.provider_mode == "mock"
    assert runtime.storage_mode == "local_workspace"
    assert isinstance(runtime.provider, DeterministicMockProvider)
    assert isinstance(runtime.storage, LocalWorkspaceAdapter)
    assert runtime.storage.root == Path("/workspace/data/workspaces")


@pytest.mark.parametrize(
    ("provider_mode", "storage_mode", "message"),
    [
        ("unknown", "local_workspace", "unsupported provider mode"),
        ("mock", "unknown", "unsupported storage mode"),
    ],
)
def test_runtime_rejects_unknown_modes_without_fallback(
    provider_mode: str, storage_mode: str, message: str, tmp_path: Path
) -> None:
    settings = RuntimeSettings(provider_mode, storage_mode, tmp_path)

    with pytest.raises(AdapterNotConfiguredError, match=message):
        build_runtime(settings)


def test_runtime_exposes_explicit_unconfigured_tos(tmp_path: Path) -> None:
    runtime = build_runtime(RuntimeSettings("mock", "tos", tmp_path))

    assert isinstance(runtime.storage, TOSAdapter)
    with pytest.raises(AdapterNotConfiguredError, match="not configured"):
        runtime.storage.put("asset.bin", b"content", correlation_id="trace-tos")


def test_live_runtime_selection_never_falls_back_to_mock_or_local(tmp_path: Path) -> None:
    runtime = build_runtime(
        RuntimeSettings.from_mapping(
            {
                "PROVIDER_MODE": "live",
                "PROVIDER_PROFILE_ID": "profile-live",
                "PROVIDER_MODEL_ID": "model-live",
                "PROVIDER_CREDENTIAL_REF": "credential-live",
                "STORAGE_MODE": "tos",
                "STORAGE_PROFILE_ID": "storage-live",
                "STORAGE_BUCKET_BINDING_ID": "bucket-live",
                "RENDERER_PROFILE_ID": "renderer-live",
                "FFMPEG_PATH": "/opt/media/ffmpeg",
                "FFPROBE_PATH": "/opt/media/ffprobe",
                "WORKSPACE_ROOT": str(tmp_path),
            }
        )
    )

    assert runtime.provider_mode == "live"
    assert runtime.selection.provider_profile_id == "profile-live"
    assert runtime.selection.provider_model_id == "model-live"
    assert runtime.selection.provider_credential_ref == "credential-live"
    assert runtime.selection.storage_profile_id == "storage-live"
    assert runtime.selection.storage_bucket_binding_id == "bucket-live"
    assert runtime.selection.renderer_profile_id == "renderer-live"
    assert runtime.selection.ffmpeg_path == "/opt/media/ffmpeg"
    assert runtime.selection.ffprobe_path == "/opt/media/ffprobe"
    assert isinstance(runtime.provider, UnconfiguredLiveProvider)
    assert isinstance(runtime.storage, TOSAdapter)
    with pytest.raises(AdapterNotConfiguredError, match="catalog resolver"):
        runtime.provider.generate_text("prompt", ModelSelection("p", "q", "m", "live"), "trace")


def test_runtime_live_readiness_requires_each_explicit_reference(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    async def database_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    async def catalog_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    monkeypatch.setattr("video_agent_api.db.check_database", database_ready)
    monkeypatch.setattr("video_agent_api.db.check_catalog_bootstrap", catalog_ready)
    monkeypatch.setattr("video_agent_api.db._temporal_queue_reachable", lambda _address: True)
    monkeypatch.setenv("DATABASE_URL", "sqlite+aiosqlite:///:memory:")
    monkeypatch.setenv("TEMPORAL_ADDRESS", "temporal:7233")
    monkeypatch.setenv("WORKSPACE_ROOT", str(tmp_path))
    monkeypatch.setenv("PROVIDER_MODE", "live")
    monkeypatch.setenv("STORAGE_MODE", "tos")
    monkeypatch.setenv("PROVIDER_PROFILE_ID", "provider-profile")
    monkeypatch.setenv("PROVIDER_MODEL_ID", "")
    monkeypatch.setenv("PROVIDER_CREDENTIAL_REF", "provider-credential")
    monkeypatch.setenv("STORAGE_PROFILE_ID", "storage-profile")
    monkeypatch.setenv("STORAGE_BUCKET_BINDING_ID", "storage-binding")
    monkeypatch.setenv("STORAGE_CREDENTIAL_REF", "storage-credential")

    from video_agent_api.db import default_readiness_assessment

    assessment = default_readiness_assessment()

    assert assessment.status == "unconfigured"
    assert "selected_capability_unconfigured" in assessment.diagnostics


def test_runtime_live_readiness_does_not_treat_environment_references_as_composition(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    async def database_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    async def catalog_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    monkeypatch.setattr("video_agent_api.db.check_database", database_ready)
    monkeypatch.setattr("video_agent_api.db.check_catalog_bootstrap", catalog_ready)
    monkeypatch.setattr("video_agent_api.db._temporal_queue_reachable", lambda _address: True)
    monkeypatch.setenv("DATABASE_URL", "sqlite+aiosqlite:///:memory:")
    monkeypatch.setenv("TEMPORAL_ADDRESS", "temporal:7233")
    monkeypatch.setenv("WORKSPACE_ROOT", str(tmp_path))
    monkeypatch.setenv("PROVIDER_MODE", "live")
    monkeypatch.setenv("STORAGE_MODE", "tos")
    monkeypatch.setenv("PROVIDER_PROFILE_ID", "provider-profile")
    monkeypatch.setenv("PROVIDER_MODEL_ID", "provider-model")
    monkeypatch.setenv("PROVIDER_CREDENTIAL_REF", "provider-credential")
    monkeypatch.setenv("STORAGE_PROFILE_ID", "storage-profile")
    monkeypatch.setenv("STORAGE_BUCKET_BINDING_ID", "storage-binding")
    monkeypatch.setenv("STORAGE_CREDENTIAL_REF", "storage-credential")
    monkeypatch.setenv("RENDERER_REQUIRED", "true")
    monkeypatch.setenv("RENDERER_PROFILE_ID", "renderer-profile")
    monkeypatch.setenv("FFMPEG_PATH", "/bin/true")
    monkeypatch.setenv("FFPROBE_PATH", "/bin/true")

    from video_agent_api.db import default_readiness_assessment

    assessment = default_readiness_assessment()

    assert assessment.status == "unconfigured"
    assert "selected_capability_unconfigured" in assessment.diagnostics


def test_runtime_mock_local_readiness_allows_required_local_renderer_without_profile(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    async def database_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    async def catalog_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    monkeypatch.setattr("video_agent_api.db.check_database", database_ready)
    monkeypatch.setattr("video_agent_api.db.check_catalog_bootstrap", catalog_ready)
    monkeypatch.setattr("video_agent_api.db._temporal_queue_reachable", lambda _address: True)
    monkeypatch.setenv("DATABASE_URL", "sqlite+aiosqlite:///:memory:")
    monkeypatch.setenv("TEMPORAL_ADDRESS", "temporal:7233")
    monkeypatch.setenv("WORKSPACE_ROOT", str(tmp_path))
    monkeypatch.setenv("PROVIDER_MODE", "mock")
    monkeypatch.setenv("STORAGE_MODE", "local_workspace")
    monkeypatch.setenv("RENDERER_REQUIRED", "true")
    monkeypatch.delenv("RENDERER_PROFILE_ID", raising=False)
    monkeypatch.setenv("FFMPEG_PATH", "/bin/true")
    monkeypatch.setenv("FFPROBE_PATH", "/bin/true")

    from video_agent_api.db import default_readiness_assessment

    assessment = default_readiness_assessment()

    assert assessment.status == "renderer_unconfigured"
    assert assessment.diagnostics == ("renderer_unconfigured",)


def test_runtime_readiness_rejects_non_empty_renderer_profile_when_not_required(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    async def database_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    async def catalog_ready(*_args: object, **_kwargs: object) -> bool:
        return True

    monkeypatch.setattr("video_agent_api.db.check_database", database_ready)
    monkeypatch.setattr("video_agent_api.db.check_catalog_bootstrap", catalog_ready)
    monkeypatch.setattr("video_agent_api.db._temporal_queue_reachable", lambda _address: True)
    monkeypatch.setenv("DATABASE_URL", "sqlite+aiosqlite:///:memory:")
    monkeypatch.setenv("TEMPORAL_ADDRESS", "temporal:7233")
    monkeypatch.setenv("WORKSPACE_ROOT", str(tmp_path))
    monkeypatch.setenv("PROVIDER_MODE", "mock")
    monkeypatch.setenv("STORAGE_MODE", "local_workspace")
    monkeypatch.setenv("RENDERER_REQUIRED", "false")
    monkeypatch.setenv("RENDERER_PROFILE_ID", "incomplete-renderer")
    monkeypatch.setenv("FFMPEG_PATH", "/bin/true")
    monkeypatch.setenv("FFPROBE_PATH", "/bin/true")

    from video_agent_api.db import default_readiness_assessment

    assessment = default_readiness_assessment()

    assert assessment.ready is False
    assert assessment.status == "unconfigured"
    assert assessment.diagnostics == ("selected_capability_unconfigured",)


@pytest.mark.asyncio
async def test_catalog_live_resolver_separates_probe_from_runnable_invocation() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Live", "gpt", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Sandbox",
        "gpt",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
        base_url="https://provider.example.test/v1",
        credential_ref="credential-live",
        timeout_ms=12_000,
    )
    model = Model(
        profile.id,
        "gpt-test",
        enabled=True,
        default_parameters={"temperature": 0.2},
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model
    resolver = CatalogRuntimeResolver(lambda: uow)

    # A first probe validates explicit live prerequisites but does not require a prior snapshot.
    probe = await resolver.resolve_probe(profile.id, model.id, "image.generate", timeout_seconds=5)
    assert probe.profile_id == profile.id

    with pytest.raises(ValidationDomainError, match="capability_snapshot_unavailable"):
        await resolver.resolve_invocation(profile.id, model.id, "image.generate")
    assert uow.provider_calls == {}

    current_profile = uow.profiles[profile.id]
    current_profile.capability_snapshots["image.generate"] = CapabilitySnapshot(
        provider.id,
        current_profile.id,
        "image.generate",
        current_profile.revision,
        True,
        ("generate",),
        "probe",
        model.id,
    )
    resolved = await resolver.resolve_invocation(profile.id, model.id, "image.generate")
    capability = current_profile.capability_snapshots["image.generate"]
    assert resolved.capability_snapshot_id == capability.id
    assert resolved.policy_revision == current_profile.revision
    assert resolved.adapter_key == provider.adapter_key
    assert resolved.model_key == model.model_key
    assert resolved.base_url == "https://provider.example.test/v1"
    assert resolved.credential_ref == "credential-live"
    assert resolved.timeout_seconds == 12.0
    assert resolved.default_parameters == {"temperature": 0.2}

    with pytest.raises(ValidationDomainError, match="catalog_profile_revision_stale"):
        await resolver.resolve_invocation(
            profile.id,
            model.id,
            "image.generate",
            expected_profile_revision=current_profile.revision + 1,
        )
    with pytest.raises(ValidationDomainError, match="capability_snapshot_stale"):
        await resolver.resolve_invocation(
            profile.id,
            model.id,
            "image.generate",
            expected_capability_snapshot_id=capability.id,
            expected_capability_revision=capability.revision + 1,
        )


@pytest.mark.asyncio
async def test_catalog_invocation_rejects_foreign_project_scope_before_provider_call() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Live", "openai", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Scoped",
        "openai",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
        project_scope=("project-1",),
    )
    model = Model(profile.id, "model", enabled=True)
    profile.capability_snapshots["text.generate"] = CapabilitySnapshot(
        provider.id,
        profile.id,
        "text.generate",
        profile.revision,
        True,
        ("idempotency-key-header:X-Idempotency-Key",),
        "probe",
        model.id,
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model
    resolver = CatalogRuntimeResolver(lambda: uow)
    with pytest.raises(ValidationDomainError, match="project scope"):
        await resolver.resolve_invocation(
            profile.id, model.id, "text.generate", project_id="project-2"
        )


@pytest.mark.asyncio
async def test_catalog_composition_builds_only_the_selected_live_adapter() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Image", "gpt_image", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Sandbox",
        "gpt_image",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
        base_url="https://images.example.test",
        credential_ref="credential-image",
    )
    model = Model(
        profile.id,
        "gpt-image-1",
        enabled=True,
        default_parameters={"size": "1024x1024"},
    )
    profile.capability_snapshots["image.generate"] = CapabilitySnapshot(
        provider.id,
        profile.id,
        "image.generate",
        profile.revision,
        True,
        (),
        "probe",
        model.id,
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model

    class Credentials:
        def resolve(self, credential_ref: str, profile_id: str) -> str:
            assert (credential_ref, profile_id) == ("credential-image", profile.id)
            return "secret"

    composed = await CatalogRuntimeComposition(
        CatalogRuntimeResolver(lambda: uow), Credentials()
    ).resolve_provider(profile.id, model.id, "image.generate")

    assert isinstance(composed.port, GPTImageProvider)
    assert composed.identity.default_parameters == {"size": "1024x1024"}
    assert composed.port.base_url == "https://images.example.test"
    assert composed.port.api_key == "secret"


@pytest.mark.asyncio
async def test_catalog_composition_accepts_async_catalog_credential_resolution() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Text", "openai", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Sandbox",
        "openai",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
        base_url="https://text.example.test",
        credential_ref="credential-text",
    )
    model = Model(profile.id, "catalog-text", enabled=True)
    profile.capability_snapshots["text.generate"] = CapabilitySnapshot(
        provider.id, profile.id, "text.generate", profile.revision, True, (), "probe", model.id
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model

    class AsyncCredentials:
        async def resolve_credential(self, credential_ref: str, profile_id: str) -> str:
            assert (credential_ref, profile_id) == ("credential-text", profile.id)
            return "secret"

    composed = await CatalogRuntimeComposition(
        CatalogRuntimeResolver(lambda: uow), AsyncCredentials()
    ).resolve_provider(profile.id, model.id, "text.generate")

    assert composed.identity.selection({"temperature": 0.3}).adapter_key == "openai"
    assert composed.port.api_key == "secret"


@pytest.mark.asyncio
async def test_catalog_composition_rejects_adapter_mismatch_before_credential_resolution() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Image", "gpt_image", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Mismatched",
        "openai",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
        base_url="https://images.example.test",
        credential_ref="credential-image",
    )
    model = Model(profile.id, "gpt-image-1", enabled=True)
    profile.capability_snapshots["image.generate"] = CapabilitySnapshot(
        provider.id, profile.id, "image.generate", profile.revision, True, (), "probe", model.id
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model

    class Credentials:
        calls = 0

        def resolve(self, credential_ref: str, profile_id: str) -> str:
            del credential_ref, profile_id
            self.calls += 1
            return "must-not-be-resolved"

    credentials = Credentials()
    composition = CatalogRuntimeComposition(CatalogRuntimeResolver(lambda: uow), credentials)

    with pytest.raises(ValidationDomainError, match="live_provider_adapter_unconfigured"):
        await composition.resolve_provider(profile.id, model.id, "image.generate")

    assert credentials.calls == 0


@pytest.mark.asyncio
async def test_catalog_composition_rejects_missing_credential_resolver_without_provider_call() -> (
    None
):
    uow = InMemoryUnitOfWork()
    provider = Provider("Image", "gpt_image", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Credentialless",
        "gpt_image",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
        base_url="https://images.example.test",
        credential_ref="credential-image",
    )
    model = Model(profile.id, "gpt-image-1", enabled=True)
    profile.capability_snapshots["image.generate"] = CapabilitySnapshot(
        provider.id, profile.id, "image.generate", profile.revision, True, (), "probe", model.id
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model

    composition = CatalogRuntimeComposition(CatalogRuntimeResolver(lambda: uow), object())

    with pytest.raises(ValidationDomainError, match="live_provider_unconfigured"):
        await composition.resolve_provider(profile.id, model.id, "image.generate")

    assert uow.provider_calls == {}


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("approval", "enabled", "snapshot_runnable", "credential_ref", "error"),
    [
        ("pending", True, True, "credential-image", "live_provider_unconfigured"),
        ("approved", False, True, "credential-image", "live_provider_unconfigured"),
        ("approved", True, False, "credential-image", "capability_snapshot_unavailable"),
        ("approved", True, True, None, "live_provider_unconfigured"),
    ],
)
async def test_catalog_composition_fails_closed_without_resolving_credentials(
    approval: str,
    enabled: bool,
    snapshot_runnable: bool,
    credential_ref: str | None,
    error: str,
) -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Image", "gpt_image", approval, "MVP-A", True, enabled)
    profile = ProviderProfile(
        provider.id,
        "Guarded",
        "gpt_image",
        enabled=enabled,
        explicit_live_opt_in=True,
        credential_status="configured",
        base_url="https://images.example.test",
        credential_ref=credential_ref,
    )
    model = Model(profile.id, "gpt-image-1", enabled=enabled)
    profile.capability_snapshots["image.generate"] = CapabilitySnapshot(
        provider.id,
        profile.id,
        "image.generate",
        profile.revision,
        snapshot_runnable,
        (),
        "probe",
        model.id,
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model

    class Credentials:
        calls = 0

        def resolve(self, credential_ref: str, profile_id: str) -> str:
            del credential_ref, profile_id
            self.calls += 1
            return "must-not-be-resolved"

    credentials = Credentials()
    composition = CatalogRuntimeComposition(CatalogRuntimeResolver(lambda: uow), credentials)

    with pytest.raises(ValidationDomainError, match=error):
        await composition.resolve_provider(profile.id, model.id, "image.generate")

    assert credentials.calls == 0


@pytest.mark.asyncio
async def test_storage_composition_freezes_project_profile_and_binding_without_fallback() -> None:
    profile = StorageProfile(
        "storage-profile",
        "project-1",
        "https://tos.example.test",
        "private-bucket",
        "cn-test",
        credential_status="configured",
        enabled=True,
        adapter_key="tos",
        bucket_binding_id="binding-1",
        credential_ref="credential-storage",
        project_scope=("project-1",),
    )

    class StorageProfiles:
        async def resolve_upload_profile(
            self, project_id: str, profile_id: str, expected_revision: int
        ) -> StorageProfile:
            assert (project_id, profile_id, expected_revision) == ("project-1", profile.id, 1)
            return profile

    class Credentials:
        def resolve(self, credential_ref: str, profile_id: str) -> str:
            assert (credential_ref, profile_id) == ("credential-storage", profile.id)
            return "access:secret"

    composition = CatalogRuntimeComposition(
        CatalogRuntimeResolver(lambda: InMemoryUnitOfWork()), Credentials()
    )
    composed = await composition.resolve_storage(
        StorageProfiles(),
        project_id="project-1",
        profile_id=profile.id,
        expected_profile_revision=profile.revision,
        expected_bucket_binding_id="binding-1",
    )

    assert isinstance(composed.port, TOSAdapter)
    assert composed.identity.storage_profile_id == profile.id
    assert composed.identity.profile_revision == profile.revision
    assert composed.identity.bucket == profile.bucket
    assert composed.identity.endpoint == profile.endpoint
    assert composed.identity.region == profile.region
    assert composed.identity.capability == {
        "profileRevision": profile.revision,
        "minPartSizeBytes": 1,
        "maxPartSizeBytes": 64 * 1024 * 1024,
        "maxPartCount": 10_000,
        "maxObjectSizeBytes": 8 * 1024**4,
    }
    assert len(composed.identity.snapshot_hash) == 64
    with pytest.raises(ValidationDomainError, match="bucket binding"):
        await composition.resolve_storage(
            StorageProfiles(),
            project_id="project-1",
            profile_id=profile.id,
            expected_profile_revision=profile.revision,
            expected_bucket_binding_id="foreign-binding",
        )


@pytest.mark.asyncio
async def test_renderer_composition_requires_durable_catalog_identity_and_paths() -> None:
    uow = InMemoryUnitOfWork()
    composition = CatalogRuntimeComposition(CatalogRuntimeResolver(lambda: uow), object())
    with pytest.raises(ValidationDomainError, match="renderer_unconfigured"):
        await composition.resolve_renderer(
            profile_id=None,
            ffmpeg_path="/bin/ffmpeg",
            ffprobe_path="/bin/ffprobe",
            renderer_factory=lambda *_args: object(),
        )
    with pytest.raises(ValidationDomainError, match="renderer_unconfigured"):
        await composition.resolve_renderer(
            profile_id="renderer",
            ffmpeg_path=None,
            ffprobe_path="/bin/ffprobe",
            renderer_factory=lambda *_args: object(),
        )

    provider = Provider(
        "FFmpeg",
        "ffmpeg",
        approval="approved",
        adapter_installed=True,
        enabled=True,
    )
    profile = ProviderProfile(
        provider.id,
        "local renderer",
        adapter_identity="ffmpeg",
        enabled=True,
        explicit_live_opt_in=True,
        revision=4,
    )
    snapshot = CapabilitySnapshot(
        provider.id,
        profile.id,
        "media.render",
        7,
        True,
        ("h264", "aac", "mp4"),
        "now",
    )
    profile.capability_snapshots[snapshot.operation] = snapshot
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile

    renderer = await composition.resolve_renderer(
        profile_id=profile.id,
        ffmpeg_path="/bin/ffmpeg",
        ffprobe_path="/bin/ffprobe",
        renderer_factory=lambda *_args: object(),
    )
    assert renderer.profile_id == profile.id
    assert renderer.revision == 4
    assert renderer.capability_snapshot_id == snapshot.id
    assert renderer.capability_revision == 7


def test_renderer_environment_paths_alone_do_not_create_runtime_renderer(tmp_path: Path) -> None:
    runtime = build_runtime(
        RuntimeSettings.from_mapping(
            {
                "PROVIDER_MODE": "mock",
                "STORAGE_MODE": "local_workspace",
                "WORKSPACE_ROOT": str(tmp_path),
                "FFMPEG_PATH": "/usr/bin/ffmpeg",
                "FFPROBE_PATH": "/usr/bin/ffprobe",
            }
        )
    )
    assert runtime.selection.renderer_profile_id is None


def test_image_generation_api_requires_project_scope_and_configured_service(tmp_path: Path) -> None:
    app = create_app(
        readiness_probe=lambda: True,
        settings=RuntimeSettings("mock", "local_workspace", tmp_path),
    )
    client = TestClient(app)

    missing_scope = client.post("/v1/projects/project-1/image-candidates", json={})
    assert missing_scope.status_code == 403
    unavailable = client.post(
        "/v1/projects/project-1/image-candidates",
        headers={"X-Project-Scope": "project-1"},
        json={},
    )
    assert unavailable.status_code == 503


def test_image_generation_api_enqueues_without_sync_execution(tmp_path: Path) -> None:
    app = create_app(
        readiness_probe=lambda: True,
        settings=RuntimeSettings("mock", "local_workspace", tmp_path),
    )

    class AsyncOnlyService:
        calls: list[object] = []

        async def enqueue(self, command: object, *, project_scope: str | None = None) -> object:
            self.calls.append((command, project_scope))
            return ImageGenerationOperation("operation-1", "run-1", "image.generate:1", "pending")

    service = AsyncOnlyService()
    app.state.image_generation_service = service
    client = TestClient(app)
    response = client.post(
        "/v1/projects/project-1/image-candidates",
        headers={"X-Project-Scope": "project-1"},
        json={
            "episodeId": "episode-1",
            "targetId": "shot-1",
            "assetId": "asset-1",
            "runId": "run-1",
            "logicalOperation": "image.generate:1",
            "operation": "generate",
            "prompt": "frame",
            "providerId": "provider-1",
            "profileId": "profile-1",
            "profileRevision": 1,
            "modelId": "model-1",
            "capabilitySnapshotId": "snapshot-1",
            "capabilityRevision": 1,
            "continuitySnapshotId": "continuity-1",
            "continuitySnapshotRevision": 1,
            "continuitySnapshotHash": "a" * 64,
            "targetRevision": 1,
        },
    )

    assert response.status_code == 202
    assert response.json() == {
        "id": "operation-1",
        "runId": "run-1",
        "logicalOperation": "image.generate:1",
        "status": "pending",
        "candidateId": None,
        "candidateStatus": None,
    }
    assert len(service.calls) == 1


def test_api_consumes_runtime_settings(tmp_path: Path) -> None:
    app = create_app(
        readiness_probe=lambda: True,
        settings=RuntimeSettings("mock", "local_workspace", tmp_path),
    )

    assert app.state.runtime.provider_mode == "mock"
    assert app.state.runtime.storage_mode == "local_workspace"


def test_compose_passes_runtime_modes_to_api_and_all_workers() -> None:
    path = Path(__file__).parents[3] / "infra/compose/compose.yaml"
    compose = yaml.safe_load(path.read_text(encoding="utf-8"))

    for service_name in ("api", "agent", "generation", "media"):
        environment = compose["services"][service_name]["environment"]
        assert environment["PROVIDER_MODE"] == "${PROVIDER_MODE}"
        assert environment["STORAGE_MODE"] == "${STORAGE_MODE}"
        assert environment["WORKSPACE_ROOT"] == "${WORKSPACE_ROOT}"
    assert compose["services"]["agent"]["environment"]["DATABASE_URL"] == "${DATABASE_URL}"
    assert compose["services"]["media"]["environment"]["DATABASE_URL"] == "${DATABASE_URL}"
    assert compose["services"]["api"]["volumes"] == ["local-workspace:${WORKSPACE_ROOT}"]
    assert compose["services"]["media"]["volumes"] == ["local-workspace:${WORKSPACE_ROOT}"]
    assert "local-workspace" in compose["volumes"]


def test_compose_passes_explicit_live_references_without_implicit_defaults() -> None:
    path = Path(__file__).parents[3] / "infra/compose/compose.yaml"
    compose = yaml.safe_load(path.read_text(encoding="utf-8"))

    for service_name in ("api", "generation", "media"):
        environment = compose["services"][service_name]["environment"]
        assert environment["PROVIDER_PROFILE_ID"] == "${PROVIDER_PROFILE_ID}"
        assert environment["PROVIDER_MODEL_ID"] == "${PROVIDER_MODEL_ID}"
        assert environment["PROVIDER_CREDENTIAL_REF"] == "${PROVIDER_CREDENTIAL_REF}"
        assert environment["STORAGE_PROFILE_ID"] == "${STORAGE_PROFILE_ID}"
        assert environment["STORAGE_BUCKET_BINDING_ID"] == "${STORAGE_BUCKET_BINDING_ID}"
        assert environment["STORAGE_CREDENTIAL_REF"] == "${STORAGE_CREDENTIAL_REF}"
    assert compose["services"]["media"]["environment"]["RENDERER_PROFILE_ID"] == (
        "${RENDERER_PROFILE_ID}"
    )


def test_compose_declares_seven_runtime_services_and_fail_closed_prerequisites() -> None:
    path = Path(__file__).parents[3] / "infra/compose/compose.yaml"
    compose = yaml.safe_load(path.read_text(encoding="utf-8"))
    services = compose["services"]

    assert {"postgres", "temporal", "api", "web", "agent", "generation", "media"} <= set(services)
    for service_name in ("api", "generation", "media"):
        environment = services[service_name]["environment"]
        assert environment["EXPECTED_MIGRATION_HEAD"] == "${EXPECTED_MIGRATION_HEAD}"
        assert services[service_name]["depends_on"]["temporal"]["condition"] == "service_healthy"
    assert services["media"]["environment"]["FFMPEG_PATH"] == "${FFMPEG_PATH}"
    assert services["media"]["environment"]["FFPROBE_PATH"] == "${FFPROBE_PATH}"
    assert services["migrate"]["profiles"] == ["migration"]

    example = _example_environment()
    assert example["EXPECTED_MIGRATION_HEAD"] == CURRENT_MIGRATION_HEAD
    assert example["PROVIDER_CREDENTIAL_REF"] == ""
    assert example["STORAGE_CREDENTIAL_REF"] == ""
    assert all(
        not port or str(port).startswith("127.0.0.1:")
        for service in services.values()
        for port in service.get("ports", [])
    )


def test_mock_provider_and_storage_emit_structured_boundary_events(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    provider_events: list[tuple[str, str | None, dict[str, object]]] = []
    storage_events: list[tuple[str, str | None, dict[str, object]]] = []

    def capture_provider(event: str, *, correlation_id: str | None = None, **data: object) -> None:
        provider_events.append((event, correlation_id, data))

    def capture_storage(event: str, *, correlation_id: str | None = None, **data: object) -> None:
        storage_events.append((event, correlation_id, data))

    monkeypatch.setattr("video_agent_api.ports.mocks.log_event", capture_provider)
    monkeypatch.setattr("video_agent_api.ports.storage.log_event", capture_storage)

    selection = ModelSelection("mock", "profile", "model", "mock")
    provider = DeterministicMockProvider()
    provider.generate_text("hello", selection, correlation_id="trace-provider")
    with pytest.raises(RuntimeError, match="explicit error"):
        provider.generate_text("__mock_error__", selection, correlation_id="trace-error")

    storage = LocalWorkspaceAdapter(tmp_path)
    storage.put("asset.txt", b"content", correlation_id="trace-storage")
    with pytest.raises(ValueError, match="escapes workspace root"):
        storage.put("../escape.txt", b"content", correlation_id="trace-escape")
    with pytest.raises(AdapterNotConfiguredError):
        TOSAdapter().put("asset.bin", b"content", correlation_id="trace-tos")

    assert provider_events == [
        (
            "provider.call",
            "trace-provider",
            {"operation": "text.generate", "adapter": "mock", "result": "success"},
        ),
        (
            "provider.call",
            "trace-error",
            {
                "operation": "text.generate",
                "adapter": "mock",
                "result": "error",
                "error_type": "RuntimeError",
            },
        ),
    ]
    assert storage_events == [
        (
            "storage.call",
            "trace-storage",
            {"operation": "put", "adapter": "local_workspace", "result": "success"},
        ),
        (
            "storage.call",
            "trace-escape",
            {
                "operation": "put",
                "adapter": "local_workspace",
                "result": "error",
                "error_type": "ValueError",
            },
        ),
        (
            "storage.call",
            "trace-tos",
            {
                "operation": "put",
                "adapter": "tos",
                "result": "error",
                "error_type": "AdapterNotConfiguredError",
            },
        ),
    ]
