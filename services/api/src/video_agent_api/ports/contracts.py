"""所有外部副作用使用的六个 Protocol、DTO 和可诊断错误。"""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass, field
from typing import Protocol


class PortError(RuntimeError):
    pass


class AdapterNotConfiguredError(PortError):
    pass


class DisabledConfigurationError(PortError):
    pass


class StorageValidationError(PortError):
    code = "storage_validation_failed"


class StorageConflictError(PortError):
    code = "storage_conflict"


class StorageRetryableError(PortError):
    code = "storage_retryable"


class StorageAuthorizationError(PortError):
    code = "storage_authorization_failed"


class StorageObjectInUseError(PortError):
    code = "object_in_use"


class StorageMediaValidationError(StorageValidationError):
    code = "media_validation_failed"


class StorageProfileUnavailableError(PortError):
    code = "credential_master_key_unavailable"


@dataclass(frozen=True, slots=True)
class ModelSelection:
    provider_id: str
    profile_id: str
    model_id: str
    adapter_key: str
    default_parameters: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class PortResult:
    request_id: str
    correlation_id: str
    payload: dict[str, object]


class RemoteLookupPort(Protocol):
    """Lookup is permitted only through a preselected provider port."""

    def lookup_provider_request(self, correlation_id: str, protocol: str) -> object: ...


@dataclass(frozen=True, slots=True)
class FrozenRemoteLookup:
    """Bind a lookup port to the exact capability that admitted the call."""

    capability_snapshot_id: str
    operation: str
    protocol: str
    port: RemoteLookupPort
    profile_id: str
    model_id: str
    profile_revision: int
    capability_revision: int

    def __post_init__(self) -> None:
        if not self.capability_snapshot_id or not self.operation or not self.protocol:
            raise ValueError("frozen_remote_lookup_identity_incomplete")
        for identity_value in (self.profile_id, self.model_id):
            if not identity_value:
                raise ValueError("frozen_remote_lookup_identity_incomplete")
        for revision_value in (self.profile_revision, self.capability_revision):
            if isinstance(revision_value, bool) or revision_value < 1:
                raise ValueError("frozen_remote_lookup_revision_invalid")


@dataclass(frozen=True, slots=True)
class StoredObject:
    object_ref: str
    size_bytes: int
    checksum: str
    etag: str | None = None
    project_id: str | None = None
    profile_id: str | None = None
    operation_key: str | None = None


@dataclass(frozen=True, slots=True)
class MultipartSession:
    session_id: str
    object_ref: str
    operation_key: str | None = None
    project_id: str | None = None
    status: str = "active"


@dataclass(frozen=True, slots=True)
class MultipartPart:
    part_number: int
    etag: str
    checksum: str | None = None


@dataclass(frozen=True, slots=True)
class StorageWriteIntent:
    operation_key: str
    project_id: str
    profile_id: str
    object_key: str
    expected_size_bytes: int | None = None
    expected_checksum: str | None = None
    expected_mime_type: str | None = None


@dataclass(frozen=True, slots=True)
class StorageCapability:
    profile_revision: int
    min_part_size_bytes: int
    max_part_size_bytes: int
    max_part_count: int
    max_object_size_bytes: int

    def supports(self, size_bytes: int, part_size_bytes: int) -> bool:
        if size_bytes < 0 or not (
            self.min_part_size_bytes <= part_size_bytes <= self.max_part_size_bytes
        ):
            return False
        count = (size_bytes + part_size_bytes - 1) // part_size_bytes
        return count <= self.max_part_count and size_bytes <= self.max_object_size_bytes


@dataclass(frozen=True, slots=True)
class UploadSessionRef:
    session_id: str
    operation_key: str
    project_id: str
    profile_id: str
    object_key: str
    status: str = "active"
    expected_size_bytes: int | None = None
    expected_checksum: str | None = None
    expected_mime_type: str | None = None

    def __post_init__(self) -> None:
        if self.status not in {"active", "completed", "aborted", "unknown", "failed"}:
            raise StorageValidationError("upload session status is invalid")
        if not self.session_id or not self.operation_key or not self.project_id:
            raise StorageValidationError("upload session identity is incomplete")
        if not self.profile_id or not self.object_key:
            raise StorageValidationError("upload session storage binding is incomplete")
        if self.expected_size_bytes is not None and self.expected_size_bytes < 0:
            raise StorageMediaValidationError("upload session expected size is invalid")
        if self.expected_checksum is not None and len(self.expected_checksum) != 64:
            raise StorageMediaValidationError("upload session expected checksum is invalid")


@dataclass(frozen=True, slots=True)
class PartReceipt:
    part_number: int
    checksum: str
    etag: str
    size_bytes: int


@dataclass(frozen=True, slots=True)
class StoredObjectRef:
    project_id: str
    profile_id: str
    bucket: str
    object_key: str
    size_bytes: int
    checksum: str
    mime_type: str
    etag: str | None
    operation_key: str
    verified: bool = True

    def __post_init__(self) -> None:
        if not self.project_id or not self.profile_id or not self.bucket:
            raise StorageValidationError("stored object scope is required")
        if not self.object_key or self.object_key.startswith(("/", "\\")):
            raise StorageValidationError("stored object key is unsafe")
        if self.size_bytes < 0 or len(self.checksum) != 64:
            raise StorageMediaValidationError("stored object metadata is invalid")
        if "/" not in self.mime_type or not self.operation_key:
            raise StorageValidationError("stored object metadata is incomplete")
        if not self.verified:
            raise StorageValidationError("stored object reference must be verified")


