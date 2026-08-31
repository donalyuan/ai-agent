from __future__ import annotations

import hashlib
from dataclasses import replace
from io import BytesIO
from pathlib import Path
from types import SimpleNamespace

import pytest
from alembic.config import Config
from fastapi.testclient import TestClient
from sqlalchemy import create_engine, inspect, text

from alembic import command
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.assets import (
    AssetsService,
    CreateAssetCommand,
    CreateReservationCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.storage_handoffs import (
    AssetUploadCoordinator,
    asset_upload_intent,
    audio_asset_handoff,
    source_material_handoff,
)
from video_agent_api.application.storage_profiles import StorageProfileService
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.source_material import SourceMaterialUploadIntent
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    DeleteProof,
    PartReceipt,
    StorageAuthorizationError,
    StorageMediaValidationError,
    StorageObjectInUseError,
    StorageValidationError,
    StorageWriteIntent,
    StoredObjectRef,
)
from video_agent_api.ports.credentials import CatalogCredentialResolver, CredentialKeyring
from video_agent_api.ports.storage import (
    CompositeStorageReferenceProof,
    LocalWorkspaceAdapter,
    TOSAdapter,
)


def _receipt(part_number: int, content: bytes) -> PartReceipt:
    digest = hashlib.sha256(content).hexdigest()
    return PartReceipt(part_number, digest, f"etag-{part_number}", len(content))


def _intent(content: bytes = b"hello") -> StorageWriteIntent:
    return StorageWriteIntent(
        "asset-upload:project:asset:reservation",
        "project",
        "local",
        "projects/project/assets/asset/reservation/original.bin",
        len(content),
        hashlib.sha256(content).hexdigest(),
        "application/octet-stream",
    )


def test_local_multipart_survives_restart_and_validates_before_object_publish(
    tmp_path: Path,
) -> None:
    root = tmp_path / "workspace"
    first = LocalWorkspaceAdapter(root)
    intent = _intent()
    session = first.create_multipart(intent, "corr-create")
    first_part = _receipt(1, b"hel")
    first.upload_part(session, first_part, b"hel", "corr-part")

    restarted = LocalWorkspaceAdapter(root)
    assert restarted.resume_multipart(intent, "corr-resume") == session
    second_part = _receipt(2, b"lo")
    restarted.upload_part(session, second_part, b"lo", "corr-part")
    result = restarted.complete_multipart(session, (first_part, second_part), "corr-complete")
    assert result.size_bytes == 5
    assert result.checksum == intent.expected_checksum
    assert restarted.stat(f"workspace://{result.object_key}").checksum == result.checksum
    assert (
        LocalWorkspaceAdapter(root).complete_multipart(
            session, (first_part, second_part), "corr-retry"
        )
        == result
    )

    invalid = replace(
        _intent(b"other"),
        operation_key="asset-upload:project:asset:other",
        object_key="projects/project/assets/asset/other/original.bin",
    )
    invalid_session = restarted.create_multipart(invalid, "corr-invalid")
    actual = _receipt(1, b"wrong")
    restarted.upload_part(invalid_session, actual, b"wrong", "corr-invalid")
    with pytest.raises(StorageMediaValidationError, match="media_validation_failed"):
        restarted.complete_multipart(invalid_session, (actual,), "corr-invalid")
    assert not (root / invalid.object_key).exists()


def test_local_scope_presign_conflict_capacity_and_delete_proof(tmp_path: Path) -> None:
    storage = LocalWorkspaceAdapter(tmp_path)
    with pytest.raises(StorageValidationError):
        storage.create_multipart(
            StorageWriteIntent("op", "project", "local", "projects/foreign/object"), "corr"
        )
    intent = _intent(b"one")
    session = storage.create_multipart(intent, "corr")
    receipt = _receipt(1, b"one")
    storage.upload_part(session, receipt, b"one", "corr")
    with pytest.raises(StorageValidationError, match="multipart_part_conflict"):
        storage.upload_part(session, _receipt(1, b"two"), b"two", "corr")
    result = storage.complete_multipart(session, (receipt,), "corr")
    with pytest.raises(StorageAuthorizationError):
        storage.presign_read_ref(result, "foreign", 10)
    with pytest.raises(StorageAuthorizationError):
        storage.presign_read_ref(result, "project", 901)
    assert storage.presign_read_ref(result, "project", 10).action == "read"
    storage.admit_upload(2_147_483_648, 64 * 1024 * 1024)
    with pytest.raises(StorageValidationError, match="storage_object_size_unsupported"):
        storage.admit_upload(2_147_483_648, 128 * 1024 * 1024)
    with pytest.raises(StorageObjectInUseError):
        storage.delete_with_proof(
            result, DeleteProof("project", "other", "now", True, "proof"), "corr"
        )
    with pytest.raises(StorageObjectInUseError, match="unavailable"):
        storage.prove_no_references(result, "project", "corr")


