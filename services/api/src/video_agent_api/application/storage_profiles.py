from __future__ import annotations

import hashlib
import json
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, cast
from uuid import uuid4

from video_agent_api.domain.errors import (
    CredentialMasterKeyUnavailableError,
    ProjectAccessForbiddenError,
    StorageProfileNotFoundError,
    StorageProfileRevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.ports.contracts import StorageCapability, StorageValidationError
from video_agent_api.ports.storage import StorageProfile


@dataclass(frozen=True, slots=True)
class CreateStorageProfileCommand:
    project_id: str
    endpoint: str
    bucket: str
    region: str
    name: str = ""
    credential_ref: str | None = None
    private_bucket: bool = True
    project_scope: tuple[str, ...] = ()
    enabled: bool = False
    adapter_key: str = "tos"
    bucket_binding_id: str = ""
    connect_timeout_ms: int = 10_000
    read_timeout_ms: int = 30_000
    write_timeout_ms: int = 60_000
    presign_max_ttl_seconds: int = 900


class StorageProfileService:
    def __init__(
        self,
        uow_factory: Any,
        connection_probe: Callable[[StorageProfile, str], dict[str, object]] | None = None,
        storage_mode: str = "tos",
    ) -> None:
        self._uow_factory = uow_factory
        self._connection_probe = connection_probe
        self._storage_mode = storage_mode

    @staticmethod
    def snapshot_hash(profile: StorageProfile, capability: StorageCapability) -> str:
        payload = {
            "adapterKey": profile.adapter_key,
            "bucket": profile.bucket,
            "bucketBindingId": profile.bucket_binding_id,
            "credentialRef": profile.credential_ref,
            "enabled": profile.enabled,
            "endpoint": profile.endpoint,
            "maxObjectSizeBytes": capability.max_object_size_bytes,
            "maxPartCount": capability.max_part_count,
            "maxPartSizeBytes": capability.max_part_size_bytes,
            "minPartSizeBytes": capability.min_part_size_bytes,
            "privateBucket": profile.private_bucket,
            "profileId": profile.id,
            "projectId": profile.project_id,
            "projectScope": sorted(profile.project_scope),
            "region": profile.region,
            "revision": profile.revision,
        }
        encoded = json.dumps(payload, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
        return hashlib.sha256(encoded.encode("utf-8")).hexdigest()

    @staticmethod
    def _local_profile(project_id: str) -> StorageProfile:
        return StorageProfile(
            "local-test-offline",
            project_id,
            "workspace://local",
            "workspace",
            "local",
            credential_status="configured",
            enabled=True,
            name="Local test/offline",
            adapter_key="local_workspace",
            private_bucket=True,
            bucket_binding_id="local-workspace",
            project_scope=(project_id,),
        )

    @staticmethod
    def _validate_upload_profile(profile: StorageProfile, project_id: str) -> None:
        if profile.project_id != project_id or project_id not in profile.project_scope:
            raise ValidationDomainError("storage profile is outside the project scope")
        if not profile.enabled:
            raise ValidationDomainError("storage profile is disabled")
        if not profile.private_bucket or not profile.bucket_binding_id:
            raise ValidationDomainError("storage profile private bucket binding is invalid")
        if profile.adapter_key == "tos" and (
            not profile.credential_ref or profile.credential_status != "configured"
        ):
            raise ValidationDomainError("storage profile credential is unconfigured")

    async def list_upload_profiles(
        self, project_id: str, project_scope: str | None = None
    ) -> tuple[StorageProfile, ...]:
        if project_scope != project_id:
            raise ValidationDomainError("project scope is required")
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise StorageProfileNotFoundError("storage profile project not found")
            profiles = [
                cast(StorageProfile, profile)
                for profile in uow.storage_profiles.values()
                if getattr(profile, "project_id", None) == project_id
                and getattr(profile, "adapter_key", None) == self._storage_mode
            ]
        if self._storage_mode == "local_workspace":
            profiles.insert(0, self._local_profile(project_id))
        return tuple(profiles)

    async def resolve_upload_profile(
        self,
        project_id: str,
        profile_id: str,
        expected_revision: int,
        project_scope: str | None = None,
    ) -> StorageProfile:
        if project_scope != project_id:
            raise ValidationDomainError("project scope is required")
        if profile_id == "local-test-offline":
            if self._storage_mode != "local_workspace":
                raise StorageProfileNotFoundError("local storage profile is unavailable")
            async with self._uow_factory() as uow:
                if await uow.projects.get(project_id) is None:
                    raise StorageProfileNotFoundError("storage profile project not found")
            profile = self._local_profile(project_id)
        else:
            profile = await self.get(profile_id, project_id)
            if profile.adapter_key != self._storage_mode:
                raise ValidationDomainError("storage profile adapter is not selected")
        if profile.revision != expected_revision:
            raise StorageProfileRevisionConflictError(
                profile.id, expected_revision, profile.revision
            )
        self._validate_upload_profile(profile, project_id)
        return profile

    async def create(self, command: CreateStorageProfileCommand) -> StorageProfile:
        if command.project_id not in command.project_scope:
            raise ValidationDomainError("storage profile project scope is invalid")
        try:
            profile = StorageProfile(
                str(uuid4()),
                command.project_id,
                command.endpoint,
                command.bucket,
                command.region,
                name=command.name,
                credential_ref=command.credential_ref,
                private_bucket=command.private_bucket,
                project_scope=command.project_scope,
                enabled=command.enabled,
                adapter_key=command.adapter_key,
                bucket_binding_id=command.bucket_binding_id,
                connect_timeout_ms=command.connect_timeout_ms,
                read_timeout_ms=command.read_timeout_ms,
                write_timeout_ms=command.write_timeout_ms,
                presign_max_ttl_seconds=command.presign_max_ttl_seconds,
            )
        except StorageValidationError as error:
            raise ValidationDomainError(str(error)) from error
        async with self._uow_factory() as uow:
            if await uow.projects.get(command.project_id) is None:
                raise StorageProfileNotFoundError("storage profile project not found")
            uow.storage_profiles[profile.id] = profile
            await uow.commit()
        return profile

    async def get(self, profile_id: str, project_scope: str | None = None) -> StorageProfile:
        if not project_scope:
            raise ValidationDomainError("project scope is required")
        async with self._uow_factory() as uow:
            profile = uow.storage_profiles.get(profile_id)
            if profile is None:
                raise StorageProfileNotFoundError("storage profile not found")
            if profile.project_id != project_scope or project_scope not in profile.project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            return cast(StorageProfile, profile)

    async def update(
        self,
        profile_id: str,
        expected_revision: int,
        changes: dict[str, object],
        project_scope: str | None = None,
    ) -> StorageProfile:
        if not project_scope:
            raise ValidationDomainError("project scope is required")
        async with self._uow_factory() as uow:
            profile = uow.storage_profiles.get(profile_id)
            if profile is None:
                raise StorageProfileNotFoundError("storage profile not found")
            if profile.project_id != project_scope or project_scope not in profile.project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if "project_scope" in changes:
                requested_scope = changes["project_scope"]
                if (
                    not isinstance(requested_scope, (tuple, list))
                    or profile.project_id not in requested_scope
                ):
                    raise ValidationDomainError("storage profile project scope is invalid")
            if profile.revision != expected_revision:
                raise StorageProfileRevisionConflictError(
                    profile_id, expected_revision, profile.revision
                )
            try:
                profile.update(expected_revision, **changes)
            except ValueError as error:
                raise StorageProfileRevisionConflictError(
                    profile_id, expected_revision, profile.revision
                ) from error
            except StorageValidationError as error:
                raise ValidationDomainError(str(error)) from error
            await uow.commit()
            return cast(StorageProfile, profile)

    async def set_enabled(
        self,
        profile_id: str,
        expected_revision: int,
        enabled: bool,
        project_scope: str | None = None,
    ) -> StorageProfile:
        return await self.update(profile_id, expected_revision, {"enabled": enabled}, project_scope)

    async def connection_test(
        self,
        profile_id: str,
        expected_revision: int,
        probe_correlation_id: str,
        project_scope: str | None = None,
    ) -> dict[str, object]:
        if not project_scope:
            raise ValidationDomainError("project scope is required")
        async with self._uow_factory() as uow:
            profile = uow.storage_profiles.get(profile_id)
            if profile is None:
                return {
                    "status": "unconfigured",
                    "diagnostic": "storage_profile_not_found",
                    "probeCorrelationId": probe_correlation_id,
                }
            if profile.project_id != project_scope or project_scope not in profile.project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if profile.revision != expected_revision:
                raise StorageProfileRevisionConflictError(
                    profile_id, expected_revision, profile.revision
                )
            if profile.credential_status == "master_key_unavailable":
                raise CredentialMasterKeyUnavailableError()
            if profile.credential_status != "configured":
                return {
                    "status": "unconfigured",
                    "diagnostic": "storage_credential_unconfigured",
                    "probeCorrelationId": probe_correlation_id,
                }
            if not profile.enabled or not profile.private_bucket:
                return {
                    "status": "unconfigured",
                    "diagnostic": "storage_profile_disabled_or_public",
                    "probeCorrelationId": probe_correlation_id,
                }
            if self._connection_probe is not None:
                result = self._connection_probe(profile, probe_correlation_id)
                return {**result, "probeCorrelationId": probe_correlation_id}
            # A configured row is not evidence of a live connection without a transport probe.
            return {
                "status": "unconfigured",
                "diagnostic": "tos_sdk_or_credentials_unconfigured",
                "profileId": profile.id,
                "revision": profile.revision,
                "probeCorrelationId": probe_correlation_id,
            }
