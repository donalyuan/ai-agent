from __future__ import annotations

import json
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest
from alembic.config import Config
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from alembic import command
from video_agent_api.adapters.sqlalchemy import _encode_phase_one, make_sqlalchemy_uow_factory
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.runs import EnsureWorkflowCommand, RunsService
from video_agent_api.domain.errors import RevisionConflictError
from video_agent_api.domain.runs import ProjectDefaultWorkflowBinding, WorkflowVersion
from video_agent_api.skills.router import RankedSkill, RouteDecision

API_ROOT = Path(__file__).parents[1]
CURRENT_HEAD = "0029_lookup_binding"
WORKFLOW_TABLES = {
    "published_workflow_versions",
    "project_default_workflow_bindings",
    "workflow_runs",
    "workflow_node_runs",
    "workflow_run_input_snapshots",
    "workflow_run_events",
    "workflow_idempotency_keys",
    "workflow_temporal_starts",
    "workflow_budget_gates",
    "workflow_outbox_events",
}
WORKFLOW_DOCUMENT_COLLECTIONS = {
    "workflow_by_project",
    "workflow_bindings",
    "workflow_runs",
    "workflow_run_keys",
    "workflow_run_key_fingerprints",
    "workflow_signal_keys",
    "run_events",
    "run_input_snapshots",
    "budget_gates",
    "temporal_starts",
}


def _config(database_url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    return config


async def _prepare_run_owners(factory: Any, project_id: str) -> None:
    await CatalogService(factory).bootstrap()
    async with factory() as uow:
        skill = next(item for item in uow.skills if item.name == "novel-writing")
        ranked = RankedSkill(skill.name, skill.version, 10, skill.digest)
        decision = RouteDecision(
            (ranked,),
            ranked,
            False,
            None,
            ("deterministic_filter", "policy_decide"),
            f"route-{project_id}",
            1,
            f"fingerprint-{project_id}",
            project_id,
            "text.generate",
            f"launch-{project_id}",
        )
        uow.skill_route_decisions[decision.id] = decision
        await uow.commit()


def test_workflows_runs_migration_cycle_and_constraints(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'workflows-owner.db'}"
    config = _config(database_url)
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    assert WORKFLOW_TABLES <= set(inspect(engine).get_table_names())
    with engine.begin() as connection:
        connection.execute(text("PRAGMA foreign_keys = ON"))
        connection.execute(
            text(
                "INSERT INTO projects (id, revision, schema_version, name, status) "
                "VALUES ('project-wf', 1, '1.0.0', 'Workflow', 'draft')"
            )
        )
        source = {
            "id": "source-wf",
            "revision": 1,
            "schema_version": "1.0.0",
            "project_id": "project-wf",
            "template_key": "drama-mvp-a-default",
            "version_number": 1,
            "status": "published",
            "scope_type": "project",
            "scope_ids": '["project-wf"]',
            "definition": '{"nodes":[]}',
            "content_hash": "a" * 64,
        }
        columns = ", ".join(source)
        values = ", ".join(f":{key}" for key in source)
        connection.execute(
            text(f"INSERT INTO published_workflow_versions ({columns}) VALUES ({values})"),
            source,
        )
        with pytest.raises(IntegrityError):
            connection.execute(
                text(f"INSERT INTO published_workflow_versions ({columns}) VALUES ({values})"),
                {**source, "id": "source-bad", "content_hash": "z" * 64},
            )
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
            == CURRENT_HEAD
        )
    command.downgrade(config, "0014_asset_bible_owner")
    assert (WORKFLOW_TABLES - {"workflow_runs"}).isdisjoint(inspect(engine).get_table_names())
    command.upgrade(config, "head")
    engine.dispose()


