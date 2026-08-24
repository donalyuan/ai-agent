from __future__ import annotations

import hashlib

import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.assets import AssetsService
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.domain.media import MediaDerivative, MediaInspection, source_fingerprint
from video_agent_api.ports.storage import StorageProfile

CONTENT = b"abc"
HASH = hashlib.sha256(CONTENT).hexdigest()
SCHEMA = "1.0.0"


def _client() -> tuple[TestClient, InMemoryUnitOfWork]:
    uow = InMemoryUnitOfWork()
    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=ProjectsEpisodesService(lambda: uow),
        assets_service=AssetsService(lambda: uow),
    )
    return TestClient(app), uow


def _create_asset(client: TestClient, project_id: str, **overrides: object) -> dict[str, object]:
    payload: dict[str, object] = {
        "kind": "audio",
        "name": "Voice",
        "sourceType": "user_upload",
        "catalogRole": "dialogue",
        "tags": ["lead"],
        "authorizationStatus": "verified",
        "licenseLabel": "Owned",
        "schemaVersion": SCHEMA,
    }
    payload.update(overrides)
    response = client.post(f"/v1/projects/{project_id}/assets", json=payload)
    assert response.status_code == 201, response.text
    return response.json()


def _reservation_payload(
    client: TestClient,
    project_id: str,
    asset: dict[str, object],
    fingerprint: str = HASH,
) -> dict[str, object]:
    admission = client.post(
        f"/v1/projects/{project_id}/asset-upload-admissions",
        json={
            "storageProfileId": "local-test-offline",
            "storageProfileRevision": 1,
            "declaredMimeType": "audio/wav",
            "declaredSizeBytes": 3,
            "partSizeBytes": 1,
            "schemaVersion": SCHEMA,
        },
    )
    assert admission.status_code == 200, admission.text
    return {
        "fingerprint": fingerprint,
        "expectedAssetRevision": asset["revision"],
        "declaredKind": asset["kind"],
        "declaredMimeType": "audio/wav",
        "declaredSizeBytes": 3,
        "declaredChecksum": HASH,
        "storageProfileId": "local-test-offline",
        "storageProfileRevision": 1,
        "storageProfileSnapshotHash": admission.json()["storageProfileSnapshotHash"],
        "partSizeBytes": 1,
        "schemaVersion": SCHEMA,
    }


def _media_metadata(checksum: str) -> dict[str, object]:
    return {
        "mimeType": "audio/wav",
        "sizeBytes": len(CONTENT),
        "checksum": checksum,
        "durationFrames": 3,
        "timebase": "1/30",
        "fpsNumerator": 30,
        "fpsDenominator": 1,
        "frameCount": 3,
        "width": 0,
        "height": 0,
        "videoCodec": "none",
        "pixelFormat": "none",
        "audioTracks": 1,
        "sampleRate": 48000,
        "channels": 2,
    }


def test_assets_http_contract_catalog_cas_and_no_read_side_effects() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    asset = _create_asset(client, project["id"])
    assert "schema_version" not in asset
    assert asset["schemaVersion"] == SCHEMA
    assert asset["authorizationStatus"] == "verified"
    assert (
        client.get(
            f"/v1/assets/{asset['id']}",
            headers={"X-Project-Scope": "foreign"},
        ).status_code
        == 403
    )
    before = (
        len(uow.state.asset_versions),
        len(uow.state.asset_reservations),
        len(uow.state.provider_calls),
        sum(len(events) for events in uow.state.run_events.values()),
    )
    catalog = client.get(
        f"/v1/projects/{project['id']}/assets",
        params={"kind": "audio", "tag": "lead", "authorizationStatus": "verified"},
    )
    assert catalog.status_code == 200
    assert catalog.json()["items"][0]["versionCount"] == 0
    assert catalog.json()["items"][0]["processingStatus"] == "unknown"
    assert before == (
        len(uow.state.asset_versions),
        len(uow.state.asset_reservations),
        len(uow.state.provider_calls),
        sum(len(events) for events in uow.state.run_events.values()),
    )

    patched = client.patch(
        f"/v1/assets/{asset['id']}",
        headers={"If-Match": str(asset["revision"])},
        json={
            "expectedRevision": asset["revision"],
            "tags": ["lead", "episode-1"],
            "licenseReference": None,
            "schemaVersion": SCHEMA,
        },
    )
    assert patched.status_code == 200
    assert patched.json()["revision"] == 2
    assert patched.json()["tags"] == ["lead", "episode-1"]
    assert len(uow.state.audit_events) == 2

    foreign_patch = client.patch(
        f"/v1/assets/{asset['id']}",
        headers={"If-Match": "2", "X-Project-Scope": "foreign"},
        json={
            "expectedRevision": 2,
            "tags": ["foreign"],
            "schemaVersion": SCHEMA,
        },
    )
    assert foreign_patch.status_code == 403
    assert client.get(f"/v1/assets/{asset['id']}").json()["tags"] == [
        "lead",
        "episode-1",
    ]

    stale = client.patch(
        f"/v1/assets/{asset['id']}",
        headers={"If-Match": "1"},
        json={"expectedRevision": 1, "tags": ["stale"], "schemaVersion": SCHEMA},
    )
    assert stale.status_code == 409
    assert client.get(f"/v1/assets/{asset['id']}").json()["tags"] == ["lead", "episode-1"]
    assert (
        client.get(f"/v1/projects/{project['id']}/assets", params={"cursor": "invalid"}).status_code
        == 422
    )
    assert (
        client.get(
            f"/v1/projects/{project['id']}/assets",
            params={"processingStatus": "complete"},
        ).status_code
        == 422
    )


