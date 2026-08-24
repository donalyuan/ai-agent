"""Persist normalized provider/model/skill catalog owner facts."""

from __future__ import annotations

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0016_catalog_owner"
down_revision = "0015_workflows_runs_owner"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")


def _identity() -> list[sa.Column[object]]:
    return [
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column(
            "created_at", sa.DateTime(timezone=True), nullable=False, server_default=sa.func.now()
        ),
    ]


def upgrade() -> None:
    # Existing phase-zero catalog tables predate the catalog owner revision
    # contract.  Add the owner fields in this additive change so normalized
    # catalog writes can use the same CAS semantics as the newer tables.
    with op.batch_alter_table("providers") as batch:
        batch.add_column(sa.Column("revision", sa.Integer(), nullable=False, server_default="1"))
        batch.add_column(
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0")
        )
        batch.add_column(
            sa.Column("approval", sa.String(64), nullable=False, server_default="pending")
        )
        batch.add_column(
            sa.Column("feature_gate", sa.String(32), nullable=False, server_default="MVP-A")
        )
        batch.add_column(
            sa.Column("adapter_installed", sa.Boolean(), nullable=False, server_default=sa.false())
        )
    with op.batch_alter_table("provider_profiles") as batch:
        batch.add_column(sa.Column("revision", sa.Integer(), nullable=False, server_default="1"))
        batch.add_column(
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0")
        )
        batch.add_column(
            sa.Column(
                "adapter_identity", sa.String(128), nullable=False, server_default="local_workspace"
            )
        )
        batch.add_column(
            sa.Column(
                "explicit_live_opt_in", sa.Boolean(), nullable=False, server_default=sa.false()
            )
        )
        batch.add_column(
            sa.Column(
                "credential_status", sa.String(32), nullable=False, server_default="unconfigured"
            )
        )
    with op.batch_alter_table("models") as batch:
        batch.add_column(sa.Column("revision", sa.Integer(), nullable=False, server_default="1"))
        batch.add_column(
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0")
        )
    with op.batch_alter_table("credential_metadata") as batch:
        batch.add_column(sa.Column("profile_id", sa.String(36), nullable=True))
        batch.add_column(sa.Column("credential_id", sa.String(255), nullable=True))
        batch.add_column(sa.Column("algorithm", sa.String(64), nullable=True))
        batch.add_column(sa.Column("aad_version", sa.String(32), nullable=True))

    op.create_table(
        "capability_snapshots",
        *_identity(),
        sa.Column("provider_id", sa.String(36), sa.ForeignKey("providers.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("model_id", sa.String(36), sa.ForeignKey("models.id"), nullable=True),
        sa.Column("operation", sa.String(128), nullable=False),
        sa.Column("runnable", sa.Boolean(), nullable=False),
        sa.Column("capabilities", JSON_DOCUMENT, nullable=False),
        sa.Column("captured_at", sa.String(64), nullable=False),
        sa.Column(
            "retention_policy", sa.String(64), nullable=False, server_default="long-term-audit"
        ),
        sa.Column("retention_version", sa.String(32), nullable=False, server_default="1"),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.UniqueConstraint(
            "profile_id", "operation", "revision", name="uq_capability_snapshot_revision"
        ),
    )
    op.create_table(
        "skill_revisions",
        *_identity(),
        sa.Column("name", sa.String(255), nullable=False),
        sa.Column("version", sa.String(64), nullable=False),
        sa.Column("provenance", sa.String(64), nullable=False),
        sa.Column("approval", sa.String(64), nullable=False),
        sa.Column("enabled", sa.Boolean(), nullable=False),
        sa.Column("source_identity", sa.String(2048), nullable=False),
        sa.Column("digest", sa.String(128), nullable=False),
        sa.Column("source_type", sa.String(64), nullable=False),
        sa.Column("license_status", sa.String(64), nullable=False),
        sa.Column("capabilities", JSON_DOCUMENT, nullable=False),
        sa.UniqueConstraint("name", "version", "digest", name="uq_skill_revision_identity"),
    )
    op.create_table(
        "provider_calls",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), nullable=False),
        sa.Column("node_run_id", sa.String(36), nullable=True),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("operation", sa.String(128), nullable=False),
        sa.Column("provider_id", sa.String(36), sa.ForeignKey("providers.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("model_id", sa.String(36), sa.ForeignKey("models.id"), nullable=False),
        sa.Column(
            "capability_snapshot_id",
            sa.String(36),
            sa.ForeignKey("capability_snapshots.id"),
            nullable=True,
        ),
        sa.Column("request_fingerprint", sa.String(128), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("cost_status", sa.String(16), nullable=False),
        sa.Column("cost_value", sa.String(64), nullable=True),
        sa.Column("cost_currency", sa.String(16), nullable=True),
        sa.Column("cost_source", sa.String(255), nullable=True),
        sa.Column("provider_request_id", sa.String(255), nullable=True),
        sa.Column("native_usage", JSON_DOCUMENT, nullable=True),
        sa.Column("failure_code", sa.String(128), nullable=True),
        sa.Column(
            "retention_policy", sa.String(64), nullable=False, server_default="long-term-audit"
        ),
        sa.Column("retention_version", sa.String(32), nullable=False, server_default="1"),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_provider_call_run_operation"),
        sa.CheckConstraint(
            "cost_status IN ('known', 'unknown')", name="ck_provider_call_cost_status"
        ),
    )
    op.create_table(
        "provider_quota_snapshots",
        *_identity(),
        sa.Column("provider_id", sa.String(36), sa.ForeignKey("providers.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("operation", sa.String(128), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("remaining", sa.Integer(), nullable=True),
        sa.Column("reset_at", sa.String(64), nullable=True),
        sa.Column("source", sa.String(255), nullable=False),
        sa.Column("captured_at", sa.String(64), nullable=False),
        sa.CheckConstraint("status IN ('known', 'unknown', 'exhausted')", name="ck_quota_status"),
    )
    op.create_table(
        "provider_operation_policies",
        *_identity(),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("operation", sa.String(128), nullable=False),
        sa.Column("max_concurrency", sa.Integer(), nullable=False),
        sa.Column("rate_limit", sa.Integer(), nullable=False),
        sa.Column("rate_window_seconds", sa.Integer(), nullable=False),
        sa.UniqueConstraint("profile_id", "operation", name="uq_provider_operation_policy"),
        sa.CheckConstraint("max_concurrency >= 1", name="ck_policy_concurrency_positive"),
        sa.CheckConstraint("rate_limit >= 1", name="ck_policy_rate_positive"),
        sa.CheckConstraint("rate_window_seconds >= 1", name="ck_policy_window_positive"),
    )
    op.create_table(
        "cost_confirmations",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), nullable=False),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("request_fingerprint", sa.String(128), nullable=False),
        sa.Column("user_uuid", sa.String(36), nullable=False),
        sa.Column("threshold_snapshot_id", sa.String(255), nullable=True),
        sa.Column("threshold_revision", sa.Integer(), nullable=True),
        sa.Column("estimated_cost", sa.String(64), nullable=True),
        sa.Column("cost_status", sa.String(16), nullable=False),
        sa.Column("operation_kind", sa.String(128), nullable=False),
        sa.Column("batch_size", sa.Integer(), nullable=False),
        sa.Column(
            "retention_policy", sa.String(64), nullable=False, server_default="diagnostic-30d"
        ),
        sa.Column("retention_version", sa.String(32), nullable=False, server_default="1"),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_cost_confirmation_operation"),
        sa.CheckConstraint(
            "cost_status IN ('known', 'unknown')", name="ck_cost_confirmation_status"
        ),
        sa.CheckConstraint("batch_size >= 1", name="ck_cost_confirmation_batch"),
    )
    op.create_table(
        "model_sync_candidates",
        *_identity(),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("remote_models", JSON_DOCUMENT, nullable=False),
        sa.Column("added", JSON_DOCUMENT, nullable=False),
        sa.Column("removed", JSON_DOCUMENT, nullable=False),
        sa.Column("changed", JSON_DOCUMENT, nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
    )
    op.create_table(
        "skill_access_audits",
        *_identity(),
        sa.Column(
            "skill_revision_id", sa.String(36), sa.ForeignKey("skill_revisions.id"), nullable=False
        ),
        sa.Column("run_id", sa.String(36), nullable=False),
        sa.Column("node_run_id", sa.String(36), nullable=False),
        sa.Column("access", sa.String(32), nullable=False),
        sa.Column("allowed", sa.Boolean(), nullable=False),
        sa.Column("reason", sa.String(255), nullable=False),
    )


def downgrade() -> None:
    for table in (
        "skill_access_audits",
        "model_sync_candidates",
        "cost_confirmations",
        "provider_operation_policies",
        "provider_quota_snapshots",
        "provider_calls",
        "skill_revisions",
        "capability_snapshots",
    ):
        op.drop_table(table)
    with op.batch_alter_table("credential_metadata") as batch:
        for name in ("aad_version", "algorithm", "credential_id", "profile_id"):
            batch.drop_column(name)
    with op.batch_alter_table("models") as batch:
        batch.drop_column("schema_version")
        batch.drop_column("revision")
    with op.batch_alter_table("provider_profiles") as batch:
        for name in (
            "credential_status",
            "explicit_live_opt_in",
            "adapter_identity",
            "schema_version",
            "revision",
        ):
            batch.drop_column(name)
    with op.batch_alter_table("providers") as batch:
        for name in ("adapter_installed", "feature_gate", "approval", "schema_version", "revision"):
            batch.drop_column(name)
