"""Catalog-owned gates for the explicit live runtime boundary.

This module deliberately returns identity only. Provider transports and durable
owner ledgers remain separate so a rejected selection cannot create a call or
cause an external side effect.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from hashlib import sha256
from inspect import isawaitable
from json import dumps
from pathlib import Path
from typing import Any, cast

from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.ports.contracts import (
    FrozenRemoteLookup,
    RemoteLookupPort,
    StorageCapability,
)
from video_agent_api.providers.agnes import AgnesVideoProvider
from video_agent_api.providers.gpt_image import GPTImageProvider
from video_agent_api.providers.text import OpenAICompatibleTextModelAdapter


@dataclass(frozen=True, slots=True)
class ResolvedRuntimeIdentity:
    provider_id: str
    profile_id: str
    model_id: str
    adapter_key: str
    adapter_identity: str
    model_key: str
    operation: str
    capability_snapshot_id: str | None
    capability_revision: int | None
    policy_revision: int
    capabilities: tuple[str, ...]
    base_url: str | None
    credential_ref: str | None
    timeout_seconds: float
    default_parameters: dict[str, object]
    project_scope: tuple[str, ...] = ()
    idempotency_key_header: str | None = None
    correlation_header: str | None = None
    remote_lookup_protocol: str | None = None

    def selection(self, parameters: dict[str, object] | None = None) -> Any:
        """Expose the frozen catalog model rather than a caller-supplied adapter name."""
        from video_agent_api.ports.contracts import ModelSelection

        defaults = dict(self.default_parameters)
        if parameters:
            defaults.update(parameters)
        return ModelSelection(
            self.provider_id,
            self.profile_id,
            self.model_id,
            self.adapter_key,
            defaults,
        )


class CatalogRuntimeResolver:
    """Resolve only explicitly selected catalog resources with fail-closed gates."""

    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def resolve_probe(
        self,
        profile_id: str,
        model_id: str,
        operation: str,
        *,
        timeout_seconds: float,
        project_id: str | None = None,
    ) -> ResolvedRuntimeIdentity:
        """Probe prerequisites intentionally do not require a previous runnable snapshot."""
        if timeout_seconds <= 0:
            raise ValidationDomainError("provider_probe_timeout_invalid")
        async with self._uow_factory() as uow:
            provider, profile, model = self._selection(uow, profile_id, model_id)
            self._validate_project_scope(profile, project_id)
            if (
                provider.approval != "approved"
                or provider.feature_gate != "MVP-A"
                or not provider.adapter_installed
                or not profile.explicit_live_opt_in
                or profile.credential_status != "configured"
            ):
                raise ValidationDomainError("live_provider_unconfigured")
            return ResolvedRuntimeIdentity(
                provider.id,
                profile.id,
                model.id,
                provider.adapter_key,
                profile.adapter_identity,
                model.model_key,
                operation,
                None,
                None,
                profile.revision,
                (),
                profile.base_url,
                profile.credential_ref,
                profile.timeout_ms / 1000,
                dict(model.default_parameters),
                tuple(profile.project_scope),
            )

    async def resolve_invocation(
        self,
        profile_id: str,
        model_id: str,
        operation: str,
        *,
        project_id: str | None = None,
        expected_profile_revision: int | None = None,
        expected_capability_snapshot_id: str | None = None,
        expected_capability_revision: int | None = None,
    ) -> ResolvedRuntimeIdentity:
        """Freeze the usable catalog identity before a durable attempt can be created."""
        async with self._uow_factory() as uow:
            provider, profile, model = self._selection(uow, profile_id, model_id)
            self._validate_project_scope(profile, project_id)
            if (
                provider.approval != "approved"
                or provider.feature_gate != "MVP-A"
                or not provider.adapter_installed
                or not provider.enabled
                or not profile.enabled
                or not profile.explicit_live_opt_in
                or profile.credential_status != "configured"
                or not model.enabled
            ):
                raise ValidationDomainError("live_provider_unconfigured")
            if (
                expected_profile_revision is not None
                and profile.revision != expected_profile_revision
            ):
                raise ValidationDomainError("catalog_profile_revision_stale")
            snapshot = profile.capability_snapshots.get(operation)
            if (
                snapshot is None
                or not snapshot.runnable
                or snapshot.provider_id != provider.id
                or snapshot.profile_id != profile.id
                or snapshot.operation != operation
                or (snapshot.model_id is not None and snapshot.model_id != model.id)
            ):
                raise ValidationDomainError("capability_snapshot_unavailable")
            if (
                expected_capability_snapshot_id is not None
                and snapshot.id != expected_capability_snapshot_id
            ) or (
                expected_capability_revision is not None
                and snapshot.revision != expected_capability_revision
            ):
                raise ValidationDomainError("capability_snapshot_stale")
            return ResolvedRuntimeIdentity(
                provider.id,
                profile.id,
                model.id,
                provider.adapter_key,
                profile.adapter_identity,
                model.model_key,
                operation,
                snapshot.id,
                snapshot.revision,
                profile.revision,
                snapshot.capabilities,
                profile.base_url,
                profile.credential_ref,
                profile.timeout_ms / 1000,
                dict(model.default_parameters),
                tuple(profile.project_scope),
                snapshot.idempotency_key_header,
                snapshot.correlation_header,
                snapshot.remote_lookup_protocol,
            )

    async def release_invocation(self, identity: ResolvedRuntimeIdentity) -> None:
        """Reject resolver-only release because only an owner ledger owns a reservation."""
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(identity.profile_id)
            if profile is None or profile.provider_id != identity.provider_id:
                raise ValidationDomainError("catalog_selection_unconfigured")
            raise ValidationDomainError("provider_policy_reservation_owner_required")

    @staticmethod
    def _selection(uow: Any, profile_id: str, model_id: str) -> tuple[Any, Any, Any]:
        profile = uow.profiles.get(profile_id)
        model = uow.models.get(model_id)
        provider = uow.providers.get(profile.provider_id) if profile is not None else None
        if provider is None or profile is None or model is None or model.profile_id != profile.id:
            raise ValidationDomainError("catalog_selection_unconfigured")
        return provider, profile, model

    @staticmethod
    def _validate_project_scope(profile: Any, project_id: str | None) -> None:
        scope = tuple(getattr(profile, "project_scope", ()) or ())
        if project_id is not None and scope and project_id not in scope:
            raise ValidationDomainError("catalog project scope is foreign")


@dataclass(frozen=True, slots=True)
class ComposedLiveProvider:
    """A frozen catalog identity paired with its one explicitly selected live port."""

    identity: ResolvedRuntimeIdentity
    port: OpenAICompatibleTextModelAdapter | GPTImageProvider | AgnesVideoProvider


@dataclass(frozen=True, slots=True)
class ResolvedStorageIdentity:
    storage_profile_id: str
    profile_revision: int
    snapshot_hash: str
    bucket_binding_id: str
    project_id: str
    credential_ref: str
    bucket: str
    endpoint: str
    region: str
    capability: dict[str, int]


@dataclass(frozen=True, slots=True)
class ComposedStoragePort:
    identity: ResolvedStorageIdentity
    port: Any

    def __getattr__(self, name: str) -> Any:
        """Expose only the explicitly composed adapter while retaining frozen identity."""
        # Services consume the StoragePort protocol; forwarding keeps that API stable while
        # callers can still inspect ``identity`` instead of deriving profile facts from a
        # process-global runtime adapter.
        return getattr(self.port, name)


@dataclass(frozen=True, slots=True)
class ComposedRendererPort:
    """Renderer bound to one explicit profile and frozen executable paths."""

    profile_id: str
    revision: int
    capability_snapshot_id: str
    capability_revision: int
    ffmpeg_path: str
    ffprobe_path: str
    port: Any
    snapshot_id: str | None = None
    capability: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True, slots=True)
class _FrozenCredentialResolver:
    credential_ref: str
    profile_id: str
    credential: str

    def resolve(self, credential_ref: str, profile_id: str) -> str:
        if credential_ref != self.credential_ref or profile_id != self.profile_id:
            raise ValidationDomainError("storage credential reference is stale")
        return self.credential


class CatalogRuntimeComposition:
    """Construct live adapters only from already-admitted catalog identity facts."""

    def __init__(self, resolver: CatalogRuntimeResolver, credential_resolver: Any) -> None:
        self._resolver = resolver
        self._credential_resolver = credential_resolver

    async def _credential(self, credential_ref: str, profile_id: str) -> str:
        resolve = getattr(self._credential_resolver, "resolve_credential", None)
        if resolve is None:
            resolve = getattr(self._credential_resolver, "resolve", None)
        if resolve is None:
            raise ValidationDomainError("live_provider_unconfigured")
        try:
            credential = resolve(credential_ref, profile_id)
            if isawaitable(credential):
                credential = await credential
        except Exception as error:
            raise ValidationDomainError("live_provider_unconfigured") from error
        if not isinstance(credential, str) or not credential:
            raise ValidationDomainError("live_provider_unconfigured")
        return credential

    async def resolve_provider(
        self,
        profile_id: str,
        model_id: str,
        operation: str,
        *,
        project_id: str | None = None,
        expected_profile_revision: int | None = None,
        expected_capability_snapshot_id: str | None = None,
        expected_capability_revision: int | None = None,
    ) -> ComposedLiveProvider:
        identity = await self._resolver.resolve_invocation(
            profile_id,
            model_id,
            operation,
            project_id=project_id,
            expected_profile_revision=expected_profile_revision,
            expected_capability_snapshot_id=expected_capability_snapshot_id,
            expected_capability_revision=expected_capability_revision,
        )
        if not identity.base_url or not identity.credential_ref:
            raise ValidationDomainError("live_provider_unconfigured")
        adapter_identities = {
            "agentscope": {"agentscope"},
            "openai": {"openai"},
            "text": {"text"},
            "gpt": {"gpt"},
            "gpt-image": {"gpt-image"},
            "gpt_image": {"gpt_image"},
            "agnes": {"agnes"},
            "agnes_video": {"agnes_video"},
        }
        if identity.adapter_identity not in adapter_identities.get(identity.adapter_key, set()):
            raise ValidationDomainError("live_provider_adapter_unconfigured")
        credential = await self._credential(identity.credential_ref, identity.profile_id)
        if identity.adapter_key in {"agentscope", "openai", "text"}:
            port: OpenAICompatibleTextModelAdapter | GPTImageProvider | AgnesVideoProvider = (
                OpenAICompatibleTextModelAdapter(
                    identity.base_url,
                    credential,
                    timeout_seconds=identity.timeout_seconds,
                    idempotency_key_header=identity.idempotency_key_header,
                    correlation_header=identity.correlation_header,
                )
            )
        elif identity.adapter_key in {"gpt", "gpt-image", "gpt_image"}:
            port = GPTImageProvider(
                configured=True,
                base_url=identity.base_url,
                api_key=credential,
                timeout_seconds=identity.timeout_seconds,
                idempotency_key_header=identity.idempotency_key_header,
                correlation_header=identity.correlation_header,
            )
        elif identity.adapter_key in {"agnes", "agnes_video"}:
            port = AgnesVideoProvider(
                configured=True,
                base_url=identity.base_url,
                api_key=credential,
                timeout_seconds=identity.timeout_seconds,
                idempotency_key_header=identity.idempotency_key_header,
                correlation_header=identity.correlation_header,
            )
        else:
            raise ValidationDomainError("live_provider_adapter_unconfigured")
        return ComposedLiveProvider(identity, port)

    async def resolve_remote_lookups(
        self, bindings: tuple[dict[str, object], ...]
    ) -> tuple[FrozenRemoteLookup, ...]:
        """Compose only persisted capability/operation/protocol lookup contracts.

        A provider transport without an explicit lookup method is deliberately
        excluded.  This keeps recovery in ``unknown`` rather than inventing a
        request protocol or submitting a replacement request.
        """
        lookups: list[FrozenRemoteLookup] = []
        for binding in bindings:
            required = {
                "profileId",
                "modelId",
                "profileRevision",
                "capabilitySnapshotId",
                "capabilityRevision",
                "operation",
                "protocol",
            }
            if (
                set(binding) != required
                or any(
                    not isinstance(binding[name], str) or not str(binding[name]).strip()
                    for name in {
                        "profileId",
                        "modelId",
                        "capabilitySnapshotId",
                        "operation",
                        "protocol",
                    }
                )
                or any(
                    isinstance(binding[name], bool)
                    or not isinstance(binding[name], int)
                    or cast(int, binding[name]) < 1
                    for name in {"profileRevision", "capabilityRevision"}
                )
            ):
                continue
            try:
                composed = await self.resolve_provider(
                    str(binding["profileId"]),
                    str(binding["modelId"]),
                    str(binding["operation"]),
                    expected_profile_revision=cast(int, binding["profileRevision"]),
                    expected_capability_snapshot_id=str(binding["capabilitySnapshotId"]),
                    expected_capability_revision=cast(int, binding["capabilityRevision"]),
                )
            except ValidationDomainError:
                continue
            if not callable(getattr(composed.port, "lookup_provider_request", None)):
                continue
            lookups.append(
                FrozenRemoteLookup(
                    str(binding["capabilitySnapshotId"]),
                    str(binding["operation"]),
                    str(binding["protocol"]),
                    cast(RemoteLookupPort, composed.port),
                    profile_id=str(binding["profileId"]),
                    model_id=str(binding["modelId"]),
                    profile_revision=cast(int, binding["profileRevision"]),
                    capability_revision=cast(int, binding["capabilityRevision"]),
                )
            )
        return tuple(lookups)

    async def resolve_storage(
        self,
        storage_profiles: Any,
        *,
        project_id: str,
        profile_id: str,
        expected_profile_revision: int,
        expected_bucket_binding_id: str,
        expected_identity: dict[str, object] | None = None,
        expected_capability: dict[str, int] | None = None,
        local_workspace_root: Path | None = None,
    ) -> ComposedStoragePort:
        """Resolve one private TOS profile for one project without a Local fallback."""
        if not expected_bucket_binding_id:
            raise ValidationDomainError("storage bucket binding is required")
        profile = await storage_profiles.resolve_upload_profile(
            project_id, profile_id, expected_profile_revision, project_id
        )
        if profile.bucket_binding_id != expected_bucket_binding_id:
            raise ValidationDomainError("storage bucket binding is stale or foreign")
        if profile.adapter_key == "local_workspace":
            if local_workspace_root is None:
                raise ValidationDomainError("storage profile composition is unconfigured")
            from video_agent_api.ports.storage import LocalWorkspaceAdapter

            port: Any = LocalWorkspaceAdapter(local_workspace_root)
            credential = ""
        elif profile.adapter_key == "tos" and profile.credential_ref:
            credential = await self._credential(profile.credential_ref, profile.id)
            port = None
        else:
            raise ValidationDomainError("storage profile is unconfigured")
        snapshot = {
            "bucketBindingId": profile.bucket_binding_id,
            "endpoint": profile.endpoint,
            "profileId": profile.id,
            "projectId": project_id,
            "region": profile.region,
            "revision": profile.revision,
        }
        capability = StorageCapability(profile.revision, 1, 64 * 1024 * 1024, 10_000, 8 * 1024**4)
        capability_payload = {
            "profileRevision": capability.profile_revision,
            "minPartSizeBytes": capability.min_part_size_bytes,
            "maxPartSizeBytes": capability.max_part_size_bytes,
            "maxPartCount": capability.max_part_count,
            "maxObjectSizeBytes": capability.max_object_size_bytes,
        }
        if expected_identity is not None:
            required = {
                "adapterKey",
                "profileId",
                "projectId",
                "revision",
                "bucketBindingId",
                "bucket",
                "endpoint",
                "region",
                "credentialRef",
            }
            actual = {
                "adapterKey": profile.adapter_key,
                "profileId": profile.id,
                "projectId": profile.project_id,
                "revision": profile.revision,
                "bucketBindingId": profile.bucket_binding_id,
                "bucket": profile.bucket,
                "endpoint": profile.endpoint,
                "region": profile.region,
                "credentialRef": profile.credential_ref,
            }
            if set(expected_identity) != required or expected_identity != actual:
                raise ValidationDomainError("storage profile identity changed after submission")
        if expected_capability is not None and expected_capability != capability_payload:
            raise ValidationDomainError("storage capability changed after submission")
        snapshot["capability"] = capability_payload
        identity = ResolvedStorageIdentity(
            profile.id,
            profile.revision,
            sha256(dumps(snapshot, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            profile.bucket_binding_id,
            project_id,
            profile.credential_ref or "",
            profile.bucket,
            profile.endpoint,
            profile.region,
            capability_payload,
        )
        from video_agent_api.ports.storage import TOSAdapter

        if port is None:
            port = TOSAdapter.from_profile(
                profile,
                _FrozenCredentialResolver(profile.credential_ref or "", profile.id, credential),
            )
        return ComposedStoragePort(
            identity,
            port,
        )

    async def resolve_renderer(
        self,
        *,
        profile_id: str | None,
        ffmpeg_path: str | None,
        ffprobe_path: str | None,
        renderer_factory: Callable[[str, str], Any],
    ) -> ComposedRendererPort:
        """Compose FFmpeg from one durable, runnable renderer catalog identity."""
        if not profile_id or not profile_id.strip() or not ffmpeg_path or not ffprobe_path:
            raise ValidationDomainError("renderer_unconfigured")
        async with self._resolver._uow_factory() as uow:
            profile = uow.profiles.get(profile_id.strip())
            provider = uow.providers.get(profile.provider_id) if profile is not None else None
            snapshot = (
                profile.capability_snapshots.get("media.render") if profile is not None else None
            )
            if (
                provider is None
                or profile is None
                or provider.approval != "approved"
                or provider.feature_gate != "MVP-A"
                or provider.adapter_key != "ffmpeg"
                or not provider.adapter_installed
                or not provider.enabled
                or profile.adapter_identity != "ffmpeg"
                or not profile.enabled
                or not profile.explicit_live_opt_in
                or snapshot is None
                or not snapshot.runnable
                or snapshot.provider_id != provider.id
                or snapshot.profile_id != profile.id
                or snapshot.operation != "media.render"
                or snapshot.model_id is not None
            ):
                raise ValidationDomainError("renderer_unconfigured")

        port = renderer_factory(ffmpeg_path, ffprobe_path)
        probe = getattr(port, "probe", None)
        capability: dict[str, object] = {}
        snapshot_id: str | None = None
        if callable(probe):
            try:
                raw = probe()
                capability = {
                    "ffmpegVersion": raw.ffmpeg_version,
                    "ffprobeVersion": raw.ffprobe_version,
                    "h264Decoder": raw.h264_decoder,
                    "h264Encoder": raw.h264_encoder,
                    "aacDecoder": raw.aac_decoder,
                    "aacEncoder": raw.aac_encoder,
                    "yuv420p": raw.yuv420p,
                    "mp4Muxer": raw.mp4_muxer,
                    "mp4Demuxer": raw.mp4_demuxer,
                }
                snapshot_id = sha256(
                    dumps(
                        {
                            "profileId": profile.id,
                            "profileRevision": profile.revision,
                            "capabilitySnapshotId": snapshot.id,
                            "capabilityRevision": snapshot.revision,
                            "capability": capability,
                        },
                        sort_keys=True,
                        separators=(",", ":"),
                    ).encode()
                ).hexdigest()
            except Exception as error:
                raise ValidationDomainError("renderer_unconfigured") from error
        return ComposedRendererPort(
            profile.id,
            profile.revision,
            snapshot.id,
            snapshot.revision,
            ffmpeg_path,
            ffprobe_path,
            port,
            snapshot_id,
            capability,
        )
