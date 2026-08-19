"""phase zero foundation

Revision ID: 0001_phase_zero_foundation
Revises:
Create Date: 2026-08-18
"""

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0001_phase_zero_foundation"
down_revision = None
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")
STATUS_CHECK = (
    "status IN ('draft', 'generated', 'pending_review', 'approved', 'rejected', "
    "'superseded', 'archived')"
)


def _identity_columns() -> list[sa.Column[object]]:
    return [
        sa.Column("id", sa.String(length=36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False),
        sa.Column("schema_version", sa.String(length=32), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("updated_at", sa.DateTime(timezone=True), nullable=True),
    ]


def upgrade() -> None:
    op.create_table(
        "projects",
        *_identity_columns(),
        sa.Column("name", sa.String(255), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "episodes",
        *_identity_columns(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("display_number", sa.Integer(), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "scenes",
        *_identity_columns(),
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
        sa.Column("display_number", sa.Integer(), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "shots",
        *_identity_columns(),
        sa.Column("scene_id", sa.String(36), sa.ForeignKey("scenes.id"), nullable=False),
        sa.Column("display_number", sa.Integer(), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "assets",
        *_identity_columns(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("kind", sa.String(64), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "workflow_drafts",
        *_identity_columns(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("scope_type", sa.String(64), nullable=False),
        sa.Column("scope_ids", JSON_DOCUMENT, nullable=False),
        sa.Column("definition", JSON_DOCUMENT, nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint("scope_type IN ('project', 'episode', 'scene', 'shot')"),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "asset_versions",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("asset_id", sa.String(36), sa.ForeignKey("assets.id"), nullable=False),
        sa.Column("version_number", sa.Integer(), nullable=False),
        sa.Column("schema_version", sa.String(32), nullable=False),
        sa.Column("storage_ref", sa.String(1024), nullable=False),
        sa.Column("checksum", sa.String(128)),
        sa.Column("metadata_json", JSON_DOCUMENT, nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True)),
        sa.UniqueConstraint("asset_id", "version_number", name="uq_asset_version_number"),
    )
    op.create_table(
        "workflow_versions",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "workflow_draft_id", sa.String(36), sa.ForeignKey("workflow_drafts.id"), nullable=False
        ),
        sa.Column("version_number", sa.Integer(), nullable=False),
        sa.Column("schema_version", sa.String(32), nullable=False),
        sa.Column("definition", JSON_DOCUMENT, nullable=False),
        sa.Column("content_hash", sa.String(128), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True)),
        sa.UniqueConstraint(
            "workflow_draft_id", "version_number", name="uq_workflow_version_number"
        ),
    )
    op.create_table(
        "timeline_documents",
        *_identity_columns(),
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
        sa.Column("name", sa.String(255), nullable=False),
        sa.Column("document", JSON_DOCUMENT, nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.CheckConstraint(STATUS_CHECK),
    )
    op.create_table(
        "providers",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("name", sa.String(255), nullable=False, unique=True),
        sa.Column("adapter_key", sa.String(128), nullable=False),
        sa.Column("enabled", sa.Boolean(), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True)),
    )
    op.create_table(
        "credential_metadata",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("provider_id", sa.String(36), sa.ForeignKey("providers.id"), nullable=False),
        sa.Column("key_version", sa.String(64), nullable=False),
        sa.Column("masked_prefix", sa.String(32)),
        sa.Column("last4", sa.String(4)),
        sa.Column("ciphertext", sa.Text(), nullable=False),
        sa.Column("nonce", sa.String(128), nullable=False),
        sa.Column("tag", sa.String(128), nullable=False),
    )
    op.create_table(
        "provider_profiles",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("provider_id", sa.String(36), sa.ForeignKey("providers.id"), nullable=False),
        sa.Column("name", sa.String(255), nullable=False),
        sa.Column("enabled", sa.Boolean(), nullable=False),
        sa.Column("base_url", sa.String(1024)),
        sa.Column("endpoint", sa.String(1024)),
        sa.Column("bucket", sa.String(255)),
        sa.Column("region", sa.String(255)),
        sa.Column("settings", JSON_DOCUMENT, nullable=False),
        sa.Column("auth", JSON_DOCUMENT, nullable=False),
        sa.Column("credential_metadata_id", sa.String(36), sa.ForeignKey("credential_metadata.id")),
        sa.Column("timeout_ms", sa.Integer(), nullable=False),
    )
    op.create_table(
        "models",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("provider_profiles.id"), nullable=False
        ),
        sa.Column("model_key", sa.String(255), nullable=False),
        sa.Column("enabled", sa.Boolean(), nullable=False),
        sa.Column("default_parameters", JSON_DOCUMENT, nullable=False),
        sa.Column("parameter_schema", JSON_DOCUMENT, nullable=False),
    )


def downgrade() -> None:
    for table in (
        "models",
        "provider_profiles",
        "credential_metadata",
        "providers",
        "timeline_documents",
        "workflow_versions",
        "asset_versions",
        "workflow_drafts",
        "assets",
        "shots",
        "scenes",
        "episodes",
        "projects",
    ):
        op.drop_table(table)