@dataclass(frozen=True, slots=True)
class PresignedAccess:
    project_id: str
    profile_id: str
    object_key: str
    action: str
    expires_at: int
    url: str

    def __post_init__(self) -> None:
        if self.action not in {"read", "write"}:
            raise StorageAuthorizationError("presign action is invalid")
        if self.expires_at <= 0 or not self.project_id or not self.object_key:
            raise StorageValidationError("presign scope is invalid")


@dataclass(frozen=True, slots=True)
class OpaqueReadGrant:
    artifact_id: str
    token: str
    expires_at: int
    action: str = "read"

    def __post_init__(self) -> None:
        if not self.artifact_id or len(self.token) < 32 or self.expires_at <= 0:
            raise StorageValidationError("opaque read grant is invalid")
        if self.action != "read":
            raise StorageAuthorizationError("export grant must be read-only")


@dataclass(frozen=True, slots=True)
class DeleteProof:
    project_id: str
    object_key: str
    checked_at: str
    no_references: bool
    proof_id: str

    def __post_init__(self) -> None:
        if not self.project_id or not self.object_key or not self.proof_id:
            raise StorageValidationError("delete proof is incomplete")


@dataclass(frozen=True, slots=True)
class PresignedReference:
    url: str
    expires_in_seconds: int


class TextModelPort(Protocol):
    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult: ...


class ImageGenerationPort(Protocol):
    def generate_image(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult: ...

    def edit_image(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult: ...


class VideoGenerationPort(Protocol):
    def submit_video(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult: ...

    def get_video_status(self, job_id: str, correlation_id: str) -> PortResult: ...

    def cancel_video(self, job_id: str, correlation_id: str) -> PortResult: ...


class MediaInspectPort(Protocol):
    """Media Worker owns observed metadata; Provider adapters must not implement this."""

    def inspect(self, stored_object: StoredObject, correlation_id: str) -> dict[str, object]: ...


class MediaDerivativePort(Protocol):
    """Media Worker owns independently replaceable derivative records."""

    def derive(
        self,
        stored_object: StoredObject,
        metadata: dict[str, object],
        correlation_id: str,
    ) -> tuple[dict[str, object], ...]: ...


class TtsPort(Protocol):
    def synthesize(
        self, text: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult: ...


class AsrPort(Protocol):
    def transcribe(
        self, object_ref: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult: ...


class StoragePort(Protocol):
    def put(self, object_key: str, content: bytes, correlation_id: str) -> StoredObject: ...

    def get(self, object_ref: str) -> bytes: ...

    def iter_chunks(self, object_ref: str, chunk_size: int = 1024 * 1024) -> Iterator[bytes]: ...

    def delete(self, object_ref: str) -> None: ...

    def stat(self, object_ref: str) -> StoredObject: ...

    def create_multipart_session(
        self, object_key: str, correlation_id: str
    ) -> MultipartSession: ...

    def complete_multipart_session(
        self, session_id: str, parts: list[MultipartPart], correlation_id: str
    ) -> StoredObject: ...

    def presign_read(self, object_ref: str, expires_in_seconds: int) -> PresignedReference: ...

    def presign_write(self, object_key: str, expires_in_seconds: int) -> PresignedReference: ...

    def download_to_workspace(
        self, object_ref: str, workspace_key: str, correlation_id: str
    ) -> StoredObject: ...

    def upload_from_workspace(
        self, workspace_key: str, object_key: str, correlation_id: str
    ) -> StoredObject: ...

    def create_multipart(
        self, intent: StorageWriteIntent, correlation_id: str
    ) -> UploadSessionRef: ...

    def resume_multipart(
        self, intent: StorageWriteIntent, correlation_id: str
    ) -> UploadSessionRef: ...

    def reconcile_multipart(
        self, intent: StorageWriteIntent, correlation_id: str
    ) -> StoredObjectRef | None: ...

    def upload_part(
        self,
        session: UploadSessionRef,
        receipt: PartReceipt,
        content: bytes,
        correlation_id: str,
    ) -> PartReceipt: ...

    def complete_multipart(
        self,
        session: UploadSessionRef,
        manifest: tuple[PartReceipt, ...],
        correlation_id: str,
    ) -> StoredObjectRef: ...

    def abort_multipart(
        self, session: UploadSessionRef, correlation_id: str
    ) -> UploadSessionRef: ...

    def delete_with_proof(
        self, object_ref: StoredObjectRef, proof: DeleteProof, correlation_id: str
    ) -> None: ...


class StorageReferenceProofPort(Protocol):
    """跨 owner 的 fail-closed 引用证明；storage 不猜测消费者引用。"""

    def prove_no_references(
        self, object_ref: StoredObjectRef, project_id: str, correlation_id: str
    ) -> DeleteProof: ...


class ExportDownloadGrantPort(Protocol):
    """Issue application-level opaque grants; object keys never enter public responses."""

    def issue_read_grant(
        self,
        artifact_id: str,
        object_ref: StoredObjectRef,
        actor_project_id: str,
        ttl_seconds: int,
    ) -> OpaqueReadGrant: ...


class CredentialResolver(Protocol):
    """由 catalog/security owner 提供的受限凭据解析边界。"""

    def resolve(self, credential_ref: str, profile_id: str) -> str: ...
