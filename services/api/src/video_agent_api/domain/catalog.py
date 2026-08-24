"""Provider/Model/Skill catalog owner facts and admission rules."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError
from .provider_ops import ProviderQuotaSnapshot

SKILL_CANDIDATES = (
    "drama-skills",
    "novel-writing",
    "zy-cinematic-realism",
    "seedance-2.0",
    "storyboard-tiktok-video-skill",
    "hell-grind/cinedance-higgsfield",
    "hell-grind/acting",
    "hell-grind/lira",
)

SKILL_SOURCE_METADATA: dict[str, tuple[str, str, str, str]] = {
    "drama-skills": (
        "git",
        "https://github.com/worldwonderer/drama-skills@c5eec4ccff385b999ba5e4d0d870095b37e22e26",
        "a7217641a828b8c52778b9c2e0a19772c0faa0e830bc57ffa48e362a9a748b9f",
        "verified",
    ),
    "novel-writing": (
        "git",
        "https://github.com/wgwtest/novel-writing@805de0f36544f4ca19f258890924796ffef5af8b",
        "63ad1206cdcd49aad86a96ffbd2d49f1b0b56a45cceea7cc09b190ceef0cbcce",
        "verified",
    ),
    "zy-cinematic-realism": (
        "git",
        "https://github.com/popopo-99/zy-cinematic-realism",
        "",
        "pending_commercial_review",
    ),
    "seedance-2.0": ("git", "https://github.com/Emily2040/seedance-2.0", "", "pending"),
    "storyboard-tiktok-video-skill": (
        "git",
        "https://github.com/huifer/storyboard-tiktok-video-skill",
        "",
        "pending",
    ),
    "hell-grind/cinedance-higgsfield": (
        "public_markdown",
        "https://hellgrind.com/skills/cinedance-higgsfield",
        "",
        "pending",
    ),
    "hell-grind/acting": (
        "public_markdown",
        "https://hellgrind.com/skills/acting",
        "",
        "pending",
    ),
    "hell-grind/lira": (
        "public_markdown",
        "https://hellgrind.com/skills/lira",
        "",
        "pending",
    ),
}


@dataclass(frozen=True, slots=True)
class CapabilitySnapshot:
    provider_id: str
    profile_id: str
    operation: str
    revision: int
    runnable: bool
    capabilities: tuple[str, ...]
    captured_at: str
    model_id: str | None = None
    id: str = field(default_factory=lambda: str(uuid4()))
    retention_policy: str = "long-term-audit"
    retention_version: str = "1"
    hold: bool = False


@dataclass(slots=True)
class Provider:
    name: str
    adapter_key: str
    approval: Literal["approved", "pending", "rejected"] = "pending"
    feature_gate: Literal["MVP-A", "MVP-B"] = "MVP-A"
    adapter_installed: bool = False
    enabled: bool = False
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1

    def update(self, expected_revision: int, **changes: object) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        for key, value in changes.items():
            if not hasattr(self, key):
                raise ValidationDomainError(f"unknown provider field: {key}")
            setattr(self, key, value)
        self.revision += 1


@dataclass(slots=True)
class ProviderProfile:
    provider_id: str
    name: str
    adapter_identity: str = "local_workspace"
    enabled: bool = False
    explicit_live_opt_in: bool = False
    credential_status: str = "unconfigured"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    operation_policies: dict[str, dict[str, object]] = field(default_factory=dict)
    capability_snapshots: dict[str, CapabilitySnapshot] = field(default_factory=dict)
    quota_snapshots: dict[str, ProviderQuotaSnapshot] = field(default_factory=dict)
    active_operations: dict[str, int] = field(default_factory=dict)
    request_windows: dict[str, tuple[int, float]] = field(default_factory=dict)

    def update(self, expected_revision: int, **changes: object) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        for key, value in changes.items():
            if not hasattr(self, key):
                raise ValidationDomainError(f"unknown profile field: {key}")
            setattr(self, key, value)
        self.revision += 1

    def admit(self, operation: str, now: float, *, live: bool = False) -> None:
        validate_profile_operation(self, operation)
        policy = self.operation_policies.get(operation, {})
        max_concurrency = int(str(policy.get("maxConcurrency", 1)))
        active = self.active_operations.get(operation, 0)
        if active >= max_concurrency:
            raise ValidationDomainError("provider_operation_concurrency_exhausted")
        limit = int(str(policy.get("rateLimit", 60)))
        window = int(str(policy.get("rateWindowSeconds", 60)))
        previous_count, started = self.request_windows.get(operation, (0, now))
        if now - started >= window:
            previous_count, started = 0, now
        if previous_count >= limit:
            raise ValidationDomainError("provider_operation_rate_limited")
        quota = self.quota_snapshots.get(operation)
        if quota is not None and quota.status == "exhausted":
            raise ValidationDomainError("provider_quota_exhausted")
        if live and not self.explicit_live_opt_in:
            raise ValidationDomainError("live_provider_unconfigured")
        self.active_operations[operation] = active + 1
        self.request_windows[operation] = (previous_count + 1, started)

    def release(self, operation: str) -> None:
        active = self.active_operations.get(operation, 0)
        if active <= 1:
            self.active_operations.pop(operation, None)
        else:
            self.active_operations[operation] = active - 1


@dataclass(slots=True)
class Model:
    profile_id: str
    model_key: str
    enabled: bool = False
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    historical_references: int = 0

    def update(self, expected_revision: int, **changes: object) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        for key, value in changes.items():
            if key not in {"model_key", "enabled"}:
                raise ValidationDomainError(f"unknown model field: {key}")
            setattr(self, key, value)
        self.revision += 1

    def disable_or_delete(self, expected_revision: int, delete: bool = False) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if delete and self.historical_references:
            raise ValidationDomainError("historically referenced model is disable-only")
        self.enabled = False
        self.revision += 1


@dataclass(frozen=True, slots=True)
class SkillRevisionRecord:
    name: str
    version: str
    provenance: Literal["verified_snapshot", "pending_provenance"]
    approval: Literal["approved", "not_approved"]
    enabled: bool
    source_identity: str
    digest: str
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"
    revision: int = 1
    source_type: Literal["git", "public_markdown"] = "git"
    license_status: str = "verified"
    capabilities: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class ModelSyncCandidate:
    profile_id: str
    remote_models: tuple[str, ...]
    added: tuple[str, ...]
    removed: tuple[str, ...]
    changed: tuple[str, ...]
    status: Literal["pending", "accepted", "rejected"] = "pending"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))


@dataclass(frozen=True, slots=True)
class SkillAccessAudit:
    skill_revision_id: str
    run_id: str
    node_run_id: str
    access: Literal["metadata", "content", "reference", "network", "file", "subprocess", "secret"]
    allowed: bool
    reason: str
    id: str = field(default_factory=lambda: str(uuid4()))


def default_skill_revisions() -> tuple[SkillRevisionRecord, ...]:
    records: list[SkillRevisionRecord] = []
    for name in SKILL_CANDIDATES:
        source_type, source_identity, digest, license_status = SKILL_SOURCE_METADATA[name]
        approved = name in {"drama-skills", "novel-writing"}
        records.append(
            SkillRevisionRecord(
                name,
                "1.0.0",
                "verified_snapshot" if approved else "pending_provenance",
                "approved" if approved else "not_approved",
                approved,
                source_identity,
                digest,
                source_type=source_type,  # type: ignore[arg-type]
                license_status=license_status,
            )
        )
    return tuple(records)


def admit_operation(
    provider: Provider, profile: ProviderProfile, operation: str, *, live: bool
) -> CapabilitySnapshot:
    if (
        provider.approval != "approved"
        or provider.feature_gate != "MVP-A"
        or not provider.adapter_installed
    ):
        raise ValidationDomainError("provider_unconfigured")
    if live and (not profile.explicit_live_opt_in or profile.credential_status != "configured"):
        raise ValidationDomainError("live_provider_unconfigured")
    snapshot = profile.capability_snapshots.get(operation)
    if live and (snapshot is None or not snapshot.runnable):
        raise ValidationDomainError("capability_snapshot_unavailable")
    return snapshot or CapabilitySnapshot(
        provider.id, profile.id, operation, profile.revision, True, ("mock",), "local"
    )


def validate_profile_operation(profile: ProviderProfile, operation: str) -> None:
    policy = profile.operation_policies.get(operation)
    if policy is None:
        return
    if int(str(policy.get("maxConcurrency", 1))) < 1 or int(str(policy.get("rateLimit", 1))) < 1:
        raise ValidationDomainError("operation policy is invalid")
    quota = profile.quota_snapshots.get(operation)
    if quota is not None and quota.status == "exhausted":
        raise ValidationDomainError("provider_quota_exhausted")
