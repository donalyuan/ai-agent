"""Persist Agnes video operations and immutable review candidates."""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0017_agnes_video_owner"
down_revision = "0016_catalog_owner"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON()


def upgrade() -> None:
    op.create_table(
        "video_operations",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("run_id", sa.String(36), nullable=False),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("provider_id", sa.String(36), sa.ForeignKey("providers.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("model_id", sa.String(36), sa.ForeignKey("models.id"), nullable=False),
        sa.Column(
            "capability_snapshot_id",
            sa.String(36),
            sa.ForeignKey("capability_snapshots.id"),
            nullable=False,
        ),
        sa.Column("source_asset_version_id", sa.String(36), nullable=False),
        sa.Column("source_asset_version_revision", sa.Integer(), nullable=False),
        sa.Column("source_asset_version_hash", sa.String(64), nullable=False),
        sa.Column("source_candidate_id", sa.String(36)),
        sa.Column("source_provenance", sa.String(128)),
        sa.Column("shot_spec_id", sa.String(36), nullable=False),
        sa.Column("shot_spec_revision", sa.Integer(), nullable=False),
        sa.Column("shot_spec_hash", sa.String(64), nullable=False),
        sa.Column("duration_seconds", sa.Float(), nullable=False),
        sa.Column("aspect_ratio", sa.String(16), nullable=False),
        sa.Column("episode_id", sa.String(36), nullable=False),
        sa.Column("target_id", sa.String(36), nullable=False),
        sa.Column("asset_id", sa.String(36), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("provider_request_id", sa.String(255)),
        sa.Column("cancel_requested", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.Column("observation_fingerprints", JSON_DOCUMENT, nullable=False, server_default="[]"),
        sa.Column(
            "retention_policy", sa.String(64), nullable=False, server_default="long-term-audit"
        ),
        sa.Column("retention_version", sa.String(32), nullable=False, server_default="1"),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_video_operation_run_logical"),
    )
    op.create_table(
        "video_take_candidates",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("episode_id", sa.String(36), nullable=False),
        sa.Column("target_id", sa.String(36), nullable=False),
        sa.Column("run_id", sa.String(36), nullable=False),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("source_asset_version_id", sa.String(36), nullable=False),
        sa.Column("source_asset_version_revision", sa.Integer(), nullable=False),
        sa.Column("source_asset_version_hash", sa.String(64), nullable=False),
        sa.Column("source_candidate_id", sa.String(36)),
        sa.Column(
            "source_provenance", sa.String(128), nullable=False, server_default="agnes_video"
        ),
        sa.Column("shot_spec_id", sa.String(36), nullable=False),
        sa.Column("shot_spec_revision", sa.Integer(), nullable=False),
        sa.Column("shot_spec_hash", sa.String(64), nullable=False),
        sa.Column("duration_seconds", sa.Float(), nullable=False),
        sa.Column("aspect_ratio", sa.String(16), nullable=False),
        sa.Column("asset_version_id", sa.String(36), nullable=False),
        sa.Column("asset_version_revision", sa.Integer(), nullable=False),
        sa.Column("asset_version_hash", sa.String(64), nullable=False),
        sa.Column("provider_request_id", sa.String(255)),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column(
            "retention_policy", sa.String(64), nullable=False, server_default="long-term-audit"
        ),
        sa.Column("retention_version", sa.String(32), nullable=False, server_default="1"),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.UniqueConstraint("run_id", "logical_operation", name="uq_video_candidate_run_logical"),
    )


def downgrade() -> None:
    op.drop_table("video_take_candidates")
    op.drop_table("video_operations")
