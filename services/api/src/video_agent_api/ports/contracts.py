"""所有外部副作用使用的六个 Protocol、DTO 和可诊断错误。"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Protocol


class PortError(RuntimeError):
    pass


class AdapterNotConfiguredError(PortError):
    pass


class DisabledConfigurationError(PortError):
    pass


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


@dataclass(frozen=True, slots=True)
class StoredObject:
    object_ref: str
    size_bytes: int
    checksum: str


@dataclass(frozen=True, slots=True)
class MultipartSession:
    session_id: str
    object_ref: str


@dataclass(frozen=True, slots=True)
class MultipartPart:
    part_number: int
    etag: str


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

    def delete(self, object_ref: str) -> None: ...

    def stat(self, object_ref: str) -> StoredObject: ...

    def create_multipart_session(
        self, object_key: str, correlation_id: str
    ) -> MultipartSession: ...

    def upload_part(
        self, session_id: str, part_number: int, content: bytes, correlation_id: str
    ) -> MultipartPart: ...

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
