from __future__ import annotations

import ast
from pathlib import Path

SOURCE_ROOT = Path(__file__).parents[1] / "src" / "video_agent_api"


def _imports(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    modules: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            modules.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            modules.add(node.module)
    return modules


def _names(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    return {node.id for node in ast.walk(tree) if isinstance(node, ast.Name)}


def test_domain_entities_and_errors_are_framework_free() -> None:
    for path in (SOURCE_ROOT / "domain").glob("*.py"):
        imports = _imports(path)
        assert not any(
            module == "fastapi" or module.startswith("sqlalchemy") for module in imports
        ), path


def test_application_layer_does_not_import_concrete_adapters() -> None:
    application_root = SOURCE_ROOT / "application"
    for path in application_root.glob("*.py"):
        imports = _imports(path)
        assert not any(
            module.startswith("video_agent_api.adapters") or module.startswith("sqlalchemy")
            for module in imports
        ), path


def test_http_interfaces_do_not_access_database_sessions() -> None:
    interfaces_root = SOURCE_ROOT / "interfaces"
    for path in interfaces_root.rglob("*.py"):
        imports = _imports(path)
        assert not any(module.startswith("sqlalchemy") for module in imports), path
        assert not ({"Session", "AsyncSession"} & _names(path)), path


def test_scenes_slice_does_not_depend_on_orchestration_or_media_owners() -> None:
    forbidden = (
        "video_agent_api.application.runs",
        "video_agent_api.application.timeline",
        "video_agent_api.providers",
        "temporalio",
    )
    for relative in ("domain/scenes.py", "application/scenes.py", "interfaces/http/scenes.py"):
        path = SOURCE_ROOT / relative
        imports = _imports(path)
        assert not any(module.startswith(forbidden) for module in imports), path


def test_only_asset_bible_owner_mutates_asset_bible_collections() -> None:
    mutation_methods = {"append", "clear", "extend", "pop", "remove", "setdefault", "update"}
    allowed = {SOURCE_ROOT / "application" / "asset_bible.py"}
    for path in (SOURCE_ROOT / "application").glob("*.py"):
        if path in allowed:
            continue
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.Attribute) and isinstance(node.ctx, ast.Store):
                assert not node.attr.startswith("asset_bible"), path
            if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
                continue
            if node.func.attr not in mutation_methods:
                continue
            owner = node.func.value
            if isinstance(owner, ast.Attribute):
                assert not owner.attr.startswith("asset_bible"), path


def test_provider_adapters_do_not_import_or_bind_asset_bible_owner() -> None:
    for path in (SOURCE_ROOT / "providers").rglob("*.py"):
        source = path.read_text(encoding="utf-8")
        assert "asset_bible" not in source.lower(), path


def test_storage_and_media_adapters_do_not_own_consumer_aggregates_or_resilience_policy() -> None:
    storage = SOURCE_ROOT / "ports" / "storage.py"
    imports = _imports(storage)
    forbidden_imports = {
        "video_agent_api.application.assets",
        "video_agent_api.application.runs",
        "video_agent_api.application.timeline",
        "video_agent_api.application.exports",
        "video_agent_api.resilience",
    }
    assert not (imports & forbidden_imports)
    source = storage.read_text(encoding="utf-8")
    for forbidden in (
        "AssetVersion(",
        "ProviderCall(",
        "RunEvent(",
        "ExportJob(",
        "verify_restore(",
        "probe_resources(",
    ):
        assert forbidden not in source

    for path in (SOURCE_ROOT / "providers").glob("*.py"):
        imports = _imports(path)
        assert not any(module.startswith("tos") for module in imports), path
        source = path.read_text(encoding="utf-8")
        assert "AssetVersion(" not in source, path
        assert "ExportJob(" not in source, path


def test_asset_center_keeps_storage_and_media_owner_boundaries() -> None:
    assets_http = SOURCE_ROOT / "interfaces" / "http" / "assets.py"
    source = assets_http.read_text(encoding="utf-8")
    assert "/asset-reservations/{reservationId}/register" not in source
    assert '"objectKey"' not in source
    assert '"presignedUrl"' not in source
    assert '"base64"' not in source
    assert "await request.body()" in source  # bounded part transport, not a JSON DTO

    assets_application = SOURCE_ROOT / "application" / "assets.py"
    imports = _imports(assets_application)
    assert not any(module.startswith("video_agent_api.ports.storage") for module in imports)
    assert not any(module.startswith("video_agent_api.providers") for module in imports)

    storage = SOURCE_ROOT / "ports" / "storage.py"
    storage_source = storage.read_text(encoding="utf-8")
    assert "AssetVersionReservation(" not in storage_source
    assert "AssetVersion(" not in storage_source
