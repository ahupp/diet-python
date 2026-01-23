from __future__ import annotations

import os
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


def _print_integration_failure_context(module_path: Path) -> None:
    try:
        source = module_path.read_text(encoding="utf-8")
    except OSError as err:
        source = f"<<failed to read source: {err}>>"

    try:
        transformed = diet_import_hook._transform_source(str(module_path))
    except Exception as err:
        transformed = f"<<failed to transform source: {err}>>"

    print("\n--- diet-python integration failure context ---", file=sys.stderr)
    print(f"module: {module_path}", file=sys.stderr)
    print("--- input module ---", file=sys.stderr)
    print(source, file=sys.stderr)
    print("--- transformed module ---", file=sys.stderr)
    print(transformed, file=sys.stderr)
    print("--- end diet-python integration context ---", file=sys.stderr)


@contextmanager
def transformed_module(
    tmp_path: Path, module_name: str, source: str
) -> Iterator[ModuleType]:
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text(source, encoding="utf-8")

    module_dir = str(module_path.parent)
    sys.path.insert(0, module_dir)
    prior_allow_temp = os.environ.get("DIET_PYTHON_ALLOW_TEMP")
    os.environ["DIET_PYTHON_ALLOW_TEMP"] = "1"

    try:
        diet_import_hook.install()
        sys.modules.pop(module_name, None)
        module = importlib.import_module(module_name)
        if prior_allow_temp is None:
            os.environ.pop("DIET_PYTHON_ALLOW_TEMP", None)
        else:
            os.environ["DIET_PYTHON_ALLOW_TEMP"] = prior_allow_temp
        yield module
    except Exception:
        _print_integration_failure_context(module_path)
        raise
    finally:
        sys.modules.pop(module_name, None)
        if sys.path and sys.path[0] == module_dir:
            sys.path.pop(0)
        else:
            try:
                sys.path.remove(module_dir)
            except ValueError:
                pass
        if prior_allow_temp is None:
            os.environ.pop("DIET_PYTHON_ALLOW_TEMP", None)
        else:
            os.environ["DIET_PYTHON_ALLOW_TEMP"] = prior_allow_temp
