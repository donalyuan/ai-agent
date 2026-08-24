"""Resource admission and explicit checksum/ETag recovery evidence."""

from __future__ import annotations

import os
import shutil
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from uuid import UUID


@dataclass(frozen=True, slots=True)
class RuntimeResourceSnapshot:
    cpu_count: int
    available_concurrency: int
    memory_available_bytes: int | None
    memory_limit_bytes: int | None
    disk_free_bytes: int | None
    disk_total_bytes: int | None
    captured_at: str
    source: str = "local"
    config_revision: int = 1
    revision: int = 1
    error: str | None = None
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class CapacitySnapshot:
    scope: str
    observed_bytes: int | None
    limit_bytes: int | None
    available_bytes: int | None
    captured_at: str
    source: str
    config_revision: int = 1
    revision: int = 1
    error: str | None = None
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class OperationAdmission:
    allowed: bool
    diagnostic: str | None
    warning: str | None = None
    snapshot_revision: int | None = None
    observed: int | None = None
    required: int | None = None
    source: str | None = None
    operation_key: str | None = None
    correlation_id: str | None = None


@dataclass(frozen=True, slots=True)
class CapacityAggregate:
    """只读合并各 owner 资源观测；不执行 cleanup 或 adapter 切换。"""

    snapshots: tuple[CapacitySnapshot, ...]
    observed_bytes: int | None
    limit_bytes: int | None
    available_bytes: int | None
    captured_at: str
    revision: int
    error: str | None = None


@dataclass(frozen=True, slots=True)
class RecoveryCheck:
    status: str
    missing_requirements: tuple[str, ...] = ()
    diagnostic: str | None = None


def aggregate_capacity(snapshots: Iterable[CapacitySnapshot]) -> CapacityAggregate:
    items = tuple(snapshots)
    if not items:
        return CapacityAggregate(
            (), None, None, None, datetime.now(UTC).isoformat(), 0, "capacity_probe_unavailable"
        )
    errors = tuple(item.error for item in items if item.error)
    observed_values = [item.observed_bytes for item in items if item.observed_bytes is not None]
    limit_values = [item.limit_bytes for item in items if item.limit_bytes is not None]
    available_values = [item.available_bytes for item in items if item.available_bytes is not None]
    return CapacityAggregate(
        items,
        sum(observed_values) if observed_values else None,
        sum(limit_values) if limit_values else None,
        sum(available_values) if available_values else None,
        max(item.captured_at for item in items),
        max(item.revision for item in items),
        "; ".join(errors) if errors else None,
    )


def check_recovery_requirements(
    *, required: Iterable[str], available: Iterable[str]
) -> RecoveryCheck:
    missing = tuple(sorted(set(required).difference(available)))
    return RecoveryCheck(
        "blocked" if missing else "ready",
        missing,
        "restore_requirements_missing" if missing else None,
    )


def cleanup_allowed(*, referenced: bool, hold: bool, retention_expired: bool) -> bool:
    """容量维护仅可清理无引用、无 hold 且已过期的临时对象。"""
    return retention_expired and not referenced and not hold


def probe_resources(path: Path) -> RuntimeResourceSnapshot:
    captured_at = datetime.now(UTC).isoformat()
    cpu_count = os.cpu_count() or 1
    memory: int | None = None
    memory_limit: int | None = None
    try:
        pages = os.sysconf("SC_AVPHYS_PAGES")
        page_size = os.sysconf("SC_PAGE_SIZE")
        memory = int(pages * page_size)
        total_pages = os.sysconf("SC_PHYS_PAGES")
        memory_limit = int(total_pages * page_size)
    except (OSError, ValueError):
        pass
    try:
        usage = shutil.disk_usage(path)
    except OSError as error:
        return RuntimeResourceSnapshot(
            cpu_count,
            cpu_count,
            memory,
            memory_limit,
            None,
            None,
            captured_at,
            error=f"{type(error).__name__}: resource probe failed",
        )
    return RuntimeResourceSnapshot(
        cpu_count,
        cpu_count,
        memory,
        memory_limit,
        usage.free,
        usage.total,
        captured_at,
    )