@pytest.mark.parametrize(
    "object_key",
    (
        "projects/project/a/./file.bin",
        "projects/project/a//file.bin",
        "projects/project/a/",
        "projects/project/file.bin?x",
        "projects/project/file.bin#x",
    ),
)
def test_local_storage_rejects_noncanonical_intent_keys(tmp_path: Path, object_key: str) -> None:
    storage = LocalWorkspaceAdapter(tmp_path)
    with pytest.raises(StorageValidationError):
        storage.create_multipart(
            StorageWriteIntent("operation", "project", "local", object_key), "corr"
        )


def test_operation_key_reuse_requires_complete_frozen_binding(tmp_path: Path) -> None:
    storage = LocalWorkspaceAdapter(tmp_path)
    intent = _intent(b"one")
    session = storage.create_multipart(intent, "corr")
    mutations = (
        replace(intent, project_id="other", object_key="projects/other/object.bin"),
        replace(intent, profile_id="other"),
        replace(intent, object_key="projects/project/other.bin"),
        replace(intent, expected_size_bytes=999),
        replace(intent, expected_checksum="f" * 64),
        replace(intent, expected_mime_type="text/plain"),
    )
    for mutated in mutations:
        with pytest.raises(StorageValidationError, match="binding conflict"):
            storage.create_multipart(mutated, "corr-conflict")

    forged = replace(session, project_id="other")
    with pytest.raises(StorageValidationError, match="session binding"):
        storage.complete_multipart(forged, (), "corr-complete")


@pytest.mark.parametrize(
    ("forged", "field"),
    [
        (PartReceipt(1, "0" * 64, "etag-1", 3), "checksum"),
        (PartReceipt(1, hashlib.sha256(b"one").hexdigest(), "forged-etag", 3), "etag"),
        (PartReceipt(1, hashlib.sha256(b"one").hexdigest(), "etag-1", 999), "size"),
    ],
)
def test_complete_rejects_forged_receipt_without_persisting_manifest(
    tmp_path: Path, forged: PartReceipt, field: str
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path)
    intent = _intent(b"one")
    session = storage.create_multipart(intent, "corr")
    actual = storage.upload_part(session, _receipt(1, b"one"), b"one", "corr")

    with pytest.raises(StorageValidationError, match="multipart manifest mismatch"):
        storage.complete_multipart(session, (forged,), f"corr-{field}")

    assert storage.resume_multipart(intent, "corr-resume").status == "active"
    result = storage.complete_multipart(session, (actual,), "corr-complete")
    assert storage.complete_multipart(session, (actual,), "corr-retry") == result


def test_reference_proof_fails_closed_until_every_owner_is_available() -> None:
    object_ref = StoredObjectRef(
        "project",
        "profile",
        "bucket",
        "projects/project/object",
        1,
        "a" * 64,
        "application/octet-stream",
        "etag",
        "operation",
    )
    with pytest.raises(StorageObjectInUseError, match="incomplete"):
        CompositeStorageReferenceProof({}).prove_no_references(object_ref, "project", "corr")
    checks = {name: (lambda _ref: False) for name in CompositeStorageReferenceProof.REQUIRED_OWNERS}
    proof = CompositeStorageReferenceProof(checks).prove_no_references(
        object_ref, "project", "corr"
    )
    assert proof.no_references
    checks["timeline"] = lambda _ref: True
    with pytest.raises(StorageObjectInUseError, match="referenced"):
        CompositeStorageReferenceProof(checks).prove_no_references(object_ref, "project", "corr")


def test_tos_is_unconfigured_by_default_and_fake_is_explicit(tmp_path: Path) -> None:
    with pytest.raises(AdapterNotConfiguredError):
        TOSAdapter().create_multipart(_intent(), "corr")
    fake = LocalWorkspaceAdapter(tmp_path)
    adapter = TOSAdapter(fake)
    session = adapter.create_multipart(_intent(), "corr")
    receipt = _receipt(1, b"hello")
    adapter.upload_part(session, receipt, b"hello", "corr")
    assert adapter.complete_multipart(session, (receipt,), "corr").verified

    keyring = CredentialKeyring(b"k" * 32, "v1")
    envelope = keyring.seal("ak:sk", profile_id="profile", credential_id="credential")
    resolver = CatalogCredentialResolver(keyring, {"profile": envelope})
    configured = TOSAdapter(fake, resolver, "credential", "profile")
    assert configured.resolve_credential() == "ak:sk"
    with pytest.raises(AdapterNotConfiguredError):
        TOSAdapter(fake).resolve_credential()


