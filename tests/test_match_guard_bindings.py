from __future__ import annotations

import importlib
import sys
from pathlib import Path
from types import ModuleType

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import diet_import_hook


MODULE_SOURCE = """
def probe(value):
    match value:
        case iterable if not hasattr(iterable, "__next__"):
            return f"no next for {type(iterable).__name__}"
        case _:
            return "has next"
"""


def _import_module(module_name: str, module_path: Path) -> ModuleType:
    diet_import_hook.install()
    module_dir = str(module_path.parent)
    sys.path.insert(0, module_dir)
    try:
        return importlib.import_module(module_name)
    finally:
        if module_dir in sys.path:
            sys.path.remove(module_dir)


def test_guard_bindings_are_available(tmp_path):
    module_name = "match_guard"
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text(MODULE_SOURCE, encoding="utf-8")

    sys.modules.pop(module_name, None)

    module = _import_module(module_name, module_path)

    try:
        assert module.probe([1, 2, 3]) == "no next for list"
        assert module.probe(iter([1, 2, 3])) == "has next"
    finally:
        sys.modules.pop(module_name, None)