def capacity_snapshot(snapshot: RuntimeResourceSnapshot, scope: str) -> CapacitySnapshot:
    observed = (
        snapshot.disk_total_bytes - snapshot.disk_free_bytes
        if snapshot.disk_total_bytes is not None and snapshot.disk_free_bytes is not None
        else None
    )
    return CapacitySnapshot(
        scope,
        observed,
        snapshot.disk_total_bytes,
        snapshot.disk_free_bytes,
        snapshot.captured_at,
        snapshot.source,
        snapshot.config_revision,
        snapshot.revision,
        snapshot.error,
    )


def admit(
    snapshot: RuntimeResourceSnapshot,
    *,
    required_bytes: int = 0,
    min_cpu: int = 1,
    min_memory_bytes: int = 0,
    soft_ratio: float = 0.1,
    hard_ratio: float = 0.05,
    operation_key: str | None = None,
    correlation_id: str | None = None,
) -> OperationAdmission:
    if snapshot.error or snapshot.disk_free_bytes is None or snapshot.disk_total_bytes is None:
        return OperationAdmission(
            False,
            "resource_probe_unavailable",
            snapshot_revision=snapshot.revision,
            source=snapshot.source,
            operation_key=operation_key,
            correlation_id=correlation_id,
        )
    if snapshot.cpu_count < min_cpu or (
        snapshot.memory_available_bytes is not None
        and snapshot.memory_available_bytes < min_memory_bytes
    ):
        observed = min(snapshot.cpu_count, snapshot.memory_available_bytes or snapshot.cpu_count)
        return OperationAdmission(
            False,
            "resource_capability_unsupported",
            snapshot_revision=snapshot.revision,
            observed=observed,
            required=max(min_cpu, min_memory_bytes),
            source=snapshot.source,
            operation_key=operation_key,
            correlation_id=correlation_id,
        )
    remaining = snapshot.disk_free_bytes - required_bytes
    if remaining < 0 or remaining / snapshot.disk_total_bytes <= hard_ratio:
        return OperationAdmission(
            False,
            "resource_capacity_hard_limit",
            snapshot_revision=snapshot.revision,
            observed=snapshot.disk_free_bytes,
            required=required_bytes,
            source=snapshot.source,
            operation_key=operation_key,
            correlation_id=correlation_id,
        )
    if remaining / snapshot.disk_total_bytes <= soft_ratio:
        return OperationAdmission(
            True,
            None,
            "resource_capacity_soft_limit",
            snapshot.revision,
            snapshot.disk_free_bytes,
            required_bytes,
            snapshot.source,
            operation_key,
            correlation_id,
        )
    return OperationAdmission(
        True,
        None,
        snapshot_revision=snapshot.revision,
        observed=snapshot.disk_free_bytes,
        required=required_bytes,
        source=snapshot.source,
        operation_key=operation_key,
        correlation_id=correlation_id,
    )


@dataclass(frozen=True, slots=True)
class BackupRestoreMetadata:
    operation_key: str
    status: str
    backup_fingerprint: str
    manifest_revision: int
    operator_uuid: str
    correlation_id: str
    missing_requirements: tuple[str, ...] = ()
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class RestoreEvidence:
    operation_key: str
    status: str
    expected_checksum: str
    observed_checksum: str
    expected_etag: str
    observed_etag: str
    manifest_revision: int
    operator_uuid: str | None = None
    correlation_id: str | None = None
    captured_at: str = ""
    diagnostic: str | None = None
    schema_version: str = "1.0.0"


def verify_restore(
    operation_key: str,
    expected_checksum: str,
    observed_checksum: str,
    expected_etag: str,
    observed_etag: str,
    manifest_revision: int,
    *,
    operator_uuid: str | None = None,
    correlation_id: str | None = None,
) -> RestoreEvidence:
    diagnostic: str | None = None
    if not operation_key or manifest_revision < 1:
        diagnostic = "restore_manifest_invalid"
    if operator_uuid is not None:
        try:
            UUID(operator_uuid)
        except ValueError:
            diagnostic = "restore_operator_invalid"
    if not expected_checksum or not expected_etag or not observed_checksum or not observed_etag:
        diagnostic = "restore_object_metadata_missing"
    elif expected_checksum != observed_checksum or expected_etag != observed_etag:
        diagnostic = "restore_object_identity_mismatch"
    status = "passed" if diagnostic is None else "failed"
    return RestoreEvidence(
        operation_key,
        status,
        expected_checksum,
        observed_checksum,
        expected_etag,
        observed_etag,
        manifest_revision,
        operator_uuid,
        correlation_id,
        datetime.now(UTC).isoformat(),
        diagnostic,
    )
