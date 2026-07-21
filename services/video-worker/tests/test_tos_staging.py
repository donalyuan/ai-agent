import hashlib
from io import BytesIO
from types import SimpleNamespace
from uuid import UUID

import pytest

from video_worker.tos_staging import (
    TosAudioStaging,
    TosConnectionChecker,
    TosMediaStaging,
    TosStagingConfig,
    TosStagingError,
)


class FakeTosClient:
    def __init__(self) -> None:
        self.uploads = []
        self.presigns = []
        self.deletes = []
        self.head_buckets = []

    def head_bucket(self, bucket):
        self.head_buckets.append(bucket)

    def put_object(self, bucket, key, **kwargs):
        content = kwargs["content"]
        assert isinstance(content, BytesIO)
        self.uploads.append((bucket, key, content.read(), kwargs))

    def pre_signed_url(self, method, bucket, key, **kwargs):
        self.presigns.append((method, bucket, key, kwargs))
        return SimpleNamespace(
            signed_url=f"https://tos.example.invalid/{bucket}/{key}?X-Tos-Signature=secret"
        )

    def delete_object(self, bucket, key):
        self.deletes.append((bucket, key))


def config() -> TosStagingConfig:
    return TosStagingConfig(
        endpoint="https://tos-cn-beijing.volces.com",
        region="cn-beijing",
        bucket="private-staging",
        object_prefix="novex/asr",
        signed_url_ttl_seconds=600,
        max_file_bytes=1024,
        access_key="fixture-access-key",
        secret_key="fixture-secret-key",
    )


def test_staging_uses_deterministic_private_object_and_redacts_signed_url_from_audit() -> None:
    client = FakeTosClient()
    staging = TosAudioStaging(config(), client)
    content = b"ID3 fixture audio"
    project_id = UUID("11111111-1111-4111-8111-111111111111")
    task_id = UUID("22222222-2222-4222-8222-222222222222")

    result = staging.stage(
        project_id=project_id,
        task_id=task_id,
        content=content,
        extension="mp3",
        content_type="audio/mpeg",
    )

    digest = hashlib.sha256(content).hexdigest()
    expected_key = f"novex/asr/{project_id}/{task_id}/{digest}.mp3"
    assert result.object_key == expected_key
    assert client.uploads[0][0:3] == ("private-staging", expected_key, content)
    assert client.uploads[0][3]["forbid_overwrite"] is True
    assert client.presigns[0][3]["expires"] == 600
    assert "X-Tos-Signature" in result.signed_get_url
    assert result.audit_snapshot() == {
        "object_key": expected_key,
        "source_sha256": digest,
        "source_size_bytes": len(content),
    }
    assert "signed" not in str(result.audit_snapshot()).lower()

    staging.cleanup(expected_key)
    assert client.deletes == [("private-staging", expected_key)]

    renewed_url = staging.signed_get_url(expected_key)
    assert "X-Tos-Signature" in renewed_url
    assert len(client.uploads) == 1
    assert len(client.presigns) == 2


def test_connection_check_uses_official_head_bucket_operation() -> None:
    client = FakeTosClient()

    TosConnectionChecker(
        config(),
        client,
        signed_url_reader=lambda _url: b"novex-tos-capability-check",
    ).check()

    assert client.head_buckets == ["private-staging"]
    assert client.uploads[0][1] == "novex/asr/.novex-connection-check/probe.txt"
    assert client.uploads[0][2] == b"novex-tos-capability-check"
    assert client.presigns[0][3]["expires"] == 600
    assert client.deletes == [
        ("private-staging", "novex/asr/.novex-connection-check/probe.txt")
    ]


