from __future__ import annotations

import json
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.assets import AssetsService

HASH = "c" * 64
CONTENT_HASH = "d" * 64
OBJECT_KEY_CORPUS = json.loads(
    (
        Path(__file__).parents[3]
        / "packages/contracts/tests/fixtures/object-key-contract-corpus.json"
    ).read_text(encoding="utf-8")
)


def test_assets_http_contract_and_aliases() -> None:
    uow = InMemoryUnitOfWork()
    app = create_app(readiness_probe=lambda: True, assets_service=AssetsService(lambda: uow))
    client = TestClient(app)
    project = client.post("/v1/projects", json={"name": "Demo"})
    assert project.status_code == 503  # no projects service is an explicit non-fallback boundary


def test_assets_http_with_shared_uow() -> None:
    uow = InMemoryUnitOfWork()
    from video_agent_api.application.projects_episodes import ProjectsEpisodesService

    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=ProjectsEpisodesService(lambda: uow),
        assets_service=AssetsService(lambda: uow),
    )
    client = TestClient(app)
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    created = client.post(
        f"/v1/projects/{project['id']}/assets", json={"kind": "audio", "name": "Voice"}
    )
    assert created.status_code == 201
    asset = created.json()
    version = client.post(
        f"/v1/assets/{asset['id']}/versions",
        json={
            "contentHash": CONTENT_HASH,
            "storageObject": {
                "storageProvider": "local",
                "bucket": "workspace",
                "objectKey": OBJECT_KEY_CORPUS["canonicalObjectKeys"][1],
                "mimeType": "audio/wav",
                "sizeBytes": 3,
                "checksum": HASH,
                "media": {"durationMs": 100},
            },
        },
    )
    assert version.status_code == 201
    assert version.json()["versionNumber"] == 1
    assert version.json()["contentHash"] == CONTENT_HASH
    assert version.json()["storageObject"]["checksum"] == HASH
    assert "schema_version" in version.json()
    assert "region" not in version.json()["storageObject"]
    assert "width" not in version.json()["storageObject"]["media"]
    listed = client.get(f"/v1/assets/{asset['id']}/versions").json()
    assert listed[0]["storageObject"]["sizeBytes"] == 3
    assert listed[0]["contentHash"] == CONTENT_HASH
    assert client.get(f"/v1/assets/{asset['id']}").status_code == 200
    assert client.get(f"/v1/projects/{project['id']}/assets").status_code == 200
    assert client.get("/v1/assets/missing").status_code == 404
    assert client.get("/v1/asset-versions/missing").status_code == 404
    assert (
        client.post(
            f"/v1/assets/{asset['id']}/versions",
            json={
                "storageObject": {
                    "storageProvider": "local",
                    "bucket": "workspace",
                    "objectKey": "/escape",
                    "mimeType": "audio/wav",
                    "sizeBytes": 1,
                    "checksum": HASH,
                }
            },
        ).status_code
        == 422
    )
    openapi = client.get("/openapi.json").json()
    assert "/v1/projects/{projectId}/assets" in openapi["paths"]
    assert "/v1/assets/{assetId}/versions" in openapi["paths"]
    assert "/v1/asset-versions/{versionId}" in openapi["paths"]


def test_assets_http_without_assets_service_returns_503() -> None:
    app = create_app(readiness_probe=lambda: True)
    client = TestClient(app)
    response = client.get("/v1/assets/missing")
    assert response.status_code == 503
    assert response.json()["detail"]["type"] == "database_unavailable"


@pytest.mark.parametrize("object_key", OBJECT_KEY_CORPUS["invalidObjectKeys"])
def test_assets_http_rejects_non_canonical_object_key_before_write(object_key: str) -> None:
    uow = InMemoryUnitOfWork()
    from video_agent_api.application.projects_episodes import ProjectsEpisodesService

    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=ProjectsEpisodesService(lambda: uow),
        assets_service=AssetsService(lambda: uow),
    )
    client = TestClient(app)
    project = client.post("/v1/projects", json={"name": "Demo"}).json()
    asset = client.post(
        f"/v1/projects/{project['id']}/assets", json={"kind": "audio", "name": "Voice"}
    ).json()

    response = client.post(
        f"/v1/assets/{asset['id']}/versions",
        json={
            "contentHash": CONTENT_HASH,
            "storageObject": {
                "storageProvider": "local",
                "bucket": "workspace",
                "objectKey": object_key,
                "mimeType": "audio/wav",
                "sizeBytes": 1,
                "checksum": HASH,
            },
        },
    )

    assert response.status_code == 422
    assert client.get(f"/v1/assets/{asset['id']}/versions").json() == []
