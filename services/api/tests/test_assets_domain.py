from __future__ import annotations

import json
from dataclasses import FrozenInstanceError
from pathlib import Path

import pytest

from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.errors import ImmutableAssetVersionError, ValidationDomainError

HASH = "a" * 64
OBJECT_KEY_CORPUS = json.loads(
    (
        Path(__file__).parents[3]
        / "packages/contracts/tests/fixtures/object-key-contract-corpus.json"
    ).read_text(encoding="utf-8")
)


def storage(**overrides: object) -> StorageObject:
    values: dict[str, object] = {
        "storage_provider": "local",
        "bucket": "workspace",
        "object_key": "projects/p/assets/a/v1.mp4",
        "mime_type": "video/mp4",
        "size_bytes": 10,
        "checksum": HASH,
        "media": {"duration_ms": 1000, "width": 1920, "height": 1080},
    }
    values.update(overrides)
    return StorageObject(**values)


def test_asset_and_version_domain_rules() -> None:
    asset = Asset(project_id="p", kind="audio", name="Voice")
    assert asset.revision == 1
    version = AssetVersion(
        asset_id=asset.id, project_id="p", version_number=1, storage_object=storage()
    )
    assert version.content_hash == HASH
    with pytest.raises(ImmutableAssetVersionError):
        version.update_storage(storage())


def test_content_hash_is_independent_from_storage_checksum() -> None:
    asset = Asset(project_id="p", kind="video", name="Video")
    content_hash = "b" * 64
    version = AssetVersion(
        asset_id=asset.id,
        project_id="p",
        version_number=1,
        storage_object=storage(checksum=HASH),
        content_hash=content_hash,
    )
    assert version.content_hash == content_hash
    assert version.storage_object.checksum == HASH
    assert version.content_hash != version.storage_object.checksum


def test_asset_version_and_nested_storage_are_immutable() -> None:
    version = AssetVersion(
        asset_id="asset",
        project_id="project",
        version_number=1,
        storage_object=storage(),
        content_hash="b" * 64,
    )

    with pytest.raises(FrozenInstanceError):
        version.content_hash = "c" * 64
    with pytest.raises(FrozenInstanceError):
        version.storage_object.checksum = "d" * 64
    with pytest.raises(TypeError):
        assert version.storage_object.media is not None
        version.storage_object.media["width"] = 1  # type: ignore[index]


@pytest.mark.parametrize("key", OBJECT_KEY_CORPUS["canonicalObjectKeys"])
def test_storage_object_accepts_shared_canonical_keys(key: str) -> None:
    assert storage(object_key=key).object_key == key


@pytest.mark.parametrize("key", OBJECT_KEY_CORPUS["invalidObjectKeys"])
def test_storage_object_rejects_non_canonical_keys(key: str) -> None:
    with pytest.raises(ValidationDomainError):
        storage(object_key=key)


@pytest.mark.parametrize(
    "kwargs",
    [{"kind": "sound"}, {"size_bytes": -1}, {"checksum": "x"}, {"mime_type": "video"}],
)
def test_asset_and_storage_validation(kwargs: dict[str, object]) -> None:
    if "kind" in kwargs:
        with pytest.raises(ValidationDomainError):
            Asset(project_id="p", kind=str(kwargs["kind"]), name="x")
    else:
        with pytest.raises(ValidationDomainError):
            storage(**kwargs)
