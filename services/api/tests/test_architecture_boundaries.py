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
