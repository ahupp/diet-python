from __future__ import annotations

import importlib
import sys
from contextlib import contextmanager
from pathlib import Path
from types import ModuleType
from typing import Iterator

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import diet_import_hook


@contextmanager
def transformed_module(
    tmp_path: Path, module_name: str, source: str
) -> Iterator[ModuleType]:
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text(source, encoding="utf-8")

    module_dir = str(module_path.parent)
    sys.path.insert(0, module_dir)

    try:
        diet_import_hook.install()
        sys.modules.pop(module_name, None)
        module = importlib.import_module(module_name)
        yield module
    finally:
        sys.modules.pop(module_name, None)
        if sys.path and sys.path[0] == module_dir:
            sys.path.pop(0)
        else:
            try:
                sys.path.remove(module_dir)
            except ValueError:
                pass
