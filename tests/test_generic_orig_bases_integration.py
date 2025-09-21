from __future__ import annotations

import importlib
import sys
from pathlib import Path
from types import ModuleType

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import diet_import_hook


MODULE_SOURCE = """
from typing import Generic, TypeVar


T = TypeVar("T")


class Box(Generic[T]):
    pass


def make_specialization():
    class IntBox(Box[int]):
        pass
    return IntBox
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


def test_generic_orig_bases_preserved(tmp_path):
    module_name = "generic_module"
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text(MODULE_SOURCE, encoding="utf-8")

    previous_typing = sys.modules.get("typing")
    sys.modules.pop("typing", None)
    sys.modules.pop(module_name, None)

    module = _import_module(module_name, module_path)

    try:
        transformed_typing = sys.modules["typing"]
        assert isinstance(
            transformed_typing.__spec__.loader, diet_import_hook.DietPythonLoader
        ), "typing should be transformed"

        assert "__dp__" in module.__dict__, "module should be transformed"

        assert module.Box.__orig_bases__ == (transformed_typing.Generic[module.T],)

        specialized = module.make_specialization()
        assert specialized.__orig_bases__[0].__args__ == (int,)
        assert issubclass(specialized, module.Box)
    finally:
        sys.modules.pop(module_name, None)
        if previous_typing is not None:
            sys.modules["typing"] = previous_typing
        else:
            sys.modules.pop("typing", None)
