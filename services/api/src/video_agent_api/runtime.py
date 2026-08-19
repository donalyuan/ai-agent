"""阶段 0 的唯一运行时装配入口；只允许显式 Mock/Local 或未配置 TOS。"""

from __future__ import annotations

import os
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import AdapterNotConfiguredError, StoragePort
from video_agent_api.ports.mocks import DeterministicMockProvider
from video_agent_api.ports.storage import LocalWorkspaceAdapter, TOSAdapter


@dataclass(frozen=True, slots=True)
class RuntimeSettings:
    provider_mode: str
    storage_mode: str
    workspace_root: Path

    @classmethod
    def from_mapping(cls, values: Mapping[str, str]) -> RuntimeSettings:
        """从显式配置读取模式；默认值仍保持零网络、零付费。"""
        return cls(
            provider_mode=values.get("PROVIDER_MODE", "mock"),
            storage_mode=values.get("STORAGE_MODE", "local_workspace"),
            workspace_root=Path(values.get("WORKSPACE_ROOT", "/tmp/video-agent-workspaces")),
        )

    @classmethod
    def from_env(cls) -> RuntimeSettings:
        return cls.from_mapping(os.environ)


@dataclass(frozen=True, slots=True)
class RuntimeComponents:
    provider_mode: str
    storage_mode: str
    provider: DeterministicMockProvider
    storage: StoragePort


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
    """严格选择阶段 0 Adapter；未知值不得静默退回 Mock/Local。"""
    if settings.provider_mode != "mock":
        raise _unsupported("provider", settings.provider_mode)
    provider = DeterministicMockProvider()

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
    )


def build_runtime_from_env() -> RuntimeComponents:
    return build_runtime(RuntimeSettings.from_env())
