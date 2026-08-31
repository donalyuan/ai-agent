"""本地开发存储与真实 TOS 的显式未配置占位 Adapter。"""

from __future__ import annotations

import json
from collections.abc import Callable, Iterator
from dataclasses import asdict, dataclass
from hashlib import sha256
from pathlib import Path
from time import time
from typing import Any, NoReturn, TypeVar, cast, overload
from uuid import uuid4

from video_agent_api.domain.object_key_contract import canonical_object_key
from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    CredentialResolver,
    DeleteProof,
    MultipartPart,
    MultipartSession,
    OpaqueReadGrant,
    PartReceipt,
    PresignedAccess,
    PresignedReference,
    StorageAuthorizationError,
    StorageCapability,
    StorageMediaValidationError,
    StorageObjectInUseError,
    StoragePort,
    StorageValidationError,
    StorageWriteIntent,
    StoredObject,
    StoredObjectRef,
    UploadSessionRef,
)

_ResultT = TypeVar("_ResultT")


class CompositeStorageReferenceProof:
    """只有所有 owner 查询都成功且均无引用时才签发删除证明。"""

    REQUIRED_OWNERS = ("asset_version", "run", "timeline", "package_manifest")

    def __init__(self, checks: dict[str, Callable[[StoredObjectRef], bool]]) -> None:
        self._checks = checks

    def prove_no_references(
        self, object_ref: StoredObjectRef, project_id: str, correlation_id: str
    ) -> DeleteProof:
        del correlation_id
        if object_ref.project_id != project_id or set(self._checks) != set(self.REQUIRED_OWNERS):
            raise StorageObjectInUseError("reference proof is incomplete")
        try:
            referenced = any(self._checks[owner](object_ref) for owner in self.REQUIRED_OWNERS)
        except Exception as error:
            raise StorageObjectInUseError("reference index is unavailable") from error
        if referenced:
            raise StorageObjectInUseError("object is referenced")
        return DeleteProof(project_id, object_ref.object_key, str(time()), True, uuid4().hex)


class LocalOpaqueReadGrantIssuer:
    """Keep the object reference server-side and expose only a short-lived opaque token."""

    def __init__(self, max_ttl_seconds: int = 300) -> None:
        self._max_ttl_seconds = max_ttl_seconds
        self._grants: dict[str, tuple[StoredObjectRef, int]] = {}

    def issue_read_grant(
        self,
        artifact_id: str,
        object_ref: StoredObjectRef,
        actor_project_id: str,
        ttl_seconds: int,
    ) -> OpaqueReadGrant:
        if (
            object_ref.project_id != actor_project_id
            or ttl_seconds < 1
            or ttl_seconds > self._max_ttl_seconds
        ):
            raise StorageAuthorizationError("export grant scope or TTL is invalid")
        token = uuid4().hex + uuid4().hex
        expires_at = int(time()) + ttl_seconds
        self._grants[token] = (object_ref, expires_at)
        return OpaqueReadGrant(artifact_id, token, expires_at)

    def resolve(self, token: str, now: int | None = None) -> StoredObjectRef:
        grant = self._grants.get(token)
        if grant is None or grant[1] <= (int(time()) if now is None else now):
            raise StorageAuthorizationError("export grant is expired or unknown")
        return grant[0]


@dataclass(slots=True)
class _LocalUpload:
    object_key: str
    parts: dict[int, PartReceipt]
    operation_key: str = ""
    project_id: str = ""
    profile_id: str = "local"
    expected_size_bytes: int | None = None
    expected_checksum: str | None = None
    expected_mime_type: str | None = None
    manifest: tuple[PartReceipt, ...] | None = None
    result: StoredObjectRef | None = None
    status: str = "active"
    created_at: float = 0.0
    failed: bool = False


