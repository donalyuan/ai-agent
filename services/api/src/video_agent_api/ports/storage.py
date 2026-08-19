"""本地开发存储与真实 TOS 的显式未配置占位 Adapter。"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path, PurePosixPath
from typing import NoReturn, TypeVar
from uuid import uuid4

from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    MultipartPart,
    MultipartSession,
    PresignedReference,
    StoredObject,
)

_ResultT = TypeVar("_ResultT")


@dataclass(slots=True)
class _LocalUpload:
    object_key: str
    parts: dict[int, bytes]


class LocalWorkspaceAdapter:
    """只允许受控根目录下的相对对象键，向业务返回抽象引用。"""

    def __init__(self, root: Path) -> None:
        self._root = root.resolve()
        self._uploads: dict[str, _LocalUpload] = {}

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
        path = PurePosixPath(key)
        if not key or path.is_absolute() or ".." in path.parts:
            raise ValueError("object key escapes workspace root")
        return path.as_posix()

    def _path(self, value: str) -> tuple[str, Path]:
        key = self._key_from_ref(value)
        path = (self._root / key).resolve()
        if path != self._root and self._root not in path.parents:
            raise ValueError("object key escapes workspace root")
        return key, path

    def put(self, object_key: str, content: bytes, correlation_id: str) -> StoredObject:
        return self._logged("put", correlation_id, lambda: self._put_unlogged(object_key, content))

    def _put_unlogged(self, object_key: str, content: bytes) -> StoredObject:
        key, path = self._path(object_key)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return StoredObject(f"workspace://{key}", len(content), sha256(content).hexdigest())

    def get(self, object_ref: str) -> bytes:
        return self._logged("get", None, lambda: self._get_unlogged(object_ref))

    def _get_unlogged(self, object_ref: str) -> bytes:
        return self._path(object_ref)[1].read_bytes()

    def delete(self, object_ref: str) -> None:
        self._logged("delete", None, lambda: self._path(object_ref)[1].unlink())

    def stat(self, object_ref: str) -> StoredObject:
        return self._logged("stat", None, lambda: self._stat_unlogged(object_ref))

    def _stat_unlogged(self, object_ref: str) -> StoredObject:
        key, path = self._path(object_ref)
        content = path.read_bytes()
        return StoredObject(f"workspace://{key}", len(content), sha256(content).hexdigest())

    def create_multipart_session(self, object_key: str, correlation_id: str) -> MultipartSession:
        def create() -> MultipartSession:
            key = self._key_from_ref(object_key)
            session_id = uuid4().hex
            self._uploads[session_id] = _LocalUpload(key, {})
            return MultipartSession(session_id, f"workspace://{key}")

        return self._logged("create_multipart_session", correlation_id, create)

    def upload_part(
        self, session_id: str, part_number: int, content: bytes, correlation_id: str
    ) -> MultipartPart:
        def upload() -> MultipartPart:
            if part_number < 1 or session_id not in self._uploads:
                raise ValueError("unknown multipart session or invalid part number")
            self._uploads[session_id].parts[part_number] = content
            return MultipartPart(part_number, sha256(content).hexdigest())

        return self._logged("upload_part", correlation_id, upload)

    def complete_multipart_session(
        self, session_id: str, parts: list[MultipartPart], correlation_id: str
    ) -> StoredObject:
        def complete() -> StoredObject:
            upload = self._uploads.pop(session_id, None)
            if upload is None:
                raise ValueError("unknown multipart session")
            expected = sorted(part.part_number for part in parts)
            if expected != sorted(upload.parts):
                raise ValueError("multipart parts do not match uploaded content")
            return self._put_unlogged(
                upload.object_key, b"".join(upload.parts[number] for number in expected)
            )

        return self._logged("complete_multipart_session", correlation_id, complete)

    def presign_read(self, object_ref: str, expires_in_seconds: int) -> PresignedReference:
        def presign() -> PresignedReference:
            self._stat_unlogged(object_ref)
            if expires_in_seconds <= 0:
                raise ValueError("expiry must be positive")
            return PresignedReference(object_ref, expires_in_seconds)

        return self._logged("presign_read", None, presign)

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


class TOSAdapter:
    """阶段 0 故意不加载 TOS SDK，也绝不降级到其他外部服务。"""

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

    def put(self, object_key: str, content: bytes, correlation_id: str) -> StoredObject:
        self._missing("put", correlation_id)

    def get(self, object_ref: str) -> bytes:
        self._missing("get")

    def delete(self, object_ref: str) -> None:
        self._missing("delete")

    def stat(self, object_ref: str) -> StoredObject:
        self._missing("stat")

    def create_multipart_session(self, object_key: str, correlation_id: str) -> MultipartSession:
        self._missing("create_multipart_session", correlation_id)

    def upload_part(
        self, session_id: str, part_number: int, content: bytes, correlation_id: str
    ) -> MultipartPart:
        self._missing("upload_part", correlation_id)

    def complete_multipart_session(
        self, session_id: str, parts: list[MultipartPart], correlation_id: str
    ) -> StoredObject:
        self._missing("complete_multipart_session", correlation_id)

    def presign_read(self, object_ref: str, expires_in_seconds: int) -> PresignedReference:
        self._missing("presign_read")

    def presign_write(self, object_key: str, expires_in_seconds: int) -> PresignedReference:
        self._missing("presign_write")

    def download_to_workspace(
        self, object_ref: str, workspace_key: str, correlation_id: str
    ) -> StoredObject:
        self._missing("download_to_workspace", correlation_id)

    def upload_from_workspace(
        self, workspace_key: str, object_key: str, correlation_id: str
    ) -> StoredObject:
        self._missing("upload_from_workspace", correlation_id)
