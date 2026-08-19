"""enforce asset-version ownership and hexadecimal integrity facts"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0005_assets_integrity_repair"
down_revision = "0004_assets_asset_versions_slice"
branch_labels = None
depends_on = None


def _hex64_check(column: str) -> str:
    stripped = f"lower({column})"
    for character in "0123456789abcdef":
        stripped = f"replace({stripped}, '{character}', '')"
    return f"length({column}) = 64 AND length({stripped}) = 0"


def _existing_checks(table_name: str) -> set[str]:
    return {
        constraint["name"]
        for constraint in sa.inspect(op.get_bind()).get_check_constraints(table_name)
        if constraint.get("name")
    }


def _existing_foreign_keys(table_name: str) -> set[str]:
    return {
        constraint["name"]
        for constraint in sa.inspect(op.get_bind()).get_foreign_keys(table_name)
        if constraint.get("name")
    }


def _existing_uniques(table_name: str) -> set[str]:
    return {
        constraint["name"]
        for constraint in sa.inspect(op.get_bind()).get_unique_constraints(table_name)
        if constraint.get("name")
    }


def _validate_existing_rows() -> None:
    connection = op.get_bind()
    invalid = connection.execute(
        sa.text(
            "SELECT COUNT(*) FROM asset_versions av "
            "LEFT JOIN assets a ON a.id = av.asset_id "
            "AND a.project_id = av.project_id "
            f"WHERE NOT ({_hex64_check('av.checksum')}) "
            f"OR NOT ({_hex64_check('av.content_hash')}) "
            "OR a.id IS NULL"
        )
    ).scalar_one()
    if invalid:
        raise RuntimeError(
            "0005 cannot enforce asset-version ownership or hexadecimal hash integrity"
        )


def upgrade() -> None:
    _validate_existing_rows()
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("assets", recreate="always") as batch:
            batch.create_unique_constraint("uq_assets_id_project_id", ["id", "project_id"])
    else:
        op.create_unique_constraint("uq_assets_id_project_id", "assets", ["id", "project_id"])

    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("asset_versions", recreate="always") as batch:
            batch.create_check_constraint(
                "ck_asset_versions_checksum_hex64", _hex64_check("checksum")
            )
            batch.create_check_constraint(
                "ck_asset_versions_content_hash_hex64", _hex64_check("content_hash")
            )
            batch.create_foreign_key(
                "fk_asset_versions_project_id", "projects", ["project_id"], ["id"]
            )
            batch.create_foreign_key(
                "fk_asset_versions_asset_project",
                "assets",
                ["asset_id", "project_id"],
                ["id", "project_id"],
            )
    else:
        op.create_check_constraint(
            "ck_asset_versions_checksum_hex64", "asset_versions", _hex64_check("checksum")
        )
        op.create_check_constraint(
            "ck_asset_versions_content_hash_hex64",
            "asset_versions",
            _hex64_check("content_hash"),
        )
        op.create_foreign_key(
            "fk_asset_versions_project_id",
            "asset_versions",
            "projects",
            ["project_id"],
            ["id"],
        )
        op.create_foreign_key(
            "fk_asset_versions_asset_project",
            "asset_versions",
            "assets",
            ["asset_id", "project_id"],
            ["id", "project_id"],
        )


def downgrade() -> None:
    foreign_keys = ("fk_asset_versions_asset_project", "fk_asset_versions_project_id")
    checks = ("ck_asset_versions_checksum_hex64", "ck_asset_versions_content_hash_hex64")
    if op.get_context().dialect.name == "sqlite":
        existing_foreign_keys = _existing_foreign_keys("asset_versions")
        existing_checks = _existing_checks("asset_versions")
        with op.batch_alter_table("asset_versions", recreate="always") as batch:
            for name in foreign_keys:
                if name in existing_foreign_keys:
                    batch.drop_constraint(name, type_="foreignkey")
            for name in checks:
                if name in existing_checks:
                    batch.drop_constraint(name, type_="check")
    else:
        existing_foreign_keys = _existing_foreign_keys("asset_versions")
        existing_checks = _existing_checks("asset_versions")
        for name in foreign_keys:
            if name in existing_foreign_keys:
                op.drop_constraint(name, "asset_versions", type_="foreignkey")
        for name in checks:
            if name in existing_checks:
                op.drop_constraint(name, "asset_versions", type_="check")

    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("assets", recreate="always") as batch:
            if "uq_assets_id_project_id" in _existing_uniques("assets"):
                batch.drop_constraint("uq_assets_id_project_id", type_="unique")
    elif "uq_assets_id_project_id" in _existing_uniques("assets"):
        op.drop_constraint("uq_assets_id_project_id", "assets", type_="unique")
