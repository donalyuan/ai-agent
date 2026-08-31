"""Dispatch committed Run start facts to Temporal with stable workflow identity."""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, Protocol, cast

from video_agent_api.domain.runs import TemporalStart, WorkflowRun


@dataclass(frozen=True, slots=True)
class TemporalRunLaunch:
    project_id: str
    workflow_version_id: str
    start: TemporalStart
    selection_snapshot: dict[str, object]
    traceparent: str | None = None
    tracestate: str | None = None


class RunStarter(Protocol):
    async def start(self, launch: TemporalRunLaunch) -> object: ...


class RunDispatchService:
    """Consume the durable start ledger; retrying a pending row is idempotent in Temporal."""

    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def dispatch_pending(self, starter: RunStarter, *, limit: int = 100) -> dict[str, int]:
        async with self._uow_factory() as uow:
            pending = sorted(
                (
                    cast(TemporalStart, start)
                    for start in uow.temporal_starts.values()
                    if start.status == "pending"
                ),
                key=lambda start: (start.created_at, start.id),
            )[:limit]

        dispatched = 0
        failed = 0
        for snapshot in pending:
            async with self._uow_factory() as uow:
                run = cast(WorkflowRun | None, uow.workflow_runs.get(snapshot.run_id))
                current = cast(TemporalStart | None, uow.temporal_starts.get(snapshot.workflow_id))
                if run is None or current is None or current.status != "pending":
                    failed += 1
                    continue
                launch = TemporalRunLaunch(
                    run.project_id,
                    run.workflow_version_id,
                    current,
                    dict(run.selection_snapshot),
                )
                node = next((item for item in run.nodes if item.id == current.node_run_id), None)
                if node is not None and node.execution_route == "generation":
                    # Generation owns this route.  Mark the legacy start as drained so
                    # an old Agent dispatcher cannot claim it after a restart.
                    uow.temporal_starts[current.workflow_id] = replace(
                        current, status="reconciled", revision=current.revision + 1
                    )
                    await uow.commit()
                    continue
            try:
                await starter.start(launch)
            except Exception:
                # Keep the durable row pending. The stable workflow ID makes the next attempt safe.
                failed += 1
                continue
            async with self._uow_factory() as uow:
                current = cast(TemporalStart | None, uow.temporal_starts.get(snapshot.workflow_id))
                if current is not None and current.status == "pending":
                    uow.temporal_starts[current.workflow_id] = replace(
                        current, status="started", revision=current.revision + 1
                    )
                    run = cast(WorkflowRun | None, uow.workflow_runs.get(current.run_id))
                    node = (
                        next(
                            (item for item in run.nodes if item.id == current.node_run_id),
                            None,
                        )
                        if run is not None
                        else None
                    )
                    if run is not None and run.status == "running" and node is not None:
                        if node.status == "pending":
                            node.transition("running")
                    await uow.commit()
            dispatched += 1
        return {"dispatched": dispatched, "failed": failed}