def test_asset_center_requires_matching_project_scope_before_owner_read() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Scoped"}).json()
    path = f"/v1/projects/{project['id']}/assets"

    assert client.get(path).status_code == 403
    assert client.get(path, headers={"X-Project-Scope": "foreign"}).status_code == 403
    assert uow.state.assets == {}
    assert client.get(path, headers={"X-Project-Scope": project["id"]}).status_code == 200


def test_reservation_register_is_idempotent_and_public_responses_are_safe() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    asset = _create_asset(client, project["id"])
    path = f"/v1/projects/{project['id']}/assets/{asset['id']}/reservations"
    created = client.post(path, json=_reservation_payload(client, project["id"], asset))
    assert created.status_code == 201, created.text
    reservation = created.json()
    retry = client.post(path, json=_reservation_payload(client, project["id"], asset))
    assert retry.status_code == 201
    assert retry.json()["id"] == reservation["id"]
    assert reservation["operationKey"].endswith(reservation["id"])

    upload_root = f"/v1/projects/{project['id']}/asset-reservations/{reservation['id']}/uploads"
    resumed = client.post(
        f"{upload_root}/resume",
        json={"correlationId": "asset-http", "schemaVersion": SCHEMA},
    )
    assert resumed.status_code == 200, resumed.text
    session = resumed.json()
    same_session = client.post(
        f"{upload_root}/resume",
        json={"correlationId": "asset-http", "schemaVersion": SCHEMA},
    ).json()
    assert same_session["sessionId"] == session["sessionId"]
    part = client.put(
        f"{upload_root}/{session['sessionId']}/parts/1",
        content=CONTENT,
        headers={
            "Content-Type": "application/octet-stream",
            "X-Part-Checksum": HASH,
            "X-Part-ETag": HASH,
            "X-Correlation-ID": "asset-http",
        },
    )
    assert part.status_code == 200, part.text
    complete_body = {
        "sessionId": session["sessionId"],
        "parts": [
            {
                "partNumber": 1,
                "checksum": HASH,
                "eTag": HASH,
                "sizeBytes": len(CONTENT),
            }
        ],
        "correlationId": "asset-http",
        "schemaVersion": SCHEMA,
    }
    first = client.post(f"{upload_root}/complete", json=complete_body)
    second = client.post(f"{upload_root}/complete", json=complete_body)
    assert first.status_code == second.status_code == 201
    assert first.json()["id"] == second.json()["id"]
    assert len(uow.state.asset_versions) == 1
    for response in (first, second, client.get(f"/v1/assets/{asset['id']}/versions")):
        assert "objectKey" not in response.text
        assert "workspace://" not in response.text
        assert "bucket" not in response.text
    assert first.json()["checksum"] == HASH

    media = client.get(f"/v1/projects/{project['id']}/asset-versions/{first.json()['id']}/media")
    usage = client.get(f"/v1/projects/{project['id']}/asset-versions/{first.json()['id']}/usage")
    assert media.json()["status"] == "unavailable"
    assert media.json()["diagnostic"] == "media_projection_unavailable"
    assert usage.json()["status"] == "complete"
    assert usage.json()["references"] == []

    version = next(iter(uow.state.asset_versions.values()))
    inspection = MediaInspection(
        project["id"],
        version.id,
        version.revision,
        version.content_hash,
        "ready",
        _media_metadata(version.content_hash),
        "ffprobe",
        "7.1",
        "inspect:http",
    )
    proxy_bytes = b"proxy-audio"
    proxy_checksum = hashlib.sha256(proxy_bytes).hexdigest()
    object_key = f"projects/{project['id']}/proxy.wav"
    derivative = MediaDerivative(
        project["id"],
        inspection.id,
        version.id,
        version.revision,
        version.content_hash,
        source_fingerprint(version.id, version.revision, version.content_hash),
        "proxy",
        "ready",
        {"audio": "normalized"},
        "ffmpeg",
        "7.1",
        "derive:http:proxy",
        object_ref={
            "profileId": "local-test-offline",
            "objectKey": object_key,
            "operationKey": "derive:http:proxy",
        },
        checksum=proxy_checksum,
        size_bytes=len(proxy_bytes),
    )
    uow.state.media_inspections[inspection.id] = inspection
    uow.state.media_derivatives[derivative.id] = derivative
    client.app.state.runtime.storage.put(object_key, proxy_bytes, "asset-http")
    ready = client.get(f"/v1/projects/{project['id']}/asset-versions/{version.id}/media")
    assert ready.json()["derivatives"][0]["grantAvailable"] is True
    grant = client.post(
        f"/v1/projects/{project['id']}/asset-versions/{version.id}/media/{derivative.id}/grant",
        json={"ttlSeconds": 120, "schemaVersion": SCHEMA},
    )
    assert grant.status_code == 200
    assert "objectKey" not in grant.text and "workspace://" not in grant.text
    client.app.state.runtime.storage.get = lambda _ref: pytest.fail(
        "media grant must stream instead of buffering the complete object"
    )
    streamed = client.get(grant.json()["accessPath"])
    assert streamed.status_code == 200
    assert streamed.content == proxy_bytes
    assert streamed.headers["cache-control"] == "private, no-store"

    episode = client.post(
        f"/v1/projects/{project['id']}/episodes",
        json={"title": "Episode", "number": 1},
    ).json()
    selection = client.post(
        f"/v1/projects/{project['id']}/asset-versions/{version.id}/timeline-selection",
        json={"episodeId": episode["id"], "schemaVersion": SCHEMA},
    )
    assert selection.status_code == 200
    assert selection.json()["assetVersionHash"] == version.content_hash
    assert "objectKey" not in selection.text

    access_token = grant.json()["accessPath"].rsplit("/", 1)[1]
    client.app.state.opaque_read_grants.resolve(access_token, now=grant.json()["expiresAt"] - 1)
    with pytest.raises(Exception, match="expired"):
        client.app.state.opaque_read_grants.resolve(access_token, now=grant.json()["expiresAt"])


