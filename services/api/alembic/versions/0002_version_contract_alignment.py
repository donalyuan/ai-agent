"""align version persistence with shared contracts

Revision ID: 0002_version_contract_alignment
Revises: 0001_phase_zero_foundation
Create Date: 2026-08-18
"""

import sqlalchemy as sa

from alembic import op

revision = "0002_version_contract_alignment"
down_revision = "0001_phase_zero_foundation"
branch_labels = None
depends_on = None

STATUS_CHECK = (
    "status IN ('draft', 'generated', 'pending_review', 'approved', 'rejected', "
    "'superseded', 'archived')"
)
CONTRACT_TABLES = (
    "projects",
    "episodes",
    "scenes",
    "shots",
    "assets",
    "asset_versions",
    "workflow_drafts",
    "workflow_versions",
    "timeline_documents",
)
VERSION_TABLES = ("asset_versions", "workflow_versions")


def upgrade() -> None:
    for table_name in CONTRACT_TABLES:
        table = sa.table(table_name, sa.column("schema_version", sa.String()))
        op.execute(
            table.update().where(table.c.schema_version == "1.0").values(schema_version="1.0.0")
        )

    for table_name in VERSION_TABLES:
        columns = (
            sa.Column("revision", sa.Integer(), server_default=sa.text("0"), nullable=False),
            sa.Column(
                "status", sa.String(length=32), server_default=sa.text("'draft'"), nullable=False
            ),
        )
        if op.get_context().dialect.name == "sqlite":
            # Batch mode keeps the foundation migration runnable on SQLite test databases.
            with op.batch_alter_table(table_name, recreate="always") as batch:
                for column in columns:
                    batch.add_column(column)
                batch.create_check_constraint(
                    f"ck_{table_name}_revision_nonnegative", "revision >= 0"
                )
                batch.create_check_constraint(f"ck_{table_name}_status", STATUS_CHECK)
        else:
            for column in columns:
                op.add_column(table_name, column)
            op.create_check_constraint(
                f"ck_{table_name}_revision_nonnegative", table_name, "revision >= 0"
            )
            op.create_check_constraint(f"ck_{table_name}_status", table_name, STATUS_CHECK)


def downgrade() -> None:
    for table_name in reversed(VERSION_TABLES):
        if op.get_context().dialect.name == "sqlite":
            with op.batch_alter_table(table_name, recreate="always") as batch:
                batch.drop_constraint(f"ck_{table_name}_status", type_="check")
                batch.drop_constraint(f"ck_{table_name}_revision_nonnegative", type_="check")
                batch.drop_column("status")
                batch.drop_column("revision")
        else:
            op.drop_constraint(f"ck_{table_name}_status", table_name, type_="check")
            op.drop_constraint(f"ck_{table_name}_revision_nonnegative", table_name, type_="check")
            op.drop_column(table_name, "status")
            op.drop_column(table_name, "revision")

    # Keep 1.0.0 values valid for the shared Schema after structural rollback.