def test_connection_check_cleans_probe_when_signed_read_fails() -> None:
    client = FakeTosClient()
    checker = TosConnectionChecker(
        config(),
        client,
        signed_url_reader=lambda _url: (_ for _ in ()).throw(RuntimeError("failed")),
    )

    with pytest.raises(TosStagingError) as captured:
        checker.check()

    assert captured.value.code == "tos_connection_check_failed"
    assert client.deletes == [
        ("private-staging", "novex/asr/.novex-connection-check/probe.txt")
    ]


def test_staging_rejects_oversized_audio_before_any_external_call() -> None:
    client = FakeTosClient()
    staging = TosAudioStaging(config(), client)

    with pytest.raises(TosStagingError) as error:
        staging.stage(
            project_id=UUID("11111111-1111-4111-8111-111111111111"),
            task_id=UUID("22222222-2222-4222-8222-222222222222"),
            content=b"x" * 1025,
            extension="mp3",
            content_type="audio/mpeg",
        )

    assert error.value.code == "audio_too_large"
    assert client.uploads == []


def test_cleanup_rejects_object_outside_configured_prefix() -> None:
    staging = TosAudioStaging(config(), FakeTosClient())

    with pytest.raises(TosStagingError) as error:
        staging.cleanup("another-prefix/file.mp3")

    assert error.value.code == "tos_object_key_invalid"


def test_presign_failure_keeps_deterministic_identity_for_cleanup() -> None:
    class PresignFailureClient(FakeTosClient):
        def pre_signed_url(self, method, bucket, key, **kwargs):
            raise RuntimeError("fixture presign failure")

    staging = TosAudioStaging(config(), PresignFailureClient())
    with pytest.raises(TosStagingError) as captured:
        staging.stage(
            project_id=UUID("11111111-1111-4111-8111-111111111111"),
            task_id=UUID("22222222-2222-4222-8222-222222222222"),
            content=b"ID3 fixture audio",
            extension="mp3",
            content_type="audio/mpeg",
        )

    assert captured.value.code == "tos_presign_failed"
    assert captured.value.object_key.startswith("novex/asr/")
    assert len(captured.value.source_sha256) == 64


def test_image_staging_uses_deterministic_key_and_never_audits_signed_url() -> None:
    client = FakeTosClient()
    staging = TosMediaStaging(config(), client)
    content = b"\x89PNG\r\n\x1a\nfixture"

    result = staging.stage_media(
        project_id=UUID("11111111-1111-4111-8111-111111111111"),
        task_id=UUID("22222222-2222-4222-8222-222222222222"),
        content=content,
        extension="png",
        content_type="image/png",
    )

    assert result.object_key.endswith(f"/{hashlib.sha256(content).hexdigest()}.png")
    assert client.uploads[0][3]["content_type"] == "image/png"
    assert "signed_get_url" not in result.audit_snapshot()
    assert "Signature" not in str(result.audit_snapshot())


def test_image_staging_rejects_non_image_before_upload() -> None:
    client = FakeTosClient()
    staging = TosMediaStaging(config(), client)

    with pytest.raises(TosStagingError) as captured:
        staging.stage_media(
            project_id=UUID("11111111-1111-4111-8111-111111111111"),
            task_id=UUID("22222222-2222-4222-8222-222222222222"),
            content=b"not an image",
            extension="txt",
            content_type="text/plain",
        )

    assert captured.value.code == "media_type_unsupported"
    assert client.uploads == []


def test_image_staging_reuses_existing_deterministic_object() -> None:
    class ExistingObjectClient(FakeTosClient):
        def put_object(self, bucket, key, **kwargs):
            error = RuntimeError("already exists")
            error.status_code = 409
            raise error

    client = ExistingObjectClient()
    result = TosMediaStaging(config(), client).stage_media(
        project_id=UUID("11111111-1111-4111-8111-111111111111"),
        task_id=UUID("22222222-2222-4222-8222-222222222222"),
        content=b"\x89PNG\r\n\x1a\nfixture",
        extension="png",
        content_type="image/png",
    )

    assert result.signed_get_url.startswith("https://")
    assert len(client.presigns) == 1