def test_tos_client_requires_complete_private_profile_and_never_uses_local_fallback() -> None:
    class Profile:
        id = "profile"
        adapter_key = "tos"
        endpoint = "https://tos.example.test"
        region = "cn-test"
        bucket = "private-bucket"
        bucket_binding_id = "private-binding"
        project_id = "project"
        project_scope = ("project",)
        credential_ref = "credential"
        credential_status = "configured"
        enabled = True
        private_bucket = True
        revision = 3

    calls: list[tuple[str, str, str, str]] = []

    def client_factory(ak: str, sk: str, endpoint: str, region: str) -> object:
        calls.append((ak, sk, endpoint, region))
        return object()

    class Resolver:
        def resolve(self, credential_ref: str, profile_id: str) -> str:
            assert (credential_ref, profile_id) == ("credential", "profile")
            return "access-key:secret-key"

    adapter = TOSAdapter.from_profile(Profile(), Resolver(), client_factory=client_factory)
    assert adapter.adapter_key == "tos"
    assert adapter.profile_id == "profile"
    assert adapter.profile_revision == 3
    assert calls == [("access-key", "secret-key", "https://tos.example.test", "cn-test")]

    Profile.private_bucket = False
    with pytest.raises(AdapterNotConfiguredError, match="profile"):
        TOSAdapter.from_profile(Profile(), Resolver(), client_factory=client_factory)
    assert len(calls) == 1


def test_tos_profile_uses_sdk_for_multipart_stat_read_and_write() -> None:
    class Profile:
        id = "profile"
        adapter_key = "tos"
        endpoint = "https://tos.example.test"
        region = "cn-test"
        bucket = "private-bucket"
        bucket_binding_id = "private-binding"
        project_id = "project"
        project_scope = ("project",)
        credential_ref = "credential"
        credential_status = "configured"
        enabled = True
        private_bucket = True
        revision = 3

    class Client:
        def __init__(self) -> None:
            self.parts: dict[int, bytes] = {}
            self.objects: dict[str, tuple[bytes, str, dict[str, str]]] = {}

        def create_multipart_upload(self, bucket: str, key: str, **kwargs: object) -> object:
            assert bucket == "private-bucket"
            assert kwargs["meta"] == {
                "operation-key": "asset-upload:project:asset:reservation",
                "project-id": "project",
                "profile-id": "profile",
                "sha256": hashlib.sha256(b"hello").hexdigest(),
            }
            assert kwargs["content_type"] == "application/octet-stream"
            return SimpleNamespace(upload_id="upload-1")

        def upload_part(
            self, bucket: str, key: str, upload_id: str, part_number: int, **kwargs: object
        ) -> object:
            assert (bucket, key, upload_id, part_number) == (
                "private-bucket",
                "projects/project/assets/asset/reservation/original.bin",
                "upload-1",
                1,
            )
            self.parts[part_number] = kwargs["content"]  # type: ignore[assignment]
            return SimpleNamespace(etag="tos-etag-1")

        def complete_multipart_upload(
            self, bucket: str, key: str, upload_id: str, parts: list[dict[str, object]]
        ) -> object:
            assert (bucket, key, upload_id) == (
                "private-bucket",
                "projects/project/assets/asset/reservation/original.bin",
                "upload-1",
            )
            assert parts == [{"part_number": 1, "etag": "tos-etag-1"}]
            content = b"".join(self.parts.values())
            self.objects[key] = (
                content,
                "application/octet-stream",
                {
                    "operation-key": "asset-upload:project:asset:reservation",
                    "project-id": "project",
                    "profile-id": "profile",
                    "sha256": hashlib.sha256(content).hexdigest(),
                },
            )
            return SimpleNamespace()

        def head_object(self, bucket: str, key: str) -> object:
            assert bucket == "private-bucket"
            content, mime_type, meta = self.objects[key]
            return SimpleNamespace(
                content_length=len(content),
                content_type=mime_type,
                etag="tos-object-etag",
                meta=meta,
            )

        def get_object(self, bucket: str, key: str) -> object:
            assert bucket == "private-bucket"
            return SimpleNamespace(read=BytesIO(self.objects[key][0]).read)

        def put_object(self, bucket: str, key: str, **kwargs: object) -> object:
            assert bucket == "private-bucket"
            content = kwargs["content"]
            assert isinstance(content, bytes)
            self.objects[key] = (content, str(kwargs["content_type"]), dict(kwargs["meta"]))
            return SimpleNamespace(etag="tos-put-etag")

    client = Client()

    class Resolver:
        def resolve(self, credential_ref: str, profile_id: str) -> str:
            assert (credential_ref, profile_id) == ("credential", "profile")
            return "access-key:secret-key"

    adapter = TOSAdapter.from_profile(Profile(), Resolver(), client_factory=lambda *_args: client)
    intent = replace(_intent(), profile_id="profile")
    session = adapter.create_multipart(intent, "corr-create")
    receipt = adapter.upload_part(session, _receipt(1, b"hello"), b"hello", "corr-part")
    ref = adapter.complete_multipart(session, (receipt,), "corr-complete")

    assert ref == StoredObjectRef(
        "project",
        "profile",
        "private-bucket",
        intent.object_key,
        5,
        hashlib.sha256(b"hello").hexdigest(),
        "application/octet-stream",
        "tos-object-etag",
        intent.operation_key,
    )
    assert adapter.reconcile_multipart(intent, "corr-reconcile") == ref
    assert b"".join(adapter.iter_chunks(intent.object_key, 2)) == b"hello"
    stored = adapter.put("projects/project/direct.bin", b"direct", "corr-put")
    assert stored.size_bytes == 6 and stored.checksum == hashlib.sha256(b"direct").hexdigest()


