"""Storage verified-object handoffs without crossing aggregate ownership boundaries."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import UTC, datetime
from hashlib import sha256
from typing import Literal

from video_agent_api.application.assets import AssetsService, CompleteReservationCommand
from video_agent_api.domain.assets import AssetVersion, AssetVersionReservation, StorageObject
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.source_material import (
    SourceMaterialUploadIntent,
    VerifiedStoredObjectHandoff,
)
from video_agent_api.ports.contracts import (
    PartReceipt,
    StoragePort,
    StorageWriteIntent,
    StoredObjectRef,
    UploadSessionRef,
)


@dataclass(frozen=True, slots=True)
class StorageRecoveryRecord:
    operation_key: str
    session_id: str
    correlation_id: str
    status: Literal["reconciliation_required", "failed", "aborted", "resolved"]
    diagnostic: str
    object_ref: StoredObjectRef | None = None


@dataclass(frozen=True, slots=True)
class AudioAssetHandoff:
    project_id: str
    reservation_id: str
    object_ref: StoredObjectRef
    authorization_status: Literal["authorized"]
    license: str
    selected: bool = False

    def __post_init__(self) -> None:
        if self.object_ref.project_id != self.project_id or not self.object_ref.verified:
            raise ValidationDomainError("audio stored object is unverified or foreign")
        if not self.object_ref.mime_type.startswith("audio/") or not self.license.strip():
            raise ValidationDomainError("audio authorization or license is invalid")


class AssetUploadCoordinator:
    """Completes storage first, then asks the Assets owner to append exactly one version."""

    def __init__(self, storage: StoragePort, assets: AssetsService) -> None:
        self._storage = storage
        self._assets = assets
        self.recovery_records: list[StorageRecoveryRecord] = []

    async def complete_and_register(
        self,
        reservation_id: str,
        session: UploadSessionRef,
        manifest: tuple[PartReceipt, ...],
        correlation_id: str,
    ) -> AssetVersion:
        try:
            object_ref = self._storage.complete_multipart(session, manifest, correlation_id)
        except Exception as error:
            self.recovery_records.append(
                StorageRecoveryRecord(
                    session.operation_key,
                    session.session_id,
                    correlation_id,
                    "reconciliation_required",
                    type(error).__name__,
                )
            )
            raise
        try:
            version = await self._assets.complete_reservation(
                CompleteReservationCommand(
                    reservation_id,
                    StorageObject(
                        "local_workspace" if object_ref.bucket == "workspace" else "tos",
                        object_ref.bucket,
                        object_ref.object_key,
                        object_ref.mime_type,
                        object_ref.size_bytes,
                        object_ref.checksum,
                        e_tag=object_ref.etag,
                    ),
                    object_ref.checksum,
                )
            )
        except Exception as error:
            self.recovery_records.append(
                StorageRecoveryRecord(
                    session.operation_key,
                    session.session_id,
                    correlation_id,
                    "failed",
                    type(error).__name__,
                    object_ref,
                )
            )
            raise
        self.recovery_records.append(
            StorageRecoveryRecord(
                session.operation_key,
                session.session_id,
                correlation_id,
                "resolved",
                "asset_registered",
                object_ref,
            )
        )
        return version


def asset_upload_intent(
    reservation: AssetVersionReservation,
    profile_id: str,
    object_key: str,
    *,
    expected_size_bytes: int,
    expected_checksum: str,
    expected_mime_type: str,
) -> StorageWriteIntent:
    expected_key = f"asset-upload:{reservation.project_id}:{reservation.asset_id}:{reservation.id}"
    if (
        reservation.status != "reserved"
        or reservation.operation_key != expected_key
        or object_key != reservation.upload_key
        or profile_id != reservation.storage_profile_id
        or expected_size_bytes != reservation.declared_size_bytes
        or expected_checksum != reservation.declared_checksum
        or expected_mime_type != reservation.declared_mime_type
    ):
        raise ValidationDomainError("asset reservation is unavailable or operation key is invalid")
    return StorageWriteIntent(
        reservation.operation_key,
        reservation.project_id,
        profile_id,
        object_key,
        expected_size_bytes,
        expected_checksum,
        expected_mime_type,
    )


def upload_verified_bytes(
    storage: StoragePort,
    *,
    operation_key: str,
    project_id: str,
    profile_id: str,
    object_key: str,
    content: bytes,
    mime_type: str,
    correlation_id: str,
) -> StoredObjectRef:
    checksum = sha256(content).hexdigest()
    intent = StorageWriteIntent(
        operation_key,
        project_id,
        profile_id,
        object_key,
        len(content),
        checksum,
        mime_type,
    )
    session = storage.create_multipart(intent, correlation_id)
    receipt = PartReceipt(1, checksum, checksum, len(content))
    storage.upload_part(session, receipt, content, correlation_id)
    return storage.complete_multipart(session, (receipt,), correlation_id)


def source_material_handoff(
    intent: SourceMaterialUploadIntent,
    session: UploadSessionRef,
    object_ref: StoredObjectRef,
    profile_revision: int,
) -> VerifiedStoredObjectHandoff:
    if (
        session.operation_key != intent.operation_key
        or object_ref.operation_key != intent.operation_key
        or object_ref.project_id != intent.project_id
        or not object_ref.verified
    ):
        raise ValidationDomainError("source material storage handoff is invalid")
    return VerifiedStoredObjectHandoff(
        intent.operation_key,
        intent.project_id,
        intent.source_material_id,
        intent.source_material_revision,
        object_ref.object_key,
        object_ref.size_bytes,
        object_ref.checksum,
        object_ref.mime_type,
        object_ref.etag,
        profile_revision,
        storage_profile_id=object_ref.profile_id,
        bucket_id=object_ref.bucket,
        upload_session_id=session.session_id,
        reservation_id=intent.reservation_id,
        verified_at=datetime.now(UTC).isoformat(),
    )


def audio_asset_handoff(
    project_id: str,
    reservation_id: str,
    object_ref: StoredObjectRef,
    authorization_status: str,
    license_name: str,
) -> AudioAssetHandoff:
    if authorization_status != "authorized":
        raise ValidationDomainError("audio authorization failed")
    return AudioAssetHandoff(
        project_id,
        reservation_id,
        object_ref,
        "authorized",
        license_name,
    )
