"""add relational workflows/runs owner facts

Revision ID: 0015_workflows_runs_owner
Revises: 0014_asset_bible_owner
"""

from __future__ import annotations

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0015_workflows_runs_owner"
down_revision = "0014_asset_bible_owner"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")
RUN_STATUSES = (
    "'queued', 'running', 'waiting_review', 'succeeded', 'failed', 'cancel_requested', 'cancelled'"
)
NODE_STATUSES = (
    "'pending', 'running', 'waiting_review', 'succeeded', 'failed', "
    "'cancel_requested', 'cancelled', 'skipped'"
)


def _hex64(column: str) -> str:
    stripped = f"lower({column})"
    for character in "0123456789abcdef":
        stripped = f"replace({stripped}, '{character}', '')"
    return f"length({column}) = 64 AND length({stripped}) = 0"


def _identity() -> list[sa.Column[object]]:
    return [
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column(
            "created_at", sa.DateTime(timezone=True), nullable=False, server_default=sa.func.now()
        ),
        sa.Column(
            "updated_at", sa.DateTime(timezone=True), nullable=False, server_default=sa.func.now()
        ),
    ]


def upgrade() -> None:
    op.create_table(
        "published_workflow_versions",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("template_key", sa.String(128), nullable=False),
        sa.Column("version_number", sa.Integer(), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("scope_type", sa.String(32), nullable=False),
        sa.Column("scope_ids", JSON_DOCUMENT, nullable=False),
        sa.Column("definition", JSON_DOCUMENT, nullable=False),
        sa.Column("content_hash", sa.String(64), nullable=False),
        sa.UniqueConstraint(
            "project_id", "template_key", "version_number", name="uq_published_workflow_source"
        ),
        sa.UniqueConstraint("id", "project_id", name="uq_published_workflow_id_project"),
        sa.CheckConstraint("revision >= 1", name="ck_published_workflow_revision_positive"),
        sa.CheckConstraint("version_number >= 1", name="ck_published_workflow_version_positive"),
        sa.CheckConstraint("status = 'published'", name="ck_published_workflow_status"),
        sa.CheckConstraint("scope_type = 'project'", name="ck_published_workflow_scope_type"),
        sa.CheckConstraint(_hex64("content_hash"), name="ck_published_workflow_content_hash"),
    )
    op.create_index(
        "ix_published_workflow_versions_project_id",
        "published_workflow_versions",
        ["project_id"],
    )

    op.create_table(
        "project_default_workflow_bindings",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("workflow_version_id", sa.String(36), nullable=False),
        sa.Column("workflow_content_hash", sa.String(64), nullable=False),
        sa.Column("template_key", sa.String(128), nullable=False),
        sa.ForeignKeyConstraint(
            ["workflow_version_id", "project_id"],
            ["published_workflow_versions.id", "published_workflow_versions.project_id"],
            name="fk_default_workflow_binding_source_project",
        ),
        sa.UniqueConstraint("project_id", name="uq_default_workflow_binding_project"),
        sa.CheckConstraint("revision >= 1", name="ck_default_workflow_binding_revision"),
        sa.CheckConstraint(
            "template_key = 'drama-mvp-a-default'", name="ck_default_workflow_template_key"
        ),
        sa.CheckConstraint(
            _hex64("workflow_content_hash"), name="ck_default_workflow_content_hash"
        ),
    )

    with op.batch_alter_table("workflow_runs") as batch:
        batch.add_column(sa.Column("workflow_version_id", sa.String(36), nullable=True))
        batch.add_column(sa.Column("rerun_of_run_id", sa.String(36), nullable=True))
        batch.add_column(sa.Column("predecessor_run_id", sa.String(36), nullable=True))
        batch.add_column(sa.Column("input_snapshot", JSON_DOCUMENT, nullable=True))
        batch.add_column(
            sa.Column("selection_snapshot", JSON_DOCUMENT, nullable=False, server_default="{}")
        )
        batch.add_column(
            sa.Column("source_snapshot", JSON_DOCUMENT, nullable=False, server_default="{}")
        )
        batch.create_foreign_key(
            "fk_workflow_runs_published_source",
            "published_workflow_versions",
            ["workflow_version_id"],
            ["id"],
        )
        batch.create_foreign_key(
            "fk_workflow_runs_rerun_source", "workflow_runs", ["rerun_of_run_id"], ["id"]
        )
        batch.create_foreign_key(
            "fk_workflow_runs_predecessor", "workflow_runs", ["predecessor_run_id"], ["id"]
        )
        batch.create_check_constraint("ck_workflow_runs_status", f"status IN ({RUN_STATUSES})")
        batch.create_check_constraint("ck_workflow_runs_revision_positive", "revision >= 1")
    op.create_index(
        "ix_workflow_runs_workflow_version_id", "workflow_runs", ["workflow_version_id"]
    )

    op.create_table(
        "workflow_node_runs",
        *_identity(),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("node_key", sa.String(128), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("scope_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("output_evidence", JSON_DOCUMENT, nullable=True),
        sa.Column("failure", JSON_DOCUMENT, nullable=True),
        sa.Column("submission_state", sa.String(32), nullable=False),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_node_run_logical_operation"),
        sa.UniqueConstraint("id", "run_id", name="uq_node_run_id_run"),
        sa.CheckConstraint(f"status IN ({NODE_STATUSES})", name="ck_node_run_status"),
        sa.CheckConstraint("revision >= 1", name="ck_node_run_revision_positive"),
        sa.CheckConstraint(
            "submission_state IN ("
            "'not_submitted', 'submitted', 'submission_unknown', 'reconciled')",
            name="ck_node_run_submission_state",
        ),
    )
    op.create_index("ix_workflow_node_runs_run_id", "workflow_node_runs", ["run_id"])

    op.create_table(
        "workflow_run_input_snapshots",
        *_identity(),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("workflow_version_id", sa.String(36), nullable=False),
        sa.Column("workflow_content_hash", sa.String(64), nullable=False),
        sa.Column("scope_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("owner_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("selection_snapshot", JSON_DOCUMENT, nullable=False),
        sa.Column("source_snapshot", JSON_DOCUMENT, nullable=False),
        sa.Column("node_inputs", JSON_DOCUMENT, nullable=False),
        sa.Column("runnable", sa.Boolean(), nullable=False),
        sa.Column("diagnostic", sa.String(255), nullable=True),
        sa.ForeignKeyConstraint(
            ["workflow_version_id", "project_id"],
            ["published_workflow_versions.id", "published_workflow_versions.project_id"],
            name="fk_run_snapshot_source_project",
        ),
        sa.UniqueConstraint("run_id", "id", name="uq_run_input_snapshot_run_id"),
        sa.CheckConstraint("revision >= 1", name="ck_run_input_snapshot_revision"),
        sa.CheckConstraint(_hex64("workflow_content_hash"), name="ck_run_snapshot_content_hash"),
    )
    op.create_index(
        "ix_workflow_run_input_snapshots_project_id",
        "workflow_run_input_snapshots",
        ["project_id"],
    )

    op.create_table(
        "workflow_run_events",
        *_identity(),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("node_run_id", sa.String(36), nullable=True),
        sa.Column("sequence", sa.Integer(), nullable=False),
        sa.Column("event_type", sa.String(128), nullable=False),
        sa.Column("correlation_id", sa.String(255), nullable=False),
        sa.Column("payload", JSON_DOCUMENT, nullable=False),
        sa.Column("retention_policy", sa.String(64), nullable=False),
        sa.Column("retention_version", sa.String(32), nullable=False),
        sa.Column("hold", sa.Boolean(), nullable=False),
        sa.ForeignKeyConstraint(
            ["node_run_id", "run_id"],
            ["workflow_node_runs.id", "workflow_node_runs.run_id"],
            name="fk_run_event_node_run",
        ),
        sa.UniqueConstraint("run_id", "sequence", name="uq_workflow_run_event_sequence"),
        sa.CheckConstraint("sequence >= 1", name="ck_workflow_run_event_sequence_positive"),
        sa.CheckConstraint("revision = 1", name="ck_workflow_run_event_immutable"),
    )
    op.create_index("ix_workflow_run_events_run_id", "workflow_run_events", ["run_id"])

    op.create_table(
        "workflow_idempotency_keys",
        *_identity(),
        sa.Column("key_kind", sa.String(32), nullable=False),
        sa.Column("idempotency_key", sa.String(255), nullable=False),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("request_fingerprint", sa.String(64), nullable=False),
        sa.UniqueConstraint("key_kind", "idempotency_key", name="uq_workflow_idempotency_key"),
        sa.CheckConstraint("revision = 1", name="ck_workflow_idempotency_immutable"),
        sa.CheckConstraint(_hex64("request_fingerprint"), name="ck_workflow_idempotency_hash"),
    )

    op.create_table(
        "workflow_temporal_starts",
        *_identity(),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("node_run_id", sa.String(36), nullable=False),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("workflow_id", sa.String(512), nullable=False),
        sa.Column("request_fingerprint", sa.String(64), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.ForeignKeyConstraint(
            ["node_run_id", "run_id"],
            ["workflow_node_runs.id", "workflow_node_runs.run_id"],
            name="fk_temporal_start_node_run",
        ),
        sa.UniqueConstraint("workflow_id", name="uq_temporal_start_workflow_id"),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_temporal_start_run_operation"),
        sa.CheckConstraint(
            "status IN ('pending', 'started', 'submission_unknown', 'reconciled')",
            name="ck_temporal_start_status",
        ),
        sa.CheckConstraint(_hex64("request_fingerprint"), name="ck_temporal_start_hash"),
    )

    op.create_table(
        "workflow_budget_gates",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("node_run_id", sa.String(36), nullable=False),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("request_fingerprint", sa.String(255), nullable=False),
        sa.Column("operation_kind", sa.String(128), nullable=False),
        sa.Column("batch_size", sa.Integer(), nullable=False),
        sa.Column("cost_status", sa.String(16), nullable=False),
        sa.Column("estimated_cost", sa.String(64), nullable=True),
        sa.Column("currency", sa.String(16), nullable=True),
        sa.Column("threshold_snapshot_id", sa.String(255), nullable=True),
        sa.Column("threshold_revision", sa.Integer(), nullable=True),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("confirmation_id", sa.String(255), nullable=True),
        sa.Column("user_uuid", sa.String(36), nullable=True),
        sa.Column("retention_policy", sa.String(64), nullable=False),
        sa.Column("retention_version", sa.String(32), nullable=False),
        sa.Column("hold", sa.Boolean(), nullable=False),
        sa.ForeignKeyConstraint(
            ["node_run_id", "run_id"],
            ["workflow_node_runs.id", "workflow_node_runs.run_id"],
            name="fk_budget_gate_node_run",
        ),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_budget_gate_run_operation"),
        sa.CheckConstraint("revision >= 1", name="ck_budget_gate_revision"),
        sa.CheckConstraint("batch_size >= 1", name="ck_budget_gate_batch_size"),
        sa.CheckConstraint(
            "cost_status IN ('known', 'unknown')", name="ck_budget_gate_cost_status"
        ),
        sa.CheckConstraint(
            "status IN ('pending_confirmation', 'confirmed')", name="ck_budget_gate_status"
        ),
    )

    op.create_table(
        "workflow_outbox_events",
        *_identity(),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column(
            "run_event_id", sa.String(36), sa.ForeignKey("workflow_run_events.id"), nullable=False
        ),
        sa.Column("event_type", sa.String(128), nullable=False),
        sa.Column("payload", JSON_DOCUMENT, nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.UniqueConstraint("run_event_id", name="uq_workflow_outbox_run_event"),
        sa.CheckConstraint("revision >= 1", name="ck_workflow_outbox_revision"),
        sa.CheckConstraint("status IN ('pending', 'published')", name="ck_workflow_outbox_status"),
    )


def downgrade() -> None:
    op.drop_table("workflow_outbox_events")
    op.drop_table("workflow_budget_gates")
    op.drop_table("workflow_temporal_starts")
    op.drop_table("workflow_idempotency_keys")
    op.drop_index("ix_workflow_run_events_run_id", table_name="workflow_run_events")
    op.drop_table("workflow_run_events")
    op.drop_index(
        "ix_workflow_run_input_snapshots_project_id",
        table_name="workflow_run_input_snapshots",
    )
    op.drop_table("workflow_run_input_snapshots")
    op.drop_index("ix_workflow_node_runs_run_id", table_name="workflow_node_runs")
    op.drop_table("workflow_node_runs")
    op.drop_index("ix_workflow_runs_workflow_version_id", table_name="workflow_runs")
    with op.batch_alter_table("workflow_runs") as batch:
        batch.drop_constraint("ck_workflow_runs_revision_positive", type_="check")
        batch.drop_constraint("ck_workflow_runs_status", type_="check")
        batch.drop_constraint("fk_workflow_runs_predecessor", type_="foreignkey")
        batch.drop_constraint("fk_workflow_runs_rerun_source", type_="foreignkey")
        batch.drop_constraint("fk_workflow_runs_published_source", type_="foreignkey")
        batch.drop_column("source_snapshot")
        batch.drop_column("selection_snapshot")
        batch.drop_column("input_snapshot")
        batch.drop_column("predecessor_run_id")
        batch.drop_column("rerun_of_run_id")
        batch.drop_column("workflow_version_id")
    op.drop_table("project_default_workflow_bindings")
    op.drop_index(
        "ix_published_workflow_versions_project_id", table_name="published_workflow_versions"
    )
    op.drop_table("published_workflow_versions")
