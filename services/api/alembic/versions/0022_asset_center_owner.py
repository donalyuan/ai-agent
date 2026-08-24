"""add asset catalog metadata and normalized upload reservations

Revision ID: 0022_asset_center_owner
Revises: 0021_catalog_owner_column_repair
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0022_asset_center_owner"
down_revision = "0021_catalog_owner_column_repair"
branch_labels = None
depends_on = None


def _hex64(column: str) -> str:
    stripped = f"lower({column})"
    for character in "0123456789abcdef":
        stripped = f"replace({stripped}, '{character}', '')"
    return f"length({column}) = 64 AND length({stripped}) = 0"


def upgrade() -> None:
    op.execute(sa.text("UPDATE assets SET updated_at = CURRENT_TIMESTAMP WHERE updated_at IS NULL"))
    with op.batch_alter_table("assets") as batch:
        batch.alter_column(
            "updated_at",
            existing_type=sa.DateTime(timezone=True),
            nullable=False,
            server_default=sa.func.now(),
        )
        batch.add_column(
            sa.Column("source_type", sa.String(32), nullable=False, server_default="imported")
        )
        batch.add_column(sa.Column("catalog_role", sa.String(32), nullable=True))
        batch.add_column(sa.Column("tags", sa.JSON(), nullable=False, server_default="[]"))
        batch.add_column(
            sa.Column(
                "authorization_status", sa.String(32), nullable=False, server_default="unknown"
            )
        )
        batch.add_column(sa.Column("copyright_owner", sa.String(255), nullable=True))
        batch.add_column(sa.Column("license_label", sa.String(255), nullable=True))
        batch.add_column(sa.Column("license_reference", sa.String(1024), nullable=True))
        batch.create_check_constraint(
            "ck_assets_source_type",
            "source_type IN ('user_upload','provider_generated','source_material','imported')",
        )
        batch.create_check_constraint(
            "ck_assets_catalog_role",
            "catalog_role IS NULL OR catalog_role IN "
            "('character','location','prop','storyboard','video_take','dialogue','music',"
            "'ambience','effects','other')",
        )
        batch.create_check_constraint(
            "ck_assets_authorization_status",
            "authorization_status IN ('unknown','declared','verified','restricted','expired')",
        )

    op.create_table(
        "asset_version_reservations",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), nullable=False),
        sa.Column("asset_id", sa.String(36), nullable=False),
        sa.Column("operation_key", sa.String(512), nullable=False, unique=True),
        sa.Column("fingerprint", sa.String(64), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("registered_version_id", sa.String(36), nullable=True),
        sa.Column("expected_asset_revision", sa.Integer(), nullable=False),
        sa.Column("declared_kind", sa.String(32), nullable=False),
        sa.Column("declared_mime_type", sa.String(255), nullable=False),
        sa.Column("declared_size_bytes", sa.BigInteger(), nullable=False),
        sa.Column("declared_checksum", sa.String(64), nullable=False),
        sa.Column("storage_profile_id", sa.String(255), nullable=False),
        sa.Column("storage_profile_revision", sa.Integer(), nullable=False),
        sa.Column("storage_profile_snapshot_hash", sa.String(64), nullable=False),
        sa.Column("upload_key", sa.String(1024), nullable=False),
        sa.Column("diagnostic", sa.String(255), nullable=True),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column(
            "created_at", sa.DateTime(timezone=True), nullable=False, server_default=sa.func.now()
        ),
        sa.ForeignKeyConstraint(["project_id"], ["projects.id"]),
        sa.ForeignKeyConstraint(["asset_id", "project_id"], ["assets.id", "assets.project_id"]),
        sa.ForeignKeyConstraint(["registered_version_id"], ["asset_versions.id"]),
        sa.UniqueConstraint("asset_id", "fingerprint", name="uq_asset_reservation_fingerprint"),
        sa.CheckConstraint(
            "status IN ('reserved','registered','cancelled','failed')",
            name="ck_asset_reservation_status",
        ),
        sa.CheckConstraint("revision >= 1", name="ck_asset_reservation_revision"),
        sa.CheckConstraint("declared_size_bytes >= 0", name="ck_asset_reservation_declared_size"),
        sa.CheckConstraint(_hex64("fingerprint"), name="ck_asset_reservation_fingerprint_hex64"),
        sa.CheckConstraint(_hex64("declared_checksum"), name="ck_asset_reservation_checksum_hex64"),
        sa.CheckConstraint(
            _hex64("storage_profile_snapshot_hash"),
            name="ck_asset_reservation_profile_hash_hex64",
        ),
    )
    op.create_index(
        "ix_asset_reservations_project_asset",
        "asset_version_reservations",
        ["project_id", "asset_id"],
    )


def downgrade() -> None:
    op.drop_index("ix_asset_reservations_project_asset", table_name="asset_version_reservations")
    op.drop_table("asset_version_reservations")
    with op.batch_alter_table("assets") as batch:
        batch.drop_constraint("ck_assets_authorization_status", type_="check")
        batch.drop_constraint("ck_assets_catalog_role", type_="check")
        batch.drop_constraint("ck_assets_source_type", type_="check")
        batch.drop_column("license_reference")
        batch.drop_column("license_label")
        batch.drop_column("copyright_owner")
        batch.drop_column("authorization_status")
        batch.drop_column("tags")
        batch.drop_column("catalog_role")
        batch.drop_column("source_type")
        batch.alter_column(
            "updated_at",
            existing_type=sa.DateTime(timezone=True),
            nullable=True,
            server_default=None,
        )