def test_cancelled_reservation_rejects_late_registration() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    asset = _create_asset(client, project["id"])
    created = client.post(
        f"/v1/projects/{project['id']}/assets/{asset['id']}/reservations",
        json=_reservation_payload(
            client, project["id"], asset, hashlib.sha256(b"cancel").hexdigest()
        ),
    ).json()
    cancelled = client.post(
        f"/v1/projects/{project['id']}/asset-reservations/{created['id']}/cancel",
        headers={"If-Match": str(created["revision"])},
        json={"expectedRevision": created["revision"], "schemaVersion": SCHEMA},
    )
    assert cancelled.status_code == 200
    assert cancelled.json()["status"] == "cancelled"
    late = client.post(
        f"/v1/projects/{project['id']}/asset-reservations/{created['id']}/uploads/resume",
        json={"correlationId": "late", "schemaVersion": SCHEMA},
    )
    assert late.status_code == 422
    assert len(uow.state.asset_versions) == 0


def test_schema_alias_conflict_and_missing_schema_write_nothing() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    base = {"kind": "audio", "name": "Voice", "schemaVersion": SCHEMA}
    assert (
        client.post(
            f"/v1/projects/{project['id']}/assets",
            json={**base, "schema_version": SCHEMA},
        ).status_code
        == 422
    )
    without_schema = {key: value for key, value in base.items() if key != "schemaVersion"}
    assert (
        client.post(f"/v1/projects/{project['id']}/assets", json=without_schema).status_code == 422
    )
    assert uow.state.assets == {}