def test_workspace_cleaner_obeys_success_failure_retention(tmp_path: Path) -> None:
    storage = LocalWorkspaceAdapter(tmp_path)
    storage.put("tmp/success.bin", b"ok", "corr")
    storage.put("tmp/failure.bin", b"bad", "corr")
    storage.put("projects/project/kept.bin", b"business", "corr")
    storage.register_temporary("tmp/success.bin", "succeeded", created_at=0)
    storage.register_temporary("tmp/failure.bin", "failed", created_at=0)
    with pytest.raises(StorageValidationError):
        storage.register_temporary("projects/project/kept.bin", "succeeded", created_at=0)
    assert storage.clean_workspace(now=24 * 3600) == ("tmp/success.bin",)
    assert (tmp_path / "tmp/failure.bin").exists()
    assert storage.clean_workspace(now=7 * 24 * 3600) == ("tmp/failure.bin",)
    assert (tmp_path / "projects/project/kept.bin").exists()


def test_storage_profile_http_lifecycle_is_masked_and_uses_if_match() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=projects,
        assets_service=AssetsService(lambda: uow),
    )
    client = TestClient(app)
    project = client.post("/v1/projects", json={"name": "Storage"}).json()
    created = client.post(
        "/v1/storage-profiles",
        json={
            "projectId": project["id"],
            "name": "TOS private",
            "endpoint": "https://tos.example.invalid",
            "bucket": "private-bucket",
            "region": "cn-test",
            "bucketBindingId": "binding",
            "credentialRef": "credential-ref",
            "projectScope": [project["id"]],
        },
    )
    assert created.status_code == 201
    profile = created.json()
    assert profile["credentialRef"] == "credential-ref"
    assert profile["credentialStatus"] == "unconfigured"
    assert "secret" not in str(profile).lower()
    profile_id = profile["storageProfileId"]
    missing_match = client.post(
        f"/v1/storage-profiles/{profile_id}/enable", json={"expectedRevision": 1}
    )
    assert missing_match.status_code == 409
    enabled = client.post(
        f"/v1/storage-profiles/{profile_id}/enable",
        headers={"If-Match": "1"},
        json={"expectedRevision": 1},
    )
    assert enabled.status_code == 200 and enabled.json()["revision"] == 2
    stale = client.post(
        f"/v1/storage-profiles/{profile_id}/disable",
        headers={"If-Match": "1"},
        json={"expectedRevision": 1},
    )
    assert stale.status_code == 409
    probe = client.post(
        f"/v1/storage-profiles/{profile_id}/connection-test",
        json={"expectedRevision": 2, "probeCorrelationId": "probe-1"},
    )
    assert probe.json() == {
        "status": "unconfigured",
        "diagnostic": "storage_credential_unconfigured",
        "probeCorrelationId": "probe-1",
    }