async def test_workflows_runs_relational_round_trip_and_document_boundary(
    tmp_path: Path,
) -> None:
    sync_url = f"sqlite:///{tmp_path / 'workflows-round-trip.db'}"
    command.upgrade(_config(sync_url), "head")
    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    project = await ProjectsEpisodesService(factory).create_project("P")
    await _prepare_run_owners(factory, project.id)
    service = RunsService(factory)
    workflow = await service.ensure_workflow(
        EnsureWorkflowCommand(project.id, scope_ids=(project.id,))
    )
    run = await service.start_run(
        project.id,
        workflow.id,
        ["text.generate"],
        idempotency_key="sql-start",
    )
    node = run.nodes[0]
    await service.transition_node(run.id, node.id, "running")
    await service.transition_node(run.id, node.id, "waiting_review")

    detail = await service.detail(run.id, project.id)
    events = await service.events(run.id, 0, project.id)
    snapshots = await service.list_input_snapshots(project.id)
    assert detail["status"] == "waiting_review"
    assert [event.sequence for event in events] == list(range(1, len(events) + 1))
    assert snapshots[0]["runId"] == run.id
    await async_engine.dispose()

    engine = create_engine(sync_url)
    with engine.connect() as connection:
        counts = {
            table: connection.execute(text(f"SELECT COUNT(*) FROM {table}")).scalar_one()
            for table in WORKFLOW_TABLES
        }
        collections = set(
            connection.execute(
                text("SELECT collection FROM phase_one_documents WHERE owner = 'phase-one'")
            ).scalars()
        )
    assert counts["published_workflow_versions"] == 1
    assert counts["project_default_workflow_bindings"] == 1
    assert counts["workflow_runs"] == 1
    assert counts["workflow_node_runs"] == 1
    assert counts["workflow_run_input_snapshots"] == 1
    assert counts["workflow_run_events"] == 3
    assert counts["workflow_temporal_starts"] == 1
    assert counts["workflow_outbox_events"] == 3
    assert collections.isdisjoint(WORKFLOW_DOCUMENT_COLLECTIONS)
    engine.dispose()


async def test_workflow_node_row_level_cas_rejects_second_concurrent_uow(
    tmp_path: Path,
) -> None:
    sync_url = f"sqlite:///{tmp_path / 'workflows-cas.db'}"
    command.upgrade(_config(sync_url), "head")
    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    project = await ProjectsEpisodesService(factory).create_project("CAS")
    await _prepare_run_owners(factory, project.id)
    service = RunsService(factory)
    workflow = await service.ensure_workflow(
        EnsureWorkflowCommand(project.id, scope_ids=(project.id,))
    )
    run = await service.start_run(project.id, workflow.id, ["text.generate"])

    first = factory()
    second = factory()
    await first.__aenter__()
    await second.__aenter__()
    try:
        first.workflow_runs[run.id].nodes[0].transition("running")
        second.workflow_runs[run.id].nodes[0].transition("running")
        await first.commit()
        with pytest.raises(RevisionConflictError):
            await second.commit()
        await second.rollback()
    finally:
        await first.__aexit__(None, None, None)
        await second.__aexit__(None, None, None)
    assert (await service.detail(run.id))["nodes"][0]["status"] == "running"
    await async_engine.dispose()


async def test_workflow_legacy_document_fallback_is_migrated_once(tmp_path: Path) -> None:
    sync_url = f"sqlite:///{tmp_path / 'workflows-legacy.db'}"
    config = _config(sync_url)
    command.upgrade(config, "0014_asset_bible_owner")
    project_id = str(uuid4())
    source = WorkflowVersion(
        project_id,
        scope_ids=(project_id,),
        definition={"nodes": [{"key": "text.generate"}]},
    )
    binding = ProjectDefaultWorkflowBinding(project_id, source.id, source.content_hash)
    legacy = {
        "workflow_by_project": {project_id: source},
        "workflow_bindings": {project_id: binding},
        "workflow_runs": {},
        "workflow_run_keys": {},
        "workflow_run_key_fingerprints": {},
        "workflow_signal_keys": {},
        "run_events": {},
        "run_input_snapshots": {},
        "budget_gates": {},
        "temporal_starts": {},
    }
    engine = create_engine(sync_url)
    with engine.begin() as connection:
        connection.execute(
            text(
                "INSERT INTO projects (id, revision, schema_version, name, status) "
                "VALUES (:id, 1, '1.0.0', 'Legacy', 'draft')"
            ),
            {"id": project_id},
        )
        for collection, value in legacy.items():
            connection.execute(
                text(
                    "INSERT INTO phase_one_documents "
                    "(id, owner, collection, revision, document, retention_policy, "
                    "retention_version, hold) VALUES "
                    "(:id, 'phase-one', :collection, 0, :document, 'phase-one', '1', 0)"
                ),
                {
                    "id": str(uuid4()),
                    "collection": collection,
                    "document": json.dumps({"payload": _encode_phase_one(value)}),
                },
            )
    engine.dispose()
    command.upgrade(config, "head")

    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    async with factory() as uow:
        assert uow.workflow_by_project[project_id].id == source.id
        await uow.commit()
    await async_engine.dispose()

    engine = create_engine(sync_url)
    with engine.connect() as connection:
        assert (
            connection.execute(
                text("SELECT COUNT(*) FROM published_workflow_versions")
            ).scalar_one()
            == 1
        )
        collections = set(
            connection.execute(
                text("SELECT collection FROM phase_one_documents WHERE owner = 'phase-one'")
            ).scalars()
        )
    assert collections.isdisjoint(WORKFLOW_DOCUMENT_COLLECTIONS)
    engine.dispose()
