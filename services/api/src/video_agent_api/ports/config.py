"""Provider/Profile/Model 选择只来自记录，业务调用不携带固定端点。"""

from __future__ import annotations

from dataclasses import dataclass

from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    DisabledConfigurationError,
    ModelSelection,
)


@dataclass(frozen=True, slots=True)
class ProfileRecord:
    profile_id: str
    enabled: bool
    selection: ModelSelection


class ProviderCatalog:
    def __init__(self) -> None:
        self._profiles: dict[str, ProfileRecord] = {}

    @classmethod
    def empty(cls) -> ProviderCatalog:
        return cls()

    def add_profile(self, profile_id: str, enabled: bool, selection: ModelSelection) -> None:
        self._profiles[profile_id] = ProfileRecord(profile_id, enabled, selection)

    def select(self, profile_id: str, correlation_id: str | None = None) -> ModelSelection:
        record = self._profiles.get(profile_id)
        if record is None:
            missing_error = AdapterNotConfiguredError(
                f"provider profile is not configured: {profile_id}"
            )
            self._log_selection(profile_id, correlation_id, "error", missing_error)
            raise missing_error
        if not record.enabled:
            disabled_error = DisabledConfigurationError(
                f"provider profile is disabled: {profile_id}"
            )
            self._log_selection(profile_id, correlation_id, "error", disabled_error)
            raise disabled_error
        self._log_selection(profile_id, correlation_id, "success")
        return record.selection

    @staticmethod
    def _log_selection(
        profile_id: str,
        correlation_id: str | None,
        result: str,
        error: Exception | None = None,
    ) -> None:
        data = {
            "operation": "select",
            "adapter": "provider_catalog",
            "profile_id": profile_id,
            "result": result,
        }
        if error is not None:
            data["error_type"] = type(error).__name__
        log_event("provider.config.select", correlation_id=correlation_id, **data)
