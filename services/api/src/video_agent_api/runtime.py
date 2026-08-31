"""阶段 0 的唯一运行时装配入口；只允许显式 Mock/Local 或未配置 TOS。"""

from __future__ import annotations

import os
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    ModelSelection,
    PortResult,
    StoragePort,
)
from video_agent_api.ports.mocks import DeterministicMockProvider
from video_agent_api.ports.storage import LocalWorkspaceAdapter, TOSAdapter


@dataclass(frozen=True, slots=True)
class RuntimeSettings:
    provider_mode: str
    storage_mode: str
    workspace_root: Path
    provider_profile_id: str | None = None
    provider_model_id: str | None = None
    provider_credential_ref: str | None = None
    storage_profile_id: str | None = None
    storage_bucket_binding_id: str | None = None
    storage_credential_ref: str | None = None
    renderer_profile_id: str | None = None
    ffmpeg_path: str | None = None
    ffprobe_path: str | None = None

    @property
    def selection(self) -> RuntimeSelection:
        """Keep requested live identities separate from catalog-owned resolved facts."""
        return RuntimeSelection(
            provider_profile_id=self.provider_profile_id,
            provider_model_id=self.provider_model_id,
            provider_credential_ref=self.provider_credential_ref,
            storage_profile_id=self.storage_profile_id,
            storage_bucket_binding_id=self.storage_bucket_binding_id,
            storage_credential_ref=self.storage_credential_ref,
            renderer_profile_id=self.renderer_profile_id,
            ffmpeg_path=self.ffmpeg_path,
            ffprobe_path=self.ffprobe_path,
        )

    @classmethod
    def from_mapping(cls, values: Mapping[str, str]) -> RuntimeSettings:
        """从显式配置读取模式；默认值仍保持零网络、零付费。"""
        return cls(
            provider_mode=values.get("PROVIDER_MODE", "mock"),
            storage_mode=values.get("STORAGE_MODE", "local_workspace"),
            workspace_root=Path(values.get("WORKSPACE_ROOT", "/tmp/video-agent-workspaces")),
            provider_profile_id=values.get("PROVIDER_PROFILE_ID") or None,
            provider_model_id=values.get("PROVIDER_MODEL_ID") or None,
            provider_credential_ref=values.get("PROVIDER_CREDENTIAL_REF") or None,
            storage_profile_id=values.get("STORAGE_PROFILE_ID") or None,
            storage_bucket_binding_id=values.get("STORAGE_BUCKET_BINDING_ID") or None,
            storage_credential_ref=values.get("STORAGE_CREDENTIAL_REF") or None,
            renderer_profile_id=values.get("RENDERER_PROFILE_ID") or None,
            ffmpeg_path=values.get("FFMPEG_PATH") or None,
            ffprobe_path=values.get("FFPROBE_PATH") or None,
        )

    @classmethod
    def from_env(cls) -> RuntimeSettings:
        return cls.from_mapping(os.environ)


@dataclass(frozen=True, slots=True)
class RuntimeSelection:
    """Explicit configuration references, resolved later by the catalog composition boundary."""

    provider_profile_id: str | None
    provider_model_id: str | None
    provider_credential_ref: str | None
    storage_profile_id: str | None
    storage_bucket_binding_id: str | None
    storage_credential_ref: str | None
    renderer_profile_id: str | None
    ffmpeg_path: str | None
    ffprobe_path: str | None


@dataclass(frozen=True, slots=True)
class UnconfiguredLiveProvider:
    """Reject live side effects until catalog, capability and credential gates are resolved."""

    selection: RuntimeSelection

    def _missing(self) -> NoReturn:
        raise AdapterNotConfiguredError("live provider requires catalog resolver")

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        del prompt, selection, correlation_id
        self._missing()

    def generate_image(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        del prompt, selection, correlation_id
        self._missing()

    def edit_image(self, prompt: str, selection: ModelSelection, correlation_id: str) -> PortResult:
        del prompt, selection, correlation_id
        self._missing()

    def submit_video(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        del prompt, selection, correlation_id
        self._missing()

    def get_video_status(self, job_id: str, correlation_id: str) -> PortResult:
        del job_id, correlation_id
        self._missing()

    def cancel_video(self, job_id: str, correlation_id: str) -> PortResult:
        del job_id, correlation_id
        self._missing()


@dataclass(frozen=True, slots=True)
class RuntimeComponents:
    provider_mode: str
    storage_mode: str
    provider: DeterministicMockProvider | UnconfiguredLiveProvider
    storage: StoragePort
    selection: RuntimeSelection


def _unsupported(kind: str, mode: str) -> AdapterNotConfiguredError:
    error = AdapterNotConfiguredError(f"unsupported {kind} mode: {mode}")
    log_event(
        "runtime.adapter.select",
        operation="select",
        adapter=mode,
        mode=mode,
        boundary=kind,
        result="error",
        error_type=type(error).__name__,
    )
    return error


def build_runtime(settings: RuntimeSettings) -> RuntimeComponents:
    """Select only explicit offline or unresolved live boundaries; never substitute adapters."""
    if settings.provider_mode == "mock":
        provider: DeterministicMockProvider | UnconfiguredLiveProvider = DeterministicMockProvider()
    elif settings.provider_mode == "live":
        provider = UnconfiguredLiveProvider(settings.selection)
    else:
        raise _unsupported("provider", settings.provider_mode)

    storage: StoragePort
    if settings.storage_mode == "local_workspace":
        storage = LocalWorkspaceAdapter(settings.workspace_root)
    elif settings.storage_mode == "tos":
        storage = TOSAdapter()
    else:
        raise _unsupported("storage", settings.storage_mode)

    for boundary, mode in (
        ("provider", settings.provider_mode),
        ("storage", settings.storage_mode),
    ):
        log_event(
            "runtime.adapter.select",
            operation="select",
            adapter=mode,
            mode=mode,
            boundary=boundary,
            result="success",
        )
    return RuntimeComponents(
        provider_mode=settings.provider_mode,
        storage_mode=settings.storage_mode,
        provider=provider,
        storage=storage,
        selection=settings.selection,
    )


def build_runtime_from_env() -> RuntimeComponents:
    return build_runtime(RuntimeSettings.from_env())
