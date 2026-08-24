"""Persist Agent AssetEdit sessions, plans, executions and review facts."""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0018_asset_edit_owner"
down_revision = "0017_agnes_video_owner"
branch_labels = None
depends_on = None


def _table(name: str, columns: list[sa.Column], *constraints: sa.Constraint) -> None:
    op.create_table(name, *columns, *constraints)


def upgrade() -> None:
    json_type = sa.JSON()
    _table(
        "asset_edit_sessions",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
            sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
            sa.Column("status", sa.String(32), nullable=False, server_default="active"),
            sa.Column("payload", json_type, nullable=False),
        ],
        sa.UniqueConstraint("id", "project_id", name="uq_asset_edit_session_project"),
    )
    _table(
        "asset_edit_plans",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
            sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
            sa.Column("status", sa.String(32), nullable=False, server_default="pending_review"),
            sa.Column("payload", json_type, nullable=False),
        ],
    )
    _table(
        "asset_edit_conversations",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
            sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        ],
    )
    _table(
        "asset_edit_messages",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column(
                "session_id", sa.String(36), sa.ForeignKey("asset_edit_sessions.id"), nullable=False
            ),
            sa.Column("sequence", sa.Integer(), nullable=False),
            sa.Column("role", sa.String(16), nullable=False),
            sa.Column("content_hash", sa.String(64), nullable=False),
            sa.Column("status", sa.String(16), nullable=False),
            sa.Column("correlation_id", sa.String(255), nullable=False),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.UniqueConstraint("session_id", "sequence", name="uq_asset_edit_message_sequence"),
        ],
    )
    _table(
        "asset_edit_turns",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column(
                "session_id", sa.String(36), sa.ForeignKey("asset_edit_sessions.id"), nullable=False
            ),
            sa.Column("sequence", sa.Integer(), nullable=False),
            sa.Column(
                "user_message_id",
                sa.String(36),
                sa.ForeignKey("asset_edit_messages.id"),
                nullable=False,
            ),
            sa.Column("agent_message_id", sa.String(36)),
            sa.Column("status", sa.String(16), nullable=False, server_default="pending"),
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.UniqueConstraint("session_id", "sequence", name="uq_asset_edit_turn_sequence"),
        ],
    )
    _table(
        "asset_edit_executions",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column(
                "plan_id", sa.String(36), sa.ForeignKey("asset_edit_plans.id"), nullable=False
            ),
            sa.Column("run_id", sa.String(36), nullable=False),
            sa.Column("node_run_id", sa.String(36), nullable=False),
            sa.Column("logical_operation", sa.String(255), nullable=False),
            sa.Column("status", sa.String(32), nullable=False, server_default="queued"),
            sa.Column("payload", json_type, nullable=False),
        ],
        sa.UniqueConstraint(
            "run_id", "node_run_id", "logical_operation", name="uq_asset_edit_execution_key"
        ),
    )
    _table(
        "asset_edit_candidates",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column(
                "plan_id", sa.String(36), sa.ForeignKey("asset_edit_plans.id"), nullable=False
            ),
            sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
            sa.Column("status", sa.String(32), nullable=False, server_default="pending_review"),
            sa.Column("payload", json_type, nullable=False),
        ],
    )
    _table(
        "asset_edit_accept_decisions",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column(
                "candidate_id",
                sa.String(36),
                sa.ForeignKey("asset_edit_candidates.id"),
                nullable=False,
            ),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column("action", sa.String(32), nullable=False),
            sa.Column("payload", json_type, nullable=False),
        ],
    )
    _table(
        "asset_edit_impacts",
        [
            sa.Column("id", sa.String(36), primary_key=True),
            sa.Column(
                "plan_id", sa.String(36), sa.ForeignKey("asset_edit_plans.id"), nullable=False
            ),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column("status", sa.String(32), nullable=False),
            sa.Column("payload", json_type, nullable=False),
        ],
    )


def downgrade() -> None:
    for name in (
        "asset_edit_turns",
        "asset_edit_messages",
        "asset_edit_conversations",
        "asset_edit_impacts",
        "asset_edit_accept_decisions",
        "asset_edit_candidates",
        "asset_edit_executions",
        "asset_edit_plans",
        "asset_edit_sessions",
    ):
        op.drop_table(name)