class LocalWorkspaceAdapter:
    """只允许受控根目录下的相对对象键，向业务返回抽象引用。"""

    def __init__(self, root: Path) -> None:
        self._root = root.resolve()
        self._uploads: dict[str, _LocalUpload] = {}
        self._operations: dict[str, str] = {}
        self._objects: dict[str, StoredObjectRef] = {}
        self._temporary: dict[str, tuple[Path, str, float]] = {}
        self._session_root = self._root / ".multipart"
        self._session_root.mkdir(parents=True, exist_ok=True)
        self._load_sessions()

    @property
    def adapter_key(self) -> str:
        return "local_workspace"

    @property
    def profile_id(self) -> str:
        return "local-test-offline"

    @property
    def profile_revision(self) -> int:
        return 1

    @property
    def endpoint(self) -> str:
        return "workspace://local"

    @property
    def region(self) -> str:
        return "local"

    @property
    def bucket_binding_id(self) -> str:
        return "local-workspace"

    @property
    def bucket(self) -> str:
        return "workspace"

    @property
    def root(self) -> Path:
        return self._root

    def _logged(
        self, operation: str, correlation_id: str | None, action: Callable[[], _ResultT]
    ) -> _ResultT:
        try:
            result = action()
        except Exception as error:
            log_event(
                "storage.call",
                correlation_id=correlation_id,
                operation=operation,
                adapter="local_workspace",
                result="error",
                error_type=type(error).__name__,
            )
            raise
        log_event(
            "storage.call",
            correlation_id=correlation_id,
            operation=operation,
            adapter="local_workspace",
            result="success",
        )
        return result

    def _key_from_ref(self, value: str) -> str:
        key = value.removeprefix("workspace://")
        canonical = canonical_object_key(key)
        if canonical is None:
            raise ValueError("object key escapes workspace root")
        return canonical

    @staticmethod
    def _validate_intent(intent: StorageWriteIntent) -> None:
        if not intent.operation_key or not intent.project_id or not intent.profile_id:
            raise StorageValidationError("storage intent scope is required")
        canonical = canonical_object_key(intent.object_key)
        if (
            canonical is None
            or canonical != intent.object_key
            or not intent.object_key.startswith(f"projects/{intent.project_id}/")
        ):
            raise StorageValidationError("object key is foreign or unsafe")
        if intent.expected_size_bytes is not None and intent.expected_size_bytes < 0:
            raise StorageValidationError("expected size is invalid")
        if intent.object_key.startswith("workspace://") or "\\" in intent.object_key:
            raise StorageValidationError("object key is not canonical")

    def _path(self, value: str) -> tuple[str, Path]:
        key = self._key_from_ref(value)
        path = (self._root / key).resolve()
        if path != self._root and self._root not in path.parents:
            raise ValueError("object key escapes workspace root")
        return key, path

    def _session_dir(self, session_id: str) -> Path:
        if not session_id or not session_id.isalnum():
            raise StorageValidationError("multipart session id is invalid")
        return self._session_root / session_id

    def _part_path(self, session_id: str, part_number: int) -> Path:
        return self._session_dir(session_id) / f"part-{part_number:05d}"

    def _persist_session(self, session_id: str) -> None:
        state = self._uploads[session_id]
        directory = self._session_dir(session_id)
        directory.mkdir(parents=True, exist_ok=True)
        payload: dict[str, object] = {
            "object_key": state.object_key,
            "operation_key": state.operation_key,
            "project_id": state.project_id,
            "profile_id": state.profile_id,
            "expected_size_bytes": state.expected_size_bytes,
            "expected_checksum": state.expected_checksum,
            "expected_mime_type": state.expected_mime_type,
            "status": state.status,
            "created_at": state.created_at,
            "failed": state.failed,
            "parts": [asdict(item) for _, item in sorted(state.parts.items())],
            "manifest": [asdict(item) for item in state.manifest or ()],
            "result": asdict(state.result) if state.result is not None else None,
        }
        temporary = directory / "session.json.tmp"
        temporary.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
        temporary.replace(directory / "session.json")

    def _load_sessions(self) -> None:
        for metadata in self._session_root.glob("*/session.json"):
            try:
                raw = json.loads(metadata.read_text(encoding="utf-8"))
                session_id = metadata.parent.name
                parts = {int(item["part_number"]): PartReceipt(**item) for item in raw["parts"]}
                manifest = tuple(PartReceipt(**item) for item in raw.get("manifest", []))
                result_raw = raw.get("result")
                result = StoredObjectRef(**result_raw) if isinstance(result_raw, dict) else None
                state = _LocalUpload(
                    str(raw["object_key"]),
                    parts,
                    str(raw["operation_key"]),
                    str(raw["project_id"]),
                    str(raw["profile_id"]),
                    int(raw["expected_size_bytes"])
                    if raw.get("expected_size_bytes") is not None
                    else None,
                    str(raw["expected_checksum"])
                    if raw.get("expected_checksum") is not None
                    else None,
                    str(raw["expected_mime_type"])
                    if raw.get("expected_mime_type") is not None
                    else None,
                    manifest or None,
                    result,
                    str(raw["status"]),
                    float(raw["created_at"]),
                    bool(raw.get("failed", False)),
                )
            except (KeyError, TypeError, ValueError, json.JSONDecodeError):
                continue
            self._uploads[session_id] = state
            if state.operation_key:
                self._operations[state.operation_key] = session_id
            if result is not None:
                self._objects[result.object_key] = result

    def put(self, object_key: str, content: bytes, correlation_id: str) -> StoredObject:
        return self._logged("put", correlation_id, lambda: self._put_unlogged(object_key, content))

    def _put_unlogged(self, object_key: str, content: bytes) -> StoredObject:
        key, path = self._path(object_key)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        checksum = sha256(content).hexdigest()
        return StoredObject(f"workspace://{key}", len(content), checksum, checksum)

    def get(self, object_ref: str) -> bytes:
        return self._logged("get", None, lambda: self._get_unlogged(object_ref))

    def _get_unlogged(self, object_ref: str) -> bytes:
        return self._path(object_ref)[1].read_bytes()

    def iter_chunks(self, object_ref: str, chunk_size: int = 1024 * 1024) -> Iterator[bytes]:
        if chunk_size < 1:
            raise StorageValidationError("stream chunk size is invalid")
        path = self._path(object_ref)[1]

        def stream() -> Iterator[bytes]:
            try:
                with path.open("rb") as source:
                    while chunk := source.read(chunk_size):
                        yield chunk
            except Exception as error:
                log_event(
                    "storage.call",
                    operation="iter_chunks",
                    adapter="local_workspace",
                    result="error",
                    error_type=type(error).__name__,
                )
                raise
            log_event(
                "storage.call",
                operation="iter_chunks",
                adapter="local_workspace",
                result="success",
            )

        return stream()

    def delete(self, object_ref: str) -> None:
        self._logged("delete", None, lambda: self._path(object_ref)[1].unlink())

    def stat(self, object_ref: str) -> StoredObject:
        return self._logged("stat", None, lambda: self._stat_unlogged(object_ref))

    def _stat_unlogged(self, object_ref: str) -> StoredObject:
        key, path = self._path(object_ref)
        digest = sha256()
        size_bytes = 0
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
                size_bytes += len(chunk)
        checksum = digest.hexdigest()
        return StoredObject(f"workspace://{key}", size_bytes, checksum, checksum)

    def create_multipart_session(self, object_key: str, correlation_id: str) -> MultipartSession:
        def create() -> MultipartSession:
            key = self._key_from_ref(object_key)
            session_id = uuid4().hex
            self._uploads[session_id] = _LocalUpload(key, {})
            self._persist_session(session_id)
            return MultipartSession(session_id, f"workspace://{key}")

        return self._logged("create_multipart_session", correlation_id, create)

    @overload
    def upload_part(
        self,
        session_id: UploadSessionRef,
        part_number: PartReceipt,
        content: bytes,
        correlation_id: str,
    ) -> PartReceipt: ...

    @overload
    def upload_part(
        self, session_id: str, part_number: int, content: bytes, correlation_id: str
    ) -> MultipartPart: ...

    def upload_part(
        self,
        session_id: UploadSessionRef | str,
        part_number: PartReceipt | int,
        content: bytes,
        correlation_id: str,
    ) -> PartReceipt | MultipartPart:
        def upload() -> PartReceipt | MultipartPart:
            if isinstance(session_id, str):
                if (
                    not isinstance(part_number, int)
                    or part_number < 1
                    or session_id not in self._uploads
                ):
                    raise ValueError("unknown multipart session or invalid part number")
                digest = sha256(content).hexdigest()
                self._uploads[session_id].parts[part_number] = PartReceipt(
                    part_number, digest, digest, len(content)
                )
                self._part_path(session_id, part_number).write_bytes(content)
                self._persist_session(session_id)
                return MultipartPart(part_number, sha256(content).hexdigest())
            if not isinstance(part_number, PartReceipt):
                raise StorageValidationError("part receipt is required")
            upload_state = self._uploads.get(session_id.session_id)
            if upload_state is None or upload_state.status != "active":
                raise StorageValidationError("unknown multipart session")
            if part_number.part_number < 1:
                raise StorageValidationError("part number is invalid")
            digest = sha256(content).hexdigest()
            if digest != part_number.checksum:
                raise StorageValidationError("multipart part checksum mismatch")
            uploaded_receipt = PartReceipt(
                part_number.part_number,
                digest,
                part_number.etag,
                len(content),
            )
            existing = upload_state.parts.get(part_number.part_number)
            if existing is not None and existing != uploaded_receipt:
                raise StorageValidationError("multipart_part_conflict")
            if existing is None:
                upload_state.parts[part_number.part_number] = uploaded_receipt
                self._part_path(session_id.session_id, part_number.part_number).write_bytes(content)
                self._persist_session(session_id.session_id)
            return uploaded_receipt

        return self._logged("upload_part", correlation_id, upload)

    def complete_multipart_session(
        self, session_id: str, parts: list[MultipartPart], correlation_id: str
    ) -> StoredObject:
        def complete() -> StoredObject:
            upload = self._uploads.get(session_id)
            if upload is None:
                raise ValueError("unknown multipart session")
            expected = sorted(part.part_number for part in parts)
            if expected != sorted(upload.parts):
                raise ValueError("multipart parts do not match uploaded content")
            content = b"".join(
                self._part_path(session_id, number).read_bytes() for number in expected
            )
            result = self._put_unlogged(upload.object_key, content)
            upload.status = "completed"
            self._persist_session(session_id)
            return result

        return self._logged("complete_multipart_session", correlation_id, complete)

    def create_multipart(self, intent: StorageWriteIntent, correlation_id: str) -> UploadSessionRef:
        self._validate_intent(intent)
        existing_id = self._operations.get(intent.operation_key)
        if existing_id is not None:
            state = self._uploads[existing_id]
            if (
                state.project_id != intent.project_id
                or state.profile_id != intent.profile_id
                or state.object_key != intent.object_key
                or state.expected_size_bytes != intent.expected_size_bytes
                or state.expected_checksum != intent.expected_checksum
                or state.expected_mime_type != intent.expected_mime_type
            ):
                raise StorageValidationError("multipart operation binding conflict")
            return UploadSessionRef(
                existing_id,
                intent.operation_key,
                intent.project_id,
                intent.profile_id,
                state.object_key,
                state.status,
                intent.expected_size_bytes,
                intent.expected_checksum,
                intent.expected_mime_type,
            )
        session_id = uuid4().hex
        self._uploads[session_id] = _LocalUpload(
            intent.object_key,
            {},
            intent.operation_key,
            intent.project_id,
            intent.profile_id,
            intent.expected_size_bytes,
            intent.expected_checksum,
            intent.expected_mime_type,
            created_at=time(),
        )
        self._operations[intent.operation_key] = session_id
        self._persist_session(session_id)
        return UploadSessionRef(
            session_id,
            intent.operation_key,
            intent.project_id,
            intent.profile_id,
            intent.object_key,
            "active",
            intent.expected_size_bytes,
            intent.expected_checksum,
            intent.expected_mime_type,
        )

    def resume_multipart(self, intent: StorageWriteIntent, correlation_id: str) -> UploadSessionRef:
        return self.create_multipart(intent, correlation_id)

    def reconcile_multipart(
        self, intent: StorageWriteIntent, correlation_id: str
    ) -> StoredObjectRef | None:
        self._validate_intent(intent)
        session_id = self._operations.get(intent.operation_key)
        if session_id is None:
            return None
        state = self._uploads[session_id]
        result = state.result
        if result is None:
            return None
        if (
            result.project_id != intent.project_id
            or result.profile_id != intent.profile_id
            or result.object_key != intent.object_key
            or result.size_bytes != intent.expected_size_bytes
            or result.checksum != intent.expected_checksum
            or result.mime_type != intent.expected_mime_type
        ):
            raise StorageValidationError("multipart_reconcile_conflict")
        log_event(
            "storage.call",
            correlation_id=correlation_id,
            operation="reconcile_multipart",
            adapter="local_workspace",
            result="success",
        )
        return result

    def complete_multipart(
        self, session: UploadSessionRef, manifest: tuple[PartReceipt, ...], correlation_id: str
    ) -> StoredObjectRef:
        state = self._uploads.get(session.session_id)
        if (
            state is None
            or state.operation_key != session.operation_key
            or state.project_id != session.project_id
            or state.profile_id != session.profile_id
            or state.object_key != session.object_key
            or state.expected_size_bytes != session.expected_size_bytes
            or state.expected_checksum != session.expected_checksum
            or state.expected_mime_type != session.expected_mime_type
        ):
            raise StorageValidationError("multipart session binding is invalid")
        if state.result is not None:
            if state.manifest != manifest:
                raise StorageValidationError("multipart_complete_conflict")
            return state.result
        if state.status != "active":
            raise StorageValidationError("multipart session is terminal")
        ordered = tuple(sorted(manifest, key=lambda item: item.part_number))
        uploaded = tuple(state.parts[number] for number in sorted(state.parts))
        if ordered != uploaded:
            raise StorageValidationError("multipart manifest mismatch")
        if session.object_key.startswith("workspace://"):
            key = session.object_key
        else:
            key = f"workspace://{session.object_key}"
        _, target = self._path(key)
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary = target.with_suffix(target.suffix + ".uploading")
        digest = sha256()
        size_bytes = 0
        with temporary.open("wb") as output:
            for item in ordered:
                with self._part_path(session.session_id, item.part_number).open("rb") as source:
                    while chunk := source.read(1024 * 1024):
                        output.write(chunk)
                        digest.update(chunk)
                        size_bytes += len(chunk)
        checksum = digest.hexdigest()
        if session.expected_size_bytes is not None and size_bytes != session.expected_size_bytes:
            temporary.unlink(missing_ok=True)
            raise StorageMediaValidationError("media_validation_failed: size mismatch")
        if session.expected_checksum is not None and checksum != session.expected_checksum:
            temporary.unlink(missing_ok=True)
            raise StorageMediaValidationError("media_validation_failed: checksum mismatch")
        mime_type = session.expected_mime_type or "application/octet-stream"
        temporary.replace(target)
        result = StoredObjectRef(
            session.project_id,
            session.profile_id,
            "workspace",
            self._key_from_ref(key),
            size_bytes,
            checksum,
            mime_type,
            checksum,
            session.operation_key,
        )
        state.manifest = ordered
        state.result = result
        state.status = "completed"
        self._persist_session(session.session_id)
        self._objects[result.object_key] = result
        return result

    def abort_multipart(self, session: UploadSessionRef, correlation_id: str) -> UploadSessionRef:
        state = self._uploads.get(session.session_id)
        if state is None or state.operation_key != session.operation_key:
            raise StorageValidationError("multipart session binding is invalid")
        if state.status == "completed":
            raise StorageValidationError("multipart session is terminal")
        state.status = "aborted"
        self._persist_session(session.session_id)
        return UploadSessionRef(
            session.session_id,
            session.operation_key,
            session.project_id,
            session.profile_id,
            session.object_key,
            state.status,
            session.expected_size_bytes,
            session.expected_checksum,
            session.expected_mime_type,
        )

    def delete_with_proof(
        self, object_ref: StoredObjectRef, proof: DeleteProof, correlation_id: str
    ) -> None:
        if (
            not proof.no_references
            or proof.project_id != object_ref.project_id
            or proof.object_key != object_ref.object_key
        ):
            raise StorageObjectInUseError("object reference proof is missing")
        self.delete(f"workspace://{object_ref.object_key}")
        self._objects.pop(object_ref.object_key, None)

    def prove_no_references(
        self, object_ref: StoredObjectRef, project_id: str, correlation_id: str
    ) -> DeleteProof:
        del object_ref, project_id, correlation_id
        raise StorageObjectInUseError("reference proof is unavailable for Local storage")

    def presign_read(self, object_ref: str, expires_in_seconds: int) -> PresignedReference:
        def presign() -> PresignedReference:
            self._stat_unlogged(object_ref)
            if expires_in_seconds <= 0:
                raise ValueError("expiry must be positive")
            return PresignedReference(object_ref, expires_in_seconds)

        return self._logged("presign_read", None, presign)

    def presign_read_ref(
        self,
        object_ref: StoredObjectRef,
        actor_project_id: str,
        ttl_seconds: int,
        max_ttl_seconds: int = 900,
    ) -> PresignedAccess:
        if (
            object_ref.project_id != actor_project_id
            or ttl_seconds <= 0
            or ttl_seconds > max_ttl_seconds
        ):
            raise StorageAuthorizationError("presign scope or TTL is invalid")
        self._stat_unlogged(f"workspace://{object_ref.object_key}")
        return PresignedAccess(
            actor_project_id,
            object_ref.profile_id,
            object_ref.object_key,
            "read",
            int(time()) + ttl_seconds,
            f"workspace://{object_ref.object_key}",
        )

    def presign_write_intent(
        self,
        intent: StorageWriteIntent,
        ttl_seconds: int,
        max_ttl_seconds: int = 900,
    ) -> PresignedAccess:
        self._validate_intent(intent)
        if ttl_seconds <= 0 or ttl_seconds > max_ttl_seconds:
            raise StorageAuthorizationError("presign TTL is invalid")
        return PresignedAccess(
            intent.project_id,
            intent.profile_id,
            intent.object_key,
            "write",
            int(time()) + ttl_seconds,
            f"workspace://{intent.object_key}",
        )

    def presign_write(self, object_key: str, expires_in_seconds: int) -> PresignedReference:
        def presign() -> PresignedReference:
            key = self._key_from_ref(object_key)
            if expires_in_seconds <= 0:
                raise ValueError("expiry must be positive")
            return PresignedReference(f"workspace://{key}", expires_in_seconds)

        return self._logged("presign_write", None, presign)

    def download_to_workspace(
        self, object_ref: str, workspace_key: str, correlation_id: str
    ) -> StoredObject:
        return self._logged(
            "download_to_workspace",
            correlation_id,
            lambda: self._put_unlogged(workspace_key, self._get_unlogged(object_ref)),
        )

    def upload_from_workspace(
        self, workspace_key: str, object_key: str, correlation_id: str
    ) -> StoredObject:
        return self._logged(
            "upload_from_workspace",
            correlation_id,
            lambda: self._put_unlogged(
                object_key, self._get_unlogged(f"workspace://{workspace_key}")
            ),
        )

    def capability(self, profile_revision: int = 1) -> StorageCapability:
        return StorageCapability(profile_revision, 1, 64 * 1024 * 1024, 10_000, 8 * 1024**4)

    def admit_upload(
        self, size_bytes: int, part_size_bytes: int, *, profile_revision: int = 1
    ) -> None:
        if not self.capability(profile_revision).supports(size_bytes, part_size_bytes):
            raise StorageValidationError("storage_object_size_unsupported")

    def clean_workspace(self, now: float | None = None) -> tuple[str, ...]:
        """只清理受控临时文件；业务对象和已登记 StoredObject 不在 cleaner 范围。"""
        current = time() if now is None else now
        removed: list[str] = []
        for key, (path, status, created_at) in list(self._temporary.items()):
            retention = 24 * 3600 if status == "succeeded" else 7 * 24 * 3600
            if current - created_at >= retention:
                path.unlink(missing_ok=True)
                self._temporary.pop(key, None)
                removed.append(key)
        return tuple(removed)

    def register_temporary(
        self, workspace_key: str, status: str, *, created_at: float | None = None
    ) -> None:
        if status not in {"succeeded", "failed"}:
            raise StorageValidationError("temporary workspace status is invalid")
        key, path = self._path(workspace_key)
        if not path.exists() or key.startswith("projects/"):
            raise StorageValidationError("only existing non-business workspace files are cleanable")
        self._temporary[key] = (path, status, time() if created_at is None else created_at)