@pytest.mark.asyncio
@pytest.mark.parametrize("status", ["connected", "authentication", "network", "timeout"])
async def test_storage_connection_probe_preserves_safe_transport_status(status: str) -> None:
    uow = InMemoryUnitOfWork()
    project = await ProjectsEpisodesService(lambda: uow).create_project("Probe")
    from video_agent_api.ports.storage import StorageProfile

    profile = StorageProfile(
        "profile",
        project.id,
        "https://tos.invalid",
        "private",
        "cn-test",
        credential_status="configured",
        enabled=True,
        project_scope=(project.id,),
    )
    uow.storage_profiles[profile.id] = profile
    service = StorageProfileService(
        lambda: uow,
        lambda _profile, _correlation: {"status": status, "providerCode": "safe-code"},
    )
    before = profile.revision
    result = await service.connection_test(profile.id, before, "probe-status")
    assert result == {
        "status": status,
        "providerCode": "safe-code",
        "probeCorrelationId": "probe-status",
    }
    assert profile.revision == before


@pytest.mark.asyncio
async def test_asset_source_audio_handoffs_preserve_owner_registration(tmp_path: Path) -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("Storage")
    assets = AssetsService(lambda: uow)
    asset = await assets.create_asset(CreateAssetCommand(project.id, "Audio", "audio"))
    content = b"audio"
    checksum = hashlib.sha256(content).hexdigest()
    reservation = await assets.create_reservation(
        CreateReservationCommand(
            project.id,
            asset.id,
            "f" * 64,
            asset.revision,
            "audio",
            "audio/wav",
            len(content),
            checksum,
            "local",
            1,
            "a" * 64,
        )
    )
    intent = asset_upload_intent(
        reservation,
        "local",
        f"projects/{project.id}/assets/{asset.id}/{reservation.id}/original.wav",
        expected_size_bytes=len(content),
        expected_checksum=hashlib.sha256(content).hexdigest(),
        expected_mime_type="audio/wav",
    )
    storage = LocalWorkspaceAdapter(tmp_path)
    session = storage.create_multipart(intent, "corr")
    receipt = _receipt(1, content)
    storage.upload_part(session, receipt, content, "corr")
    coordinator = AssetUploadCoordinator(storage, assets)
    version = await coordinator.complete_and_register(reservation.id, session, (receipt,), "corr")
    retry = await coordinator.complete_and_register(reservation.id, session, (receipt,), "corr")
    assert retry.id == version.id and len(uow.state.asset_versions) == 1
    object_ref = coordinator.recovery_records[-1].object_ref
    assert object_ref is not None
    audio = audio_asset_handoff(project.id, reservation.id, object_ref, "authorized", "CC0")
    assert not audio.selected
    with pytest.raises(ValidationDomainError, match="authorization"):
        audio_asset_handoff(project.id, reservation.id, object_ref, "unknown", "")

    source_intent = SourceMaterialUploadIntent(
        project.id,
        "source",
        1,
        "novel",
        "uploaded_file",
        "a" * 64,
        f"source-material-upload:{project.id}:source:1",
        "source-reservation",
        project_scope=project.id,
    )
    source_session = session.__class__(
        "source-session",
        source_intent.operation_key,
        project.id,
        "local",
        f"projects/{project.id}/sources/source/original.txt",
    )
    source_ref = StoredObjectRef(
        project.id,
        "local",
        "workspace",
        source_session.object_key,
        5,
        "a" * 64,
        "text/plain",
        "etag",
        source_intent.operation_key,
    )
    handoff = source_material_handoff(source_intent, source_session, source_ref, 1)
    assert handoff.reservation_id == "source-reservation"


def test_storage_owner_migration_tables_and_constraints(tmp_path: Path) -> None:
    api_root = Path(__file__).parents[1]
    database_url = f"sqlite:///{tmp_path / 'storage.db'}"
    config = Config(str(api_root / "alembic.ini"))
    config.set_main_option("script_location", str(api_root / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    tables = set(inspect(engine).get_table_names())
    assert {
        "storage_profiles",
        "storage_bucket_bindings",
        "storage_upload_operations",
        "storage_upload_sessions",
        "storage_upload_parts",
        "stored_objects",
        "storage_reference_proofs",
        "storage_recovery_records",
    } <= tables
    with engine.connect() as connection:
        assert connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one() == (
            "0029_lookup_binding"
        )
        assert {
            row[1] for row in connection.execute(text("PRAGMA table_info(workflow_node_runs)"))
        } >= {"admission_refs"}
        assert {
            row[1] for row in connection.execute(text("PRAGMA table_info(video_operations)"))
        } >= {"admission_refs"}
    command.downgrade(config, "0018_asset_edit_owner")
    assert not ({"storage_profiles", "stored_objects"} & set(inspect(engine).get_table_names()))
    engine.dispose()
