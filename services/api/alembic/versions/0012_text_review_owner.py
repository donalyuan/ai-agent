"""add text review and skill route owner facts"""

from __future__ import annotations

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0012_text_review_owner"
down_revision = "0011_phase_one_documents"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")


def _identity() -> list[sa.Column[object]]:
    return [
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("created_at", sa.DateTime(timezone=True)),
    ]


def upgrade() -> None:
    op.create_table(
        "skill_route_decisions",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("node_key", sa.String(128), nullable=False),
        sa.Column("launch_id", sa.String(128), nullable=False),
        sa.Column("input_fingerprint", sa.String(64), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("candidate_set", JSON_DOCUMENT, nullable=False),
        sa.UniqueConstraint("project_id", "launch_id", "node_key", name="uq_skill_route_launch"),
        sa.CheckConstraint(
            "status IN ('selected', 'needs_human_selection', 'rejected')",
            name="ck_skill_route_decision_status",
        ),
    )
    op.create_table(
        "skill_route_selections",
        *_identity(),
        sa.Column(
            "decision_id",
            sa.String(36),
            sa.ForeignKey("skill_route_decisions.id"),
            nullable=False,
        ),
        sa.Column("skill_revision_id", sa.String(36), nullable=False),
        sa.Column("skill_digest", sa.String(64), nullable=False),
        sa.Column("actor_uuid", sa.String(36), nullable=False),
        sa.Column("fingerprint", sa.String(64), nullable=False),
        sa.UniqueConstraint("decision_id", name="uq_skill_route_selection_decision"),
    )
    op.create_table(
        "text_generation_candidates",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("kind", sa.String(32), nullable=False),
        sa.Column("scope_id", sa.String(36), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("payload_hash", sa.String(64), nullable=False),
        sa.Column("payload", JSON_DOCUMENT, nullable=False),
        sa.Column("source_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("supersedes_id", sa.String(36), sa.ForeignKey("text_generation_candidates.id")),
        sa.UniqueConstraint("run_id", "id", "payload_hash", name="uq_text_candidate_run_hash"),
        sa.CheckConstraint(
            "status IN ('provisional', 'accepted', 'rejected', 'stale')",
            name="ck_text_candidate_status",
        ),
    )
    op.create_table(
        "text_review_batches",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("brief_revision", sa.Integer(), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("fingerprint", sa.String(64), nullable=False, unique=True),
        sa.Column("supersedes_batch_id", sa.String(36), sa.ForeignKey("text_review_batches.id")),
        sa.CheckConstraint(
            "status IN ('pending_review', 'accepted', 'rejected', 'stale')",
            name="ck_text_review_batch_status",
        ),
    )
    op.create_table(
        "text_review_batch_members",
        sa.Column(
            "batch_id", sa.String(36), sa.ForeignKey("text_review_batches.id"), primary_key=True
        ),
        sa.Column(
            "candidate_id",
            sa.String(36),
            sa.ForeignKey("text_generation_candidates.id"),
            primary_key=True,
        ),
        sa.Column("candidate_hash", sa.String(64), nullable=False),
        sa.Column("position", sa.Integer(), nullable=False),
        sa.UniqueConstraint("batch_id", "position", name="uq_text_batch_member_position"),
    )
    op.create_table(
        "text_review_confirmations",
        *_identity(),
        sa.Column(
            "batch_id", sa.String(36), sa.ForeignKey("text_review_batches.id"), nullable=False
        ),
        sa.Column("action", sa.String(16), nullable=False),
        sa.Column("actor_uuid", sa.String(36), nullable=False),
        sa.Column("candidate_set_hash", sa.String(64), nullable=False),
        sa.UniqueConstraint("batch_id", name="uq_text_confirmation_batch"),
        sa.CheckConstraint("action IN ('accept', 'reject')", name="ck_text_confirmation_action"),
    )
    op.create_table(
        "text_owner_handoffs",
        *_identity(),
        sa.Column(
            "batch_id", sa.String(36), sa.ForeignKey("text_review_batches.id"), nullable=False
        ),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("payload_hash", sa.String(64), nullable=False),
        sa.Column("correlation_id", sa.String(128), nullable=False),
        sa.Column("candidate_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("required_owners", JSON_DOCUMENT, nullable=False),
        sa.UniqueConstraint("batch_id", name="uq_text_handoff_batch"),
    )
    op.create_table(
        "text_owner_handoff_acks",
        *_identity(),
        sa.Column(
            "handoff_id", sa.String(36), sa.ForeignKey("text_owner_handoffs.id"), nullable=False
        ),
        sa.Column("owner", sa.String(32), nullable=False),
        sa.Column("owner_revision", sa.Integer(), nullable=False),
        sa.Column("fingerprint", sa.String(64), nullable=False),
        sa.Column("correlation_id", sa.String(128), nullable=False),
        sa.UniqueConstraint("handoff_id", "owner", name="uq_text_handoff_ack_owner"),
    )
    op.create_table(
        "text_generation_audits",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), sa.ForeignKey("workflow_runs.id"), nullable=False),
        sa.Column("event_type", sa.String(64), nullable=False),
        sa.Column("correlation_id", sa.String(128), nullable=False),
        sa.Column("redacted_evidence", JSON_DOCUMENT, nullable=False),
        sa.Column("retention_policy", sa.String(64), nullable=False),
        sa.Column("retention_version", sa.String(32), nullable=False),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
    )


def downgrade() -> None:
    for table in (
        "text_generation_audits",
        "text_owner_handoff_acks",
        "text_owner_handoffs",
        "text_review_confirmations",
        "text_review_batch_members",
        "text_review_batches",
        "text_generation_candidates",
        "skill_route_selections",
        "skill_route_decisions",
    ):
        op.drop_table(table)