@dataclass(slots=True)
class _TosUpload:
    session: UploadSessionRef
    receipts: dict[int, PartReceipt]


class TOSAdapter:
    """Freeze one private TOS profile and expose only verified storage references."""

    def __init__(
        self,
        transport: StoragePort | None = None,
        credential_resolver: CredentialResolver | None = None,
        credential_ref: str | None = None,
        profile_id: str | None = None,
        *,
        tos_client: object | None = None,
        bucket: str | None = None,
        profile_revision: int | None = None,
        endpoint: str | None = None,
        region: str | None = None,
        bucket_binding_id: str | None = None,
    ) -> None:
        self._transport = transport
        self._credential_resolver = credential_resolver
        self._credential_ref = credential_ref
        self._profile_id = profile_id
        self._tos_client = tos_client
        self._bucket = bucket
        self._profile_revision = profile_revision
        self._endpoint = endpoint
        self._region = region
        self._bucket_binding_id = bucket_binding_id
        self._uploads: dict[str, _TosUpload] = {}
        self._operations: dict[str, str] = {}
        self._objects: dict[str, StoredObjectRef] = {}

    @classmethod
    def from_profile(
        cls,
        profile: Any,
        credential_resolver: CredentialResolver,
        *,
        client_factory: Callable[[str, str, str, str], object] | None = None,
    ) -> TOSAdapter:
        """Construct the official SDK client only from a complete private profile snapshot."""
        required = (
            getattr(profile, "adapter_key", None) == "tos",
            bool(getattr(profile, "id", None)),
            bool(getattr(profile, "endpoint", None)),
            bool(getattr(profile, "region", None)),
            bool(getattr(profile, "bucket", None)),
            bool(getattr(profile, "bucket_binding_id", None)),
            bool(getattr(profile, "project_id", None)),
            bool(getattr(profile, "project_scope", ())),
            getattr(profile, "credential_status", None) == "configured",
            bool(getattr(profile, "credential_ref", None)),
            bool(getattr(profile, "enabled", False)),
            bool(getattr(profile, "private_bucket", False)),
            isinstance(getattr(profile, "revision", None), int) and profile.revision >= 1,
        )
        if not all(required):
            raise AdapterNotConfiguredError("TOS profile is incomplete or not private")
        project_id = str(profile.project_id)
        if project_id not in tuple(profile.project_scope):
            raise AdapterNotConfiguredError("TOS profile is outside its project scope")
        credential = credential_resolver.resolve(profile.credential_ref, profile.id)
        access_key, separator, secret_key = credential.partition(":")
        if not access_key or not separator or not secret_key:
            raise AdapterNotConfiguredError("TOS credential is invalid")
        if client_factory is None:
            import tos  # type: ignore[import-untyped]

            client_factory = tos.TosClientV2
        client = client_factory(access_key, secret_key, profile.endpoint, profile.region)
        return cls(
            credential_resolver=credential_resolver,
            credential_ref=profile.credential_ref,
            profile_id=profile.id,
            tos_client=client,
            bucket=profile.bucket,
            profile_revision=profile.revision,
            endpoint=profile.endpoint,
            region=profile.region,
            bucket_binding_id=profile.bucket_binding_id,
        )

    @property
    def adapter_key(self) -> str:
        return "tos"

    @property
    def bucket(self) -> str | None:
        return self._bucket

    @property
    def profile_id(self) -> str | None:
        return self._profile_id

    @property
    def profile_revision(self) -> int | None:
        return self._profile_revision

    @property
    def endpoint(self) -> str | None:
        return self._endpoint

    @property
    def region(self) -> str | None:
        return self._region

    @property
    def bucket_binding_id(self) -> str | None:
        return self._bucket_binding_id

    def resolve_credential(self) -> str:
        if not self._credential_resolver or not self._credential_ref or not self._profile_id:
            self._missing("resolve_credential")
        return self._credential_resolver.resolve(self._credential_ref, self._profile_id)

    def _missing(self, operation: str, correlation_id: str | None = None) -> NoReturn:
        error = AdapterNotConfiguredError("TOS adapter is not configured in phase 0")
        log_event(
            "storage.call",
            correlation_id=correlation_id,
            operation=operation,
            adapter="tos",
            result="error",
            error_type=type(error).__name__,
        )
        raise error

    def _sdk(self, operation: str, correlation_id: str | None = None) -> tuple[Any, str]:
        if self._tos_client is None or not self._bucket or not self._profile_id:
            self._missing(operation, correlation_id)
        return self._tos_client, self._bucket

    @staticmethod
    def _key(value: str) -> str:
        key = canonical_object_key(value.removeprefix("tos://"))
        if key is None:
            raise StorageValidationError("object key is foreign or unsafe")
        return key

    def _validate_intent(self, intent: StorageWriteIntent) -> None:
        LocalWorkspaceAdapter._validate_intent(intent)
        if intent.profile_id != self._profile_id:
            raise StorageValidationError("storage profile is stale or foreign")

    @staticmethod
    def _metadata(intent: StorageWriteIntent) -> dict[str, str]:
        metadata = {
            "operation-key": intent.operation_key,
            "project-id": intent.project_id,
            "profile-id": intent.profile_id,
        }
        if intent.expected_checksum is not None:
            metadata["sha256"] = intent.expected_checksum
        return metadata

    @staticmethod
    def _head_metadata(head: object) -> dict[str, str]:
        raw = getattr(head, "meta", {}) or {}
        if not isinstance(raw, dict):
            raise StorageValidationError("TOS object metadata is invalid")
        return {str(key): str(value) for key, value in raw.items()}

    def _ref_from_head(self, intent: StorageWriteIntent, head: object) -> StoredObjectRef:
        metadata = self._head_metadata(head)
        if any(metadata.get(key) != value for key, value in self._metadata(intent).items()):
            raise StorageValidationError("TOS object metadata does not match frozen intent")
        size_bytes = getattr(head, "content_length", None)
        mime_type = getattr(head, "content_type", None)
        if not isinstance(size_bytes, int) or size_bytes < 0 or not isinstance(mime_type, str):
            raise StorageValidationError("TOS object metadata is incomplete")
        if intent.expected_size_bytes is not None and size_bytes != intent.expected_size_bytes:
            raise StorageMediaValidationError("media_validation_failed: size mismatch")
        if intent.expected_mime_type is not None and mime_type != intent.expected_mime_type:
            raise StorageMediaValidationError("media_validation_failed: MIME mismatch")
        checksum = metadata.get("sha256")
        if checksum is None or len(checksum) != 64:
            raise StorageMediaValidationError("media_validation_failed: checksum missing")
        if intent.expected_checksum is not None and checksum != intent.expected_checksum:
            raise StorageMediaValidationError("media_validation_failed: checksum mismatch")
        return StoredObjectRef(
            intent.project_id,
            intent.profile_id,
            self._bucket or "",
            intent.object_key,
            size_bytes,
            checksum,
            mime_type,
            str(getattr(head, "etag", "")) or None,
            intent.operation_key,
        )

    @staticmethod
    def _intent_from_session(session: UploadSessionRef) -> StorageWriteIntent:
        return StorageWriteIntent(
            session.operation_key,
            session.project_id,
            session.profile_id,
            session.object_key,
            session.expected_size_bytes,
            session.expected_checksum,
            session.expected_mime_type,
        )

    def put(self, object_key: str, content: bytes, correlation_id: str) -> StoredObject:
        if self._transport is not None:
            return self._transport.put(object_key, content, correlation_id)
        client, bucket = self._sdk("put", correlation_id)
        key = self._key(object_key)
        checksum = sha256(content).hexdigest()
        client.put_object(
            bucket,
            key,
            content=content,
            content_length=len(content),
            content_type="application/octet-stream",
            meta={"sha256": checksum},
        )
        head = client.head_object(bucket, key)
        size_bytes = getattr(head, "content_length", None)
        if size_bytes != len(content) or self._head_metadata(head).get("sha256") != checksum:
            raise StorageMediaValidationError("TOS direct object verification failed")
        return StoredObject(key, size_bytes, checksum, getattr(head, "etag", None))

    def get(self, object_ref: str) -> bytes:
        if self._transport is not None:
            return self._transport.get(object_ref)
        client, bucket = self._sdk("get")
        response = client.get_object(bucket, self._key(object_ref))
        content = response.read()
        if not isinstance(content, bytes):
            raise StorageValidationError("TOS read response is invalid")
        return content

    def iter_chunks(self, object_ref: str, chunk_size: int = 1024 * 1024) -> Iterator[bytes]:
        if self._transport is not None:
            return self._transport.iter_chunks(object_ref, chunk_size)
        if chunk_size <= 0:
            raise StorageValidationError("storage chunk size is invalid")
        client, bucket = self._sdk("iter_chunks")
        response = client.get_object(bucket, self._key(object_ref))

        def chunks() -> Iterator[bytes]:
            while chunk := response.read(chunk_size):
                if not isinstance(chunk, bytes):
                    raise StorageValidationError("TOS read response is invalid")
                yield chunk

        return chunks()

    def delete(self, object_ref: str) -> None:
        if self._transport is not None:
            return self._transport.delete(object_ref)
        client, bucket = self._sdk("delete")
        client.delete_object(bucket, self._key(object_ref))

    def stat(self, object_ref: str) -> StoredObject:
        if self._transport is not None:
            return self._transport.stat(object_ref)
        client, bucket = self._sdk("stat")
        key = self._key(object_ref)
        head = client.head_object(bucket, key)
        size_bytes = getattr(head, "content_length", None)
        checksum = self._head_metadata(head).get("sha256")
        if not isinstance(size_bytes, int) or checksum is None:
            raise StorageMediaValidationError("TOS object verification metadata is incomplete")
        return StoredObject(key, size_bytes, checksum, getattr(head, "etag", None))

    def create_multipart_session(self, object_key: str, correlation_id: str) -> MultipartSession:
        if self._transport is not None:
            return self._transport.create_multipart_session(object_key, correlation_id)
        self._missing("create_multipart_session", correlation_id)

    def upload_part(
        self,
        session_id: UploadSessionRef,
        part_number: PartReceipt,
        content: bytes,
        correlation_id: str,
    ) -> PartReceipt:
        if self._transport is not None:
            return self._transport.upload_part(session_id, part_number, content, correlation_id)
        upload = self._uploads.get(session_id.session_id)
        if upload is None or upload.session != session_id:
            raise StorageValidationError("multipart session binding is invalid")
        checksum = sha256(content).hexdigest()
        if checksum != part_number.checksum or len(content) != part_number.size_bytes:
            raise StorageMediaValidationError("multipart part verification failed")
        previous = upload.receipts.get(part_number.part_number)
        if previous is not None:
            if previous.checksum != checksum or previous.size_bytes != len(content):
                raise StorageValidationError("multipart part conflict")
            return previous
        client, bucket = self._sdk("upload_part", correlation_id)
        response = client.upload_part(
            bucket,
            session_id.object_key,
            session_id.session_id,
            part_number.part_number,
            content=content,
            content_length=len(content),
        )
        etag = getattr(response, "etag", None)
        if not isinstance(etag, str) or not etag:
            raise StorageValidationError("TOS multipart receipt is invalid")
        receipt = PartReceipt(part_number.part_number, checksum, etag, len(content))
        upload.receipts[receipt.part_number] = receipt
        return receipt

    def complete_multipart_session(
        self, session_id: str, parts: list[MultipartPart], correlation_id: str
    ) -> StoredObject:
        if self._transport is not None:
            return self._transport.complete_multipart_session(session_id, parts, correlation_id)
        self._missing("complete_multipart_session", correlation_id)

    def presign_read(self, object_ref: str, expires_in_seconds: int) -> PresignedReference:
        if self._transport is not None:
            return self._transport.presign_read(object_ref, expires_in_seconds)
        self._missing("presign_read")

    def presign_write(self, object_key: str, expires_in_seconds: int) -> PresignedReference:
        if self._transport is not None:
            return self._transport.presign_write(object_key, expires_in_seconds)
        self._missing("presign_write")

    def download_to_workspace(
        self, object_ref: str, workspace_key: str, correlation_id: str
    ) -> StoredObject:
        if self._transport is not None:
            return self._transport.download_to_workspace(object_ref, workspace_key, correlation_id)
        self._missing("download_to_workspace", correlation_id)

    def upload_from_workspace(
        self, workspace_key: str, object_key: str, correlation_id: str
    ) -> StoredObject:
        if self._transport is not None:
            return self._transport.upload_from_workspace(workspace_key, object_key, correlation_id)
        self._missing("upload_from_workspace", correlation_id)

    def capability(self, profile_revision: int = 1) -> StorageCapability:
        if self._transport is not None:
            capability = getattr(self._transport, "capability", None)
            if capability is not None:
                return cast(StorageCapability, capability(profile_revision))
        self._missing("capability")

    def admit_upload(
        self, size_bytes: int, part_size_bytes: int, *, profile_revision: int = 1
    ) -> None:
        if self._transport is not None:
            admission = getattr(self._transport, "admit_upload", None)
            if admission is not None:
                admission(size_bytes, part_size_bytes, profile_revision=profile_revision)
                return
        self._missing("admit_upload")

    def create_multipart(self, intent: StorageWriteIntent, correlation_id: str) -> UploadSessionRef:
        if self._transport is not None:
            return self._transport.create_multipart(intent, correlation_id)
        client, bucket = self._sdk("create_multipart", correlation_id)
        self._validate_intent(intent)
        existing_id = self._operations.get(intent.operation_key)
        if existing_id is not None:
            existing = self._uploads[existing_id].session
            if (
                existing.project_id != intent.project_id
                or existing.profile_id != intent.profile_id
                or existing.object_key != intent.object_key
                or existing.expected_size_bytes != intent.expected_size_bytes
                or existing.expected_checksum != intent.expected_checksum
                or existing.expected_mime_type != intent.expected_mime_type
            ):
                raise StorageValidationError("multipart operation binding is invalid")
            return existing
        response = client.create_multipart_upload(
            bucket,
            intent.object_key,
            content_type=intent.expected_mime_type,
            meta=self._metadata(intent),
        )
        upload_id = getattr(response, "upload_id", None)
        if not isinstance(upload_id, str) or not upload_id:
            raise StorageValidationError("TOS multipart session is invalid")
        session = UploadSessionRef(
            upload_id,
            intent.operation_key,
            intent.project_id,
            intent.profile_id,
            intent.object_key,
            "active",
            intent.expected_size_bytes,
            intent.expected_checksum,
            intent.expected_mime_type,
        )
        self._uploads[upload_id] = _TosUpload(session, {})
        self._operations[intent.operation_key] = upload_id
        return session

    def resume_multipart(self, intent: StorageWriteIntent, correlation_id: str) -> UploadSessionRef:
        return self.create_multipart(intent, correlation_id)

    def reconcile_multipart(
        self, intent: StorageWriteIntent, correlation_id: str
    ) -> StoredObjectRef | None:
        if self._transport:
            return self._transport.reconcile_multipart(intent, correlation_id)
        self._validate_intent(intent)
        existing = self._objects.get(intent.operation_key)
        if existing is not None:
            if (
                existing.project_id != intent.project_id
                or existing.profile_id != intent.profile_id
                or existing.object_key != intent.object_key
                or existing.size_bytes != intent.expected_size_bytes
                or existing.checksum != intent.expected_checksum
                or existing.mime_type != intent.expected_mime_type
            ):
                raise StorageValidationError("multipart_reconcile_conflict")
            return existing
        client, bucket = self._sdk("reconcile_multipart", correlation_id)
        try:
            head = client.head_object(bucket, intent.object_key)
        except Exception:
            return None
        ref = self._ref_from_head(intent, head)
        self._objects[intent.operation_key] = ref
        return ref

    def complete_multipart(
        self, session: UploadSessionRef, manifest: tuple[PartReceipt, ...], correlation_id: str
    ) -> StoredObjectRef:
        if self._transport is not None:
            return self._transport.complete_multipart(session, manifest, correlation_id)
        upload = self._uploads.get(session.session_id)
        if upload is None or upload.session != session:
            raise StorageValidationError("multipart session binding is invalid")
        existing = self._objects.get(session.operation_key)
        if existing is not None:
            return existing
        ordered = tuple(sorted(manifest, key=lambda item: item.part_number))
        uploaded = tuple(upload.receipts[number] for number in sorted(upload.receipts))
        if ordered != uploaded:
            raise StorageValidationError("multipart manifest mismatch")
        client, bucket = self._sdk("complete_multipart", correlation_id)
        client.complete_multipart_upload(
            bucket,
            session.object_key,
            session.session_id,
            [{"part_number": item.part_number, "etag": item.etag} for item in ordered],
        )
        ref = self._ref_from_head(
            self._intent_from_session(session), client.head_object(bucket, session.object_key)
        )
        self._objects[session.operation_key] = ref
        return ref

    def abort_multipart(self, session: UploadSessionRef, correlation_id: str) -> UploadSessionRef:
        if self._transport is not None:
            return self._transport.abort_multipart(session, correlation_id)
        self._missing("abort_multipart", correlation_id)

    def delete_with_proof(
        self, object_ref: StoredObjectRef, proof: DeleteProof, correlation_id: str
    ) -> None:
        if self._transport is not None:
            return self._transport.delete_with_proof(object_ref, proof, correlation_id)
        self._missing("delete_with_proof", correlation_id)

    def prove_no_references(
        self, object_ref: StoredObjectRef, project_id: str, correlation_id: str
    ) -> DeleteProof:
        self._missing("prove_no_references", correlation_id)