def test_usage_owner_failure_is_partial_not_false_empty() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    asset = _create_asset(client, project["id"])
    from video_agent_api.application.assets import AppendAssetVersionCommand
    from video_agent_api.domain.assets import StorageObject

    version = __import__("asyncio").run(
        AssetsService(lambda: uow).append_version(
            AppendAssetVersionCommand(
                str(asset["id"]),
                StorageObject(
                    "local_workspace",
                    "workspace",
                    f"projects/{project['id']}/usage.wav",
                    "audio/wav",
                    3,
                    HASH,
                ),
                HASH,
            )
        )
    )

    class UnavailableOwner(dict[str, object]):
        def values(self):
            raise RuntimeError("owner unavailable")

    uow.state.source_materials = UnavailableOwner()
    uow.source_materials = uow.state.source_materials
    response = client.get(f"/v1/projects/{project['id']}/asset-versions/{version.id}/usage")
    assert response.status_code == 200
    assert response.json()["status"] == "unavailable"
    assert response.json()["diagnostic"] == "usage_projection_unavailable"
    assert response.json()["references"] == []


def test_assets_http_without_service_returns_503_and_openapi_has_owner_routes() -> None:
    client = TestClient(create_app(readiness_probe=lambda: True))
    response = client.get("/v1/assets/missing", headers={"X-Project-Scope": "unknown-project"})
    assert response.status_code == 503
    assert response.json()["detail"]["type"] == "database_unavailable"

    owner_client, _ = _client()
    paths = owner_client.get("/openapi.json").json()["paths"]
    assert "/v1/projects/{projectId}/assets" in paths
    assert "/v1/projects/{projectId}/assets/{assetId}/reservations" in paths
    assert "/v1/projects/{projectId}/asset-versions/{versionId}/usage" in paths
    assert "/v1/projects/{projectId}/asset-versions/{versionId}/media" in paths


def test_upload_admission_resolves_profile_and_rejects_invalid_snapshot_before_writes() -> None:
    client, uow = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    foreign = client.post("/v1/projects", json={"name": "Foreign"}).json()
    asset = _create_asset(client, project["id"])
    path = f"/v1/projects/{project['id']}/assets/{asset['id']}/reservations"

    profiles = client.get(f"/v1/projects/{project['id']}/asset-upload-profiles")
    assert profiles.status_code == 200
    assert profiles.json() == [
        {
            "storageProfileId": "local-test-offline",
            "revision": 1,
            "name": "Local test/offline",
            "adapterKey": "local_workspace",
            "enabled": True,
        }
    ]
    payload = _reservation_payload(client, project["id"], asset)
    before = len(uow.state.asset_reservations)

    for changes, expected_status in (
        ({"storageProfileId": "missing"}, 404),
        ({"storageProfileRevision": 999}, 409),
        ({"storageProfileSnapshotHash": "f" * 64}, 422),
    ):
        response = client.post(path, json={**payload, **changes})
        assert response.status_code == expected_status, response.text
        assert len(uow.state.asset_reservations) == before

    uow.state.storage_profiles["disabled"] = StorageProfile(
        "disabled",
        project["id"],
        "workspace://local",
        "workspace",
        "local",
        credential_status="configured",
        enabled=False,
        adapter_key="local_workspace",
        bucket_binding_id="local-workspace",
        project_scope=(project["id"],),
    )
    uow.state.storage_profiles["foreign"] = StorageProfile(
        "foreign",
        foreign["id"],
        "workspace://local",
        "workspace",
        "local",
        credential_status="configured",
        enabled=True,
        adapter_key="local_workspace",
        bucket_binding_id="local-workspace",
        project_scope=(foreign["id"],),
    )
    for profile_id in ("disabled", "foreign"):
        response = client.post(path, json={**payload, "storageProfileId": profile_id})
        assert response.status_code == 422, response.text
        assert len(uow.state.asset_reservations) == before


@pytest.mark.parametrize("foreign", ["foreign-project", "missing"])
def test_foreign_project_projections_fail_closed(foreign: str) -> None:
    client, _ = _client()
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    client.headers["X-Project-Scope"] = str(project["id"])
    asset = _create_asset(client, project["id"])
    assert client.get(f"/v1/projects/{foreign}/assets").status_code == 403
    assert client.get(f"/v1/projects/{foreign}/asset-reservations/{asset['id']}").status_code == 403
