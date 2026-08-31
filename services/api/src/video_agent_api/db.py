"""数据库就绪探针；未配置数据库时明确报告未就绪。"""

from __future__ import annotations

import asyncio
import os
import socket
from dataclasses import dataclass
from pathlib import Path

from sqlalchemy import text
from sqlalchemy.exc import SQLAlchemyError
from sqlalchemy.ext.asyncio import create_async_engine

from video_agent_api.domain.errors import DomainError

CURRENT_MIGRATION_HEAD = "0029_lookup_binding"


@dataclass(frozen=True, slots=True)
class ReadinessRequirements:
    """Read-only prerequisites; command admission remains an independent owner gate."""

    database: bool
    migration_head: bool
    catalog_bootstrap: bool
    resource_probe: bool
    capacity_probe: bool
    queue: bool
    workspace: bool
    selected_capability: str = "ready"


@dataclass(frozen=True, slots=True)
class ReadinessAssessment:
    status: str
    diagnostics: tuple[str, ...]

    @property
    def ready(self) -> bool:
        return self.status == "ready"


def assess_readiness(requirements: ReadinessRequirements) -> ReadinessAssessment:
    """Classify every deployment prerequisite without writing catalog or business state."""
    missing = tuple(
        diagnostic
        for passed, diagnostic in (
            (requirements.database, "database_unavailable"),
            (requirements.migration_head, "migration_head_unavailable"),
            (requirements.catalog_bootstrap, "catalog_bootstrap_unavailable"),
            (requirements.resource_probe, "resource_probe_unavailable"),
            (requirements.capacity_probe, "capacity_probe_unavailable"),
            (requirements.queue, "queue_unavailable"),
            (requirements.workspace, "workspace_unavailable"),
        )
        if not passed
    )
    if missing:
        return ReadinessAssessment("not_ready", missing)
    if requirements.selected_capability == "renderer_unconfigured":
        return ReadinessAssessment("renderer_unconfigured", ("renderer_unconfigured",))
    if requirements.selected_capability == "renderer_capability_unsupported":
        return ReadinessAssessment(
            "renderer_capability_unsupported", ("renderer_capability_unsupported",)
        )
    if requirements.selected_capability != "ready":
        return ReadinessAssessment("unconfigured", ("selected_capability_unconfigured",))
    return ReadinessAssessment("ready", ())


async def check_database(database_url: str, *, expected_head: str | None = None) -> bool:
    """执行只读连通性与 migration-head 检查；内存 SQLite 保持测试可用。"""
    engine = create_async_engine(database_url)
    try:
        async with engine.connect() as connection:
            await connection.execute(text("SELECT 1"))
            if expected_head and ":memory:" not in database_url:
                row = await connection.execute(text("SELECT version_num FROM alembic_version"))
                if row.scalar_one_or_none() != expected_head:
                    return False
        return True
    finally:
        await engine.dispose()


async def check_catalog_bootstrap(database_url: str) -> bool:
    """Confirm the existing offline catalog seed without invoking bootstrap from readiness."""
    if ":memory:" in database_url:
        return True
    engine = create_async_engine(database_url)
    try:
        async with engine.connect() as connection:
            rows = await connection.execute(
                text(
                    "SELECT "
                    "EXISTS (SELECT 1 FROM providers WHERE adapter_key = 'mock'), "
                    "EXISTS (SELECT 1 FROM provider_profiles "
                    "WHERE adapter_identity = 'local_workspace'), "
                    "EXISTS (SELECT 1 FROM models)"
                )
            )
            row = rows.one()
            return all(bool(value) for value in row)
    except SQLAlchemyError:
        return False
    finally:
        await engine.dispose()


def _temporal_queue_reachable(address: str | None) -> bool:
    if not address or ":" not in address:
        return False
    host, port = address.rsplit(":", 1)
    try:
        with socket.create_connection((host, int(port)), timeout=1):
            return True
    except (OSError, ValueError):
        return False


def default_readiness_assessment() -> ReadinessAssessment:
    """Probe deployment preconditions only; this never bootstraps or admits a command."""
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        return assess_readiness(
            ReadinessRequirements(False, False, False, False, False, False, False)
        )
    try:
        expected_head: str | None = os.environ.get(
            "EXPECTED_MIGRATION_HEAD", CURRENT_MIGRATION_HEAD
        )
        if ":memory:" in database_url:
            expected_head = None
        database = asyncio.run(check_database(database_url, expected_head=expected_head))
        catalog = asyncio.run(check_catalog_bootstrap(database_url)) if database else False
        workspace = Path(os.environ.get("WORKSPACE_ROOT", "/tmp/video-agent-workspaces"))
        workspace_ready = workspace.is_dir()
        resource_ready = False
        capacity_ready = False
        if workspace_ready:
            from video_agent_api.resilience import capacity_snapshot, probe_resources

            resource = probe_resources(workspace)
            resource_ready = resource.error is None
            capacity_ready = capacity_snapshot(resource, "readiness").error is None
        capability = "ready"
        renderer_required = os.environ.get("RENDERER_REQUIRED", "0").lower() in {
            "1",
            "true",
            "yes",
        }
        renderer_profile_id = os.environ.get("RENDERER_PROFILE_ID", "").strip()
        # A renderer binary is not a renderer port.  Do not probe or invoke it
        # while the owning catalog composition is absent; doing so would let a
        # process-local environment claim live readiness.
        composition_requested = (
            os.environ.get("PROVIDER_MODE") == "live"
            or os.environ.get("STORAGE_MODE") == "tos"
            or bool(renderer_profile_id)
            or renderer_required
        )
        renderer_unconfigured = renderer_required and not renderer_profile_id
        if renderer_unconfigured:
            capability = "renderer_unconfigured"
        elif composition_requested:
            capability = "unconfigured"
        if capability == "ready":
            provider_references = (
                "PROVIDER_PROFILE_ID",
                "PROVIDER_MODEL_ID",
                "PROVIDER_CREDENTIAL_REF",
            )
            storage_references = (
                "STORAGE_PROFILE_ID",
                "STORAGE_BUCKET_BINDING_ID",
                "STORAGE_CREDENTIAL_REF",
            )
            live_provider_unconfigured = os.environ.get("PROVIDER_MODE") == "live" and any(
                not os.environ.get(reference) for reference in provider_references
            )
            tos_storage_unconfigured = os.environ.get("STORAGE_MODE") == "tos" and any(
                not os.environ.get(reference) for reference in storage_references
            )
            # RuntimeSettings only carries requested identifiers.  Until the selected
            # live provider/storage/renderer has been resolved through its owner
            # catalog and composed into a typed port, worker readiness must not turn
            # those strings into an operational claim.
            if live_provider_unconfigured or tos_storage_unconfigured:
                capability = "unconfigured"
        return assess_readiness(
            ReadinessRequirements(
                database=database,
                migration_head=database,
                catalog_bootstrap=catalog,
                resource_probe=resource_ready,
                capacity_probe=capacity_ready,
                queue=_temporal_queue_reachable(os.environ.get("TEMPORAL_ADDRESS")),
                workspace=workspace_ready,
                selected_capability=capability,
            )
        )
    except (DomainError, OSError, RuntimeError, SQLAlchemyError):
        return assess_readiness(
            ReadinessRequirements(False, False, False, False, False, False, False)
        )


def default_readiness_probe() -> bool:
    return default_readiness_assessment().ready