@dataclass(slots=True)
class StorageProfile:
    id: str
    project_id: str
    endpoint: str
    bucket: str
    region: str
    credential_status: str = "unconfigured"
    enabled: bool = False
    revision: int = 1
    name: str = ""
    adapter_key: str = "tos"
    private_bucket: bool = True
    bucket_binding_id: str = ""
    credential_ref: str | None = None
    connect_timeout_ms: int = 10_000
    read_timeout_ms: int = 30_000
    write_timeout_ms: int = 60_000
    presign_max_ttl_seconds: int = 900
    project_scope: tuple[str, ...] = ()
    masked_credential_summary: str | None = None

    def __post_init__(self) -> None:
        if self.adapter_key not in {"tos", "local_workspace"} or not all(
            (self.bucket, self.region, self.endpoint)
        ):
            raise StorageValidationError("storage profile configuration is invalid")
        if self.credential_status not in {
            "configured",
            "unconfigured",
            "rotating",
            "failed",
            "master_key_unavailable",
        }:
            raise StorageValidationError("storage credential status is invalid")
        if self.presign_max_ttl_seconds < 1:
            raise StorageValidationError("presign max TTL is invalid")

    def update(self, expected_revision: int, **changes: object) -> None:
        if expected_revision != self.revision:
            raise ValueError("storage profile revision conflict")
        for key, value in changes.items():
            if key in {"id", "project_id", "revision"}:
                raise StorageValidationError("storage profile identity is immutable")
            if hasattr(self, key):
                setattr(self, key, value)
        self.__post_init__()
        self.revision += 1


@dataclass(frozen=True, slots=True)
class VerifiedStoredObjectHandoff:
    operation_key: str
    project_id: str
    object_ref: str
    size_bytes: int
    checksum: str
    mime_type: str
    profile_revision: int
