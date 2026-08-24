"""Persistent export outbox dispatcher."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal, Protocol, cast

from video_agent_api.domain.exports import (
    ExportDispatchOutbox,
)


@dataclass(frozen=True, slots=True)
class TemporalExportLaunch:
    project_id: str
    batch_id: str
    job_id: str
    logical_operation: str
    workflow_id: str
    schema_version: str = "1.0.0"

    @classmethod
    def from_outbox(cls, event: ExportDispatchOutbox) -> TemporalExportLaunch:
        return cls(
            event.project_id,
            event.batch_id,
            event.job_id,
            event.logical_operation,
            event.workflow_id,
            event.schema_version,
        )


@dataclass(frozen=True, slots=True)
class TemporalExportLaunchResult:
    workflow_id: str
    status: Literal["started", "already_started"]


class ExportStarter(Protocol):
    async def start(self, launch: Any) -> Any: ...


class ExportDispatchService:
    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def dispatch_pending(self, starter: ExportStarter, *, limit: int = 100) -> dict[str, int]:
        async with self._uow_factory() as uow:
            pending = sorted(
                (
                    cast(ExportDispatchOutbox, event)
                    for event in uow.export_dispatch_outbox.values()
                    if event.status == "pending"
                ),
                key=lambda event: event.id,
            )[:limit]

        dispatched = 0
        failed = 0
        for snapshot in pending:
            try:
                await starter.start(TemporalExportLaunch.from_outbox(snapshot))
            except Exception as error:
                async with self._uow_factory() as uow:
                    current = cast(ExportDispatchOutbox, uow.export_dispatch_outbox[snapshot.id])
                    if current.status == "pending":
                        current.failed_attempt(str(error))
                        await uow.commit()
                failed += 1
                continue
            async with self._uow_factory() as uow:
                current = cast(ExportDispatchOutbox, uow.export_dispatch_outbox[snapshot.id])
                if current.status == "pending":
                    current.dispatched()
                    await uow.commit()
            dispatched += 1
        return {"dispatched": dispatched, "failed": failed}
